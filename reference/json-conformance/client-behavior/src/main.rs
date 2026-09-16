use flatbuffers::{FlatBufferBuilder, UnionWIPOffset, WIPOffset};
use searchkernel_client::model::{
    AnalyzeRequest, AnalyzeTarget, DeleteRequest, DescribeRequest, FacetRequest, FilterExpr,
    GetRequest, HighlightRequest, InsertRequest, InspectQueryRequest, PatchOperation, PatchRequest,
    QueryExpr, RankingPolicy, SearchField, SearchFieldSelection, SearchRequest, SortExpr,
    UpsertRequest, Value, ComparisonOp, FacetSort, MissingPlacement, SortDirection,
};
use searchkernel_client::{
    AsyncClient, AsyncTransport, Client, ClientError, Transport,
};
use std::collections::BTreeMap;
use std::future::Future;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};

#[allow(warnings)]
#[path = "../../../../core/crates/searchkernel-ffi/generated/mod.rs"]
mod ffi_generated;

use ffi_generated::search_kernel::ffi::v1 as wire;

#[derive(Debug, Clone)]
struct TestError;

#[derive(Default)]
struct State {
    response: Vec<u8>,
    requests: Vec<Vec<u8>>,
}

#[derive(Clone)]
struct ScriptedTransport(Arc<Mutex<State>>);

impl ScriptedTransport {
    fn new(response: Vec<u8>) -> Self {
        Self(Arc::new(Mutex::new(State {
            response,
            requests: Vec::new(),
        })))
    }

    fn take_single_request(&self) -> Vec<u8> {
        let mut state = self.0.lock().unwrap();
        assert_eq!(
            state.requests.len(),
            1,
            "each client operation must make exactly one Transport::call"
        );
        state.requests.pop().unwrap()
    }
}

impl Transport for ScriptedTransport {
    type Error = TestError;

    fn call(&self, request: &[u8]) -> Result<Vec<u8>, Self::Error> {
        let mut state = self.0.lock().unwrap();
        state.requests.push(request.to_vec());
        Ok(state.response.clone())
    }
}

impl AsyncTransport for ScriptedTransport {
    type Error = TestError;

    async fn call(&self, request: &[u8]) -> Result<Vec<u8>, Self::Error> {
        let mut state = self.0.lock().unwrap();
        state.requests.push(request.to_vec());
        Ok(state.response.clone())
    }
}

struct NoopWaker;

impl Wake for NoopWaker {
    fn wake(self: Arc<Self>) {}
}

fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(NoopWaker));
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(future);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("scripted async transport unexpectedly suspended"),
    }
}

fn fixture(name: &str) -> Vec<u8> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../core/crates/searchkernel-json/tests/fixtures/responses");
    std::fs::read(root.join(name)).unwrap()
}

fn request_kind(bytes: &[u8]) -> wire::RequestPayload {
    assert!(
        bytes.len() >= 8 && &bytes[4..8] == b"SKRQ",
        "client request must carry the SKRQ file identifier"
    );
    flatbuffers::root::<wire::RequestEnvelope<'_>>(bytes)
        .expect("valid SKRQ")
        .payload_type()
}

fn decoded_request(bytes: &[u8]) -> serde_json::Value {
    let json = searchkernel_json::request_flatbuffer_to_json(bytes)
        .expect("client request must decode as canonical JSON");
    serde_json::from_slice(&json).expect("canonical request JSON must be valid")
}

fn assert_request(bytes: &[u8], expected: serde_json::Value) {
    assert_eq!(decoded_request(bytes), expected);
}

fn exists_filter(field: &str) -> FilterExpr {
    FilterExpr::Exists {
        field: field.to_owned(),
    }
}

fn comparison_filter(field: &str, op: ComparisonOp, value: Value) -> FilterExpr {
    FilterExpr::Comparison {
        field: field.to_owned(),
        op,
        value,
    }
}

fn range_filter(field: &str, lower: Value, upper: Value) -> FilterExpr {
    FilterExpr::Range {
        field: field.to_owned(),
        lower: Some(lower),
        upper: Some(upper),
        include_lower: Some(true),
        include_upper: Some(false),
    }
}

fn lower_only_range_filter(field: &str, lower: Value) -> FilterExpr {
    FilterExpr::Range {
        field: field.to_owned(),
        lower: Some(lower),
        upper: None,
        include_lower: None,
        include_upper: None,
    }
}

fn term_query(field: &str, value: Value) -> QueryExpr {
    QueryExpr::Term {
        field: field.to_owned(),
        value,
    }
}

fn finish_response<'a>(
    mut builder: FlatBufferBuilder<'a>,
    kind: wire::ResponsePayload,
    payload: WIPOffset<UnionWIPOffset>,
) -> Vec<u8> {
    let root = wire::ResponseEnvelope::create(
        &mut builder,
        &wire::ResponseEnvelopeArgs {
            protocol_version: 1,
            payload_type: kind,
            payload: Some(payload),
        },
    );
    builder.finish(root, Some(wire::RESPONSE_ENVELOPE_IDENTIFIER));
    builder.finished_data().to_vec()
}

fn insert_response(marker: &str) -> Vec<u8> {
    let mut builder = FlatBufferBuilder::new();
    let id = builder.create_string(marker);
    let result = wire::InsertResult::create(&mut builder, &wire::InsertResultArgs { id: Some(id) });
    let payload = result.as_union_value();
    finish_response(
        builder,
        wire::ResponsePayload::SearchKernel_V1_InsertResult,
        payload,
    )
}

fn upsert_response(marker: &str) -> Vec<u8> {
    let mut builder = FlatBufferBuilder::new();
    let id = builder.create_string(marker);
    let result = wire::UpsertResult::create(&mut builder, &wire::UpsertResultArgs { id: Some(id) });
    let payload = result.as_union_value();
    finish_response(
        builder,
        wire::ResponsePayload::SearchKernel_V1_UpsertResult,
        payload,
    )
}

fn patch_response(marker: &str) -> Vec<u8> {
    let mut builder = FlatBufferBuilder::new();
    let id = builder.create_string(marker);
    let result = wire::PatchResult::create(&mut builder, &wire::PatchResultArgs { id: Some(id) });
    let payload = result.as_union_value();
    finish_response(
        builder,
        wire::ResponsePayload::SearchKernel_V1_PatchResult,
        payload,
    )
}

fn delete_response(marker: &str) -> Vec<u8> {
    let mut builder = FlatBufferBuilder::new();
    let id = builder.create_string(marker);
    let result = wire::DeleteResult::create(
        &mut builder,
        &wire::DeleteResultArgs {
            id: Some(id),
            deleted: true,
        },
    );
    let payload = result.as_union_value();
    finish_response(
        builder,
        wire::ResponsePayload::SearchKernel_V1_DeleteResult,
        payload,
    )
}

fn get_response(marker: &str) -> Vec<u8> {
    let mut builder = FlatBufferBuilder::new();
    let id = builder.create_string(marker);
    let document = wire::Document::create(
        &mut builder,
        &wire::DocumentArgs {
            id: Some(id),
            fields: None,
        },
    );
    let result = wire::GetResult::create(
        &mut builder,
        &wire::GetResultArgs {
            document: Some(document),
        },
    );
    let payload = result.as_union_value();
    finish_response(
        builder,
        wire::ResponsePayload::SearchKernel_V1_GetResult,
        payload,
    )
}

fn search_response(marker: &str) -> Vec<u8> {
    let mut builder = FlatBufferBuilder::new();
    let id = builder.create_string(marker);
    let score = wire::OptionalDouble::new(7.25);
    let hit = wire::Hit::create(
        &mut builder,
        &wire::HitArgs {
            id: Some(id),
            score: Some(&score),
            fields: None,
            highlights: None,
        },
    );
    let hits = builder.create_vector(&[hit]);
    let total_hits = wire::TotalHits::create(
        &mut builder,
        &wire::TotalHitsArgs {
            value: 37,
            relation: wire::TotalHitsRelation::Exact,
        },
    );
    let result = wire::SearchResult::create(
        &mut builder,
        &wire::SearchResultArgs {
            hits: Some(hits),
            total_hits: Some(total_hits),
            facets: None,
        },
    );
    let payload = result.as_union_value();
    finish_response(
        builder,
        wire::ResponsePayload::SearchKernel_V1_SearchResult,
        payload,
    )
}

fn analyze_response(marker: &str) -> Vec<u8> {
    let mut builder = FlatBufferBuilder::new();
    let surface = builder.create_string(marker);
    let term_text = format!("normalized-{marker}");
    let term = builder.create_string(&term_text);
    let token = wire::AnalyzedToken::create(
        &mut builder,
        &wire::AnalyzedTokenArgs {
            surface: Some(surface),
            term: Some(term),
            position: 3,
            start_byte: 4,
            end_byte: 11,
        },
    );
    let tokens = builder.create_vector(&[token]);
    let result = wire::AnalyzeResult::create(
        &mut builder,
        &wire::AnalyzeResultArgs {
            tokens: Some(tokens),
        },
    );
    let payload = result.as_union_value();
    finish_response(
        builder,
        wire::ResponsePayload::SearchKernel_V1_AnalyzeResult,
        payload,
    )
}

fn describe_response(marker: &str) -> Vec<u8> {
    let mut builder = FlatBufferBuilder::new();
    let name = builder.create_string(marker);
    let index = wire::IndexDefinition::create(
        &mut builder,
        &wire::IndexDefinitionArgs {
            name: Some(name),
            fields: None,
            default_search_fields: None,
        },
    );
    let result = wire::DescribeResult::create(
        &mut builder,
        &wire::DescribeResultArgs { index: Some(index) },
    );
    let payload = result.as_union_value();
    finish_response(
        builder,
        wire::ResponsePayload::SearchKernel_V1_DescribeResult,
        payload,
    )
}

fn inspect_query_response(marker: &str) -> Vec<u8> {
    let mut builder = FlatBufferBuilder::new();
    let query = builder.create_string(marker);
    let limit = wire::OptionalUInt32::new(17);
    let request = wire::SearchRequest::create(
        &mut builder,
        &wire::SearchRequestArgs {
            query: Some(query),
            limit: Some(&limit),
            ..Default::default()
        },
    );
    let result = wire::InspectQueryResult::create(
        &mut builder,
        &wire::InspectQueryResultArgs {
            resolved_request: Some(request),
        },
    );
    let payload = result.as_union_value();
    finish_response(
        builder,
        wire::ResponsePayload::SearchKernel_V1_InspectQueryResult,
        payload,
    )
}

macro_rules! payload_decode_case {
    ($method:ident, $request:ty, $kind:ident, $response_builder:ident) => {{
        let transport_a = ScriptedTransport::new($response_builder("c6-alpha"));
        let client_a = Client::new(transport_a.clone());
        let result_a = client_a.$method(<$request>::default()).unwrap_or_else(|_| {
            panic!(
                "{} failed to decode first matching SKRS",
                stringify!($method)
            )
        });
        assert_ne!(
            result_a,
            Default::default(),
            "{} returned Default instead of decoding its SKRS payload",
            stringify!($method)
        );
        let request_a = transport_a.take_single_request();
        assert_eq!(
            request_kind(&request_a),
            wire::RequestPayload::$kind,
            "wrong SKRQ for {}",
            stringify!($method)
        );

        let transport_b = ScriptedTransport::new($response_builder("c6-beta"));
        let client_b = Client::new(transport_b.clone());
        let result_b = client_b.$method(<$request>::default()).unwrap_or_else(|_| {
            panic!(
                "{} failed to decode second matching SKRS",
                stringify!($method)
            )
        });
        assert_ne!(
            result_b,
            Default::default(),
            "{} returned Default instead of decoding its second SKRS payload",
            stringify!($method)
        );
        assert_ne!(
            result_a,
            result_b,
            "{} result does not depend on the SKRS payload; response data was discarded",
            stringify!($method)
        );
        let request_b = transport_b.take_single_request();
        assert_eq!(
            request_kind(&request_b),
            wire::RequestPayload::$kind,
            "wrong SKRQ for second {} call",
            stringify!($method)
        );
    }};
}

macro_rules! request_shape_case {
    ($method:ident, $request:expr, $response_builder:ident, $expected:expr) => {{
        let transport = ScriptedTransport::new($response_builder("nested-request"));
        let client = Client::new(transport.clone());
        client
            .$method($request)
            .unwrap_or_else(|_| panic!("{} failed for non-default request", stringify!($method)));
        let request = transport.take_single_request();
        assert_request(&request, $expected);
    }};
}

macro_rules! async_payload_decode_case {
    ($method:ident, $request:ty, $kind:ident, $response_builder:ident) => {{
        let transport_a = ScriptedTransport::new($response_builder("async-alpha"));
        let client_a = AsyncClient::new(transport_a.clone());
        let result_a = block_on(client_a.$method(<$request>::default())).unwrap_or_else(|_| {
            panic!(
                "async {} failed to decode first matching SKRS",
                stringify!($method)
            )
        });
        assert_ne!(
            result_a,
            Default::default(),
            "async {} returned Default instead of decoding its SKRS payload",
            stringify!($method)
        );
        assert_eq!(
            request_kind(&transport_a.take_single_request()),
            wire::RequestPayload::$kind,
            "wrong async SKRQ for {}",
            stringify!($method)
        );

        let transport_b = ScriptedTransport::new($response_builder("async-beta"));
        let client_b = AsyncClient::new(transport_b.clone());
        let result_b = block_on(client_b.$method(<$request>::default())).unwrap_or_else(|_| {
            panic!(
                "async {} failed to decode second matching SKRS",
                stringify!($method)
            )
        });
        assert_ne!(
            result_a,
            result_b,
            "async {} result does not depend on the SKRS payload",
            stringify!($method)
        );
        assert_eq!(
            request_kind(&transport_b.take_single_request()),
            wire::RequestPayload::$kind,
            "wrong async SKRQ for second {} call",
            stringify!($method)
        );
    }};
}

fn async_conformance() {
    async_payload_decode_case!(
        insert,
        InsertRequest,
        SearchKernel_V1_InsertRequest,
        insert_response
    );
    async_payload_decode_case!(
        upsert,
        UpsertRequest,
        SearchKernel_V1_UpsertRequest,
        upsert_response
    );
    async_payload_decode_case!(
        patch,
        PatchRequest,
        SearchKernel_V1_PatchRequest,
        patch_response
    );
    async_payload_decode_case!(
        delete,
        DeleteRequest,
        SearchKernel_V1_DeleteRequest,
        delete_response
    );
    async_payload_decode_case!(get, GetRequest, SearchKernel_V1_GetRequest, get_response);
    async_payload_decode_case!(
        search,
        SearchRequest,
        SearchKernel_V1_SearchRequest,
        search_response
    );
    async_payload_decode_case!(
        analyze,
        AnalyzeRequest,
        SearchKernel_V1_AnalyzeRequest,
        analyze_response
    );
    async_payload_decode_case!(
        describe,
        DescribeRequest,
        SearchKernel_V1_DescribeRequest,
        describe_response
    );
    async_payload_decode_case!(
        inspect_query,
        InspectQueryRequest,
        SearchKernel_V1_InspectQueryRequest,
        inspect_query_response
    );

    let transport = ScriptedTransport::new(fixture("error-00.skrs"));
    let client = AsyncClient::new(transport.clone());
    let error = block_on(client.search(SearchRequest::default())).unwrap_err();
    assert!(matches!(error, ClientError::Portable(_)));
    assert_eq!(
        request_kind(&transport.take_single_request()),
        wire::RequestPayload::SearchKernel_V1_SearchRequest
    );

    let transport = ScriptedTransport::new(insert_response("async-wrong-operation"));
    let client = AsyncClient::new(transport.clone());
    let error = block_on(client.search(SearchRequest::default())).unwrap_err();
    assert!(matches!(error, ClientError::Protocol(_)));
    let _ = transport.take_single_request();

    struct FailingAsyncTransport;

    impl AsyncTransport for FailingAsyncTransport {
        type Error = TestError;

        async fn call(&self, _: &[u8]) -> Result<Vec<u8>, Self::Error> {
            Err(TestError)
        }
    }

    let client = AsyncClient::new(FailingAsyncTransport);
    let error = block_on(client.search(SearchRequest::default())).unwrap_err();
    assert!(matches!(error, ClientError::Transport(_)));
}

fn main() {
    async_conformance();

    payload_decode_case!(
        insert,
        InsertRequest,
        SearchKernel_V1_InsertRequest,
        insert_response
    );
    payload_decode_case!(
        upsert,
        UpsertRequest,
        SearchKernel_V1_UpsertRequest,
        upsert_response
    );
    payload_decode_case!(
        patch,
        PatchRequest,
        SearchKernel_V1_PatchRequest,
        patch_response
    );
    payload_decode_case!(
        delete,
        DeleteRequest,
        SearchKernel_V1_DeleteRequest,
        delete_response
    );
    payload_decode_case!(get, GetRequest, SearchKernel_V1_GetRequest, get_response);
    payload_decode_case!(
        search,
        SearchRequest,
        SearchKernel_V1_SearchRequest,
        search_response
    );
    payload_decode_case!(
        analyze,
        AnalyzeRequest,
        SearchKernel_V1_AnalyzeRequest,
        analyze_response
    );
    payload_decode_case!(
        describe,
        DescribeRequest,
        SearchKernel_V1_DescribeRequest,
        describe_response
    );
    payload_decode_case!(
        inspect_query,
        InspectQueryRequest,
        SearchKernel_V1_InspectQueryRequest,
        inspect_query_response
    );

    // Non-default requests must preserve their complete portable structure.
    request_shape_case!(
        patch,
        PatchRequest {
            id: Some("sku-42".to_owned()),
            operations: vec![
                PatchOperation::Set {
                    field: Some("price".to_owned()),
                    value: Some(Value::Float64(399.99)),
                },
                PatchOperation::Unset {
                    field: Some("image_url".to_owned()),
                },
            ],
        },
        patch_response,
        serde_json::json!({"protocol_version": 1, "request": {"patch": {
            "id": "sku-42",
            "operations": [
                {"set_field": {"field": "price", "value": {"float64": 399.99}}},
                {"unset_field": {"field": "image_url"}}
            ]
        }}})
    );

    request_shape_case!(
        search,
        SearchRequest {
            query: Some("x".to_owned()),
            filter: Some(FilterExpr::And(vec![exists_filter("brand")])),
            ..Default::default()
        },
        search_response,
        serde_json::json!({"protocol_version": 1, "request": {"search": {
            "query": "x",
            "filter": {"filter_and": {"clauses": [{"filter_exists": {"field": "brand"}}]}}
        }}})
    );
    request_shape_case!(
        search,
        SearchRequest {
            query: Some("x".to_owned()),
            filter: Some(lower_only_range_filter("price", Value::Float64(10.0))),
            ..Default::default()
        },
        search_response,
        serde_json::json!({"protocol_version": 1, "request": {"search": {
            "query": "x",
            "filter": {"filter_range": {
                "field": "price", "lower": {"float64": 10.0}
            }}
        }}})
    );
    request_shape_case!(
        search,
        SearchRequest {
            query: Some("x".to_owned()),
            filter: Some(FilterExpr::Or(vec![
                exists_filter("brand"),
                exists_filter("price")
            ])),
            ..Default::default()
        },
        search_response,
        serde_json::json!({"protocol_version": 1, "request": {"search": {
            "query": "x",
            "filter": {"filter_or": {"clauses": [
                {"filter_exists": {"field": "brand"}},
                {"filter_exists": {"field": "price"}}
            ]}}
        }}})
    );
    request_shape_case!(
        search,
        SearchRequest {
            query: Some("x".to_owned()),
            filter: Some(FilterExpr::Not(Box::new(exists_filter("brand")))),
            ..Default::default()
        },
        search_response,
        serde_json::json!({"protocol_version": 1, "request": {"search": {
            "query": "x",
            "filter": {"filter_not": {"clause": {"filter_exists": {"field": "brand"}}}}
        }}})
    );
    request_shape_case!(
        search,
        SearchRequest {
            query: Some("x".to_owned()),
            filter: Some(comparison_filter(
                "brand",
                ComparisonOp::Eq,
                Value::String("Sony".to_owned())
            )),
            ..Default::default()
        },
        search_response,
        serde_json::json!({"protocol_version": 1, "request": {"search": {
            "query": "x",
            "filter": {"filter_comparison": {
                "field": "brand", "op": "eq", "value": {"string": "Sony"}
            }}
        }}})
    );
    request_shape_case!(
        search,
        SearchRequest {
            query: Some("x".to_owned()),
            filter: Some(range_filter(
                "price",
                Value::Float64(10.0),
                Value::Float64(100.0)
            )),
            ..Default::default()
        },
        search_response,
        serde_json::json!({"protocol_version": 1, "request": {"search": {
            "query": "x",
            "filter": {"filter_range": {
                "field": "price", "lower": {"float64": 10.0}, "upper": {"float64": 100.0},
                "include_lower": true, "include_upper": false
            }}
        }}})
    );
    request_shape_case!(
        search,
        SearchRequest {
            query: Some("x".to_owned()),
            filter: Some(exists_filter("brand")),
            ..Default::default()
        },
        search_response,
        serde_json::json!({"protocol_version": 1, "request": {"search": {
            "query": "x", "filter": {"filter_exists": {"field": "brand"}}
        }}})
    );

    request_shape_case!(
        search,
        SearchRequest {
            query: Some("x".to_owned()),
            query_expr: Some(term_query("title", Value::String("wireless".to_owned()))),
            ..Default::default()
        },
        search_response,
        serde_json::json!({"protocol_version": 1, "request": {"search": {
            "query": "x", "query_expr": {"term_query": {
                "field": "title", "value": {"string": "wireless"}
            }}
        }}})
    );
    request_shape_case!(
        search,
        SearchRequest {
            query_expr: Some(QueryExpr::And(vec![
                term_query("title", Value::String("wireless".to_owned())),
                term_query("brand", Value::String("Sony".to_owned())),
            ])),
            ..Default::default()
        },
        search_response,
        serde_json::json!({"protocol_version": 1, "request": {"search": {
            "query_expr": {"query_and": {"clauses": [
                {"term_query": {"field": "title", "value": {"string": "wireless"}}},
                {"term_query": {"field": "brand", "value": {"string": "Sony"}}}
            ]}}
        }}})
    );
    request_shape_case!(
        search,
        SearchRequest {
            query_expr: Some(QueryExpr::Or(vec![term_query(
                "title",
                Value::String("a".to_owned())
            )])),
            ..Default::default()
        },
        search_response,
        serde_json::json!({"protocol_version": 1, "request": {"search": {
            "query_expr": {"query_or": {"clauses": [
                {"term_query": {"field": "title", "value": {"string": "a"}}}
            ]}}
        }}})
    );
    request_shape_case!(
        search,
        SearchRequest {
            query_expr: Some(QueryExpr::Not(Box::new(term_query(
                "title",
                Value::String("wireless".to_owned()),
            )))),
            ..Default::default()
        },
        search_response,
        serde_json::json!({"protocol_version": 1, "request": {"search": {
            "query_expr": {"query_not": {"clause": {"term_query": {
                "field": "title", "value": {"string": "wireless"}
            }}}}
        }}})
    );

    request_shape_case!(
        search,
        SearchRequest {
            query: Some("x".to_owned()),
            ranking: Some(RankingPolicy::DefaultRelevanceV1),
            ..Default::default()
        },
        search_response,
        serde_json::json!({"protocol_version": 1, "request": {"search": {
            "query": "x", "ranking": {"default_relevance_v1": {}}
        }}})
    );
    request_shape_case!(
        search,
        SearchRequest {
            query: Some("x".to_owned()),
            ranking: Some(RankingPolicy::Bm25Ranking),
            ..Default::default()
        },
        search_response,
        serde_json::json!({"protocol_version": 1, "request": {"search": {
            "query": "x", "ranking": {"bm25_ranking": {}}
        }}})
    );

    request_shape_case!(
        analyze,
        AnalyzeRequest {
            target: AnalyzeTarget::Analyzer(searchkernel_client::model::Analyzer {
                name: Some("standard".to_owned()),
                version: 1,
                options: BTreeMap::new(),
            }),
            text: Some("Running".to_owned()),
            language: None,
        },
        analyze_response,
        serde_json::json!({"protocol_version": 1, "request": {"analyze": {
            "target": {"analyzer": {"name": "standard", "options": {}, "version": 1}},
            "text": "Running"
        }}})
    );
    request_shape_case!(
        analyze,
        AnalyzeRequest {
            target: AnalyzeTarget::Field("title".to_owned()),
            text: Some("Running".to_owned()),
            language: Some("en".to_owned()),
        },
        analyze_response,
        serde_json::json!({"protocol_version": 1, "request": {"analyze": {
            "target": {"field": {"field": "title"}},
            "text": "Running", "language": "en"
        }}})
    );

    request_shape_case!(
        inspect_query,
        InspectQueryRequest {
            request: Some(SearchRequest {
                query: Some("wireless".to_owned()),
                search_fields: Some(SearchFieldSelection {
                    fields: vec![SearchField {
                        field: "title".to_owned(),
                        weight: Some(2.0)
                    }]
                }),
                filter: Some(FilterExpr::Not(Box::new(exists_filter("brand")))),
                sort: vec![SortExpr {
                    field: "price".to_owned(),
                    direction: SortDirection::Desc,
                    missing: Some(MissingPlacement::First)
                }],
                facets: vec![FacetRequest {
                    field: "brand".to_owned(),
                    limit: Some(10),
                    sort: Some(FacetSort::CountDesc)
                }],
                ranking: Some(RankingPolicy::Bm25Ranking),
                limit: Some(5),
                offset: Some(20),
                projection: Some(searchkernel_client::model::FieldProjection {
                    fields: vec!["title".to_owned(), "price".to_owned()]
                }),
                highlighting: Some(HighlightRequest {
                    fields: vec!["title".to_owned()]
                }),
                language: Some("en".to_owned()),
                query_expr: Some(term_query("title", Value::String("wireless".to_owned()))),
            }),
        },
        inspect_query_response,
        serde_json::json!({"protocol_version": 1, "request": {"inspect_query": {
            "request": {
                "query": "wireless",
                "search_fields": [{"field": "title", "weight": 2.0}],
                "filter": {"filter_not": {"clause": {"filter_exists": {"field": "brand"}}}},
                "sort": [{"field": "price", "direction": "desc", "missing": "first"}],
                "facets": [{"field": "brand", "limit": 10, "sort": "count_desc"}],
                "ranking": {"bm25_ranking": {}},
                "limit": 5, "offset": 20,
                "projection": ["title", "price"],
                "highlighting": ["title"],
                "language": "en",
                "query_expr": {"term_query": {"field": "title", "value": {"string": "wireless"}}}
            }
        }}})
    );

    // Portable Search Kernel errors must remain portable client errors.
    let transport = ScriptedTransport::new(fixture("error-00.skrs"));
    let client = Client::new(transport.clone());
    let error = match client.search(SearchRequest::default()) {
        Ok(_) => panic!("portable error must fail"),
        Err(error) => error,
    };
    match error {
        ClientError::Portable(_) => {}
        ClientError::Transport(_) => panic!("portable error became transport error"),
        ClientError::Protocol(message) => {
            panic!("portable error became protocol error: {message}")
        }
    }
    assert_eq!(
        request_kind(&transport.take_single_request()),
        wire::RequestPayload::SearchKernel_V1_SearchRequest
    );

    // A valid SKRS for the wrong operation must be rejected as a protocol error.
    let transport = ScriptedTransport::new(insert_response("wrong-operation"));
    let client = Client::new(transport.clone());
    let error = match client.search(SearchRequest::default()) {
        Ok(_) => panic!("mismatched response must fail"),
        Err(error) => error,
    };
    match error {
        ClientError::Protocol(_) => {}
        ClientError::Portable(_) => {
            panic!("mismatched normal response became portable error")
        }
        ClientError::Transport(_) => {
            panic!("mismatched normal response became transport error")
        }
    }
    let _ = transport.take_single_request();

    // Transport failures must remain transport failures.
    struct FailingTransport;
    impl Transport for FailingTransport {
        type Error = TestError;

        fn call(&self, _: &[u8]) -> Result<Vec<u8>, Self::Error> {
            Err(TestError)
        }
    }

    let client = Client::new(FailingTransport);
    let error = match client.search(SearchRequest::default()) {
        Ok(_) => panic!("transport failure must fail"),
        Err(error) => error,
    };
    match error {
        ClientError::Transport(_) => {}
        ClientError::Portable(_) => panic!("transport failure became portable error"),
        ClientError::Protocol(_) => panic!("transport failure became protocol error"),
    }

    println!("C6 generated client/Transport behavioral contract PASS");
}
