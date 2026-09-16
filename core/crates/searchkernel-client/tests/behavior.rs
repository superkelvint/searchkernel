use searchkernel_client::model::{
    AnalyzeRequest, DeleteRequest, DescribeRequest, GetRequest, InsertRequest, InspectQueryRequest,
    PatchRequest, SearchRequest, UpsertRequest,
};
use searchkernel_client::{Client, Transport};
use std::path::Path;
use std::sync::{Arc, Mutex};

#[allow(warnings)]
#[path = "../../searchkernel-ffi/generated/mod.rs"]
mod ffi_generated;

use ffi_generated::search_kernel::ffi::v1 as wire;

#[derive(Debug)]
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

    fn take_request(&self) -> Vec<u8> {
        let mut state = self.0.lock().unwrap();
        assert_eq!(state.requests.len(), 1);
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
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../searchkernel-json/tests/fixtures/responses");
    std::fs::read(root.join(name)).unwrap()
}

fn request_kind(bytes: &[u8]) -> wire::RequestPayload {
    assert_eq!(&bytes[4..8], b"SKRQ");
    flatbuffers::root::<wire::RequestEnvelope<'_>>(bytes)
        .unwrap()
        .payload_type()
}

macro_rules! fixture_case {
    ($method:ident, $request:ty, $kind:ident, $fixture:literal) => {{
        let transport = ScriptedTransport::new(fixture($fixture));
        let client = Client::new(transport.clone());
        client.$method(<$request>::default()).unwrap();
        assert_eq!(
            request_kind(&transport.take_request()),
            wire::RequestPayload::$kind
        );
    }};
}

#[test]
fn all_generated_client_operations_encode_and_decode_matching_payloads() {
    fixture_case!(
        insert,
        InsertRequest,
        SearchKernel_V1_InsertRequest,
        "insert.skrs"
    );
    fixture_case!(
        upsert,
        UpsertRequest,
        SearchKernel_V1_UpsertRequest,
        "upsert.skrs"
    );
    fixture_case!(
        patch,
        PatchRequest,
        SearchKernel_V1_PatchRequest,
        "patch.skrs"
    );
    fixture_case!(
        delete,
        DeleteRequest,
        SearchKernel_V1_DeleteRequest,
        "delete.skrs"
    );
    fixture_case!(get, GetRequest, SearchKernel_V1_GetRequest, "get.skrs");
    fixture_case!(
        search,
        SearchRequest,
        SearchKernel_V1_SearchRequest,
        "search.skrs"
    );
    fixture_case!(
        analyze,
        AnalyzeRequest,
        SearchKernel_V1_AnalyzeRequest,
        "analyze.skrs"
    );
    fixture_case!(
        describe,
        DescribeRequest,
        SearchKernel_V1_DescribeRequest,
        "describe.skrs"
    );
    fixture_case!(
        inspect_query,
        InspectQueryRequest,
        SearchKernel_V1_InspectQueryRequest,
        "inspect_query.skrs"
    );
}
