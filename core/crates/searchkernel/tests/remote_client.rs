#![cfg(feature = "remote")]

use searchkernel::{
    field, ErrorCode as PublicErrorCode, Field, Index, IndexDefinition, Patch, RemoteClient,
    RemoteOptions,
};
use searchkernel_dispatch::{DispatchError, Dispatcher};
use searchkernel_ir::{
    AnalyzeRequestIr, AnalyzeResult, AnalyzedToken, Backend, DeleteRequestIr, DeleteResult,
    DocumentResult, GetRequestIr, GetResult, Hit, InsertRequestIr, InsertResult, OperationIr,
    PatchRequestIr, PatchResult, PortableError, ResultGroup, ResultNode, SearchRequestIr,
    SearchResult, TotalHits, TotalHitsRelation, UpdateRequestIr, UpdateResult, UpsertRequestIr,
    UpsertResult, Value,
};
use searchkernel_rpc::{
    rpc::{
        search_kernel_server::{SearchKernel, SearchKernelServer},
        FlatBufferFrame,
    },
    MAX_GRPC_MESSAGE_SIZE,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{net::TcpListener, sync::oneshot};
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{Request, Response, Status};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Product {
    title: String,
    price: f64,
}

#[derive(Default)]
struct ProbeBackend {
    calls: Mutex<Vec<OperationIr>>,
    search_mode: AtomicU8,
    insert_mode: AtomicU8,
    get_mode: AtomicU8,
}

impl ProbeBackend {
    fn record(&self, operation: OperationIr) {
        self.calls.lock().expect("calls mutex").push(operation);
    }

    fn search_mode(&self, mode: u8) {
        self.search_mode.store(mode, Ordering::SeqCst);
    }

    fn insert_mode(&self, mode: u8) {
        self.insert_mode.store(mode, Ordering::SeqCst);
    }

    fn get_mode(&self, mode: u8) {
        self.get_mode.store(mode, Ordering::SeqCst);
    }

    fn calls(&self) -> Vec<OperationIr> {
        self.calls.lock().expect("calls mutex").clone()
    }

    fn product_fields() -> BTreeMap<String, Value> {
        BTreeMap::from([
            ("price".to_owned(), Value::Float64(42.5)),
            ("title".to_owned(), Value::String("Remote Rust".to_owned())),
        ])
    }
}

impl Backend for ProbeBackend {
    fn insert(&self, request: InsertRequestIr) -> searchkernel_ir::Result<InsertResult> {
        let id = request.document().id().as_str().to_owned();
        self.record(OperationIr::Insert(request));
        match self.insert_mode.load(Ordering::SeqCst) {
            1 => std::thread::sleep(Duration::from_secs(2)),
            2 => {
                return Ok(InsertResult {
                    id: "wrong-id".to_owned(),
                })
            }
            _ => {}
        }
        Ok(InsertResult { id })
    }

    fn upsert(&self, request: UpsertRequestIr) -> searchkernel_ir::Result<UpsertResult> {
        let id = request.document().id().as_str().to_owned();
        self.record(OperationIr::Upsert(request));
        Ok(UpsertResult { id })
    }

    fn patch(&self, request: PatchRequestIr) -> searchkernel_ir::Result<PatchResult> {
        let id = request.id().as_str().to_owned();
        self.record(OperationIr::Patch(request));
        Ok(PatchResult { id })
    }

    fn update(&self, request: UpdateRequestIr) -> searchkernel_ir::Result<UpdateResult> {
        let id = request.id().as_str().to_owned();
        self.record(OperationIr::Update(request));
        Ok(UpdateResult { id })
    }

    fn delete(&self, request: DeleteRequestIr) -> searchkernel_ir::Result<DeleteResult> {
        let id = request.id().as_str().to_owned();
        self.record(OperationIr::Delete(request));
        Ok(DeleteResult { id, deleted: true })
    }

    fn get(&self, request: GetRequestIr) -> searchkernel_ir::Result<GetResult> {
        let mut id = request.id().as_str().to_owned();
        self.record(OperationIr::Get(request));
        if self.get_mode.load(Ordering::SeqCst) == 1 {
            id = "wrong-document".to_owned();
        }
        Ok(GetResult {
            document: Some(DocumentResult {
                id,
                fields: Self::product_fields(),
            }),
        })
    }

    fn search(&self, request: SearchRequestIr) -> searchkernel_ir::Result<SearchResult> {
        self.record(OperationIr::Search(request));
        match self.search_mode.load(Ordering::SeqCst) {
            1 => Err(PortableError::invalid(
                "remote_probe_rejected",
                Some("search.query"),
                "probe rejected search",
            )),
            2 => {
                std::thread::sleep(Duration::from_secs(2));
                Ok(SearchResult::empty(TotalHits {
                    value: 0,
                    relation: TotalHitsRelation::Exact,
                }))
            }
            _ => Ok(SearchResult {
                root: ResultNode::Group(ResultGroup::root(vec![ResultNode::Hit(Hit {
                    id: "doc-1".to_owned(),
                    score: Some(3.5),
                    fields: Self::product_fields(),
                    ..Hit::default()
                })])),
                total_hits: TotalHits {
                    value: 1,
                    relation: TotalHitsRelation::Exact,
                },
                coverage: None,
                timing: None,
                errors: Vec::new(),
                trace: None,
                metadata: BTreeMap::new(),
            }),
        }
    }

    fn analyze(&self, request: AnalyzeRequestIr) -> searchkernel_ir::Result<AnalyzeResult> {
        self.record(OperationIr::Analyze(request));
        Ok(AnalyzeResult {
            tokens: vec![AnalyzedToken {
                surface: "Remote".to_owned(),
                term: "remote".to_owned(),
                position: 0,
                start_byte: 0,
                end_byte: 6,
            }],
        })
    }
}

#[derive(Clone)]
struct TestService {
    dispatcher: Arc<Dispatcher>,
    definition: searchkernel_ir::IndexDefinitionIr,
    targets: Arc<Mutex<Vec<String>>>,
}

#[tonic::async_trait]
impl SearchKernel for TestService {
    async fn execute(
        &self,
        request: Request<FlatBufferFrame>,
    ) -> Result<Response<FlatBufferFrame>, Status> {
        let request = request.into_inner().payload;
        let envelope = searchkernel_protocol::protocol::v2::root_as_request_envelope(&request)
            .map_err(|_| Status::invalid_argument("invalid request envelope"))?;
        let target = envelope
            .target()
            .and_then(|target| target.name())
            .unwrap_or_default();
        self.targets
            .lock()
            .expect("target log")
            .push(target.to_owned());
        if target == "slow-create"
            && envelope.payload_type()
                == searchkernel_protocol::protocol::v2::RequestPayload::SearchKernel_V2_CreateIndexRequest
        {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        let payload = match envelope.payload_type() {
            searchkernel_protocol::protocol::v2::RequestPayload::SearchKernel_V2_CreateIndexRequest => {
                lifecycle_definition_response(&self.definition, true)
            }
            searchkernel_protocol::protocol::v2::RequestPayload::SearchKernel_V2_OpenIndexRequest => {
                lifecycle_definition_response(&self.definition, target == "wrong-response")
            }
            searchkernel_protocol::protocol::v2::RequestPayload::SearchKernel_V2_DeleteIndexRequest => {
                lifecycle_delete_response()
            }
            _ => self.dispatcher.dispatch(&request).map_err(dispatch_status)?,
        };
        Ok(Response::new(FlatBufferFrame { payload }))
    }
}

fn lifecycle_definition_response(
    definition: &searchkernel_ir::IndexDefinitionIr,
    create: bool,
) -> Vec<u8> {
    use searchkernel_protocol::{protocol::v2 as wire, v2 as api, PROTOCOL_VERSION};
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let index = searchkernel_ir::index_definition_offset(&mut builder, definition);
    let (payload_type, payload) = if create {
        let result = api::CreateIndexResult::create(
            &mut builder,
            &api::CreateIndexResultArgs { index: Some(index) },
        );
        (
            wire::ResponsePayload::SearchKernel_V2_CreateIndexResult,
            result.as_union_value(),
        )
    } else {
        let result = api::OpenIndexResult::create(
            &mut builder,
            &api::OpenIndexResultArgs { index: Some(index) },
        );
        (
            wire::ResponsePayload::SearchKernel_V2_OpenIndexResult,
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
    use searchkernel_protocol::{protocol::v2 as wire, v2 as api, PROTOCOL_VERSION};
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let result = api::DeleteIndexResult::create(&mut builder, &api::DeleteIndexResultArgs {});
    let envelope = wire::ResponseEnvelope::create(
        &mut builder,
        &wire::ResponseEnvelopeArgs {
            protocol_version: PROTOCOL_VERSION,
            payload_type: wire::ResponsePayload::SearchKernel_V2_DeleteIndexResult,
            payload: Some(result.as_union_value()),
        },
    );
    wire::finish_response_envelope_buffer(&mut builder, envelope);
    builder.finished_data().to_vec()
}

fn dispatch_status(error: DispatchError) -> Status {
    match error {
        DispatchError::InvalidRequest => Status::invalid_argument("invalid request"),
        DispatchError::UnsupportedProtocol(_) => Status::failed_precondition("bad protocol"),
        DispatchError::UnsupportedOperation(_) => Status::unimplemented("bad operation"),
        DispatchError::Closed => Status::failed_precondition("closed"),
    }
}

struct TestServer {
    endpoint: String,
    targets: Arc<Mutex<Vec<String>>>,
    shutdown: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<Result<(), tonic::transport::Error>>,
}

impl TestServer {
    async fn start(backend: Arc<ProbeBackend>) -> Self {
        let definition = IndexDefinition::new()
            .field(Field::text("title").stored(true))
            .field(Field::float64("price").stored(true).filterable(true));
        let local = Index::from_backend(definition, backend).expect("valid probe index");
        let (context, session) = local.into_session_parts();
        let definition = context.definition().clone();
        let dispatcher = Arc::new(Dispatcher::new(context, session));
        let targets = Arc::new(Mutex::new(Vec::new()));

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind probe server");
        let address = listener.local_addr().expect("probe address");
        let incoming = TcpListenerStream::new(listener);
        let (shutdown, shutdown_rx) = oneshot::channel();
        let service = SearchKernelServer::new(TestService {
            dispatcher,
            definition,
            targets: Arc::clone(&targets),
        })
        .max_decoding_message_size(MAX_GRPC_MESSAGE_SIZE)
        .max_encoding_message_size(MAX_GRPC_MESSAGE_SIZE);
        let task = tokio::spawn(
            tonic::transport::Server::builder()
                .add_service(service)
                .serve_with_incoming_shutdown(incoming, async {
                    let _ = shutdown_rx.await;
                }),
        );
        Self {
            endpoint: address.to_string(),
            targets,
            shutdown: Some(shutdown),
            task,
        }
    }

    async fn shutdown(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.task
            .await
            .expect("probe server task")
            .expect("probe server shutdown");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn same_index_facade_executes_crud_search_and_analysis_over_tcp() {
    let backend = Arc::new(ProbeBackend::default());
    let server = TestServer::start(Arc::clone(&backend)).await;

    // Plain host:port is the common ergonomic form; the client supplies the
    // transport URI internally.
    let client = RemoteClient::connect(&server.endpoint).expect("connect remote daemon");
    let index = client.open_index("probe").expect("open remote index");

    let product = Product {
        title: "Remote Rust".to_owned(),
        price: 42.5,
    };
    index.insert("doc-1", &product).expect("remote insert");
    index.upsert("doc-1", &product).expect("remote upsert");
    index
        .patch("doc-1", Patch::new().set("price", 42.5))
        .expect("remote patch");

    let fetched = index
        .get::<Product>("doc-1")
        .expect("remote get")
        .expect("document exists");
    assert_eq!(fetched, product);

    let result = index
        .search("remote rust")
        .filter(field("price").lte(100.0))
        .limit(5)
        .execute::<Product>()
        .expect("remote search");
    assert_eq!(result.total_hits.value, 1);
    assert_eq!(result.hits.len(), 1);
    assert_eq!(result.hits[0].document, Some(product));
    assert_eq!(result.hits[0].score, Some(3.5));

    let analyzed = index
        .analyze("Remote")
        .field("title")
        .execute()
        .expect("remote analysis");
    assert_eq!(analyzed.tokens[0].term, "remote");

    assert!(index.delete("doc-1").expect("remote delete"));
    index.close().expect("close remote index");

    assert!(server
        .targets
        .lock()
        .expect("target log")
        .iter()
        .all(|target| target == "probe"));

    let calls = backend.calls();
    assert!(matches!(calls[0], OperationIr::Insert(_)));
    assert!(matches!(calls[1], OperationIr::Upsert(_)));
    assert!(matches!(calls[2], OperationIr::Patch(_)));
    assert!(matches!(calls[3], OperationIr::Get(_)));
    assert!(matches!(calls[4], OperationIr::Search(_)));
    assert!(matches!(calls[5], OperationIr::Analyze(_)));
    assert!(matches!(calls[6], OperationIr::Delete(_)));

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn daemon_lifecycle_uses_canonical_results_and_rejects_wrong_response_types() {
    let backend = Arc::new(ProbeBackend::default());
    let server = TestServer::start(Arc::clone(&backend)).await;
    let client = RemoteClient::connect(&server.endpoint).expect("connect remote daemon");
    let definition = || {
        IndexDefinition::new()
            .field(Field::text("title").stored(true))
            .field(Field::float64("price").stored(true).filterable(true))
    };

    let first = client
        .create_index("first", definition())
        .expect("decode create result");
    let second = client
        .create_index("second", definition())
        .expect("create second target");
    first
        .insert(
            "first-doc",
            &Product {
                title: "first".into(),
                price: 1.0,
            },
        )
        .expect("insert first");
    second
        .insert(
            "second-doc",
            &Product {
                title: "second".into(),
                price: 2.0,
            },
        )
        .expect("insert second");
    first.close().expect("close first handle");
    second.close().expect("close second handle");

    let opened = client.open_index("first").expect("decode open result");
    opened.close().expect("close opened handle");
    client.delete_index("first").expect("decode delete result");
    let error = client
        .open_index("wrong-response")
        .err()
        .expect("reject mismatched lifecycle result");
    assert_eq!(error.code(), PublicErrorCode::Internal);
    assert!(error
        .details()
        .iter()
        .any(|detail| detail.code == "invalid_remote_response"));

    let targets = server.targets.lock().expect("target log").clone();
    assert!(targets.contains(&"first".to_owned()));
    assert!(targets.contains(&"second".to_owned()));
    assert!(targets.contains(&"wrong-response".to_owned()));
    assert!(targets.iter().all(|target| !target.is_empty()));
    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn portable_errors_survive_the_remote_boundary_without_becoming_transport_errors() {
    let backend = Arc::new(ProbeBackend::default());
    let server = TestServer::start(Arc::clone(&backend)).await;
    let index = RemoteClient::connect(&server.endpoint)
        .expect("connect remote daemon")
        .open_index("probe")
        .expect("open remote index");

    backend.search_mode(1);
    let error = index
        .search("rejected")
        .execute::<Product>()
        .expect_err("backend rejection must remain an error");
    assert_eq!(error.code(), PublicErrorCode::InvalidArgument);
    assert_eq!(error.details()[0].code, "remote_probe_rejected");
    assert_eq!(error.details()[0].path.as_deref(), Some("search.query"));
    assert!(!error.retryable());
    assert!(
        error
            .details()
            .iter()
            .all(|detail| detail.code != "grpc_transport"),
        "portable backend errors must not be relabeled as transport failures"
    );

    index.close().expect("close remote index");
    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn request_deadlines_are_transport_errors_and_do_not_require_an_async_rust_api() {
    let backend = Arc::new(ProbeBackend::default());
    let server = TestServer::start(Arc::clone(&backend)).await;
    let client = RemoteClient::connect(&server.endpoint).expect("connect remote daemon");
    let warmup = client
        .open_index("probe")
        .expect("warm up remote connection");
    warmup.close().expect("close warmup remote connection");
    let options = RemoteOptions::default().request_timeout(Duration::from_millis(500));
    let index = RemoteClient::connect_with(&server.endpoint, options)
        .expect("connect remote daemon")
        .open_index("probe")
        .expect("open remote index");

    backend.search_mode(2);
    let error = index
        .search("slow")
        .execute::<Product>()
        .expect_err("slow remote request must time out");
    assert_eq!(error.code(), PublicErrorCode::DeadlineExceeded);
    assert!(error.retryable());
    assert_eq!(error.details()[0].code, "grpc_transport");

    index.close().expect("close remote index");
    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mutation_deadlines_do_not_claim_ambiguous_writes_are_retryable() {
    let backend = Arc::new(ProbeBackend::default());
    let server = TestServer::start(Arc::clone(&backend)).await;
    let options = RemoteOptions::default().request_timeout(Duration::from_millis(500));
    let index = RemoteClient::connect_with(&server.endpoint, options)
        .expect("connect remote daemon")
        .open_index("probe")
        .expect("open remote index");

    backend.insert_mode(1);
    let product = Product {
        title: "Remote Rust".to_owned(),
        price: 42.5,
    };
    let error = index
        .insert("ambiguous-write", &product)
        .expect_err("slow remote mutation must time out");
    assert_eq!(error.code(), PublicErrorCode::DeadlineExceeded);
    assert!(
        !error.retryable(),
        "a timed-out mutation may already have executed remotely"
    );
    assert_eq!(error.details()[0].code, "grpc_transport");
    assert!(
        error
            .details()
            .iter()
            .any(|detail| detail.code == "remote_outcome_unknown"),
        "timed-out mutations must expose ambiguous remote completion"
    );

    index.close().expect("close remote index");
    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn lifecycle_mutation_deadlines_report_ambiguous_create() {
    let backend = Arc::new(ProbeBackend::default());
    let server = TestServer::start(backend).await;
    let options = RemoteOptions::default().request_timeout(Duration::from_millis(500));
    let client =
        RemoteClient::connect_with(&server.endpoint, options).expect("connect remote daemon");
    let definition = IndexDefinition::new().field(Field::text("title").stored(true));

    let error = client
        .create_index("slow-create", definition)
        .err()
        .expect("slow lifecycle mutation must time out");
    assert_eq!(error.code(), PublicErrorCode::DeadlineExceeded);
    assert!(!error.retryable());
    assert_eq!(error.details()[0].code, "grpc_transport");
    assert!(error
        .details()
        .iter()
        .any(|detail| detail.code == "remote_outcome_unknown"));

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mismatched_mutation_response_identity_is_rejected() {
    let backend = Arc::new(ProbeBackend::default());
    let server = TestServer::start(Arc::clone(&backend)).await;
    let index = RemoteClient::connect(&server.endpoint)
        .expect("connect remote daemon")
        .open_index("probe")
        .expect("open remote index");

    backend.insert_mode(2);
    let product = Product {
        title: "Remote Rust".to_owned(),
        price: 42.5,
    };
    let error = index
        .insert("expected-id", &product)
        .expect_err("remote mutation must reject mismatched result identity");
    assert_eq!(error.code(), PublicErrorCode::Internal);
    assert!(error
        .details()
        .iter()
        .any(|detail| detail.code == "invalid_remote_response"));

    index.close().expect("close remote index");
    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mismatched_get_document_identity_is_rejected_before_deserialization() {
    let backend = Arc::new(ProbeBackend::default());
    let server = TestServer::start(Arc::clone(&backend)).await;
    let index = RemoteClient::connect(&server.endpoint)
        .expect("connect remote daemon")
        .open_index("probe")
        .expect("open remote index");

    backend.get_mode(1);
    let error = index
        .get::<Product>("expected-document")
        .expect_err("remote GET must reject a different response document ID");
    assert_eq!(error.code(), PublicErrorCode::Internal);
    assert!(error
        .details()
        .iter()
        .any(|detail| detail.code == "invalid_remote_response"));

    index.close().expect("close remote index");
    server.shutdown().await;
}
