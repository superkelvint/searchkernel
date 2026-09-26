#![cfg(unix)]

use std::{
    fs,
    os::unix::process::ExitStatusExt,
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct TempDir(PathBuf);
impl TempDir {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "searchkerneld-signal-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn wait_for_socket(child: &mut Child, socket: &PathBuf) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if socket.exists() {
            return;
        }
        if let Some(status) = child.try_wait().unwrap() {
            panic!("searchkerneld exited before becoming ready: {status}");
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("searchkerneld did not create {}", socket.display());
}

fn wait_for_exit(child: &mut Child) -> std::process::ExitStatus {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        thread::sleep(Duration::from_millis(20));
    }
    let _ = child.kill();
    panic!("searchkerneld did not exit after graceful signal");
}

#[test]
fn sigterm_gracefully_closes_index_and_cleans_owned_uds() {
    let temp = TempDir::new();
    let index = temp.0.join("index");
    let socket = temp.0.join("daemon.sock");
    let definition = temp.0.join("definition.json");
    fs::write(&definition, r#"{"name":"signal_test","document_type":"typea","fields":[{"name":"title","data_type":{"kind_type":"PrimitiveDataType","kind":{"kind":"Text"}},"stored":{"value":true},"searchable":{"value":true}}],"default_search_fields":["title"]}"#).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_searchkerneld"))
        .args([
            "--index",
            index.to_str().unwrap(),
            "--definition-json",
            definition.to_str().unwrap(),
            "--listen",
            &format!("unix://{}", socket.display()),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    wait_for_socket(&mut child, &socket);

    assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
    let status = wait_for_exit(&mut child);
    assert!(
        status.success(),
        "SIGTERM should produce normal exit, got {status:?} (signal {:?})",
        status.signal()
    );
    assert!(!socket.exists(), "owned UDS was not cleaned up");

    let reopened =
        searchkernel::Index::open(&index).expect("SIGTERM must leave a reopenable index");
    reopened.close().unwrap();
}
