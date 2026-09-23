use std::{
    net::SocketAddr,
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use flatbuffers::root;
use http_body_util::{BodyExt, Full};
use hyper::{body::Bytes, Request as HttpRequest};
#[cfg(unix)]
use hyper_util::rt::TokioIo;
use hyper_util::{client::legacy::Client, rt::TokioExecutor};
use prost::Message;
use searchkernel::{Field, Index, IndexDefinition};
use searchkernel_dispatch::{Dispatcher, PROTOCOL_VERSION};
use searchkernel_ir::{Backend, PortableError, Result as BackendResult};
use searchkernel_protocol::protocol::v2 as wire;
use searchkernel_testkit::RecordingBackend;
#[cfg(unix)]
use searchkerneld::ServerError;
use searchkerneld::{rpc, start, ServerConfig};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Notify;
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

fn valid_search_request() -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let text = builder.create_string("headphones");
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

fn dispatcher(backend: Arc<dyn Backend>) -> (Index, Arc<Dispatcher>) {
    let definition =
        IndexDefinition::new().field(Field::text("title").stored(true).searchable(true));
    let index = Index::from_backend(definition, backend).expect("test index");
    let (context, session) = index.clone().into_session_parts();
    (index, Arc::new(Dispatcher::new(context, session)))
}

struct BlockingBackend {
    started: Arc<Notify>,
    release: Arc<AtomicBool>,
    searches: Arc<AtomicUsize>,
}

impl BlockingBackend {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            started: Arc::new(Notify::new()),
            release: Arc::new(AtomicBool::new(false)),
            searches: Arc::new(AtomicUsize::new(0)),
        })
    }

    fn unsupported<T>() -> BackendResult<T> {
        Err(PortableError::unsupported(
            "blocking_test_backend",
            None::<String>,
            "the blocking test backend does not implement this operation",
        ))
    }
}

impl Backend for BlockingBackend {
    fn insert(
        &self,
        _request: searchkernel_ir::InsertRequestIr,
    ) -> BackendResult<searchkernel_ir::InsertResult> {
        Self::unsupported()
    }

    fn upsert(
        &self,
        _request: searchkernel_ir::UpsertRequestIr,
    ) -> BackendResult<searchkernel_ir::UpsertResult> {
        Self::unsupported()
    }

    fn patch(
        &self,
        _request: searchkernel_ir::PatchRequestIr,
    ) -> BackendResult<searchkernel_ir::PatchResult> {
        Self::unsupported()
    }

    fn delete(
        &self,
        _request: searchkernel_ir::DeleteRequestIr,
    ) -> BackendResult<searchkernel_ir::DeleteResult> {
        Self::unsupported()
    }

    fn get(
        &self,
        _request: searchkernel_ir::GetRequestIr,
    ) -> BackendResult<searchkernel_ir::GetResult> {
        Self::unsupported()
    }

    fn search(
        &self,
        _request: searchkernel_ir::SearchRequestIr,
    ) -> BackendResult<searchkernel_ir::SearchResult> {
        self.searches.fetch_add(1, Ordering::SeqCst);
        self.started.notify_waiters();
        while !self.release.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
        Self::unsupported()
    }

    fn analyze(
        &self,
        _request: searchkernel_ir::AnalyzeRequestIr,
    ) -> BackendResult<searchkernel_ir::AnalyzeResult> {
        Self::unsupported()
    }
}

async fn connect_client(
    address: SocketAddr,
) -> rpc::search_kernel_client::SearchKernelClient<tonic::transport::Channel> {
    rpc::search_kernel_client::SearchKernelClient::connect(format!("http://{address}"))
        .await
        .expect("connect generated gRPC client")
}

async fn execute_grpc_web_raw(
    address: SocketAddr,
    payload: Vec<u8>,
) -> (u16, Option<String>, Vec<u8>) {
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
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let body = response
        .into_body()
        .collect()
        .await
        .expect("read grpc-web response")
        .to_bytes();
    (status, content_type, body.to_vec())
}

async fn execute_grpc_web(address: SocketAddr, payload: Vec<u8>) -> Vec<u8> {
    let (status, content_type, body) = execute_grpc_web_raw(address, payload).await;
    assert_eq!(status, 200);
    assert!(content_type
        .as_deref()
        .is_some_and(|value| value.starts_with("application/grpc-web")));
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
async fn direct_dispatch_and_grpc_share_the_same_backend_operation_ir() {
    let backend = Arc::new(RecordingBackend::new());
    let (index, dispatcher) = dispatcher(backend.clone());
    let request = valid_search_request();
    let direct_response = dispatcher.dispatch(&request).expect("direct dispatch");
    let direct_calls = backend.take_calls();
    assert_eq!(direct_calls.len(), 1);

    let server = start(
        ServerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0))),
        dispatcher,
    )
    .await
    .expect("start gRPC server");
    let mut client = connect_client(server.tcp_addr().expect("TCP server address")).await;
    let remote_response = client
        .execute(Request::new(rpc::FlatBufferFrame { payload: request }))
        .await
        .expect("gRPC semantic errors are response payloads")
        .into_inner()
        .payload;
    let remote_calls = backend.take_calls();

    assert_eq!(direct_calls, remote_calls, "gRPC must add framing only");
    assert_eq!(direct_response, remote_response);
    let direct = root::<wire::ResponseEnvelope<'_>>(&direct_response).unwrap();
    let remote = root::<wire::ResponseEnvelope<'_>>(&remote_response).unwrap();
    let direct_error = direct.payload_as_search_kernel_v2_error().unwrap();
    let remote_error = remote.payload_as_search_kernel_v2_error().unwrap();
    assert_eq!(direct_error.code(), remote_error.code());
    assert_eq!(
        direct_error.details().map(|details| details
            .iter()
            .map(|detail| detail.code())
            .collect::<Vec<_>>()),
        remote_error.details().map(|details| details
            .iter()
            .map(|detail| detail.code())
            .collect::<Vec<_>>())
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
async fn admission_limit_rejects_a_large_queued_request() {
    let backend = BlockingBackend::new();
    let (index, dispatcher) = dispatcher(backend.clone());
    let server = start(
        ServerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0)))
            .with_execution_limit(1)
            .with_admission_limit(1),
        dispatcher,
    )
    .await
    .expect("start gRPC server");
    let address = server.tcp_addr().expect("TCP server address");
    let mut first_client = connect_client(address).await;
    let mut second_client = connect_client(address).await;
    let payload = large_search_request();
    let first_payload = payload.clone();
    let started = backend.started.notified();
    let first = tokio::spawn(async move {
        first_client
            .execute(Request::new(rpc::FlatBufferFrame {
                payload: first_payload,
            }))
            .await
    });
    started.await;

    let second = tokio::spawn(async move {
        second_client
            .execute(Request::new(rpc::FlatBufferFrame { payload }))
            .await
    });
    for _ in 0..1_000 {
        if second.is_finished() {
            break;
        }
        tokio::task::yield_now().await;
    }
    let second_finished = second.is_finished();
    backend.release.store(true, Ordering::Release);
    first.await.expect("first task").expect("first request");
    let second_result = second.await.expect("second task");

    assert!(
        second_finished,
        "the request beyond the configured admission bound must not wait indefinitely"
    );
    assert_eq!(
        second_result
            .expect_err("overloaded request must be rejected")
            .code(),
        Code::ResourceExhausted
    );

    server.shutdown().await.expect("shutdown server");
    index.close().expect("close test index");
}

#[tokio::test]
async fn admission_limit_is_shared_by_grpc_web() {
    let backend = BlockingBackend::new();
    let (index, dispatcher) = dispatcher(backend.clone());
    let server = start(
        ServerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0)))
            .with_web_listen_addr(SocketAddr::from(([127, 0, 0, 1], 0)))
            .with_execution_limit(1)
            .with_admission_limit(1),
        dispatcher,
    )
    .await
    .expect("start gRPC and grpc-web listeners");
    let grpc_address = server.tcp_addr().expect("gRPC listener address");
    let web_address = server.web_tcp_addr().expect("grpc-web listener address");
    let mut grpc_client = connect_client(grpc_address).await;
    let payload = large_search_request();
    let started = backend.started.notified();
    let first = tokio::spawn(async move {
        grpc_client
            .execute(Request::new(rpc::FlatBufferFrame { payload }))
            .await
    });
    started.await;

    let (status, _content_type, body) =
        execute_grpc_web_raw(web_address, large_search_request()).await;
    assert_eq!(status, 200);
    backend.release.store(true, Ordering::Release);
    let body = String::from_utf8_lossy(&body);
    assert!(
        body.contains("grpc-status: 8"),
        "grpc-web overload response must report ResourceExhausted: {body:?}"
    );

    first.await.expect("first task").expect("first request");
    server.shutdown().await.expect("shutdown both listeners");
    index.close().expect("close test index");
}

#[tokio::test]
async fn cancelled_execution_waiter_releases_admission_capacity() {
    let backend = BlockingBackend::new();
    let (index, dispatcher) = dispatcher(backend.clone());
    let server = start(
        ServerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0)))
            .with_execution_limit(1)
            .with_admission_limit(2),
        dispatcher,
    )
    .await
    .expect("start gRPC server");
    let address = server.tcp_addr().expect("TCP server address");
    let mut first_client = connect_client(address).await;
    let mut second_client = connect_client(address).await;
    let mut third_client = connect_client(address).await;
    let first_payload = large_search_request();
    let started = backend.started.notified();
    let first = tokio::spawn(async move {
        first_client
            .execute(Request::new(rpc::FlatBufferFrame {
                payload: first_payload,
            }))
            .await
    });
    started.await;

    let mut request = Request::new(rpc::FlatBufferFrame {
        payload: large_search_request(),
    });
    request.set_timeout(Duration::from_millis(50));
    let second = tokio::spawn(async move { second_client.execute(request).await });
    let second_result = tokio::time::timeout(Duration::from_secs(2), second)
        .await
        .expect("deadline waiter must finish")
        .expect("deadline task");
    let second_error = second_result.expect_err("deadline waiter must be cancelled");
    assert!(matches!(
        second_error.code(),
        Code::DeadlineExceeded | Code::Cancelled
    ));

    // Keep the first request blocked while probing the admission slot released
    // by the cancelled second waiter. If cancellation stranded its admission
    // permit, this request would be rejected immediately with ResourceExhausted.
    // If the permit was released, it is admitted and waits behind the occupied
    // execution slot until its own deadline expires.
    let mut third_request = Request::new(rpc::FlatBufferFrame {
        payload: large_search_request(),
    });
    third_request.set_timeout(Duration::from_millis(100));
    let third_result =
        tokio::time::timeout(Duration::from_secs(2), third_client.execute(third_request))
            .await
            .expect("third admission probe must finish");
    let third_error = third_result.expect_err("third request must wait for execution");
    assert!(
        matches!(third_error.code(), Code::DeadlineExceeded | Code::Cancelled),
        "cancelled waiter must release admission capacity; got {third_error:?}"
    );

    // The client may observe its deadline just before the server-side
    // grpc-timeout future is polled to completion. Give the server deadline
    // wrapper ample time to drop the admitted request while the first execution
    // permit remains occupied, then prove another request is admitted rather
    // than rejected as ResourceExhausted.
    tokio::time::sleep(Duration::from_millis(200)).await;
    let mut fourth_client = connect_client(address).await;
    let mut fourth_request = Request::new(rpc::FlatBufferFrame {
        payload: large_search_request(),
    });
    fourth_request.set_timeout(Duration::from_millis(100));
    let fourth_result = tokio::time::timeout(
        Duration::from_secs(2),
        fourth_client.execute(fourth_request),
    )
    .await
    .expect("fourth admission probe must finish");
    let fourth_error = fourth_result.expect_err("fourth request must wait for execution");
    assert!(
        matches!(
            fourth_error.code(),
            Code::DeadlineExceeded | Code::Cancelled
        ),
        "deadline-expired waiter stranded admission capacity; got {fourth_error:?}"
    );

    // Also prove timed-out waiters do not remain queued for execution and run
    // later when the occupied native slot is finally released.
    tokio::time::sleep(Duration::from_millis(200)).await;
    backend.release.store(true, Ordering::Release);
    first.await.expect("first task").expect("first request");
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        backend.searches.load(Ordering::SeqCst),
        1,
        "deadline-expired waiters must not execute after capacity is released"
    );

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
async fn inaccessible_unix_socket_is_not_removed_before_binding() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after the Unix epoch")
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "searchkerneld-inaccessible-uds-{}-{nonce}.sock",
        std::process::id(),
    ));
    std::fs::create_dir(&directory).expect("create inaccessible UDS directory");
    let path = directory.join("socket");
    let listener = UnixListener::bind(&path).expect("create inaccessible UDS listener");
    // Socket inode mode bits are not consistently enforced for same-owner
    // connections across Unix platforms. Restrict the parent directory
    // instead, so path access fails before stale-socket handling can remove it.
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o000))
        .expect("make UDS directory inaccessible");
    let lock_path = directory.join("socket.lock");
    let access_error = match std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
    {
        Ok(lock) => {
            // Privileged test processes can bypass directory permissions, so
            // this fixture cannot produce an inaccessible path in that mode.
            drop(lock);
            std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
                .expect("restore UDS directory permissions");
            drop(listener);
            std::fs::remove_file(&path).expect("remove UDS used by skipped test");
            std::fs::remove_file(&lock_path).expect("remove UDS lock used by skipped test");
            std::fs::remove_dir(&directory).expect("remove skipped UDS directory");
            eprintln!("skipping inaccessible UDS test under privileged filesystem access");
            return;
        }
        Err(error) => error,
    };

    let backend = Arc::new(RecordingBackend::new());
    let (index, dispatcher) = dispatcher(backend);
    let error = start(ServerConfig::unix(&path), dispatcher)
        .await
        .expect_err("inaccessible UDS must not be treated as stale");
    assert!(matches!(
        error,
        ServerError::Bind(error) if error.kind() == access_error.kind()
    ));

    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
        .expect("restore UDS directory permissions");
    assert!(path.exists(), "permission failure removed the UDS path");

    drop(listener);
    std::fs::remove_file(&path).expect("remove inaccessible UDS");
    let _ = std::fs::remove_file(lock_path);
    std::fs::remove_dir(&directory).expect("remove inaccessible UDS directory");
    index.close().expect("close test index");
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
