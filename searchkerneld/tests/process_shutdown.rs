#![cfg(unix)]

use std::{
    fs,
    io::Read,
    net::{SocketAddr, TcpListener, TcpStream},
    os::unix::process::ExitStatusExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use flatbuffers::root;
use searchkernel::{Field, IndexDefinition, RemoteClient};
use searchkernel_protocol::{protocol::v2 as wire, v2 as api, PROTOCOL_VERSION};
use searchkerneld::rpc;
use tonic::Request;

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

fn wait_for_daemon(child: &mut Child, endpoint: &str) -> RemoteClient {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if let Ok(client) = RemoteClient::connect(endpoint) {
            return client;
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

fn reserve_tcp_address() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("reserve daemon address");
    listener.local_addr().expect("reserved daemon address")
}

fn wait_for_tcp_daemon(child: &mut Child, address: SocketAddr) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if TcpStream::connect(address).is_ok() {
            return;
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
    panic!("searchkerneld did not listen at {address}");
}

fn target<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    name: &str,
) -> flatbuffers::WIPOffset<api::IndexTarget<'a>> {
    let name = builder.create_string(name);
    api::IndexTarget::create(builder, &api::IndexTargetArgs { name: Some(name) })
}

fn create_index_request(target_name: &str, field_name: &str) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let field_name = builder.create_string(field_name);
    let primitive = api::PrimitiveDataType::create(
        &mut builder,
        &api::PrimitiveDataTypeArgs {
            kind: api::PrimitiveDataTypeKind::String,
        },
    );
    let data_type = api::DataType::create(
        &mut builder,
        &api::DataTypeArgs {
            kind_type: api::DataTypeKind::PrimitiveDataType,
            kind: Some(primitive.as_union_value()),
        },
    );
    let stored = api::OptionalBool::new(true);
    let searchable = api::OptionalBool::new(true);
    let field = api::FieldDefinition::create(
        &mut builder,
        &api::FieldDefinitionArgs {
            name: Some(field_name),
            data_type: Some(data_type),
            stored: Some(&stored),
            searchable: Some(&searchable),
            ..Default::default()
        },
    );
    let fields = builder.create_vector(&[field]);
    let definition_name = builder.create_string(target_name);
    let definition = api::IndexDefinition::create(
        &mut builder,
        &api::IndexDefinitionArgs {
            name: Some(definition_name),
            fields: Some(fields),
            ..Default::default()
        },
    );
    let payload = api::CreateIndexRequest::create(
        &mut builder,
        &api::CreateIndexRequestArgs {
            definition: Some(definition),
        },
    );
    let target = target(&mut builder, target_name);
    let envelope = wire::RequestEnvelope::create(
        &mut builder,
        &wire::RequestEnvelopeArgs {
            protocol_version: PROTOCOL_VERSION,
            target: Some(target),
            payload_type: wire::RequestPayload::SearchKernel_V2_CreateIndexRequest,
            payload: Some(payload.as_union_value()),
        },
    );
    wire::finish_request_envelope_buffer(&mut builder, envelope);
    builder.finished_data().to_vec()
}

fn open_index_request(target_name: &str) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let payload =
        api::OpenIndexRequest::create(&mut builder, &api::OpenIndexRequestArgs::default());
    let target = target(&mut builder, target_name);
    let envelope = wire::RequestEnvelope::create(
        &mut builder,
        &wire::RequestEnvelopeArgs {
            protocol_version: PROTOCOL_VERSION,
            target: Some(target),
            payload_type: wire::RequestPayload::SearchKernel_V2_OpenIndexRequest,
            payload: Some(payload.as_union_value()),
        },
    );
    wire::finish_request_envelope_buffer(&mut builder, envelope);
    builder.finished_data().to_vec()
}

fn grpc_execute(address: SocketAddr, payload: Vec<u8>) -> Vec<u8> {
    tokio::runtime::Runtime::new()
        .expect("create Tokio runtime")
        .block_on(async move {
            let mut client =
                rpc::search_kernel_client::SearchKernelClient::connect(format!("http://{address}"))
                    .await
                    .expect("connect generated gRPC client");
            client
                .execute(Request::new(rpc::FlatBufferFrame { payload }))
                .await
                .expect("execute canonical request")
                .into_inner()
                .payload
        })
}

fn assert_response_kind(bytes: &[u8], expected: wire::ResponsePayload) {
    let envelope = root::<wire::ResponseEnvelope<'_>>(bytes).expect("response envelope");
    assert_eq!(envelope.payload_type(), expected);
}

fn spawn_multi_daemon(data_root: &Path, address: SocketAddr) -> Child {
    Command::new(env!("CARGO_BIN_EXE_searchkerneld"))
        .args([
            "--data-root",
            data_root.to_str().unwrap(),
            "--listen",
            &address.to_string(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn multi-index searchkerneld")
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
fn data_root_daemon_restarts_with_two_independent_indexes() {
    let temp = TempDir::new();

    let first_address = reserve_tcp_address();
    let mut first = spawn_multi_daemon(&temp.0, first_address);
    wait_for_tcp_daemon(&mut first, first_address);

    for (name, field) in [("products", "title"), ("articles", "body")] {
        let response = grpc_execute(first_address, create_index_request(name, field));
        assert_response_kind(
            &response,
            wire::ResponsePayload::SearchKernel_V2_CreateIndexResult,
        );
    }

    assert_eq!(unsafe { libc::kill(first.id() as i32, libc::SIGTERM) }, 0);
    let first_status = wait_for_exit(&mut first);
    assert!(
        first_status.success(),
        "multi-index daemon should exit cleanly after SIGTERM: {first_status:?}"
    );

    let second_address = reserve_tcp_address();
    let mut second = spawn_multi_daemon(&temp.0, second_address);
    wait_for_tcp_daemon(&mut second, second_address);

    for name in ["products", "articles"] {
        let response = grpc_execute(second_address, open_index_request(name));
        assert_response_kind(
            &response,
            wire::ResponsePayload::SearchKernel_V2_OpenIndexResult,
        );
    }

    assert_eq!(unsafe { libc::kill(second.id() as i32, libc::SIGTERM) }, 0);
    let second_status = wait_for_exit(&mut second);
    assert!(
        second_status.success(),
        "restarted multi-index daemon should exit cleanly: {second_status:?}"
    );
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
    let socket = temp.0.join("daemon.sock");
    let mut child = Command::new(env!("CARGO_BIN_EXE_searchkerneld"))
        .args([
            "--data-root",
            temp.0.to_str().unwrap(),
            "--listen",
            &format!("unix://{}", socket.display()),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let endpoint = format!("unix://{}", socket.display());
    let client = wait_for_daemon(&mut child, &endpoint);
    let daemon = client
        .create_index(
            "shutdown-index",
            IndexDefinition::new().field(Field::text("title").stored(true).searchable(true)),
        )
        .expect("create logical target through daemon lifecycle");
    let id = "persisted-before-sigterm";
    let document = serde_json::json!({"title": "written through the daemon"});
    daemon
        .insert(id, &document)
        .expect("daemon must accept a real document mutation");
    daemon
        .close()
        .expect("close remote index handle before shutdown");
    drop(client);

    assert_eq!(unsafe { libc::kill(child.id() as i32, signal) }, 0);
    let status = wait_for_exit(&mut child);
    assert!(
        status.success(),
        "{signal_name} should produce normal exit, got {status:?} (signal {:?})",
        status.signal()
    );
    assert!(!socket.exists(), "owned UDS was not cleaned up");

    let mut restarted = Command::new(env!("CARGO_BIN_EXE_searchkerneld"))
        .args([
            "--data-root",
            temp.0.to_str().unwrap(),
            "--listen",
            &format!("unix://{}", socket.display()),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("restart multi-index daemon");
    let client = wait_for_daemon(&mut restarted, &endpoint);
    let reopened = client
        .open_index("shutdown-index")
        .expect("SIGTERM must leave a reopenable logical index");
    let persisted = reopened
        .get::<serde_json::Value>(id)
        .expect("read document after reopening the index")
        .expect("graceful shutdown must preserve the document written through the daemon");
    assert_eq!(persisted, document);
    reopened.close().expect("close reopened index handle");
    drop(client);
    assert_eq!(
        unsafe { libc::kill(restarted.id() as i32, libc::SIGTERM) },
        0
    );
    assert!(wait_for_exit(&mut restarted).success());
    assert!(
        !socket.exists(),
        "restarted daemon must clean up its owned UDS"
    );
}
