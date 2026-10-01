use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use http::{header::CONTENT_TYPE, Method, Request as HttpRequest, StatusCode};
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper_util::{client::legacy::Client, rt::TokioExecutor};
use searchkernel::{Field, Index, IndexDefinition};
use searchkernel_dispatch::Dispatcher;
use searchkernel_protocol::protocol::v1::PROTOCOL_VERSION;
use searchkernel_ir::{Backend, PortableError, Result as BackendResult};
use searchkernel_protocol::json::{
    request_json_to_flatbuffer, response_flatbuffer_to_json, response_json_to_flatbuffer,
};
use searchkernel_protocol::protocol::v1 as wire;
use searchkernel_testkit::RecordingBackend;
use searchkerneld::{start, ServerConfig, JSON_EXECUTE_PATH};
use tokio::sync::Notify;

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

const DESCRIBE_JSON: &[u8] = br#"{
  "protocol_version": 1,
  "payload_type": "SearchKernel_Wire_DescribeRequest",
  "payload": {}
}"#;

const SEARCH_JSON: &[u8] = br#"{
  "protocol_version": 1,
  "payload_type": "SearchKernel_Wire_SearchRequest",
  "payload": {
    "query": {
      "source_type": "SearchKernelTextQuery",
      "source": {"text": "headphones"}
    }
  }
}"#;

fn dispatcher(backend: Arc<dyn Backend>) -> (Index, Arc<Dispatcher>) {
    let definition =
        IndexDefinition::new().field(Field::text("title").stored(true).searchable(true));
    let index = Index::from_backend(definition, backend).expect("test index");
    let (context, session) = index.clone().into_session_parts();
    (index, Arc::new(Dispatcher::new(context, session)))
}

async fn execute_json_raw(
    address: SocketAddr,
    method: Method,
    content_type: &'static str,
    body: impl Into<Bytes>,
) -> (StatusCode, Option<String>, Vec<u8>) {
    let request = HttpRequest::builder()
        .method(method)
        .uri(format!("http://{address}{JSON_EXECUTE_PATH}"))
        .header(CONTENT_TYPE, content_type)
        .body(Full::new(body.into()))
        .expect("build HTTP JSON request");
    let client = Client::builder(TokioExecutor::new()).build_http();
    let response = client.request(request).await.expect("HTTP JSON request");
    let status = response.status();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let body = response
        .into_body()
        .collect()
        .await
        .expect("read HTTP JSON response")
        .to_bytes()
        .to_vec();
    (status, content_type, body)
}

#[tokio::test]
async fn http_json_uses_the_canonical_flatbuffers_json_boundary() {
    let backend = Arc::new(RecordingBackend::new());
    let (index, dispatcher) = dispatcher(backend);
    let request = request_json_to_flatbuffer(DESCRIBE_JSON).expect("canonical request JSON");
    let direct = dispatcher.dispatch(&request).expect("direct dispatch");
    let direct_json = response_flatbuffer_to_json(&direct).expect("canonical response JSON");

    let server = start(
        ServerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0)))
            .with_web_listen_addr(SocketAddr::from(([127, 0, 0, 1], 0))),
        dispatcher,
    )
    .await
    .expect("start HTTP edge");
    let address = server.web_tcp_addr().expect("HTTP edge address");

    let (status, content_type, body) =
        execute_json_raw(address, Method::POST, "application/json", DESCRIBE_JSON).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type.as_deref(), Some("application/json"));
    assert_eq!(body, direct_json);

    let decoded = response_json_to_flatbuffer(&body).expect("response remains standard FB JSON");
    let envelope =
        flatbuffers::root::<wire::ResponseEnvelope<'_>>(&decoded).expect("response envelope");
    assert_eq!(envelope.protocol_version(), PROTOCOL_VERSION);
    assert_eq!(
        envelope.payload_type(),
        wire::ResponsePayload::SearchKernel_Wire_DescribeResult
    );

    server.shutdown().await.expect("shutdown HTTP edge");
    index.close().expect("close test index");
}

#[tokio::test]
async fn http_json_and_direct_dispatch_produce_identical_ir_calls() {
    let backend = Arc::new(RecordingBackend::new());
    let (index, dispatcher) = dispatcher(backend.clone());
    let request = request_json_to_flatbuffer(SEARCH_JSON).expect("canonical search JSON");

    dispatcher.dispatch(&request).expect("direct dispatch");
    let direct_calls = backend.take_calls();
    assert_eq!(direct_calls.len(), 1);

    let server = start(
        ServerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0)))
            .with_web_listen_addr(SocketAddr::from(([127, 0, 0, 1], 0))),
        dispatcher,
    )
    .await
    .expect("start HTTP edge");
    let address = server.web_tcp_addr().expect("HTTP edge address");

    let (status, _, body) =
        execute_json_raw(address, Method::POST, "application/json", SEARCH_JSON).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "semantic errors stay response payloads"
    );
    assert!(!body.is_empty());
    let decoded =
        response_json_to_flatbuffer(&body).expect("semantic error response remains canonical JSON");
    let envelope =
        flatbuffers::root::<wire::ResponseEnvelope<'_>>(&decoded).expect("semantic error envelope");
    assert_eq!(
        envelope.payload_type(),
        wire::ResponsePayload::SearchKernel_Wire_Error
    );

    let remote_calls = backend.take_calls();
    assert_eq!(
        direct_calls, remote_calls,
        "HTTP JSON must add representation only"
    );

    server.shutdown().await.expect("shutdown HTTP edge");
    index.close().expect("close test index");
}

#[tokio::test]
async fn http_json_rejects_edge_failures_before_dispatch() {
    let backend = Arc::new(RecordingBackend::new());
    let (index, dispatcher) = dispatcher(backend.clone());
    let server = start(
        ServerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0)))
            .with_web_listen_addr(SocketAddr::from(([127, 0, 0, 1], 0))),
        dispatcher,
    )
    .await
    .expect("start HTTP edge");
    let address = server.web_tcp_addr().expect("HTTP edge address");

    let (status, _, _) = execute_json_raw(
        address,
        Method::POST,
        "application/json",
        Bytes::from_static(b"{"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _, _) = execute_json_raw(address, Method::POST, "text/plain", DESCRIBE_JSON).await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);

    let (status, _, _) =
        execute_json_raw(address, Method::GET, "application/json", DESCRIBE_JSON).await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);

    assert!(
        backend.take_calls().is_empty(),
        "edge-level failures must not reach canonical semantics"
    );

    server.shutdown().await.expect("shutdown HTTP edge");
    index.close().expect("close test index");
}

#[tokio::test]
async fn http_json_admission_limit_rejects_a_queued_request() {
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
    .expect("start HTTP edge");
    let address = server.web_tcp_addr().expect("HTTP edge address");

    let started = backend.started.notified();
    let first = tokio::spawn(async move {
        execute_json_raw(address, Method::POST, "application/json", SEARCH_JSON).await
    });
    started.await;

    let second = tokio::time::timeout(
        Duration::from_secs(2),
        execute_json_raw(address, Method::POST, "application/json", SEARCH_JSON),
    )
    .await
    .expect("over-capacity JSON request must be rejected promptly");
    assert_eq!(second.0, StatusCode::TOO_MANY_REQUESTS);

    backend.release.store(true, Ordering::Release);
    assert_eq!(first.await.expect("first JSON task").0, StatusCode::OK);

    server.shutdown().await.expect("shutdown HTTP edge");
    index.close().expect("close test index");
}

#[tokio::test]
async fn http_json_execution_limit_serializes_backend_dispatch() {
    let backend = BlockingBackend::new();
    let (index, dispatcher) = dispatcher(backend.clone());
    let server = start(
        ServerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0)))
            .with_web_listen_addr(SocketAddr::from(([127, 0, 0, 1], 0)))
            .with_execution_limit(1)
            .with_admission_limit(2),
        dispatcher,
    )
    .await
    .expect("start HTTP edge");
    let address = server.web_tcp_addr().expect("HTTP edge address");

    let started = backend.started.notified();
    let first = tokio::spawn(async move {
        execute_json_raw(address, Method::POST, "application/json", SEARCH_JSON).await
    });
    started.await;

    let second = tokio::spawn(async move {
        execute_json_raw(address, Method::POST, "application/json", SEARCH_JSON).await
    });

    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        backend.searches.load(Ordering::SeqCst),
        1,
        "the second JSON dispatch must wait for the execution permit"
    );
    assert!(!second.is_finished());

    backend.release.store(true, Ordering::Release);
    assert_eq!(first.await.expect("first JSON task").0, StatusCode::OK);
    assert_eq!(second.await.expect("second JSON task").0, StatusCode::OK);
    assert_eq!(backend.searches.load(Ordering::SeqCst), 2);

    server.shutdown().await.expect("shutdown HTTP edge");
    index.close().expect("close test index");
}
