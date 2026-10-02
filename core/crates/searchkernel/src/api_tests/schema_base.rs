use super::*;

#[test]
fn describe_preserves_complete_rank_phase_state() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(
        canonical.rank_profiles()[0].first_phase,
        described.rank_profiles()[0].first_phase
    );
    assert_eq!(
        canonical.rank_profiles()[0].second_phase,
        described.rank_profiles()[0].second_phase
    );
    assert_eq!(
        canonical.rank_profiles()[0].global_phase,
        described.rank_profiles()[0].global_phase
    );
}

#[test]
fn describe_preserves_absent_rank_phases() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(
        canonical.rank_profiles()[2].first_phase,
        described.rank_profiles()[2].first_phase
    );
    assert_eq!(
        canonical.rank_profiles()[2].second_phase,
        described.rank_profiles()[2].second_phase
    );
    assert_eq!(
        canonical.rank_profiles()[2].global_phase,
        described.rank_profiles()[2].global_phase
    );
}

#[test]
fn describe_preserves_rank_feature_lists() {
    let (canonical, described) = described_canonical_fixture();
    let expected = (
        &canonical.rank_profiles()[0].rank_features,
        &canonical.rank_profiles()[0].match_features,
        &canonical.rank_profiles()[0].summary_features,
        &canonical.rank_profiles()[0].sort_features,
    );
    let actual = (
        &described.rank_profiles()[0].rank_features,
        &described.rank_profiles()[0].match_features,
        &described.rank_profiles()[0].summary_features,
        &described.rank_profiles()[0].sort_features,
    );
    assert_eq!(expected, actual);
}

#[test]
fn describe_preserves_rank_properties_strict_and_generic_properties() {
    let (canonical, described) = described_canonical_fixture();
    let expected = (
        &canonical.rank_profiles()[0].rank_properties,
        canonical.rank_profiles()[0].strict,
        &canonical.rank_profiles()[0].properties,
    );
    let actual = (
        &described.rank_profiles()[0].rank_properties,
        described.rank_profiles()[0].strict,
        &described.rank_profiles()[0].properties,
    );
    assert_eq!(expected, actual);
}

#[test]
fn public_and_portable_fuzzy_option_errors_converge_in_the_ir() {
    let definition = IndexDefinition::new().field(Field::text("title").searchable(true));
    let context = context_for(&definition);
    let recording = Arc::new(RecordingBackend::new());
    let (_runtime, index) = runtime_index(definition, recording);
    let rust_error = index
        .search("")
        .typed_query(Query::Fuzzy {
            field: "title".to_owned(),
            value: "Acme".into(),
            max_edit_distance: 0,
            prefix_length: 2,
            prefix: false,
        })
        .execute::<serde_json::Value>()
        .expect_err("invalid fuzzy options must fail before backend execution");

    let bytes = portable_fuzzy_request(0, 2, false);
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let portable_error = searchkernel_ir::wire::v1::search_to_ir(&context, request)
        .expect_err("portable invalid fuzzy options must fail in the resolver");
    assert_eq!(rust_error.code(), crate::ErrorCode::InvalidArgument);
    assert_eq!(
        portable_error.code,
        searchkernel_ir::ErrorCode::InvalidArgument
    );
    assert_eq!(rust_error.details()[0].code, portable_error.details[0].code);
    assert_eq!(rust_error.details()[0].path, portable_error.details[0].path);
}

#[test]
fn public_compositional_typed_queries_match_portable_requests() {
    let definition = scripted_definition();
    let context = context_for(&definition);
    let term = |value: &str| searchkernel_ir::QueryExprIr {
        kind: searchkernel_ir::QueryExprKindIr::Term {
            value: searchkernel_ir::RequestValueIr::String(value.to_owned()),
        },
        annotations: BTreeMap::new(),
    };
    let cases = [
        (
            Query::contains("title", Query::term_value("value")),
            typed_contains("title", term("value").kind),
        ),
        (
            Query::contains(
                "title",
                Query::phrase_terms([Query::term_value("quick"), Query::term_value("brown")]),
            ),
            typed_contains(
                "title",
                searchkernel_ir::QueryExprKindIr::Phrase(vec![term("quick"), term("brown")]),
            ),
        ),
        (
            Query::contains(
                "title",
                Query::equiv([Query::term_value("one"), Query::term_value("two")]),
            ),
            typed_contains(
                "title",
                searchkernel_ir::QueryExprKindIr::Equiv(vec![term("one"), term("two")]),
            ),
        ),
        (
            Query::contains(
                "title",
                Query::alternatives([
                    QueryAlternative::new(Query::term_value("one")).exactness(0.5)
                ]),
            ),
            typed_contains(
                "title",
                searchkernel_ir::QueryExprKindIr::Alternatives(vec![
                    searchkernel_ir::QueryAlternativeIr {
                        query: term("one"),
                        exactness: Some(0.5),
                    },
                ]),
            ),
        ),
        (
            Query::contains("title", Query::text("free form")),
            typed_contains(
                "title",
                searchkernel_ir::QueryExprKindIr::Text {
                    text: "free form".to_owned(),
                },
            ),
        ),
        (
            Query::matches("title", "value.*"),
            searchkernel_ir::QueryExprIr {
                kind: searchkernel_ir::QueryExprKindIr::Matches {
                    field: typed_field("title"),
                    pattern: "value.*".to_owned(),
                },
                annotations: BTreeMap::new(),
            },
        ),
        (
            Query::contains(
                "title",
                Query::user_input_with_default_index("free form", "title"),
            ),
            typed_contains(
                "title",
                searchkernel_ir::QueryExprKindIr::UserInput {
                    value: "free form".to_owned(),
                    default_index: Some("title".to_owned()),
                    target_hits: searchkernel_ir::DEFAULT_WEAK_AND_TARGET_HITS,
                },
            ),
        ),
        (
            Query::user_query_with_default_index("title"),
            searchkernel_ir::QueryExprIr {
                kind: searchkernel_ir::QueryExprKindIr::UserQuery {
                    default_index: Some("title".to_owned()),
                    target_hits: searchkernel_ir::DEFAULT_WEAK_AND_TARGET_HITS,
                },
                annotations: BTreeMap::new(),
            },
        ),
        (
            Query::labeled(Query::true_query(), "label", 1.0),
            searchkernel_ir::QueryExprIr {
                kind: searchkernel_ir::QueryExprKindIr::Labeled {
                    query: Box::new(searchkernel_ir::QueryExprIr {
                        kind: searchkernel_ir::QueryExprKindIr::True,
                        annotations: BTreeMap::new(),
                    }),
                    label: "label".to_owned(),
                    score: 1.0,
                },
                annotations: BTreeMap::new(),
            },
        ),
        (
            Query::non_empty(Query::false_query()),
            searchkernel_ir::QueryExprIr {
                kind: searchkernel_ir::QueryExprKindIr::NonEmpty(Box::new(
                    searchkernel_ir::QueryExprIr {
                        kind: searchkernel_ir::QueryExprKindIr::False,
                        annotations: BTreeMap::new(),
                    },
                )),
                annotations: BTreeMap::new(),
            },
        ),
        (
            Query::contains(
                "title",
                Query::near_terms([Query::term_value("quick"), Query::term_value("brown")])
                    .with_annotation("distance", 7_u32),
            ),
            typed_contains(
                "title",
                searchkernel_ir::QueryExprKindIr::Near {
                    children: vec![term("quick"), term("brown")],
                    distance: 7,
                },
            ),
        ),
    ];

    for (rust_query, portable_query) in cases {
        let recording = Arc::new(RecordingBackend::new());
        let (_runtime, index) = runtime_index(definition.clone(), recording);
        let request = index.search("typed").typed_query(rust_query);
        let rust_request = searchkernel_ir::resolve_search(
            &context,
            crate::search::to_resolver_input(&request).expect("Rust query should lower"),
        )
        .expect("Rust query should resolve");
        let bytes = portable_typed_request(portable_query, BTreeMap::new());
        let portable_request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes)
            .expect("portable request should parse");
        let portable_request = searchkernel_ir::wire::v1::search_to_ir(&context, portable_request)
            .expect("portable query should resolve");
        assert_eq!(rust_request.query(), portable_request.query());
    }
}
