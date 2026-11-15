#![cfg(feature = "test-support")]

use std::sync::Arc;

use flatbuffers::root;
use searchkernel_dispatch::Dispatcher;
use searchkernel_ir::{IndexContext, OperationIr, QueryIr};
use searchkernel_protocol::{
    protocol::v1::{self as wire, PROTOCOL_VERSION},
    wire::v1 as api,
};
use searchkernel_testkit::RecordingBackend;
use searchkerneld::search_endpoints::{PublicSearchConfig, PublicSearchError, PublicSearchRequest};

fn config() -> searchkerneld::search_endpoints::SearchEndpointRouter {
    serde_json::from_str::<PublicSearchConfig>(
        r#"{
          "behindRateLimitedProxy": true,
          "endpoints": {
            "docs": {
              "target": "docs",
              "generation": "0123456789abcdef0123456789abcdef",
              "fields": ["title"],
              "filterFields": {},
              "facets": [],
              "highlights": ["title"]
            }
          }
        }"#,
    )
    .expect("valid production Search Endpoint config")
    .compile()
    .unwrap()
}

fn request(query: &str, limit: u32, offset: u32) -> PublicSearchRequest {
    serde_json::from_value(serde_json::json!({
        "endpoint": "docs",
        "query": query,
        "limit": limit,
        "offset": offset,
        "facets": [],
        "facetLimit": 100,
        "filters": [],
        "highlights": ["title"],
        "snippets": []
    }))
    .expect("valid production public request")
}

fn decode_search(bytes: &[u8]) -> api::SearchRequest<'_> {
    let envelope = wire::root_as_request_envelope(bytes).expect("valid canonical request envelope");
    assert_eq!(envelope.protocol_version(), PROTOCOL_VERSION);
    assert_eq!(
        envelope.target().and_then(|target| target.name()),
        Some("docs")
    );
    assert_eq!(
        envelope.payload_type(),
        wire::RequestPayload::SearchKernel_Wire_SearchRequest
    );
    envelope
        .payload_as_search_kernel_wire_search_request()
        .expect("search payload")
}

fn text_query(search: api::SearchRequest<'_>) -> String {
    search
        .query()
        .and_then(|query| query.source_as_search_kernel_text_query())
        .and_then(|query| query.text())
        .expect("text query")
        .to_owned()
}

fn context() -> Arc<IndexContext> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let name = builder.create_string("title");
    let primitive = api::PrimitiveDataType::create(
        &mut builder,
        &api::PrimitiveDataTypeArgs {
            kind: api::PrimitiveDataTypeKind::String,
        },
    );
    let data_type = api::DataType::create(
        &mut builder,
        &api::DataTypeArgs {
            kind_type: api::DataTypeKind::PrimitiveDataType,
            kind: Some(primitive.as_union_value()),
        },
    );
    let stored = api::OptionalBool::new(true);
    let searchable = api::OptionalBool::new(true);
    let field = api::FieldDefinition::create(
        &mut builder,
        &api::FieldDefinitionArgs {
            name: Some(name),
            data_type: Some(data_type),
            stored: Some(&stored),
            searchable: Some(&searchable),
            ..Default::default()
        },
    );
    let fields = builder.create_vector(&[field]);
    let definition = api::IndexDefinition::create(
        &mut builder,
        &api::IndexDefinitionArgs {
            fields: Some(fields),
            ..Default::default()
        },
    );
    builder.finish(definition, None);

    let definition =
        root::<api::IndexDefinition<'_>>(builder.finished_data()).expect("valid index definition");
    Arc::new(
        searchkernel_ir::wire::v1::index_context_from_api(definition).expect("valid index context"),
    )
}

#[test]
fn production_instantsearch_contract_binds_to_canonical_search_request() {
    let config = config();
    let bytes = config
        .bind_post(&request("search kernel", 20, 40))
        .expect("production binder accepts translated InstantSearch request");

    let search = decode_search(&bytes);
    assert_eq!(text_query(search), "search kernel");
    let search = decode_search(&bytes);
    assert_eq!(search.limit().expect("limit").value(), 20);
    assert_eq!(search.offset().expect("offset").value(), 40);
}

#[test]
fn production_browse_is_canonical_match_all() {
    for query in ["", "   "] {
        let bytes = config()
            .bind_post(&request(query, 20, 0))
            .expect("browse request");
        let search = decode_search(&bytes);
        assert_eq!(
            search.query().expect("query").source_type(),
            api::QuerySourceKind::MatchAllQuery
        );
    }
}

#[test]
fn production_binder_converges_on_dispatcher_and_search_ir() {
    let backend = Arc::new(RecordingBackend::new());
    let dispatcher = Dispatcher::from_backend(context(), backend.clone());
    let bytes = config()
        .bind_post(&request("canonical instantsearch", 7, 14))
        .expect("production request binds");

    dispatcher
        .dispatch(&bytes)
        .expect("canonical dispatcher accepts production public request");

    let calls = backend.take_calls();
    assert_eq!(calls.len(), 1);
    let OperationIr::Search(search) = &calls[0] else {
        panic!("production Search Endpoint request must lower to Search IR");
    };
    assert_eq!(
        search.query(),
        &QueryIr::Text("canonical instantsearch".to_owned())
    );
    assert_eq!(search.limit(), 7);
    assert_eq!(search.offset(), 14);
}

#[test]
fn hostile_inputs_fail_at_the_production_binder_before_dispatch() {
    let config = config();

    let mut oversized = request("docs", 101, 0);
    assert_eq!(
        config.bind_post(&oversized).expect_err("limit bound"),
        PublicSearchError::InvalidRequest
    );

    oversized.limit = 10;
    oversized.offset = 10_001;
    assert_eq!(
        config.bind_post(&oversized).expect_err("offset bound"),
        PublicSearchError::InvalidRequest
    );

    let mut unknown_projection = request("docs", 10, 0);
    unknown_projection.fields = Some(vec!["private".into()]);
    assert_eq!(
        config
            .bind_post(&unknown_projection)
            .expect_err("unexposed field must fail closed"),
        PublicSearchError::Unsupported
    );

    let mut oversized_query = request(&"x".repeat(4097), 10, 0);
    oversized_query.offset = 0;
    assert_eq!(
        config
            .bind_post(&oversized_query)
            .expect_err("query bytes are bounded"),
        PublicSearchError::InvalidRequest
    );
}

#[test]
fn production_binding_is_deterministic_across_generated_bounded_requests() {
    fn next(state: &mut u64) -> u64 {
        let mut value = *state;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        *state = value;
        value
    }

    let config = config();
    let mut state = 0x1046_A17C_5EED_u64;
    for case in 0..2_048 {
        let limit = (next(&mut state) % 101) as u32;
        let offset = (next(&mut state) % 10_001) as u32;
        let query = format!("case-{case}-{}", next(&mut state));
        let request = request(&query, limit, offset);
        let first = config
            .bind_post(&request)
            .expect("bounded generated request");
        let second = config
            .bind_post(&request)
            .expect("same request binds again");
        assert_eq!(first, second, "case {case} must bind deterministically");
    }
}

#[test]
fn get_and_post_share_query_bounds_generation_and_canonical_ir() {
    let config = config();
    for (text, encoded) in [
        ("hello world", "hello+world"),
        ("café", "caf%C3%A9"),
        ("", ""),
        ("   ", "+++"),
    ] {
        let post = config.bind_post(&request(text, 7, 14)).unwrap();
        let get = config
            .bind_get(
                "/search/docs",
                Some(&format!("q={encoded}&limit=7&offset=14")),
            )
            .unwrap();
        assert_eq!(get, post, "same canonical envelope for {text:?}");
        let envelope = wire::root_as_request_envelope(&get).unwrap();
        assert_eq!(
            envelope.target().unwrap().generation(),
            Some("0123456789abcdef0123456789abcdef")
        );
        let backend = Arc::new(RecordingBackend::new());
        let dispatcher = Dispatcher::from_backend(context(), backend.clone());
        dispatcher.dispatch(&get).unwrap();
        dispatcher.dispatch(&post).unwrap();
        let calls = backend.take_calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0], calls[1]);
    }
    for (text, encoded) in [
        ("\0", "%00"),
        ("\t\n", "%09%0A"),
        ("line1\nline2", "line1%0Aline2"),
    ] {
        assert!(config
            .bind_get("/search/docs", Some(&format!("q={encoded}")))
            .is_err());
        assert_eq!(
            config.bind_post(&request(text, 7, 0)).unwrap_err(),
            PublicSearchError::InvalidRequest
        );
    }
}
