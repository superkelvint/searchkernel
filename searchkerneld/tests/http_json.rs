use std::{net::SocketAddr, sync::Arc};

use http::{header::CONTENT_TYPE, Method, Request as HttpRequest, StatusCode};
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper_util::{client::legacy::Client, rt::TokioExecutor};
use searchkernel::{Field, Index, IndexDefinition};
use searchkernel_dispatch::{Dispatcher, PROTOCOL_VERSION};
use searchkernel_ir::Backend;
use searchkernel_protocol::json::{
    request_json_to_flatbuffer, response_flatbuffer_to_json, response_json_to_flatbuffer,
};
use searchkernel_protocol::protocol::v2 as wire;
use searchkernel_testkit::RecordingBackend;
use searchkerneld::{start, ServerConfig, JSON_EXECUTE_PATH};

const DESCRIBE_JSON: &[u8] = br#"{
  "protocol_version": 2,
  "payload_type": "SearchKernel_V2_DescribeRequest",
  "payload": {}
}"#;

const SEARCH_JSON: &[u8] = br#"{
  "protocol_version": 2,
  "payload_type": "SearchKernel_V2_SearchRequest",
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
        wire::ResponsePayload::SearchKernel_V2_DescribeResult
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
