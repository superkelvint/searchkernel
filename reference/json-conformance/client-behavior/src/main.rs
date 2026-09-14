use flatbuffers::{FlatBufferBuilder, UnionWIPOffset, WIPOffset};
use searchkernel_client::model::{
    AnalyzeRequest, DeleteRequest, DescribeRequest, GetRequest, InsertRequest,
    InspectQueryRequest, PatchRequest, SearchRequest, UpsertRequest,
};
use searchkernel_client::{Client, ClientError, Transport};
use std::path::Path;
use std::sync::{Arc, Mutex};

#[allow(warnings)]
#[path = "../../../../crates/searchkernel-ffi/generated/mod.rs"]
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

fn fixture(name: &str) -> Vec<u8> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../crates/searchkernel-json/tests/fixtures/responses");
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
        let result_a = client_a
            .$method(<$request>::default())
            .unwrap_or_else(|_| panic!("{} failed to decode first matching SKRS", stringify!($method)));
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
        let result_b = client_b
            .$method(<$request>::default())
            .unwrap_or_else(|_| panic!("{} failed to decode second matching SKRS", stringify!($method)));
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

fn main() {
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
