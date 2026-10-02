use std::{
    net::SocketAddr,
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Condvar, Mutex,
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
use searchkernel::{
    field, ErrorCode as PublicErrorCode, Field, Index, IndexDefinition, Patch, Ranking,
    RemoteClient, Sort, Tensor,
};
use searchkernel_dispatch::Dispatcher;
use searchkernel_ir::{Backend, PortableError, Result as BackendResult};
use searchkernel_protocol::protocol::v1 as wire;
use searchkernel_protocol::protocol::v1::PROTOCOL_VERSION;
use searchkernel_testkit::{
    canonical_result_fixtures, ExpectedCall, RecordingBackend, ScriptedBackend, ScriptedResult,
};
#[cfg(unix)]
use searchkerneld::ServerError;
use searchkerneld::{rpc, start, ServerConfig};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::path::PathBuf;
#[cfg(unix)]
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{oneshot, Notify};
use tokio_stream::wrappers::TcpListenerStream;
#[cfg(unix)]
use tokio_stream::wrappers::UnixListenerStream;
use tonic::{Code, Request, Response, Status};

fn describe_request(version: u32) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let request =
        wire::DescribeRequest::create(&mut builder, &wire::DescribeRequestArgs::default());
    let envelope = wire::RequestEnvelope::create(
        &mut builder,
        &wire::RequestEnvelopeArgs {
            protocol_version: version,
            target: None,
            payload_type: wire::RequestPayload::SearchKernel_Wire_DescribeRequest,
            payload: Some(request.as_union_value()),
            expected_generation: None,
        },
    );
    builder.finish(envelope, Some("SKRQ"));
    builder.finished_data().to_vec()
}

fn targeted_describe_request(version: u32, target_name: &str) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let request =
        wire::DescribeRequest::create(&mut builder, &wire::DescribeRequestArgs::default());
    let target_name = builder.create_string(target_name);
    let target = wire::IndexTarget::create(
        &mut builder,
        &wire::IndexTargetArgs {
            name: Some(target_name),
            generation: None,
        },
    );
    let envelope = wire::RequestEnvelope::create(
        &mut builder,
        &wire::RequestEnvelopeArgs {
            protocol_version: version,
            target: Some(target),
            payload_type: wire::RequestPayload::SearchKernel_Wire_DescribeRequest,
            payload: Some(request.as_union_value()),
            expected_generation: None,
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
            target: None,
            payload_type: wire::RequestPayload::SearchKernel_Wire_SearchRequest,
            payload: Some(request.as_union_value()),
            expected_generation: None,
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
            target: None,
            payload_type: wire::RequestPayload::SearchKernel_Wire_SearchRequest,
            payload: Some(request.as_union_value()),
            expected_generation: None,
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
            target: None,
            payload_type: wire::RequestPayload::SearchKernel_Wire_SearchRequest,
            payload: Some(request.as_union_value()),
            expected_generation: None,
        },
    );
    builder.finish(envelope, Some("SKRQ"));
    builder.finished_data().to_vec()
}

fn dispatcher(backend: Arc<dyn Backend>) -> (Index, Arc<Dispatcher>) {
    let (index, dispatcher, _) = dispatcher_with_definition(backend);
    (index, dispatcher)
}

fn dispatcher_with_definition(
    backend: Arc<dyn Backend>,
) -> (Index, Arc<Dispatcher>, searchkernel_ir::IndexDefinitionIr) {
    let definition =
        IndexDefinition::new().field(Field::text("title").stored(true).searchable(true));
    let index = Index::from_backend(definition, backend).expect("test index");
    let (context, session) = index.clone().into_session_parts();
    let definition = context.definition().clone();
    (
        index,
        Arc::new(Dispatcher::new(context, session)),
        definition,
    )
}

/// Test-only daemon protocol fixture: lifecycle calls return the hosted schema,
/// and ordinary calls reach the injected dispatcher.
#[derive(Clone)]
struct RemoteTestService {
    dispatcher: Arc<Dispatcher>,
    definition: searchkernel_ir::IndexDefinitionIr,
}

#[tonic::async_trait]
impl rpc::search_kernel_server::SearchKernel for RemoteTestService {
    async fn execute(
        &self,
        request: Request<rpc::FlatBufferFrame>,
    ) -> Result<Response<rpc::FlatBufferFrame>, Status> {
        let payload = request.into_inner().payload;
        let envelope = wire::root_as_request_envelope(&payload)
            .map_err(|_| Status::invalid_argument("invalid Search Kernel request"))?;
        let response = match envelope.payload_type() {
            wire::RequestPayload::SearchKernel_Wire_CreateIndexRequest => {
                lifecycle_definition_response(&self.definition, true)
            }
            wire::RequestPayload::SearchKernel_Wire_OpenIndexRequest => {
                lifecycle_definition_response(&self.definition, false)
            }
            wire::RequestPayload::SearchKernel_Wire_DeleteIndexRequest => {
                lifecycle_delete_response()
            }
            _ => self
                .dispatcher
                .dispatch(&payload)
                .map_err(|_| Status::invalid_argument("invalid Search Kernel request"))?,
        };
        Ok(Response::new(rpc::FlatBufferFrame { payload: response }))
    }
}

fn lifecycle_definition_response(
    definition: &searchkernel_ir::IndexDefinitionIr,
    create: bool,
) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let index = searchkernel_ir::wire::v1::index_definition_offset(&mut builder, definition);
    let generation = builder.create_string("fixture-generation");
    let (payload_type, payload) = if create {
        let result = wire::CreateIndexResult::create(
            &mut builder,
            &wire::CreateIndexResultArgs {
                index: Some(index),
                generation: Some(generation),
            },
        );
        (
            wire::ResponsePayload::SearchKernel_Wire_CreateIndexResult,
            result.as_union_value(),
        )
    } else {
        let result = wire::OpenIndexResult::create(
            &mut builder,
            &wire::OpenIndexResultArgs {
                index: Some(index),
                generation: Some(generation),
            },
        );
        (
            wire::ResponsePayload::SearchKernel_Wire_OpenIndexResult,
            result.as_union_value(),
        )
    };
    let envelope = wire::ResponseEnvelope::create(
        &mut builder,
        &wire::ResponseEnvelopeArgs {
            protocol_version: PROTOCOL_VERSION,
            payload_type,
            payload: Some(payload),
        },
    );
    wire::finish_response_envelope_buffer(&mut builder, envelope);
    builder.finished_data().to_vec()
}

fn lifecycle_delete_response() -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let result = wire::DeleteIndexResult::create(&mut builder, &wire::DeleteIndexResultArgs {});
    let envelope = wire::ResponseEnvelope::create(
        &mut builder,
        &wire::ResponseEnvelopeArgs {
            protocol_version: PROTOCOL_VERSION,
            payload_type: wire::ResponsePayload::SearchKernel_Wire_DeleteIndexResult,
            payload: Some(result.as_union_value()),
        },
    );
    wire::finish_response_envelope_buffer(&mut builder, envelope);
    builder.finished_data().to_vec()
}

struct RemoteTestServer {
    endpoint: String,
    shutdown: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<Result<(), tonic::transport::Error>>,
    #[cfg(unix)]
    socket_path: Option<std::path::PathBuf>,
}

impl RemoteTestServer {
    async fn tcp(
        dispatcher: Arc<Dispatcher>,
        definition: searchkernel_ir::IndexDefinitionIr,
    ) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind remote fixture");
        let address = listener.local_addr().expect("remote fixture address");
        let service = rpc::search_kernel_server::SearchKernelServer::new(RemoteTestService {
            dispatcher,
            definition,
        });
        let (shutdown, shutdown_rx) = oneshot::channel();
        let task = tokio::spawn(
            tonic::transport::Server::builder()
                .add_service(service)
                .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                    let _ = shutdown_rx.await;
                }),
        );
        Self {
            endpoint: address.to_string(),
            shutdown: Some(shutdown),
            task,
            #[cfg(unix)]
            socket_path: None,
        }
    }

    #[cfg(unix)]
    async fn unix(
        path: &Path,
        dispatcher: Arc<Dispatcher>,
        definition: searchkernel_ir::IndexDefinitionIr,
    ) -> Self {
        let listener = tokio::net::UnixListener::bind(path).expect("bind remote fixture socket");
        let service = rpc::search_kernel_server::SearchKernelServer::new(RemoteTestService {
            dispatcher,
            definition,
        });
        let (shutdown, shutdown_rx) = oneshot::channel();
        let task = tokio::spawn(
            tonic::transport::Server::builder()
                .add_service(service)
                .serve_with_incoming_shutdown(UnixListenerStream::new(listener), async {
                    let _ = shutdown_rx.await;
                }),
        );
        Self {
            endpoint: format!("unix://{}", path.display()),
            shutdown: Some(shutdown),
            task,
            socket_path: Some(path.to_owned()),
        }
    }

    async fn shutdown(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.task
            .await
            .expect("remote fixture task")
            .expect("remote fixture shutdown");
        #[cfg(unix)]
        if let Some(path) = self.socket_path.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

struct BlockingBackend {
    started: Arc<Notify>,
    release: (Mutex<bool>, Condvar),
    searches: Arc<AtomicUsize>,
}

impl BlockingBackend {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            started: Arc::new(Notify::new()),
            release: (Mutex::new(false), Condvar::new()),
            searches: Arc::new(AtomicUsize::new(0)),
        })
    }

    fn release(&self) {
        let (released, wake) = &self.release;
        *released.lock().expect("blocking backend release lock") = true;
        wake.notify_all();
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
        let (released, wake) = &self.release;
        let mut released = released.lock().expect("blocking backend release lock");
        while !*released {
            released = wake.wait(released).expect("blocking backend release lock");
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ergonomic_rust_remote_client_uses_canonical_grpc_transport() {
    let backend = Arc::new(RecordingBackend::new());
    let (_local, dispatcher, lifecycle_definition) = dispatcher_with_definition(backend);
    let server = RemoteTestServer::tcp(dispatcher, lifecycle_definition).await;
    let endpoint = server.endpoint.clone();

    // The public remote API is intentionally synchronous. In an async
    // application it belongs on the application's normal blocking-work path.
    tokio::task::spawn_blocking(move || {
        let client = RemoteClient::connect(endpoint).expect("connect Rust remote daemon");
        let remote = client
            .open_index("grpc-test")
            .expect("open Rust remote index");
        let error = remote
            .search("headphones")
            .execute::<String>()
            .expect_err("recording backend rejects execution");
        assert_eq!(error.code(), PublicErrorCode::Unsupported);
        assert_eq!(error.details()[0].code, "recording_backend");
        remote.close().expect("close remote client");
    })
    .await
    .expect("remote client blocking task");

    server.shutdown().await;
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ergonomic_remote_search_preserves_direct_normalized_ir_through_daemon() {
    let definition = IndexDefinition::new()
        .field(Field::text("title").stored(true).searchable(true))
        .field(
            Field::float64("price")
                .stored(true)
                .filterable(true)
                .sortable(true),
        );

    let recording = Arc::new(RecordingBackend::new());
    let direct = Index::from_backend(definition.clone(), recording.clone())
        .expect("build direct recording index");
    let direct_error = direct
        .search("remote headphones")
        .filter(field("price").gte(10.0))
        .sort(Sort::desc("price").missing_last())
        .ranking(Ranking::bm25())
        .language("en")
        .timeout(Duration::from_millis(1750))
        .limit(7)
        .offset(3)
        .execute::<String>()
        .expect_err("recording backend rejects direct execution");
    assert_eq!(direct_error.code(), PublicErrorCode::Unsupported);
    let expected = recording
        .last_call()
        .expect("direct execution recorded normalized IR");

    let scripted = Arc::new(ScriptedBackend::new(vec![ExpectedCall::success(
        expected,
        ScriptedResult::Search(Box::new(searchkernel_ir::SearchResult::empty(
            searchkernel_ir::TotalHits {
                value: 0,
                relation: searchkernel_ir::TotalHitsRelation::Exact,
            },
        ))),
    )]));
    let hosted =
        Index::from_backend(definition, scripted.clone()).expect("build hosted scripted index");
    let (context, session) = hosted.into_session_parts();
    let lifecycle_definition = context.definition().clone();
    let dispatcher = Arc::new(Dispatcher::new(context, session));
    let server = RemoteTestServer::tcp(dispatcher, lifecycle_definition).await;
    let endpoint = server.endpoint.clone();

    tokio::task::spawn_blocking(move || {
        let client = RemoteClient::connect(endpoint).expect("connect Rust remote daemon");
        let remote = client
            .open_index("grpc-test")
            .expect("open Rust remote index");
        let result = remote
            .search("remote headphones")
            .filter(field("price").gte(10.0))
            .sort(Sort::desc("price").missing_last())
            .ranking(Ranking::bm25())
            .language("en")
            .timeout(Duration::from_millis(1750))
            .limit(7)
            .offset(3)
            .execute::<String>()
            .expect("remote normalized IR matches direct IR");
        assert_eq!(result.total_hits.value, 0);
        remote.close().expect("close remote client");
    })
    .await
    .expect("remote client blocking task");

    scripted
        .finish()
        .expect("remote daemon consumed the exact direct IR request");
    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ergonomic_remote_patch_and_projection_preserve_direct_normalized_ir_through_daemon() {
    let definition = IndexDefinition::new()
        .field(Field::text("title").stored(true).searchable(true))
        .field(Field::float64("price").stored(true).filterable(true));

    let recording = Arc::new(RecordingBackend::new());
    let direct = Index::from_backend(definition.clone(), recording.clone())
        .expect("build direct recording index");

    let patch = Patch::new().set("price", 88.25_f64).unset("title");
    assert_eq!(
        direct
            .patch("doc-1", patch.clone())
            .expect_err("recording backend rejects direct patch")
            .code(),
        PublicErrorCode::Unsupported
    );
    assert_eq!(
        direct
            .get_with_projection::<serde_json::Value>("doc-1", ["price"])
            .expect_err("recording backend rejects direct projected get")
            .code(),
        PublicErrorCode::Unsupported
    );
    let expected = recording.take_calls();
    assert_eq!(expected.len(), 2);

    let scripted = Arc::new(ScriptedBackend::new(vec![
        ExpectedCall::success(
            expected[0].clone(),
            ScriptedResult::Patch(searchkernel_ir::PatchResult {
                id: "doc-1".to_owned(),
            }),
        ),
        ExpectedCall::success(
            expected[1].clone(),
            ScriptedResult::Get(searchkernel_ir::GetResult {
                document: Some(searchkernel_ir::DocumentResult {
                    id: "doc-1".to_owned(),
                    fields: std::collections::BTreeMap::from([(
                        "price".to_owned(),
                        searchkernel_ir::Value::Float64(88.25),
                    )]),
                }),
            }),
        ),
    ]));
    let hosted =
        Index::from_backend(definition, scripted.clone()).expect("build hosted scripted index");
    let (context, session) = hosted.into_session_parts();
    let lifecycle_definition = context.definition().clone();
    let server = RemoteTestServer::tcp(
        Arc::new(Dispatcher::new(context, session)),
        lifecycle_definition,
    )
    .await;
    let endpoint = server.endpoint.clone();

    tokio::task::spawn_blocking(move || {
        let client = RemoteClient::connect(endpoint).expect("connect Rust remote daemon");
        let remote = client
            .open_index("grpc-test")
            .expect("open Rust remote index");
        remote
            .patch("doc-1", patch)
            .expect("remote patch preserves direct normalized IR");
        let projected = remote
            .get_with_projection::<serde_json::Value>("doc-1", ["price"])
            .expect("remote projected get preserves direct normalized IR")
            .expect("projected document exists");
        assert_eq!(projected, serde_json::json!({"price": 88.25}));
        remote.close().expect("close remote client");
    })
    .await
    .expect("remote client blocking task");

    scripted
        .finish()
        .expect("remote daemon consumed exact patch and projected-get IR");
    server.shutdown().await;
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
        .payload_as_search_kernel_wire_describe_result()
        .is_some());

    server.shutdown().await.expect("shutdown server");
    index.close().expect("close test index");
}

#[tokio::test]
async fn legacy_single_transport_rejects_targeted_requests() {
    let backend = Arc::new(RecordingBackend::new());
    let (index, dispatcher) = dispatcher(backend);
    let server = start(
        ServerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0))),
        dispatcher,
    )
    .await
    .expect("start single-index gRPC server");
    let mut client = connect_client(server.tcp_addr().expect("TCP server address")).await;

    let status = client
        .execute(Request::new(rpc::FlatBufferFrame {
            payload: targeted_describe_request(PROTOCOL_VERSION, "other-index"),
        }))
        .await
        .expect_err("targeted request must not bypass the multi-index router");
    assert_eq!(status.code(), Code::FailedPrecondition);
    assert!(status
        .message()
        .contains("targeted Search Kernel requests require the multi-index daemon"));

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
        wire::ResponsePayload::SearchKernel_Wire_Error
    );
    assert_eq!(
        remote.payload_type(),
        wire::ResponsePayload::SearchKernel_Wire_Error
    );
    assert_eq!(
        direct.payload_as_search_kernel_wire_error().unwrap().code(),
        remote.payload_as_search_kernel_wire_error().unwrap().code()
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
    let direct_error = direct.payload_as_search_kernel_wire_error().unwrap();
    let remote_error = remote.payload_as_search_kernel_wire_error().unwrap();
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
    backend.release();
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
    backend.release();
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
    backend.release();
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
    let request = describe_request(PROTOCOL_VERSION);
    let direct = dispatcher
        .dispatch(&request)
        .expect("closed target direct dispatch must return canonical error");
    let response = client
        .execute(Request::new(rpc::FlatBufferFrame { payload: request }))
        .await
        .expect("closed target gRPC must return canonical error payload")
        .into_inner()
        .payload;
    assert_eq!(response, direct);
    let envelope = flatbuffers::root::<wire::ResponseEnvelope<'_>>(&response).unwrap();
    assert_eq!(
        envelope.payload_type(),
        wire::ResponsePayload::SearchKernel_Wire_Error
    );
    let error = envelope.payload_as_search_kernel_wire_error().unwrap();
    assert_eq!(error.code(), wire::ErrorCode::FailedPrecondition);
    assert_eq!(
        error.details().unwrap().get(0).code(),
        Some("target_closed")
    );
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ergonomic_remote_insert_preserves_direct_normalized_ir_through_daemon() {
    let definition = IndexDefinition::new()
        .field(Field::keyword("tags").multi().stored(true))
        .field(Field::position("location").stored(true))
        .field(Field::tensor("embedding", "tensor<float>(x[3])").stored(true));

    let tags_document = std::collections::BTreeMap::from([(
        "tags".to_owned(),
        vec!["rust".to_owned(), "remote".to_owned()],
    )]);
    let location_document = std::collections::BTreeMap::from([(
        "location".to_owned(),
        std::collections::BTreeMap::from([
            ("latitude".to_owned(), 37.3317_f64),
            ("longitude".to_owned(), -121.8916_f64),
        ]),
    )]);
    let tensor_document = std::collections::BTreeMap::from([(
        "embedding".to_owned(),
        Tensor::dense("tensor<float>(x[3])", vec![0.25, 0.5, 0.75]),
    )]);

    let recording = Arc::new(RecordingBackend::new());
    let direct = Index::from_backend(definition.clone(), recording.clone())
        .expect("build direct recording index");
    assert_eq!(
        direct
            .insert("doc-tags", &tags_document)
            .expect_err("recording backend rejects direct tags insert")
            .code(),
        PublicErrorCode::Unsupported
    );
    assert_eq!(
        direct
            .insert("doc-location", &location_document)
            .expect_err("recording backend rejects direct position insert")
            .code(),
        PublicErrorCode::Unsupported
    );
    assert_eq!(
        direct
            .insert("doc-tensor", &tensor_document)
            .expect_err("recording backend rejects direct tensor insert")
            .code(),
        PublicErrorCode::Unsupported
    );
    let expected = recording.take_calls();
    assert_eq!(expected.len(), 3);

    let scripted = Arc::new(ScriptedBackend::new(vec![
        ExpectedCall::success(
            expected[0].clone(),
            ScriptedResult::Insert(searchkernel_ir::InsertResult {
                id: "doc-tags".to_owned(),
            }),
        ),
        ExpectedCall::success(
            expected[1].clone(),
            ScriptedResult::Insert(searchkernel_ir::InsertResult {
                id: "doc-location".to_owned(),
            }),
        ),
        ExpectedCall::success(
            expected[2].clone(),
            ScriptedResult::Insert(searchkernel_ir::InsertResult {
                id: "doc-tensor".to_owned(),
            }),
        ),
    ]));
    let hosted =
        Index::from_backend(definition, scripted.clone()).expect("build hosted scripted index");
    let expected_definition = hosted.describe().expect("describe hosted index");
    let (context, session) = hosted.into_session_parts();
    let lifecycle_definition = context.definition().clone();
    let dispatcher = Arc::new(Dispatcher::new(context, session));
    let server = RemoteTestServer::tcp(dispatcher, lifecycle_definition).await;
    let endpoint = server.endpoint.clone();

    tokio::task::spawn_blocking(move || {
        let client = RemoteClient::connect(endpoint).expect("connect Rust remote daemon");
        let remote = client
            .open_index("grpc-test")
            .expect("open Rust remote index");
        assert_eq!(
            remote.describe().expect("describe remote index"),
            expected_definition,
            "OpenIndex result must preserve the effective Rust schema exactly"
        );
        remote
            .insert("doc-tags", &tags_document)
            .expect("remote multi-value insert preserves direct normalized IR");
        remote
            .insert("doc-location", &location_document)
            .expect("remote position insert preserves direct normalized IR");
        remote
            .insert("doc-tensor", &tensor_document)
            .expect("remote tensor insert preserves direct normalized IR");
        remote.close().expect("close remote client");
    })
    .await
    .expect("remote client blocking task");

    scripted
        .finish()
        .expect("remote daemon consumed the exact direct insert IR");
    server.shutdown().await;
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ergonomic_rust_remote_client_connects_over_unix_socket() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let socket_path = std::env::temp_dir().join(format!(
        "searchkernel-rust-remote-{}-{nonce}.sock",
        std::process::id()
    ));
    let mut lock_name = socket_path.as_os_str().to_os_string();
    lock_name.push(".lock");
    let lock_path = PathBuf::from(lock_name);

    let backend = Arc::new(RecordingBackend::new());
    let (_local, dispatcher, lifecycle_definition) = dispatcher_with_definition(backend);
    let server = RemoteTestServer::unix(&socket_path, dispatcher, lifecycle_definition).await;
    let endpoint = server.endpoint.clone();

    tokio::task::spawn_blocking(move || {
        let client =
            RemoteClient::connect(endpoint).expect("connect Rust remote daemon over Unix socket");
        let remote = client
            .open_index("grpc-test")
            .expect("open Rust remote index");
        let error = remote
            .search("headphones")
            .execute::<String>()
            .expect_err("recording backend rejects execution");
        assert_eq!(error.code(), PublicErrorCode::Unsupported);
        assert_eq!(error.details()[0].code, "recording_backend");
        remote.close().expect("close Unix remote client");
    })
    .await
    .expect("Unix remote client blocking task");

    server.shutdown().await;
    assert!(
        !socket_path.exists(),
        "searchkerneld shutdown must remove the owned Unix socket"
    );
    let _ = std::fs::remove_file(lock_path);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ergonomic_remote_result_matches_embedded_public_projection_exactly() {
    let definition =
        IndexDefinition::new().field(Field::text("title").stored(true).searchable(true));

    // Resolve once through the public Rust facade so both backends receive the
    // exact same normalized request. This keeps the test focused on result
    // transport fidelity rather than reconstructing SearchRequestIr by hand.
    let recording = Arc::new(RecordingBackend::new());
    let probe = Index::from_backend(definition.clone(), recording.clone())
        .expect("build request probe index");
    assert_eq!(
        probe
            .search("remote result")
            .execute::<serde_json::Value>()
            .expect_err("recording backend rejects request")
            .code(),
        PublicErrorCode::Unsupported
    );
    let expected_request = recording
        .last_call()
        .expect("request probe recorded normalized IR");
    probe.close().expect("close request probe");

    let fixtures = canonical_result_fixtures();
    let embedded_backend = Arc::new(ScriptedBackend::new(
        fixtures
            .iter()
            .map(|(_, fixture)| {
                ExpectedCall::success(
                    expected_request.clone(),
                    ScriptedResult::Search(Box::new(fixture.clone())),
                )
            })
            .collect::<Vec<_>>(),
    ));
    let embedded = Index::from_backend(definition.clone(), embedded_backend.clone())
        .expect("build embedded scripted index");
    let mut embedded_results = Vec::with_capacity(fixtures.len());
    for (name, _) in &fixtures {
        let result = embedded
            .search("remote result")
            .execute::<serde_json::Value>()
            .unwrap_or_else(|error| panic!("embedded fixture {name} failed: {error}"));
        embedded_results.push((*name, result));
    }
    embedded_backend
        .finish()
        .expect("embedded path consumed result corpus");
    embedded.close().expect("close embedded index");

    let remote_backend = Arc::new(ScriptedBackend::new(
        fixtures
            .iter()
            .map(|(_, fixture)| {
                ExpectedCall::success(
                    expected_request.clone(),
                    ScriptedResult::Search(Box::new(fixture.clone())),
                )
            })
            .collect::<Vec<_>>(),
    ));
    let hosted = Index::from_backend(definition, remote_backend.clone())
        .expect("build hosted scripted index");
    let (context, session) = hosted.into_session_parts();
    let lifecycle_definition = context.definition().clone();
    let server = RemoteTestServer::tcp(
        Arc::new(Dispatcher::new(context, session)),
        lifecycle_definition,
    )
    .await;
    let endpoint = server.endpoint.clone();

    let fixture_names = fixtures.iter().map(|(name, _)| *name).collect::<Vec<_>>();
    let remote_results = tokio::task::spawn_blocking(move || {
        let client = RemoteClient::connect(endpoint).expect("connect Rust remote daemon");
        let remote = client
            .open_index("grpc-test")
            .expect("open Rust remote index");
        let mut results = Vec::with_capacity(fixture_names.len());
        for name in fixture_names {
            let result = remote
                .search("remote result")
                .execute::<serde_json::Value>()
                .unwrap_or_else(|error| panic!("remote fixture {name} failed: {error}"));
            results.push((name, result));
        }
        remote.close().expect("close remote client");
        results
    })
    .await
    .expect("remote client blocking task");

    remote_backend
        .finish()
        .expect("remote path consumed result corpus");
    assert_eq!(
        remote_results, embedded_results,
        "remote response decoding must preserve every canonical public Rust result projection"
    );
    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ergonomic_rust_remote_client_multiplexes_concurrent_calls() {
    let backend = BlockingBackend::new();
    let (local, dispatcher) = dispatcher(backend.clone());
    let server = start(
        ServerConfig::new("127.0.0.1:0".parse().expect("ephemeral TCP address"))
            .with_execution_limit(2)
            .with_admission_limit(2),
        dispatcher,
    )
    .await
    .expect("start production searchkerneld transport");
    let endpoint = server.tcp_addr().expect("TCP test server").to_string();

    let remote = tokio::task::spawn_blocking(move || {
        Index::connect(endpoint).expect("connect legacy single-index Rust remote client")
    })
    .await
    .expect("connect blocking task");
    let first = remote.clone();
    let second = remote.clone();

    let first_call = tokio::task::spawn_blocking(move || {
        first
            .search("first")
            .execute::<String>()
            .expect_err("blocking backend rejects first search")
    });
    let second_call = tokio::task::spawn_blocking(move || {
        second
            .search("second")
            .execute::<String>()
            .expect_err("blocking backend rejects second search")
    });

    tokio::time::timeout(Duration::from_secs(2), async {
        while backend.searches.load(Ordering::SeqCst) < 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("both remote calls must reach the production daemon concurrently");
    backend.release();

    let first_error = first_call.await.expect("first blocking task");
    let second_error = second_call.await.expect("second blocking task");
    assert_eq!(first_error.code(), PublicErrorCode::Unsupported);
    assert_eq!(second_error.code(), PublicErrorCode::Unsupported);

    remote.close().expect("close legacy remote client");
    server
        .shutdown()
        .await
        .expect("shutdown production searchkerneld transport");
    local.close().expect("close hosted test index");
}
