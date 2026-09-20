use std::{net::SocketAddr, path::Path, sync::Arc};

use flatbuffers::root;
use http_body_util::{BodyExt, Full};
use hyper::{body::Bytes, Request as HttpRequest};
#[cfg(unix)]
use hyper_util::rt::TokioIo;
use hyper_util::{client::legacy::Client, rt::TokioExecutor};
use prost::Message;
use searchkernel::{Field, Index, IndexDefinition};
use searchkernel_dispatch::{Dispatcher, PROTOCOL_VERSION};
use searchkernel_protocol::protocol::v2 as wire;
use searchkernel_testkit::RecordingBackend;
#[cfg(unix)]
use searchkerneld::ServerError;
use searchkerneld::{rpc, start, ServerConfig};
#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};
use tonic::{Code, Request};

fn describe_request(version: u32) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let request =
        wire::DescribeRequest::create(&mut builder, &wire::DescribeRequestArgs::default());
    let envelope = wire::RequestEnvelope::create(
        &mut builder,
        &wire::RequestEnvelopeArgs {
            protocol_version: version,
            payload_type: wire::RequestPayload::SearchKernel_V2_DescribeRequest,
            payload: Some(request.as_union_value()),
        },
    );
    builder.finish(envelope, Some("SKRQ"));
    builder.finished_data().to_vec()
}

fn search_request() -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let request = wire::SearchRequest::create(&mut builder, &wire::SearchRequestArgs::default());
    let envelope = wire::RequestEnvelope::create(
        &mut builder,
        &wire::RequestEnvelopeArgs {
            protocol_version: PROTOCOL_VERSION,
            payload_type: wire::RequestPayload::SearchKernel_V2_SearchRequest,
            payload: Some(request.as_union_value()),
        },
    );
    builder.finish(envelope, Some("SKRQ"));
    builder.finished_data().to_vec()
}

fn large_search_request() -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let text = builder.create_string(&"x".repeat(5 * 1024 * 1024));
    let query = wire::SearchKernelTextQuery::create(
        &mut builder,
        &wire::SearchKernelTextQueryArgs { text: Some(text) },
    );
    let source = wire::QuerySource::create(
        &mut builder,
        &wire::QuerySourceArgs {
            source_type: wire::QuerySourceKind::SearchKernelTextQuery,
            source: Some(query.as_union_value()),
        },
    );
    let request = wire::SearchRequest::create(
        &mut builder,
        &wire::SearchRequestArgs {
            query: Some(source),
            ..Default::default()
        },
    );
    let envelope = wire::RequestEnvelope::create(
        &mut builder,
        &wire::RequestEnvelopeArgs {
            protocol_version: PROTOCOL_VERSION,
            payload_type: wire::RequestPayload::SearchKernel_V2_SearchRequest,
            payload: Some(request.as_union_value()),
        },
    );
    builder.finish(envelope, Some("SKRQ"));
    builder.finished_data().to_vec()
}

fn dispatcher(backend: Arc<RecordingBackend>) -> (Index, Arc<Dispatcher>) {
    let definition =
        IndexDefinition::new().field(Field::text("title").stored(true).searchable(true));
    let index = Index::from_backend(definition, backend).expect("test index");
    let (context, session) = index.clone().into_session_parts();
    (index, Arc::new(Dispatcher::new(context, session)))
}

async fn connect_client(
    address: SocketAddr,
) -> rpc::search_kernel_client::SearchKernelClient<tonic::transport::Channel> {
    rpc::search_kernel_client::SearchKernelClient::connect(format!("http://{address}"))
        .await
        .expect("connect generated gRPC client")
}

async fn execute_grpc_web(address: SocketAddr, payload: Vec<u8>) -> Vec<u8> {
    let frame = rpc::FlatBufferFrame { payload }.encode_to_vec();
    let mut grpc_frame = Vec::with_capacity(5 + frame.len());
    grpc_frame.push(0);
    grpc_frame.extend_from_slice(&(frame.len() as u32).to_be_bytes());
    grpc_frame.extend_from_slice(&frame);

    let request = HttpRequest::builder()
        .method("POST")
        .uri(format!(
            "http://{address}/searchkernel.rpc.v1.SearchKernel/Execute"
        ))
        .header("content-type", "application/grpc-web+proto")
        .header("x-grpc-web", "1")
        .body(Full::new(Bytes::from(grpc_frame)))
        .expect("build grpc-web request");
    let client = Client::builder(TokioExecutor::new()).build_http();
    let response = client.request(request).await.expect("grpc-web request");
    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .map(|value| value.starts_with("application/grpc-web")),
        Some(true)
    );
    let body = response
        .into_body()
        .collect()
        .await
        .expect("read grpc-web response")
        .to_bytes();
    assert!(
        body.len() >= 5,
        "grpc-web response must contain a data frame"
    );
    assert_eq!(
        body[0], 0,
        "first grpc-web frame must contain response data"
    );
    let frame_len = u32::from_be_bytes(body[1..5].try_into().expect("frame length")) as usize;
    assert!(body.len() >= 5 + frame_len);
    rpc::FlatBufferFrame::decode(&body[5..5 + frame_len])
        .expect("decode grpc-web response frame")
        .payload
}

#[cfg(unix)]
async fn connect_uds(
    path: &Path,
) -> rpc::search_kernel_client::SearchKernelClient<tonic::transport::Channel> {
    let path = path.to_owned();
    let channel = tonic::transport::Endpoint::try_from("http://[::]:50051")
        .expect("valid placeholder endpoint")
        .connect_with_connector(tower::service_fn(move |_| {
            let path = path.clone();
            async move { UnixStream::connect(path).await.map(TokioIo::new) }
        }))
        .await
        .expect("connect generated gRPC client over UDS");
    rpc::search_kernel_client::SearchKernelClient::new(channel)
}

#[tokio::test]
async fn execute_uses_the_canonical_dispatcher_over_a_real_socket() {
    let backend = Arc::new(RecordingBackend::new());
    let (index, dispatcher) = dispatcher(backend);
    let direct = dispatcher
        .dispatch(&describe_request(PROTOCOL_VERSION))
        .expect("direct dispatch");
    let server = start(
        ServerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0))),
        dispatcher,
    )
    .await
    .expect("start gRPC server");
    let mut client = connect_client(server.tcp_addr().expect("TCP server address")).await;
    let remote = client
        .execute(Request::new(rpc::FlatBufferFrame {
            payload: describe_request(PROTOCOL_VERSION),
        }))
        .await
        .expect("gRPC Execute")
        .into_inner()
        .payload;

    assert_eq!(direct, remote, "gRPC must preserve the canonical response");
    let direct_envelope = root::<wire::ResponseEnvelope<'_>>(&direct).expect("direct response");
    let remote_envelope = root::<wire::ResponseEnvelope<'_>>(&remote).expect("remote response");
    assert_eq!(
        direct_envelope.protocol_version(),
        remote_envelope.protocol_version()
    );
    assert_eq!(
        direct_envelope.payload_type(),
        remote_envelope.payload_type()
    );
    assert!(remote_envelope
        .payload_as_search_kernel_v2_describe_result()
        .is_some());

    server.shutdown().await.expect("shutdown server");
    index.close().expect("close test index");
}

#[tokio::test]
async fn semantic_dispatch_errors_remain_canonical_error_payloads() {
    let backend = Arc::new(RecordingBackend::new());
    let (index, dispatcher) = dispatcher(backend);
    let direct = dispatcher
        .dispatch(&search_request())
        .expect("direct error response");
    let server = start(
        ServerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0))),
        dispatcher,
    )
    .await
    .expect("start gRPC server");
    let mut client = connect_client(server.tcp_addr().expect("TCP server address")).await;
    let remote = client
        .execute(Request::new(rpc::FlatBufferFrame {
            payload: search_request(),
        }))
        .await
        .expect("semantic errors are response payloads")
        .into_inner()
        .payload;
    assert_eq!(
        direct, remote,
        "gRPC must preserve the canonical error response"
    );
    let direct = root::<wire::ResponseEnvelope<'_>>(&direct).expect("direct response");
    let remote = root::<wire::ResponseEnvelope<'_>>(&remote).expect("remote response");
    assert_eq!(
        direct.payload_type(),
        wire::ResponsePayload::SearchKernel_V2_Error
    );
    assert_eq!(
        remote.payload_type(),
        wire::ResponsePayload::SearchKernel_V2_Error
    );
    assert_eq!(
        direct.payload_as_search_kernel_v2_error().unwrap().code(),
        remote.payload_as_search_kernel_v2_error().unwrap().code()
    );

    server.shutdown().await.expect("shutdown server");
    index.close().expect("close test index");
}

#[tokio::test]
async fn execute_accepts_frames_larger_than_tonic_default_limit() {
    let backend = Arc::new(RecordingBackend::new());
    let (index, dispatcher) = dispatcher(backend);
    let payload = large_search_request();
    assert!(payload.len() > 4 * 1024 * 1024);
    let server = start(
        ServerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0))),
        dispatcher,
    )
    .await
    .expect("start gRPC server");
    let mut client = connect_client(server.tcp_addr().expect("TCP server address")).await;
    let response = client
        .execute(Request::new(rpc::FlatBufferFrame { payload }))
        .await
        .expect("large gRPC frame should be accepted")
        .into_inner();

    assert!(!response.payload.is_empty());
    server.shutdown().await.expect("shutdown server");
    index.close().expect("close test index");
}

#[tokio::test]
async fn grpc_web_listener_shares_the_canonical_service_with_grpc() {
    let backend = Arc::new(RecordingBackend::new());
    let (index, dispatcher) = dispatcher(backend);
    let direct = dispatcher
        .dispatch(&describe_request(PROTOCOL_VERSION))
        .expect("direct dispatch");
    let server = start(
        ServerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0)))
            .with_web_listen_addr(SocketAddr::from(([127, 0, 0, 1], 0))),
        dispatcher,
    )
    .await
    .expect("start gRPC and grpc-web listeners");
    let grpc_address = server.tcp_addr().expect("gRPC listener address");
    let web_address = server.web_tcp_addr().expect("grpc-web listener address");

    let mut grpc_client = connect_client(grpc_address).await;
    let grpc_response = grpc_client
        .execute(Request::new(rpc::FlatBufferFrame {
            payload: describe_request(PROTOCOL_VERSION),
        }))
        .await
        .expect("gRPC Execute")
        .into_inner()
        .payload;
    let web_response = execute_grpc_web(web_address, describe_request(PROTOCOL_VERSION)).await;

    assert_eq!(direct, grpc_response);
    assert_eq!(direct, web_response);
    server.shutdown().await.expect("shutdown both listeners");
    index.close().expect("close test index");
}

#[tokio::test]
async fn framing_and_lifecycle_failures_are_transport_statuses() {
    let backend = Arc::new(RecordingBackend::new());
    let (index, dispatcher) = dispatcher(backend);
    let server = start(
        ServerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0))),
        dispatcher.clone(),
    )
    .await
    .expect("start dispatcher server");
    let mut client = connect_client(server.tcp_addr().expect("TCP server address")).await;
    let error = client
        .execute(Request::new(rpc::FlatBufferFrame {
            payload: b"malformed".to_vec(),
        }))
        .await
        .expect_err("malformed request");
    assert_eq!(error.code(), Code::InvalidArgument);
    let error = client
        .execute(Request::new(rpc::FlatBufferFrame {
            payload: describe_request(PROTOCOL_VERSION + 1),
        }))
        .await
        .expect_err("unsupported protocol");
    assert_eq!(error.code(), Code::FailedPrecondition);

    dispatcher.close().expect("close target");
    let error = client
        .execute(Request::new(rpc::FlatBufferFrame {
            payload: describe_request(PROTOCOL_VERSION),
        }))
        .await
        .expect_err("closed target");
    assert_eq!(error.code(), Code::FailedPrecondition);
    server.shutdown().await.expect("shutdown dispatcher server");
    index.close().expect("close test index");
}

#[tokio::test]
async fn shutdown_closes_the_real_socket() {
    let backend = Arc::new(RecordingBackend::new());
    let (index, dispatcher) = dispatcher(backend);
    let server = start(
        ServerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0))),
        dispatcher,
    )
    .await
    .expect("start server");
    let address = server.tcp_addr().expect("TCP server address");
    server.shutdown().await.expect("shutdown server");
    index.close().expect("close test index");
    let error = rpc::search_kernel_client::SearchKernelClient::connect(format!("http://{address}"))
        .await
        .expect_err("shutdown server must close its listener");
    assert!(error.to_string().contains("error") || error.to_string().contains("connect"));
}

#[cfg(unix)]
#[tokio::test]
async fn execute_works_over_a_unix_domain_socket() {
    let path = std::env::temp_dir().join(format!("searchkerneld-uds-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let backend = Arc::new(RecordingBackend::new());
    let (index, dispatcher) = dispatcher(backend);
    let direct = dispatcher
        .dispatch(&describe_request(PROTOCOL_VERSION))
        .expect("direct dispatch");
    let server = start(ServerConfig::unix(&path), dispatcher)
        .await
        .expect("start UDS server");
    assert_eq!(server.unix_path(), Some(path.as_path()));
    assert!(path.exists());

    let mut client = connect_uds(&path).await;
    let remote = client
        .execute(Request::new(rpc::FlatBufferFrame {
            payload: describe_request(PROTOCOL_VERSION),
        }))
        .await
        .expect("gRPC Execute over UDS")
        .into_inner()
        .payload;
    assert_eq!(direct, remote, "UDS must preserve the canonical response");

    server.shutdown().await.expect("shutdown UDS server");
    assert!(!path.exists(), "shutdown must remove the UDS path");
    index.close().expect("close test index");
}

#[cfg(unix)]
#[tokio::test]
async fn stale_unix_socket_is_removed_before_binding() {
    let path = std::env::temp_dir().join(format!(
        "searchkerneld-stale-uds-{}.sock",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let stale_listener = UnixListener::bind(&path).expect("create stale UDS listener");
    drop(stale_listener);
    assert!(path.exists());

    let backend = Arc::new(RecordingBackend::new());
    let (index, dispatcher) = dispatcher(backend);
    let server = start(ServerConfig::unix(&path), dispatcher)
        .await
        .expect("start over a stale UDS path");
    assert!(path.exists());

    server.shutdown().await.expect("shutdown UDS server");
    index.close().expect("close test index");
    assert!(!path.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn live_unix_socket_is_not_removed_or_rebound() {
    let path = std::env::temp_dir().join(format!(
        "searchkerneld-live-uds-{}.sock",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let backend = Arc::new(RecordingBackend::new());
    let (index, dispatcher) = dispatcher(backend);
    let server = start(ServerConfig::unix(&path), dispatcher.clone())
        .await
        .expect("start UDS server");

    let error = match start(ServerConfig::unix(&path), dispatcher).await {
        Ok(second_server) => {
            second_server
                .shutdown()
                .await
                .expect("shutdown unexpectedly rebound server");
            panic!("a live UDS server must prevent rebinding")
        }
        Err(error) => error,
    };
    assert!(matches!(
        error,
        ServerError::Bind(error)
            if error.kind() == std::io::ErrorKind::AddrInUse
                && error.to_string().contains("live server")
    ));
    assert!(path.exists(), "live server socket must remain in place");
    UnixStream::connect(&path)
        .await
        .expect("original UDS server must remain reachable");

    server.shutdown().await.expect("shutdown UDS server");
    index.close().expect("close test index");
    assert!(!path.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn non_socket_path_is_not_removed_before_binding() {
    let path = std::env::temp_dir().join(format!(
        "searchkerneld-not-a-uds-{}.path",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    std::fs::write(&path, b"keep me").expect("create non-socket path");

    let backend = Arc::new(RecordingBackend::new());
    let (index, dispatcher) = dispatcher(backend);
    let result = start(ServerConfig::unix(&path), dispatcher).await;
    assert!(result.is_err());
    assert_eq!(
        std::fs::read(&path).expect("read preserved path"),
        b"keep me"
    );

    index.close().expect("close test index");
    std::fs::remove_file(path).expect("remove test path");
}
