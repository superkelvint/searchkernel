#![cfg(unix)]

use std::{
    fs,
    io::Read,
    os::unix::process::ExitStatusExt,
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use searchkernel::{Field, Index, IndexDefinition};

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

fn wait_for_daemon(child: &mut Child, endpoint: &str) -> Index {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if let Ok(index) = Index::connect(endpoint) {
            return index;
        }
        if let Some(status) = child.try_wait().unwrap() {
            let mut stderr = String::new();
            if let Some(mut output) = child.stderr.take() {
                output.read_to_string(&mut stderr).unwrap();
            }
            panic!("searchkerneld exited before becoming ready: {status}; stderr: {stderr}");
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("searchkerneld did not accept requests at {endpoint}");
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
    assert_graceful_shutdown(libc::SIGTERM, "SIGTERM");
}

#[test]
fn sigint_gracefully_closes_index_and_cleans_owned_uds() {
    assert_graceful_shutdown(libc::SIGINT, "SIGINT");
}

fn assert_graceful_shutdown(signal: libc::c_int, signal_name: &str) {
    let temp = TempDir::new();
    let index = temp.0.join("index");
    let socket = temp.0.join("daemon.sock");
    Index::create(
        &index,
        IndexDefinition::new().field(Field::text("title").stored(true).searchable(true)),
    )
    .expect("create test index")
    .close()
    .expect("close test index before daemon startup");

    let mut child = Command::new(env!("CARGO_BIN_EXE_searchkerneld"))
        .args([
            "--index",
            index.to_str().unwrap(),
            "--listen",
            &format!("unix://{}", socket.display()),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let endpoint = format!("unix://{}", socket.display());
    let daemon = wait_for_daemon(&mut child, &endpoint);
    let id = "persisted-before-sigterm";
    let document = serde_json::json!({"title": "written through the daemon"});
    daemon
        .insert(id, &document)
        .expect("daemon must accept a real document mutation");
    daemon.close().expect("close remote client before shutdown");

    assert_eq!(unsafe { libc::kill(child.id() as i32, signal) }, 0);
    let status = wait_for_exit(&mut child);
    assert!(
        status.success(),
        "{signal_name} should produce normal exit, got {status:?} (signal {:?})",
        status.signal()
    );
    assert!(!socket.exists(), "owned UDS was not cleaned up");

    let reopened = Index::open(&index).expect("SIGTERM must leave a reopenable index");
    let persisted = reopened
        .get::<serde_json::Value>(id)
        .expect("read document after reopening the index")
        .expect("graceful shutdown must preserve the document written through the daemon");
    assert_eq!(persisted, document);
    reopened.close().unwrap();
}
