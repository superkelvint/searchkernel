#![cfg(feature = "test-support")]

use std::{
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};

use flatbuffers::root;
use searchkernel_dispatch::Dispatcher;
use searchkernel_ir::{IndexContext, OperationIr, QueryIr};
use searchkernel_protocol::{
    protocol::v1::{self as wire, PROTOCOL_VERSION},
    wire::v1 as api,
};
use searchkernel_testkit::RecordingBackend;
use searchkerneld::search_endpoint_test_support::{
    await_blocking_with_timeout, SearchEndpointBindError, SearchEndpointConcurrency,
    SearchEndpointConcurrencyError, SearchEndpointConfig, SearchEndpointExecutionWaitError,
    SearchEndpointPolicy, SearchEndpointRouteError, SearchEndpointRouter,
};

fn policy() -> SearchEndpointPolicy {
    SearchEndpointPolicy::new(256, 3, 64, 100, 1_000)
}

fn docs_router() -> SearchEndpointRouter {
    serde_json::from_value::<searchkerneld::search_endpoints::PublicSearchConfig>(serde_json::json!({
        "behindRateLimitedProxy": true,
        "endpoints": {"docs": {"target": "docs", "generation": "0123456789abcdef0123456789abcdef",
            "fields": ["title"], "filterFields": {}, "facets": [], "highlights": [],
            "policy": {"maxQueryStringBytes": 256, "maxParameters": 3, "maxQueryBytes": 64,
                "defaultLimit": null, "maxLimit": 100, "maxOffset": 1000}}}
    })).unwrap().compile().expect("valid production docs endpoint")
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

async fn wait_for_execution_capacity(
    concurrency: &SearchEndpointConcurrency,
    value: usize,
) -> tokio::task::JoinHandle<usize> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match concurrency.try_spawn_blocking(move || value) {
            Ok(handle) => return handle,
            Err(SearchEndpointConcurrencyError::ExecutionOverloaded) => {
                assert!(
                    Instant::now() < deadline,
                    "execution capacity did not recover after backend completion"
                );
                tokio::task::yield_now().await;
            }
            Err(error) => panic!("unexpected execution capacity error: {error:?}"),
        }
    }
}

#[test]
fn endpoint_names_define_stable_safe_routes() {
    let config = SearchEndpointConfig::new("docs-search_2", "docs", policy());
    assert_eq!(config.name(), "docs-search_2");
    assert_eq!(config.route(), "/search/docs-search_2");

    for invalid in ["", "/docs", "docs/search", "docs search", "docs%2Fsearch"] {
        assert!(matches!(
            SearchEndpointRouter::new([SearchEndpointConfig::new(invalid, "docs", policy())]),
            Err(SearchEndpointRouteError::InvalidEndpointName(name)) if name == invalid
        ));
    }
}

#[test]
fn duplicate_routes_and_invalid_policy_fail_registry_construction() {
    assert_eq!(
        SearchEndpointRouter::new([
            SearchEndpointConfig::new("docs", "docs", policy()),
            SearchEndpointConfig::new(
                "docs",
                "docs",
                SearchEndpointPolicy::new(32, 2, 16, 10, 20).with_default_limit(10),
            ),
        ])
        .expect_err("duplicate endpoint route"),
        SearchEndpointRouteError::DuplicateRoute("/search/docs".to_owned())
    );

    assert_eq!(
        SearchEndpointRouter::new([SearchEndpointConfig::new(
            "bad-default",
            "docs",
            SearchEndpointPolicy::new(64, 2, 32, 10, 20).with_default_limit(11),
        )])
        .expect_err("invalid default limit"),
        SearchEndpointRouteError::Bind(SearchEndpointBindError::InvalidPolicy(
            "default_limit exceeds max_limit"
        ))
    );

    assert_eq!(
        SearchEndpointRouter::new([SearchEndpointConfig::new(
            "unsafe-no-default",
            "docs",
            SearchEndpointPolicy::new(64, 2, 32, 10, 20),
        )])
        .expect_err("tight maximum requires an explicit endpoint default"),
        SearchEndpointRouteError::Bind(SearchEndpointBindError::InvalidPolicy(
            "max_limit below canonical default requires explicit default_limit"
        ))
    );

    let tight = SearchEndpointRouter::new([SearchEndpointConfig::new(
        "tight-default",
        "docs",
        SearchEndpointPolicy::new(64, 2, 32, 10, 20).with_default_limit(10),
    )])
    .expect("explicit default makes tight maximum safe");
    let bytes = tight
        .bind_get("/search/tight-default", Some("q=hello"))
        .expect("explicit endpoint default");
    assert_eq!(
        decode_search(&bytes)
            .limit()
            .expect("explicit default")
            .value(),
        10
    );
}

#[test]
fn get_route_decodes_text_limit_and_offset_into_canonical_request() {
    let bytes = docs_router()
        .bind_get("/search/docs", Some("q=filter+by+price&limit=10&offset=0"))
        .expect("bind public GET");

    let search = decode_search(&bytes);
    let query = search
        .query()
        .and_then(|query| query.source_as_search_kernel_text_query())
        .expect("text query");
    assert_eq!(query.text(), Some("filter by price"));
    assert_eq!(search.limit().expect("limit").value(), 10);
    assert_eq!(search.offset().expect("offset").value(), 0);
}

#[test]
fn omitted_empty_and_whitespace_query_are_browse_match_all() {
    for raw_query in [None, Some(""), Some("q="), Some("q=++"), Some("q=%20%20")] {
        let bytes = docs_router()
            .bind_get("/search/docs", raw_query)
            .expect("bind browse request");
        let query = decode_search(&bytes).query().expect("query source");
        assert_eq!(
            query.source_type(),
            api::QuerySourceKind::MatchAllQuery,
            "raw query {raw_query:?}"
        );
        assert!(query.source_as_match_all_query().is_some());
    }
}

#[test]
fn plus_and_percent_plus_have_one_explicit_interpretation() {
    let spaces = docs_router()
        .bind_get("/search/docs", Some("q=a+b"))
        .expect("plus decodes to space");
    assert_eq!(
        decode_search(&spaces)
            .query()
            .and_then(|query| query.source_as_search_kernel_text_query())
            .expect("text query")
            .text(),
        Some("a b")
    );

    let literal_plus = docs_router()
        .bind_get("/search/docs", Some("q=a%2Bb"))
        .expect("percent plus remains plus");
    assert_eq!(
        decode_search(&literal_plus)
            .query()
            .and_then(|query| query.source_as_search_kernel_text_query())
            .expect("text query")
            .text(),
        Some("a+b")
    );
}

#[test]
fn percent_decoding_is_single_pass_strict_and_utf8_preserving() {
    let bytes = docs_router()
        .bind_get("/search/docs", Some("q=caf%C3%A9"))
        .expect("valid UTF-8 escape");
    assert_eq!(
        decode_search(&bytes)
            .query()
            .and_then(|query| query.source_as_search_kernel_text_query())
            .expect("text query")
            .text(),
        Some("café")
    );

    let once = docs_router()
        .bind_get("/search/docs", Some("q=%252Fv1%252Fexecute"))
        .expect("single decode");
    assert_eq!(
        decode_search(&once)
            .query()
            .and_then(|query| query.source_as_search_kernel_text_query())
            .expect("text query")
            .text(),
        Some("%2Fv1%2Fexecute")
    );

    for malformed in ["q=%", "q=%2", "q=%GG"] {
        assert!(matches!(
            docs_router().bind_get("/search/docs", Some(malformed)),
            Err(SearchEndpointRouteError::MalformedQuery(_))
        ));
    }
    assert_eq!(
        docs_router()
            .bind_get("/search/docs", Some("q=%FF"))
            .expect_err("invalid UTF-8"),
        SearchEndpointRouteError::InvalidUtf8
    );
}

#[test]
fn encoded_parameter_names_are_not_aliases() {
    for raw_query in ["%71=hello", "q%00=hello", "lim%69t=1", "off%73et=1"] {
        assert!(matches!(
            docs_router().bind_get("/search/docs", Some(raw_query)),
            Err(SearchEndpointRouteError::UnknownParameter)
        ));
    }
}

#[test]
fn encoded_delimiters_stay_inside_query_values() {
    let bytes = docs_router()
        .bind_get("/search/docs", Some("q=alpha%26limit%3D99%26offset%3D999"))
        .expect("encoded delimiters belong to q value");

    let search = decode_search(&bytes);
    let query = search
        .query()
        .and_then(|query| query.source_as_search_kernel_text_query())
        .expect("text query");
    assert_eq!(query.text(), Some("alpha&limit=99&offset=999"));
    assert!(search.limit().is_none());
    assert!(search.offset().is_none());
}

#[test]
fn alternate_path_spellings_never_alias_a_named_endpoint() {
    for path in [
        "/search/%64ocs",
        "/search/docs/",
        "//search/docs",
        "/search//docs",
        "/SEARCH/docs",
        "/search/Docs",
    ] {
        assert_eq!(
            docs_router()
                .bind_get(path, Some("q=hello"))
                .expect_err("alternate path must not alias endpoint"),
            SearchEndpointRouteError::UnknownRoute,
            "path {path:?}"
        );
    }
}

#[test]
fn raw_query_size_and_parameter_count_fail_before_value_decoding() {
    let small = SearchEndpointRouter::new([SearchEndpointConfig::new(
        "small",
        "docs",
        SearchEndpointPolicy::new(7, 1, 64, 100, 1_000),
    )])
    .expect("small router");

    assert_eq!(
        small
            .bind_get("/search/small", Some("q=%41%42"))
            .expect_err("raw query too large"),
        SearchEndpointRouteError::QueryStringTooLarge {
            actual_bytes: 8,
            max_bytes: 7,
        }
    );

    let count_limited = SearchEndpointRouter::new([SearchEndpointConfig::new(
        "count",
        "docs",
        SearchEndpointPolicy::new(256, 2, 64, 100, 1_000),
    )])
    .expect("count router");
    assert_eq!(
        count_limited
            .bind_get("/search/count", Some("q=x&limit=1&offset=%GG"))
            .expect_err("parameter count checked before third value decode"),
        SearchEndpointRouteError::TooManyParameters { actual: 3, max: 2 }
    );
}

#[test]
fn unknown_duplicate_empty_segments_and_ambiguous_numbers_fail_closed() {
    assert_eq!(
        docs_router()
            .bind_get("/search/docs", Some("filter=price"))
            .expect_err("unknown parameter"),
        SearchEndpointRouteError::UnknownParameter
    );
    assert_eq!(
        docs_router()
            .bind_get("/search/docs", Some("q=one&q=two"))
            .expect_err("duplicate q"),
        SearchEndpointRouteError::DuplicateParameter("query")
    );
    assert!(matches!(
        docs_router().bind_get("/search/docs", Some("q=x&&limit=1")),
        Err(SearchEndpointRouteError::MalformedQuery(
            "empty parameter segment"
        ))
    ));

    for value in ["", "+1", "-1", "1.0", " 1", "4294967296"] {
        assert!(matches!(
            docs_router().bind_get(
                "/search/docs",
                Some(&format!("limit={}", value.replace('+', "%2B")))
            ),
            Err(SearchEndpointRouteError::InvalidUnsignedInteger {
                parameter: "limit",
                ..
            })
        ));
    }
}

#[test]
fn decoded_query_and_numeric_bounds_reject_without_clamping() {
    let router = SearchEndpointRouter::new([SearchEndpointConfig::new(
        "tiny",
        "docs",
        SearchEndpointPolicy::new(256, 3, 3, 10, 20).with_default_limit(10),
    )])
    .expect("router");

    assert_eq!(
        router
            .bind_get("/search/tiny", Some("q=%C3%A9%C3%A9"))
            .expect_err("decoded query too large"),
        SearchEndpointRouteError::Bind(SearchEndpointBindError::QueryTooLarge {
            actual_bytes: 4,
            max_bytes: 3,
        })
    );
    assert_eq!(
        router
            .bind_get("/search/tiny", Some("limit=11"))
            .expect_err("limit too large"),
        SearchEndpointRouteError::Bind(SearchEndpointBindError::LimitTooLarge {
            value: 11,
            max: 10,
        })
    );
    assert_eq!(
        router
            .bind_get("/search/tiny", Some("offset=21"))
            .expect_err("offset too large"),
        SearchEndpointRouteError::Bind(SearchEndpointBindError::OffsetTooLarge {
            value: 21,
            max: 20,
        })
    );
}

#[test]
fn endpoint_default_and_explicit_zero_preserve_presence() {
    let router = SearchEndpointRouter::new([SearchEndpointConfig::new(
        "defaults",
        "docs",
        policy().with_default_limit(7),
    )])
    .expect("router");

    let defaulted = router
        .bind_get("/search/defaults", Some("q=hello"))
        .expect("defaulted request");
    let search = decode_search(&defaulted);
    assert_eq!(search.limit().expect("policy default").value(), 7);
    assert!(search.offset().is_none());

    let explicit = router
        .bind_get("/search/defaults", Some("q=hello&limit=0&offset=0"))
        .expect("explicit zero request");
    let search = decode_search(&explicit);
    assert_eq!(search.limit().expect("present zero limit").value(), 0);
    assert_eq!(search.offset().expect("present zero offset").value(), 0);
}

#[test]
fn unknown_route_fails_before_query_decode() {
    assert_eq!(
        docs_router()
            .bind_get("/search/missing", Some("q=%GG"))
            .expect_err("unknown route"),
        SearchEndpointRouteError::UnknownRoute
    );
}

#[test]
fn bound_request_reaches_canonical_dispatcher_and_ir_backend() {
    let backend = Arc::new(RecordingBackend::new());
    let dispatcher = Dispatcher::from_backend(context(), backend.clone());
    let bytes = docs_router()
        .bind_get("/search/docs", Some("q=canonical+path&limit=7&offset=0"))
        .expect("bind endpoint request");

    let response = dispatcher
        .dispatch(&bytes)
        .expect("canonical dispatcher accepts endpoint bytes");
    let response = wire::root_as_response_envelope(&response).expect("response envelope");
    assert_eq!(
        response.payload_type(),
        wire::ResponsePayload::SearchKernel_Wire_Error,
        "recording backend deliberately returns a portable error after recording"
    );

    let calls = backend.take_calls();
    assert_eq!(calls.len(), 1);
    let OperationIr::Search(search) = &calls[0] else {
        panic!("endpoint request must reach backend as Search IR");
    };
    assert_eq!(search.query(), &QueryIr::Text("canonical path".to_owned()));
    assert_eq!(search.limit(), 7);
    assert_eq!(search.offset(), 0);
}

#[test]
fn omitted_limit_remains_absent_when_policy_has_no_default() {
    let bytes = docs_router()
        .bind_get("/search/docs", Some("q=canonical+path"))
        .expect("omitted limit is valid endpoint input");
    assert!(
        decode_search(&bytes).limit().is_none(),
        "endpoint binding must preserve an absent limit when policy has no explicit default"
    );

    let backend = Arc::new(RecordingBackend::new());
    let dispatcher = Dispatcher::from_backend(context(), backend.clone());
    dispatcher
        .dispatch(&bytes)
        .expect("canonical dispatcher accepts endpoint bytes");
    let calls = backend.take_calls();
    assert_eq!(calls.len(), 1);
    let OperationIr::Search(search) = &calls[0] else {
        panic!("endpoint request must reach backend as Search IR");
    };
    assert_eq!(search.limit(), searchkernel_ir::DEFAULT_SEARCH_LIMIT);
}

#[test]
fn browse_request_reaches_backend_as_match_all_not_invalid_empty_text() {
    let backend = Arc::new(RecordingBackend::new());
    let dispatcher = Dispatcher::from_backend(context(), backend.clone());
    let bytes = docs_router()
        .bind_get("/search/docs", Some("q=+++"))
        .expect("bind empty browse request");

    dispatcher
        .dispatch(&bytes)
        .expect("browse request is semantically executable");
    let calls = backend.take_calls();
    let OperationIr::Search(search) = &calls[0] else {
        panic!("browse request must reach backend as Search IR");
    };
    assert_eq!(search.query(), &QueryIr::MatchAll);
}

#[test]
fn invalid_public_input_never_reaches_backend() {
    let backend = Arc::new(RecordingBackend::new());
    let dispatcher = Dispatcher::from_backend(context(), backend.clone());

    for raw_query in [
        "q=%GG",
        "q=one&q=two",
        "filter=price",
        "limit=4294967296",
        "q=012345678901234567890123456789012345678901234567890123456789012345",
    ] {
        let result = docs_router().bind_get("/search/docs", Some(raw_query));
        if let Ok(bytes) = result {
            dispatcher
                .dispatch(&bytes)
                .expect("accepted input may traverse dispatcher");
        }
    }

    assert!(
        backend.take_calls().is_empty(),
        "all hostile inputs in this corpus must be rejected before backend execution"
    );
}

#[test]
fn request_errors_do_not_echo_untrusted_request_data() {
    let unknown_route = docs_router()
        .bind_get("/search/secret-path", Some("q=hello"))
        .expect_err("unknown route");
    let text = unknown_route.to_string();
    assert_eq!(text, "unknown Search Endpoint route");
    assert!(!text.contains("secret-path"));

    let unknown_parameter = docs_router()
        .bind_get("/search/docs", Some("secret_token=super-secret"))
        .expect_err("unknown parameter");
    let text = unknown_parameter.to_string();
    assert_eq!(text, "unknown query parameter");
    assert!(!text.contains("secret_token"));
    assert!(!text.contains("super-secret"));

    let invalid_integer = docs_router()
        .bind_get("/search/docs", Some("limit=123SECRET"))
        .expect_err("invalid integer");
    let text = invalid_integer.to_string();
    assert_eq!(
        text,
        "query parameter \"limit\" must be an unsigned decimal integer"
    );
    assert!(!text.contains("123SECRET"));
}

#[test]
fn generated_hostile_query_properties_preserve_determinism_and_execution_boundary() {
    const ALPHABET: &[u8] = b"%0123456789ABCDEFabcdef&=+qQlimitoffsetxyz_-./:;[]{}<>!@#$^*()~";
    const CASES: usize = 4_096;

    fn next(state: &mut u64) -> u64 {
        let mut value = *state;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        *state = value;
        value
    }

    let router = docs_router();
    let backend = Arc::new(RecordingBackend::new());
    let dispatcher = Dispatcher::from_backend(context(), backend.clone());
    let mut state = 0x5EA2_C469_u64;

    for case in 0..CASES {
        let length = (next(&mut state) as usize) % 96;
        let mut raw = String::with_capacity(length);
        for _ in 0..length {
            let index = (next(&mut state) as usize) % ALPHABET.len();
            raw.push(char::from(ALPHABET[index]));
        }

        let first = router.bind_get("/search/docs", Some(&raw));
        let second = router.bind_get("/search/docs", Some(&raw));
        assert_eq!(
            first, second,
            "binding must be deterministic for case {case}"
        );

        match first {
            Ok(bytes) => {
                let envelope = wire::root_as_request_envelope(&bytes).unwrap_or_else(|error| {
                    panic!("case {case} emitted invalid envelope: {error}")
                });
                assert_eq!(envelope.protocol_version(), PROTOCOL_VERSION);
                assert_eq!(
                    envelope.payload_type(),
                    wire::RequestPayload::SearchKernel_Wire_SearchRequest
                );

                dispatcher.dispatch(&bytes).unwrap_or_else(|error| {
                    panic!("case {case} failed canonical dispatch: {error:?}")
                });
                let calls = backend.take_calls();
                assert_eq!(
                    calls.len(),
                    1,
                    "accepted case {case} must execute exactly one backend request: {raw:?}"
                );
                assert!(
                    matches!(&calls[0], OperationIr::Search(_)),
                    "accepted case {case} must lower to Search IR"
                );
            }
            Err(_) => {
                assert!(
                    backend.take_calls().is_empty(),
                    "rejected case {case} must not execute backend work"
                );
            }
        }
    }
}

#[tokio::test]
async fn public_concurrency_limits_are_fail_fast_independent_and_recoverable() {
    let concurrency = SearchEndpointConcurrency::new(2, 1).expect("valid limits");

    let admission_one = concurrency.try_admit().expect("first admission permit");
    let admission_two = concurrency.try_admit().expect("second admission permit");
    assert_eq!(
        concurrency
            .try_admit()
            .expect_err("third admission must fail fast"),
        SearchEndpointConcurrencyError::AdmissionOverloaded
    );

    drop(admission_one);
    let admission_three = concurrency
        .try_admit()
        .expect("released admission capacity is reusable");

    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let first_execution = concurrency
        .try_spawn_blocking(move || {
            started_tx.send(()).expect("signal started");
            release_rx.recv().expect("wait for release");
            1usize
        })
        .expect("first execution slot");

    started_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("blocking execution started");
    assert_eq!(
        concurrency
            .try_spawn_blocking(|| 2usize)
            .expect_err("execution saturation must fail fast"),
        SearchEndpointConcurrencyError::ExecutionOverloaded
    );

    release_tx.send(()).expect("release execution");
    assert_eq!(first_execution.await.expect("blocking task joins"), 1);

    let second_execution = concurrency
        .try_spawn_blocking(|| 2usize)
        .expect("execution capacity recovers");
    assert_eq!(
        second_execution.await.expect("second blocking task joins"),
        2
    );

    drop(admission_two);
    drop(admission_three);
}

#[tokio::test]
async fn abandoning_wait_does_not_create_phantom_execution_capacity() {
    let concurrency = SearchEndpointConcurrency::new(1, 1).expect("valid limits");
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let (finish_tx, finish_rx) = mpsc::channel();

    let handle = concurrency
        .try_spawn_blocking(move || {
            started_tx.send(()).expect("signal started");
            release_rx.recv().expect("wait for release");
            done_tx.send(()).expect("signal completion");
            finish_rx.recv().expect("allow task to return");
        })
        .expect("first execution slot");

    started_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("backend work started");

    // Simulate the HTTP side timing out or disconnecting after native work has
    // started. Dropping the JoinHandle detaches the blocking task; it must not
    // release the execution permit while the task is still running.
    drop(handle);

    assert_eq!(
        concurrency
            .try_spawn_blocking(|| ())
            .expect_err("detached backend work still consumes execution budget"),
        SearchEndpointConcurrencyError::ExecutionOverloaded
    );

    release_tx.send(()).expect("allow backend completion");
    done_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("detached backend work completed");

    finish_tx.send(()).expect("allow backend task to return");
    let recovered = wait_for_execution_capacity(&concurrency, 7).await;
    assert_eq!(recovered.await.expect("recovered task joins"), 7);
}

#[tokio::test]
async fn timeout_detaches_work_without_releasing_execution_capacity_early() {
    let concurrency = SearchEndpointConcurrency::new(1, 1).expect("valid limits");
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let (finish_tx, finish_rx) = mpsc::channel();

    let handle = concurrency
        .try_spawn_blocking(move || {
            started_tx.send(()).expect("signal started");
            release_rx.recv().expect("wait for release");
            done_tx.send(()).expect("signal done");
            finish_rx.recv().expect("allow task to return");
            11usize
        })
        .expect("execution slot");

    started_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("work started");

    assert_eq!(
        await_blocking_with_timeout(handle, Duration::from_millis(10))
            .await
            .expect_err("blocked work must time out"),
        SearchEndpointExecutionWaitError::TimedOut
    );

    assert_eq!(
        concurrency
            .try_spawn_blocking(|| 12usize)
            .expect_err("timed-out native work must still consume capacity"),
        SearchEndpointConcurrencyError::ExecutionOverloaded
    );

    release_tx.send(()).expect("release work");
    done_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("detached work completes");

    finish_tx.send(()).expect("allow backend task to return");
    let recovered = wait_for_execution_capacity(&concurrency, 13).await;
    assert_eq!(
        await_blocking_with_timeout(recovered, Duration::from_secs(2))
            .await
            .expect("recovered work completes"),
        13
    );
}

#[tokio::test]
async fn blocking_task_panic_maps_to_stable_error_and_releases_capacity() {
    let concurrency = SearchEndpointConcurrency::new(1, 1).expect("valid limits");
    let handle = concurrency
        .try_spawn_blocking(|| -> usize {
            panic!("sensitive internal panic detail");
        })
        .expect("execution slot");

    let error = await_blocking_with_timeout(handle, Duration::from_secs(2))
        .await
        .expect_err("panic must become stable task failure");
    assert_eq!(error, SearchEndpointExecutionWaitError::TaskFailed);
    assert_eq!(error.to_string(), "Search Endpoint execution task failed");
    assert!(!error.to_string().contains("sensitive"));

    let recovered = concurrency
        .try_spawn_blocking(|| 17usize)
        .expect("panic must release execution capacity");
    assert_eq!(
        await_blocking_with_timeout(recovered, Duration::from_secs(2))
            .await
            .expect("recovered task"),
        17
    );
}

#[test]
fn invalid_public_concurrency_limits_fail_configuration_without_panicking() {
    assert_eq!(
        SearchEndpointConcurrency::new(0, 1).expect_err("zero admission limit"),
        SearchEndpointConcurrencyError::InvalidAdmissionLimit
    );
    assert_eq!(
        SearchEndpointConcurrency::new(1, 0).expect_err("zero execution limit"),
        SearchEndpointConcurrencyError::InvalidExecutionLimit
    );

    let too_many = tokio::sync::Semaphore::MAX_PERMITS
        .checked_add(1)
        .expect("Tokio semaphore max leaves headroom in usize");
    assert_eq!(
        SearchEndpointConcurrency::new(too_many, 1)
            .expect_err("oversized admission limit must fail before Semaphore::new"),
        SearchEndpointConcurrencyError::AdmissionLimitTooLarge
    );
    assert_eq!(
        SearchEndpointConcurrency::new(1, too_many)
            .expect_err("oversized execution limit must fail before Semaphore::new"),
        SearchEndpointConcurrencyError::ExecutionLimitTooLarge
    );
}

#[test]
fn percent_decoded_control_characters_fail_closed_before_dispatch() {
    let backend = Arc::new(RecordingBackend::new());
    let dispatcher = Dispatcher::from_backend(context(), backend.clone());

    for raw_query in [
        "q=%00",
        "q=%01",
        "q=%09",
        "q=%0A",
        "q=%0D",
        "q=%1B",
        "q=hello%00world",
        "q=line1%0Aline2",
    ] {
        let error = docs_router()
            .bind_get("/search/docs", Some(raw_query))
            .expect_err("control characters must be rejected");
        assert_eq!(error, SearchEndpointRouteError::InvalidQueryText);
        assert_eq!(
            error.to_string(),
            "query text contains unsupported control characters"
        );
    }

    assert!(
        backend.take_calls().is_empty(),
        "control-character inputs must never execute backend work"
    );
    drop(dispatcher);
}

#[test]
fn generated_unicode_percent_encoding_round_trips_without_double_decode() {
    const CASES: usize = 768;
    const CHARS: &[char] = &['a', 'Z', '0', ' ', '+', '%', '&', '=', 'é', 'Ω', '中', '🙂'];

    fn next(state: &mut u64) -> u64 {
        let mut value = *state;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        *state = value;
        value
    }

    fn percent_encode(value: &str) -> String {
        let mut encoded = String::with_capacity(value.len() * 3);
        for byte in value.as_bytes() {
            encoded.push('%');
            encoded.push(char::from(b"0123456789ABCDEF"[(byte >> 4) as usize]));
            encoded.push(char::from(b"0123456789ABCDEF"[(byte & 0x0F) as usize]));
        }
        encoded
    }

    let router = SearchEndpointRouter::new([SearchEndpointConfig::new(
        "unicode",
        "docs",
        SearchEndpointPolicy::new(1_024, 3, 256, 100, 1_000),
    )])
    .expect("unicode router");
    let mut state = 0x0004_695E_C0DE_u64;

    for case in 0..CASES {
        let length = (next(&mut state) as usize) % 25;
        let mut value = String::new();
        for _ in 0..length {
            value.push(CHARS[(next(&mut state) as usize) % CHARS.len()]);
        }

        let raw_query = format!("q={}", percent_encode(&value));
        let bytes = router
            .bind_get("/search/unicode", Some(&raw_query))
            .unwrap_or_else(|error| panic!("case {case} rejected valid Unicode: {error}"));
        let query = decode_search(&bytes).query().expect("query source");

        if value.trim().is_empty() {
            assert_eq!(
                query.source_type(),
                api::QuerySourceKind::MatchAllQuery,
                "case {case} whitespace browse"
            );
        } else {
            assert_eq!(
                query
                    .source_as_search_kernel_text_query()
                    .unwrap_or_else(|| panic!("case {case} must remain text"))
                    .text(),
                Some(value.as_str()),
                "case {case}"
            );
        }
    }
}

#[test]
fn hostile_percent_byte_corpus_never_panics() {
    for byte in 0u16..=255 {
        let raw_query = format!("q=%{byte:02X}");
        let _ = docs_router().bind_get("/search/docs", Some(&raw_query));
    }

    for raw_query in [
        "%",
        "%0",
        "%GG",
        "%00",
        "%2Fv1%2Fexecute",
        "%252Fv1%252Fexecute",
        "q=%C0%AF",
        "q=%E0%80%AF",
        "q=%F0%80%80%AF",
        "q=%ED%A0%80",
        "q=%F4%90%80%80",
    ] {
        let _ = docs_router().bind_get("/search/docs", Some(raw_query));
    }
}
