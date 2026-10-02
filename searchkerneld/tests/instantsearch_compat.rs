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
use searchkerneld::search_endpoint_test_support::{
    SearchEndpointBindError, SearchEndpointConfig, SearchEndpointPolicy, SearchEndpointRouteError,
    SearchEndpointRouter,
};

fn policy() -> SearchEndpointPolicy {
    SearchEndpointPolicy::new(512, 3, 128, 100, 10_000)
}

fn docs_router() -> SearchEndpointRouter {
    SearchEndpointRouter::new([SearchEndpointConfig::new("docs", policy())])
        .expect("valid docs endpoint")
}

fn decode_search(bytes: &[u8]) -> api::SearchRequest<'_> {
    let envelope = wire::root_as_request_envelope(bytes).expect("valid canonical request envelope");
    assert_eq!(envelope.protocol_version(), PROTOCOL_VERSION);
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
fn instantsearch_core_vocabulary_binds_to_canonical_search_request() {
    let bytes = docs_router()
        .bind_get(
            "/search/docs",
            Some("query=search+kernel&hitsPerPage=20&page=2"),
        )
        .expect("bind InstantSearch request");

    let search = decode_search(&bytes);
    assert_eq!(text_query(search), "search kernel");
    let search = decode_search(&bytes);
    assert_eq!(search.limit().expect("hitsPerPage").value(), 20);
    assert_eq!(search.offset().expect("page-derived offset").value(), 40);
}

#[test]
fn instantsearch_profile_is_canonical_equivalent_to_general_endpoint_aliases() {
    let instantsearch = docs_router()
        .bind_get(
            "/search/docs",
            Some("query=search+kernel&hitsPerPage=20&page=2"),
        )
        .expect("InstantSearch request");
    let general = docs_router()
        .bind_get("/search/docs", Some("q=search+kernel&limit=20&offset=40"))
        .expect("general endpoint aliases");

    assert_eq!(
        instantsearch, general,
        "InstantSearch vocabulary must be exposure syntax only, not a second semantic path"
    );
}

#[test]
fn instantsearch_policy_bounds_apply_after_page_lowering_without_clamping() {
    let router = SearchEndpointRouter::new([SearchEndpointConfig::new(
        "docs",
        SearchEndpointPolicy::new(512, 3, 128, 10, 20).with_default_limit(10),
    )])
    .expect("router");

    assert_eq!(
        router
            .bind_get("/search/docs", Some("query=docs&hitsPerPage=11&page=0"),)
            .expect_err("hitsPerPage above endpoint maximum"),
        SearchEndpointRouteError::Bind(SearchEndpointBindError::LimitTooLarge {
            value: 11,
            max: 10,
        })
    );
    assert_eq!(
        router
            .bind_get("/search/docs", Some("query=docs&hitsPerPage=10&page=3"),)
            .expect_err("derived offset above endpoint maximum"),
        SearchEndpointRouteError::Bind(SearchEndpointBindError::OffsetTooLarge {
            value: 30,
            max: 20,
        })
    );
}

#[test]
fn instantsearch_initial_empty_query_is_canonical_browse() {
    for raw in [
        "query=&hitsPerPage=20&page=0",
        "query=+++&hitsPerPage=20&page=0",
        "hitsPerPage=20&page=0",
    ] {
        let bytes = docs_router()
            .bind_get("/search/docs", Some(raw))
            .expect("initial InstantSearch browse");
        let search = decode_search(&bytes);
        let query = search.query().expect("query source");
        assert_eq!(query.source_type(), api::QuerySourceKind::MatchAllQuery);
        assert_eq!(search.limit().expect("hitsPerPage").value(), 20);
        assert_eq!(search.offset().expect("page zero").value(), 0);
    }
}

#[test]
fn instantsearch_page_is_zero_based_and_maps_to_checked_offset() {
    for (page, hits_per_page, expected_offset) in [
        (0u32, 20u32, 0u32),
        (1, 20, 20),
        (2, 20, 40),
        (7, 13, 91),
        (100, 100, 10_000),
    ] {
        let raw = format!("query=docs&hitsPerPage={hits_per_page}&page={page}");
        let bytes = docs_router()
            .bind_get("/search/docs", Some(&raw))
            .unwrap_or_else(|error| panic!("page case {page}/{hits_per_page}: {error}"));
        let search = decode_search(&bytes);
        assert_eq!(search.limit().expect("hitsPerPage").value(), hits_per_page);
        assert_eq!(
            search.offset().expect("page-derived offset").value(),
            expected_offset
        );
    }
}

#[test]
fn instantsearch_page_can_use_an_explicit_endpoint_default_hits_per_page() {
    let router = SearchEndpointRouter::new([SearchEndpointConfig::new(
        "docs",
        SearchEndpointPolicy::new(512, 2, 128, 100, 10_000).with_default_limit(25),
    )])
    .expect("router");

    let bytes = router
        .bind_get("/search/docs", Some("query=docs&page=3"))
        .expect("page uses endpoint default");
    let search = decode_search(&bytes);
    assert_eq!(search.limit().expect("default hits per page").value(), 25);
    assert_eq!(search.offset().expect("page-derived offset").value(), 75);
}

#[test]
fn instantsearch_nonzero_page_without_hits_per_page_fails_closed() {
    assert_eq!(
        docs_router()
            .bind_get("/search/docs", Some("query=docs&page=1"))
            .expect_err("page needs a page size"),
        SearchEndpointRouteError::PageRequiresHitsPerPage
    );

    let zero = docs_router()
        .bind_get("/search/docs", Some("query=docs&page=0"))
        .expect("page zero is representable without a page size");
    let search = decode_search(&zero);
    assert!(search.limit().is_none());
    assert_eq!(search.offset().expect("explicit page zero").value(), 0);
}

#[test]
fn instantsearch_pagination_overflow_fails_before_canonical_request_construction() {
    let router = SearchEndpointRouter::new([SearchEndpointConfig::new(
        "docs",
        SearchEndpointPolicy::new(512, 3, 128, u32::MAX, u32::MAX),
    )])
    .expect("router");

    assert_eq!(
        router
            .bind_get(
                "/search/docs",
                Some("query=docs&hitsPerPage=2&page=4294967295"),
            )
            .expect_err("page multiplication must be checked"),
        SearchEndpointRouteError::PaginationOverflow
    );
}

#[test]
fn instantsearch_and_general_aliases_cannot_be_mixed_ambiguously() {
    assert_eq!(
        docs_router()
            .bind_get("/search/docs", Some("query=one&q=two"))
            .expect_err("two query aliases"),
        SearchEndpointRouteError::DuplicateParameter("query")
    );
    assert_eq!(
        docs_router()
            .bind_get("/search/docs", Some("query=docs&hitsPerPage=20&limit=10"),)
            .expect_err("two page-size aliases"),
        SearchEndpointRouteError::DuplicateParameter("hitsPerPage")
    );
    assert_eq!(
        docs_router()
            .bind_get(
                "/search/docs",
                Some("query=docs&hitsPerPage=20&page=1&offset=20"),
            )
            .expect_err("page and offset conflict"),
        SearchEndpointRouteError::TooManyParameters { actual: 4, max: 3 }
    );

    let router = SearchEndpointRouter::new([SearchEndpointConfig::new(
        "docs",
        SearchEndpointPolicy::new(512, 4, 128, 100, 10_000),
    )])
    .expect("router allowing four parameters");
    assert_eq!(
        router
            .bind_get(
                "/search/docs",
                Some("query=docs&hitsPerPage=20&page=1&offset=20"),
            )
            .expect_err("page and offset conflict"),
        SearchEndpointRouteError::ConflictingPaginationControls
    );
}

#[test]
fn instantsearch_parameter_names_are_case_sensitive_and_encoded_names_do_not_alias() {
    for raw in [
        "Query=docs",
        "hitsperpage=20",
        "HitsPerPage=20",
        "Page=1",
        "%71uery=docs",
        "hitsPer%50age=20",
        "pa%67e=1",
    ] {
        assert_eq!(
            docs_router()
                .bind_get("/search/docs", Some(raw))
                .expect_err("non-literal InstantSearch parameter name"),
            SearchEndpointRouteError::UnknownParameter,
            "raw query {raw:?}"
        );
    }
}

#[test]
fn instantsearch_query_text_preserves_unicode_plus_percent_ampersand_and_equals() {
    let bytes = docs_router()
        .bind_get(
            "/search/docs",
            Some(
                "query=caf%C3%A9%20%2B%20%25%20%26%20%3D%20%E4%B8%AD%20%F0%9F%99%82&hitsPerPage=5&page=0",
            ),
        )
        .expect("encoded InstantSearch query");

    assert_eq!(text_query(decode_search(&bytes)), "café + % & = 中 🙂");
}

#[test]
fn instantsearch_core_request_converges_on_dispatcher_and_search_ir() {
    let backend = Arc::new(RecordingBackend::new());
    let dispatcher = Dispatcher::from_backend(context(), backend.clone());
    let bytes = docs_router()
        .bind_get(
            "/search/docs",
            Some("query=canonical+instantsearch&hitsPerPage=7&page=2"),
        )
        .expect("bind InstantSearch request");

    dispatcher
        .dispatch(&bytes)
        .expect("canonical dispatcher accepts InstantSearch-derived request");

    let calls = backend.take_calls();
    assert_eq!(calls.len(), 1);
    let OperationIr::Search(search) = &calls[0] else {
        panic!("InstantSearch-derived request must lower to Search IR");
    };
    assert_eq!(
        search.query(),
        &QueryIr::Text("canonical instantsearch".to_owned())
    );
    assert_eq!(search.limit(), 7);
    assert_eq!(search.offset(), 14);
}

#[test]
fn instantsearch_generated_pagination_matrix_is_deterministic_and_overflow_checked() {
    fn next(state: &mut u64) -> u64 {
        let mut value = *state;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        *state = value;
        value
    }

    let router = SearchEndpointRouter::new([SearchEndpointConfig::new(
        "docs",
        SearchEndpointPolicy::new(512, 3, 128, u32::MAX, u32::MAX),
    )])
    .expect("router");
    let mut state = 0x1A57_A17C_4690_u64;

    for case in 0..2_048 {
        let page = next(&mut state) as u32;
        let hits_per_page = next(&mut state) as u32;
        let raw = format!("query=matrix&hitsPerPage={hits_per_page}&page={page}");

        let first = router.bind_get("/search/docs", Some(&raw));
        let second = router.bind_get("/search/docs", Some(&raw));
        assert_eq!(first, second, "case {case} must be deterministic");

        match page.checked_mul(hits_per_page) {
            Some(expected_offset) => {
                let bytes = first.unwrap_or_else(|error| {
                    panic!("case {case} unexpectedly rejected {page} * {hits_per_page}: {error}")
                });
                let search = decode_search(&bytes);
                assert_eq!(
                    search.limit().expect("hitsPerPage").value(),
                    hits_per_page,
                    "case {case}"
                );
                assert_eq!(
                    search.offset().expect("derived offset").value(),
                    expected_offset,
                    "case {case}"
                );
            }
            None => assert_eq!(
                first.expect_err("overflow must fail"),
                SearchEndpointRouteError::PaginationOverflow,
                "case {case}"
            ),
        }
    }
}
