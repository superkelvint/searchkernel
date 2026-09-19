use std::{net::SocketAddr, path::Path, sync::Arc};

use flatbuffers::root;
#[cfg(unix)]
use hyper_util::rt::TokioIo;
use searchkernel::{Field, Index, IndexDefinition};
use searchkernel_dispatch::{Dispatcher, PROTOCOL_VERSION};
use searchkernel_protocol::protocol::v2 as wire;
use searchkernel_testkit::RecordingBackend;
use searchkerneld::{rpc, start, start_with_dispatcher, ServerConfig};
#[cfg(unix)]
use tokio::net::UnixStream;
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
    let server = start_with_dispatcher(
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
    let server = start_with_dispatcher(
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
    let server = start_with_dispatcher(
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
async fn framing_and_lifecycle_failures_are_transport_statuses() {
    let server = start(ServerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0))))
        .await
        .expect("start missing-target server");
    let mut client = connect_client(server.tcp_addr().expect("TCP server address")).await;
    let error = client
        .execute(Request::new(rpc::FlatBufferFrame {
            payload: b"malformed".to_vec(),
        }))
        .await
        .expect_err("missing target must fail before parsing");
    assert_eq!(error.code(), Code::FailedPrecondition);
    server
        .shutdown()
        .await
        .expect("shutdown missing-target server");

    let backend = Arc::new(RecordingBackend::new());
    let (index, dispatcher) = dispatcher(backend);
    let server = start_with_dispatcher(
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
    let server = start(ServerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0))))
        .await
        .expect("start server");
    let address = server.tcp_addr().expect("TCP server address");
    server.shutdown().await.expect("shutdown server");
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
    let server = start_with_dispatcher(ServerConfig::unix(&path), dispatcher)
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
