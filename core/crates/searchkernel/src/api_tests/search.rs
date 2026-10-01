use super::*;

#[test]
fn inspect_exposes_normalized_filter_defaults() {
    let definition = IndexDefinition::new()
        .field(Field::text("title"))
        .field(Field::float64("price"));
    let (_runtime, index) = runtime_index(definition, Arc::new(RecordingBackend::new()));

    let inspected = index
        .browse()
        .filter(field("price").range(crate::Range::new().lower(100.0)))
        .inspect()
        .unwrap();
    let Some(crate::Filter(crate::filter::FilterNode::Range { bounds, .. })) = inspected.filter
    else {
        panic!("expected normalized range filter");
    };
    assert_eq!(bounds.include_lower, Some(true));
    assert_eq!(bounds.include_upper, Some(true));
}

#[test]
fn inspect_exposes_canonical_filter_structure() {
    let definition = scripted_definition();
    let (_runtime, index) = runtime_index(definition, Arc::new(RecordingBackend::new()));

    let inspected = index
        .browse()
        .filter(Filter::all([
            field("brand").eq("acme"),
            Filter::all([field("category").eq("books"), field("region").eq("eu")]),
        ]))
        .inspect()
        .unwrap();

    assert_eq!(
        inspected.filter,
        Some(Filter::all([
            field("brand").eq("acme"),
            field("category").eq("books"),
            field("region").eq("eu"),
        ]))
    );
}

#[test]
fn public_rust_browse_and_portable_match_all_share_source_semantics() {
    let definition = scripted_definition();
    let context = context_for(&definition);

    let recording = Arc::new(RecordingBackend::new());
    let (_runtime, index) = runtime_index(definition.clone(), recording.clone());
    assert!(index.browse().execute::<serde_json::Value>().is_err());
    let OperationIr::Search(rust_request) = recording.last_call().unwrap() else {
        panic!("browse should reach the recording backend");
    };
    let bytes = portable_match_all_request(false);
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let portable_request = context.search_to_ir(request).unwrap();
    assert_eq!(rust_request, portable_request);

    let recording = Arc::new(RecordingBackend::new());
    let (_runtime, index) = runtime_index(definition, recording.clone());
    let rust_error = index
        .browse()
        .search_fields(["title"])
        .execute::<serde_json::Value>()
        .unwrap_err();
    let bytes = portable_match_all_request(true);
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let portable_error = context.search_to_ir(request).unwrap_err();
    assert_eq!(rust_error.code(), crate::ErrorCode::InvalidArgument);
    assert_eq!(
        portable_error.code,
        searchkernel_ir::ErrorCode::InvalidArgument
    );
    assert_eq!(rust_error.details()[0].code, portable_error.details[0].code);
    assert_eq!(rust_error.details()[0].path, portable_error.details[0].path);
}

#[test]
fn public_search_rejects_blank_text_instead_of_becoming_browse() {
    let recording = Arc::new(RecordingBackend::new());
    let (_runtime, index) = runtime_index(scripted_definition(), recording.clone());
    let error = index
        .search(" \t\n")
        .execute::<serde_json::Value>()
        .unwrap_err();
    assert_eq!(error.code(), crate::ErrorCode::InvalidArgument);
    assert!(recording.calls().is_empty());
}

#[test]
fn public_rust_typed_query_errors_match_independent_portable_requests() {
    let definition = scripted_definition();
    let context = context_for(&definition);

    let cases = vec![
        (
            "weak_and_zero",
            Query::weak_and([Query::term("title", "value")], 0),
            portable_weak_and_request(0),
        ),
        (
            "geo_nan",
            Query::GeoLocation {
                field: "title".to_owned(),
                latitude: f64::NAN,
                longitude: 1.0,
                radius: "10 km".to_owned(),
            },
            portable_geo_nan_request(),
        ),
        (
            "dotted_field",
            Query::term("people.name", "value"),
            portable_typed_contains_request("people.name"),
        ),
    ];

    for (name, query, bytes) in cases {
        let recording = Arc::new(RecordingBackend::new());
        let (_runtime, index) = runtime_index(definition.clone(), recording);
        let builder = index.search("typed").typed_query(query);
        let rust_error = builder.execute::<serde_json::Value>().unwrap_err();
        let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
        let portable_error = context.search_to_ir(request).unwrap_err();
        assert_eq!(
            rust_error.code(),
            crate::ErrorCode::InvalidArgument,
            "{name}"
        );
        assert_eq!(
            portable_error.code,
            searchkernel_ir::ErrorCode::InvalidArgument,
            "{name}"
        );
        assert_eq!(
            rust_error.details()[0].code,
            portable_error.details[0].code,
            "{name}"
        );
        assert_eq!(
            rust_error.details()[0].path,
            portable_error.details[0].path,
            "{name}"
        );
    }
}

#[test]
fn public_and_portable_advanced_query_errors_converge_in_the_ir() {
    let definition = IndexDefinition::new()
        .field(Field::text("title").searchable(true))
        .field(Field::tensor("embedding", "tensor<float>(x[2])").stored(true))
        .field(Field::int64("price").filterable(true));
    let context = context_for(&definition);
    let tensor_input = || {
        searchkernel_ir::QueryInputIr::Literal(searchkernel_ir::RequestValueIr::Tensor(
            searchkernel_ir::RequestTensorIr {
                type_spec: "tensor<float>(x[2])".to_owned(),
                payload: searchkernel_ir::RequestTensorPayloadIr::Dense(vec![1.0, 0.0]),
            },
        ))
    };
    let cases = [
        (
            "nn_non_tensor_field",
            Query::nearest_neighbor("title", "q"),
            searchkernel_ir::QueryExprIr {
                kind: searchkernel_ir::QueryExprKindIr::NearestNeighbor {
                    document_field: typed_field("title"),
                    query_tensor: "q".to_owned(),
                },
                annotations: BTreeMap::new(),
            },
            BTreeMap::from([("q".to_owned(), tensor_input())]),
        ),
        (
            "nn_scalar_query_input",
            Query::nearest_neighbor("embedding", "q"),
            searchkernel_ir::QueryExprIr {
                kind: searchkernel_ir::QueryExprKindIr::NearestNeighbor {
                    document_field: typed_field("embedding"),
                    query_tensor: "q".to_owned(),
                },
                annotations: BTreeMap::new(),
            },
            BTreeMap::from([(
                "q".to_owned(),
                searchkernel_ir::QueryInputIr::Literal(searchkernel_ir::RequestValueIr::Float64(
                    1.0,
                )),
            )]),
        ),
        (
            "term_value_type",
            Query::term("title", 42_i64),
            typed_contains(
                "title",
                searchkernel_ir::QueryExprKindIr::Term {
                    value: searchkernel_ir::RequestValueIr::Int64(42),
                },
            ),
            BTreeMap::new(),
        ),
        (
            "fuzzy_value_type",
            Query::fuzzy("title", 42_i64),
            typed_contains(
                "title",
                searchkernel_ir::QueryExprKindIr::Fuzzy {
                    value: searchkernel_ir::RequestValueIr::Int64(42),
                    max_edit_distance: 2,
                    prefix_length: 0,
                    prefix: false,
                },
            ),
            BTreeMap::new(),
        ),
        (
            "weighted_key_type",
            Query::weighted_set("price", [crate::WeightedQueryItem::new("bad", 1)]),
            searchkernel_ir::QueryExprIr {
                kind: searchkernel_ir::QueryExprKindIr::WeightedSet {
                    field: typed_field("price"),
                    items: vec![searchkernel_ir::WeightedQueryItemIr {
                        key: searchkernel_ir::RequestValueIr::String("bad".to_owned()),
                        weight: 1,
                    }],
                },
                annotations: BTreeMap::new(),
            },
            BTreeMap::new(),
        ),
        (
            "weighted_empty_key",
            Query::weighted_set("brand", [crate::WeightedQueryItem::new("", 1)]),
            searchkernel_ir::QueryExprIr {
                kind: searchkernel_ir::QueryExprKindIr::WeightedSet {
                    field: typed_field("brand"),
                    items: vec![searchkernel_ir::WeightedQueryItemIr {
                        key: searchkernel_ir::RequestValueIr::String(String::new()),
                        weight: 1,
                    }],
                },
                annotations: BTreeMap::new(),
            },
            BTreeMap::new(),
        ),
        (
            "nn_empty_query_tensor",
            Query::nearest_neighbor("embedding", ""),
            searchkernel_ir::QueryExprIr {
                kind: searchkernel_ir::QueryExprKindIr::NearestNeighbor {
                    document_field: typed_field("embedding"),
                    query_tensor: String::new(),
                },
                annotations: BTreeMap::new(),
            },
            BTreeMap::new(),
        ),
        (
            "wand_zero_threshold_boost",
            Query::Wand {
                field: "brand".to_owned(),
                items: vec![crate::WeightedQueryItem::new("acme", 1)],
                options: WandOptions {
                    threshold_boost_factor: 0.0,
                    ..WandOptions::default()
                },
            },
            searchkernel_ir::QueryExprIr {
                kind: searchkernel_ir::QueryExprKindIr::Wand {
                    field: typed_field("brand"),
                    items: vec![searchkernel_ir::WeightedQueryItemIr {
                        key: searchkernel_ir::RequestValueIr::String("acme".to_owned()),
                        weight: 1,
                    }],
                },
                annotations: BTreeMap::from([
                    (
                        "targetNumHits".to_owned(),
                        searchkernel_ir::RequestValueIr::UInt64(100),
                    ),
                    (
                        "scoreThreshold".to_owned(),
                        searchkernel_ir::RequestValueIr::Int64(0),
                    ),
                    (
                        "thresholdBoostFactor".to_owned(),
                        searchkernel_ir::RequestValueIr::Float64(0.0),
                    ),
                ]),
            },
            BTreeMap::new(),
        ),
        (
            "wand_negative_threshold_boost",
            Query::Wand {
                field: "brand".to_owned(),
                items: vec![crate::WeightedQueryItem::new("acme", 1)],
                options: WandOptions {
                    threshold_boost_factor: -1.0,
                    ..WandOptions::default()
                },
            },
            searchkernel_ir::QueryExprIr {
                kind: searchkernel_ir::QueryExprKindIr::Wand {
                    field: typed_field("brand"),
                    items: vec![searchkernel_ir::WeightedQueryItemIr {
                        key: searchkernel_ir::RequestValueIr::String("acme".to_owned()),
                        weight: 1,
                    }],
                },
                annotations: BTreeMap::from([
                    (
                        "targetNumHits".to_owned(),
                        searchkernel_ir::RequestValueIr::UInt64(100),
                    ),
                    (
                        "scoreThreshold".to_owned(),
                        searchkernel_ir::RequestValueIr::Int64(0),
                    ),
                    (
                        "thresholdBoostFactor".to_owned(),
                        searchkernel_ir::RequestValueIr::Float64(-1.0),
                    ),
                ]),
            },
            BTreeMap::new(),
        ),
    ];

    for (name, rust_query, portable_query, inputs) in cases {
        let recording = Arc::new(RecordingBackend::new());
        let (_runtime, index) = runtime_index(definition.clone(), recording);
        let mut builder = index.search("").typed_query(rust_query);
        for (input_name, input) in &inputs {
            builder = match input {
                searchkernel_ir::QueryInputIr::Literal(
                    searchkernel_ir::RequestValueIr::Float64(value),
                ) => builder.query_input(input_name.clone(), *value),
                searchkernel_ir::QueryInputIr::Literal(
                    searchkernel_ir::RequestValueIr::Tensor(tensor),
                ) => {
                    let searchkernel_ir::RequestTensorPayloadIr::Dense(values) = &tensor.payload
                    else {
                        panic!("test input must be dense")
                    };
                    builder.query_input(
                        input_name.clone(),
                        Tensor::dense(tensor.type_spec.clone(), values.clone()),
                    )
                }
                other => panic!("unexpected test query input: {other:?}"),
            };
        }
        let rust_error = builder.execute::<serde_json::Value>().unwrap_err();
        let bytes = portable_typed_request(portable_query, inputs);
        let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
        let portable_error = context.search_to_ir(request).unwrap_err();

        let portable_code = match portable_error.code {
            searchkernel_ir::ErrorCode::InvalidArgument => crate::ErrorCode::InvalidArgument,
            searchkernel_ir::ErrorCode::Unsupported => crate::ErrorCode::Unsupported,
            other => panic!("unexpected portable error code for {name}: {other:?}"),
        };
        assert_eq!(rust_error.code(), portable_code, "{name}");
        assert_eq!(
            rust_error.details()[0].code,
            portable_error.details[0].code,
            "{name}"
        );
        assert_eq!(
            rust_error.details()[0].path,
            portable_error.details[0].path,
            "{name}"
        );
    }
}

#[test]
fn public_rust_typed_query_values_survive_canonical_normalization() {
    let definition = scripted_definition();
    let context = context_for(&definition);

    let cases = [
        (
            Query::weak_and([Query::term("title", "value")], 37),
            portable_weak_and_request(37),
        ),
        (
            Query::Fuzzy {
                field: "title".to_owned(),
                value: "fone".into(),
                max_edit_distance: 4,
                prefix_length: 3,
                prefix: true,
            },
            portable_fuzzy_request(4, 3, true),
        ),
        (
            Query::Near {
                field: "title".to_owned(),
                children: vec![Query::term("title", "value")],
                distance: 7,
            },
            portable_near_request(7),
        ),
    ];

    for (query, bytes) in cases {
        let recording = Arc::new(RecordingBackend::new());
        let (_runtime, index) = runtime_index(definition.clone(), recording.clone());
        assert!(index
            .search("typed")
            .typed_query(query)
            .execute::<serde_json::Value>()
            .is_err());
        let OperationIr::Search(rust_request) = recording.last_call().unwrap() else {
            panic!("typed query should reach the recording backend");
        };
        let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
        let portable_request = context.search_to_ir(request).unwrap();
        assert_eq!(rust_request, portable_request);

        let searchkernel_ir::QueryIr::Typed(query) = rust_request.query() else {
            panic!("expected a typed query");
        };
        match &query.kind {
            searchkernel_ir::QueryExprKindIr::WeakAnd { target_hits, .. } => {
                assert_eq!(*target_hits, 37)
            }
            searchkernel_ir::QueryExprKindIr::Contains { item, .. } => match &item.kind {
                searchkernel_ir::QueryExprKindIr::Fuzzy {
                    max_edit_distance,
                    prefix_length,
                    prefix,
                    ..
                } => assert_eq!((*max_edit_distance, *prefix_length, *prefix), (4, 3, true)),
                searchkernel_ir::QueryExprKindIr::Near { distance, .. } => {
                    assert_eq!(*distance, 7)
                }
                other => panic!("unexpected contained query kind: {other:?}"),
            },
            other => panic!("unexpected typed query kind: {other:?}"),
        }
    }
}

#[test]
fn public_rust_omitted_typed_query_defaults_match_portable_requests() {
    let definition = scripted_definition().field(Field::tensor("embedding", "tensor<float>(x[2])"));
    let context = context_for(&definition);
    let term = || {
        typed_contains(
            "title",
            searchkernel_ir::QueryExprKindIr::Term {
                value: searchkernel_ir::RequestValueIr::String("value".to_owned()),
            },
        )
    };
    let cases = [
        (
            Query::near("title", [Query::term("title", "value")]),
            typed_contains(
                "title",
                searchkernel_ir::QueryExprKindIr::Near {
                    children: vec![term()],
                    distance: searchkernel_ir::DEFAULT_PROXIMITY_DISTANCE,
                },
            ),
        ),
        (
            Query::fuzzy("title", "value"),
            typed_contains(
                "title",
                searchkernel_ir::QueryExprKindIr::Fuzzy {
                    value: searchkernel_ir::RequestValueIr::String("value".to_owned()),
                    max_edit_distance: searchkernel_ir::DEFAULT_FUZZY_MAX_EDIT_DISTANCE,
                    prefix_length: searchkernel_ir::DEFAULT_FUZZY_PREFIX_LENGTH,
                    prefix: searchkernel_ir::DEFAULT_FUZZY_PREFIX,
                },
            ),
        ),
        (
            Query::wand("brand", [crate::WeightedQueryItem::new("value", 1)]),
            searchkernel_ir::QueryExprIr {
                kind: searchkernel_ir::QueryExprKindIr::Wand {
                    field: typed_field("brand"),
                    items: vec![searchkernel_ir::WeightedQueryItemIr {
                        key: searchkernel_ir::RequestValueIr::String("value".to_owned()),
                        weight: 1,
                    }],
                },
                annotations: BTreeMap::new(),
            },
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
        let portable_request = context
            .search_to_ir(portable_request)
            .expect("portable query should resolve");
        assert_eq!(rust_request.query(), portable_request.query());
    }

    let tensor = searchkernel_ir::QueryInputIr::Literal(searchkernel_ir::RequestValueIr::Tensor(
        searchkernel_ir::RequestTensorIr {
            type_spec: "tensor<float>(x[2])".to_owned(),
            payload: searchkernel_ir::RequestTensorPayloadIr::Dense(vec![1.0, 0.0]),
        },
    ));
    let recording = Arc::new(RecordingBackend::new());
    let (_runtime, index) = runtime_index(definition, recording);
    let request = index
        .search("typed")
        .typed_query(Query::nearest_neighbor("embedding", "q"))
        .query_input("q", Tensor::dense("tensor<float>(x[2])", vec![1.0, 0.0]));
    let rust_request = searchkernel_ir::resolve_search(
        &context,
        crate::search::to_resolver_input(&request).expect("Rust nearest-neighbor should lower"),
    )
    .expect("Rust nearest-neighbor should resolve");
    let portable_query = searchkernel_ir::QueryExprIr {
        kind: searchkernel_ir::QueryExprKindIr::NearestNeighbor {
            document_field: typed_field("embedding"),
            query_tensor: "q".to_owned(),
        },
        annotations: BTreeMap::new(),
    };
    let bytes = portable_typed_request(portable_query, BTreeMap::from([("q".to_owned(), tensor)]));
    let portable_request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes)
        .expect("portable request should parse");
    let portable_request = context
        .search_to_ir(portable_request)
        .expect("portable nearest-neighbor should resolve");
    assert_eq!(rust_request.query(), portable_request.query());
    assert_eq!(rust_request.inputs(), portable_request.inputs());
}

#[test]
fn blank_text_is_rejected_identically_and_match_all_remains_explicit() {
    let definition = scripted_definition();
    let context = context_for(&definition);

    for text in ["", "   "] {
        let recording = Arc::new(RecordingBackend::new());
        let (_runtime, index) = runtime_index(definition.clone(), recording);
        let rust_error = index
            .search(text)
            .execute::<serde_json::Value>()
            .unwrap_err();
        let bytes = portable_text_request(Some(text));
        let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
        let portable_error = context.search_to_ir(request).unwrap_err();
        assert_eq!(rust_error.code(), crate::ErrorCode::InvalidArgument);
        assert_eq!(
            portable_error.code,
            searchkernel_ir::ErrorCode::InvalidArgument
        );
        assert_eq!(rust_error.details()[0].code, portable_error.details[0].code);
        assert_eq!(rust_error.details()[0].path, portable_error.details[0].path);
    }

    let bytes = portable_text_request(None);
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let missing_text_error = context.search_to_ir(request).unwrap_err();
    assert_eq!(
        missing_text_error.code,
        searchkernel_ir::ErrorCode::InvalidArgument
    );

    let bytes = portable_match_all_request(false);
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let match_all = context.search_to_ir(request).unwrap();
    assert!(matches!(
        match_all.query(),
        searchkernel_ir::QueryIr::MatchAll
    ));
}

#[test]
fn empty_language_has_identical_rust_and_portable_semantics() {
    let definition = IndexDefinition::new().field(Field::text("title"));
    let context = context_for(&definition);
    let (_runtime, index) = runtime_index(definition, Arc::new(RecordingBackend::new()));

    let rust_error = index
        .search("x")
        .language("")
        .inspect()
        .expect_err("empty Rust language must be invalid");
    let bytes = portable_text_request_with_empty_controls(None, Some(""), None);
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let portable_error = context
        .search_to_ir(request)
        .expect_err("empty portable language must not disappear");

    assert_eq!(rust_error.details()[0].code, "invalid_language");
    assert_eq!(portable_error.details[0].code, "invalid_language");
}

#[test]
fn search_normalization_covers_explicit_options_and_defaults() {
    let definition = scripted_definition();
    let context = context_for(&definition);
    let (_probe_runtime, probe) =
        runtime_index(definition.clone(), Arc::new(RecordingBackend::new()));
    let expected = expected_search(&context, &normalized_search(&probe));
    let backend = Arc::new(ScriptedBackend::new([ExpectedCall::success(
        OperationIr::Search(expected),
        ScriptedResult::Search(Box::new(searchkernel_ir::SearchResult::empty(
            searchkernel_ir::TotalHits {
                value: 0,
                relation: searchkernel_ir::TotalHitsRelation::Exact,
            },
        ))),
    )]));
    let (_runtime, index) = runtime_index(definition, backend.clone());

    let inspected = normalized_search(&index).inspect().unwrap();
    assert_eq!(inspected.query, "  fox  ");
    assert_eq!(inspected.search_fields.len(), 1);
    assert_eq!(inspected.search_fields[0].field(), "title");
    assert_eq!(
        inspected.filter,
        Some(Filter::all([
            field("brand").eq("acme"),
            field("category").eq("books"),
        ]))
    );
    assert_eq!(inspected.sorts.len(), 2);
    assert_eq!(inspected.sorts[0].direction, crate::SortDirection::Asc);
    assert_eq!(
        inspected.sorts[0].missing,
        Some(crate::MissingPlacement::Last)
    );
    assert_eq!(inspected.sorts[1].direction, crate::SortDirection::Desc);
    assert_eq!(
        inspected.sorts[1].missing,
        Some(crate::MissingPlacement::First)
    );
    assert!(inspected.facets.is_empty());
    assert_eq!(inspected.ranking.kind, crate::search::RankingKind::Bm25);
    assert_eq!(inspected.limit, 7);
    assert_eq!(inspected.offset, 2);
    assert_eq!(inspected.projection, ["title", "brand"]);
    assert_eq!(inspected.highlighting, Some(vec!["title".to_owned()]));
    assert_eq!(inspected.language.as_deref(), Some("en-us"));

    let result: crate::SearchResult<serde_json::Value> =
        normalized_search(&index).execute().unwrap();
    assert!(result.hits.is_empty());
    backend.finish().unwrap();

    let single_filter = index
        .search("fox")
        .filter(field("brand").eq("acme"))
        .inspect()
        .unwrap();
    assert_eq!(single_filter.query, "fox");
    assert_eq!(single_filter.filter, Some(field("brand").eq("acme")));

    let defaults = index.browse().inspect().unwrap();
    assert_eq!(defaults.query, "");
    assert!(defaults.filter.is_none());
    assert!(defaults.sorts.is_empty());
    assert!(defaults.facets.is_empty());
    assert_eq!(
        defaults.ranking.kind,
        crate::search::RankingKind::DefaultRelevanceV1
    );
    assert_eq!(defaults.limit, 20);
    assert_eq!(defaults.offset, 0);
    assert_eq!(defaults.projection, ["title", "brand"]);
    assert!(defaults.highlighting.is_none());
    assert!(defaults.language.is_none());
}

#[test]
fn fluent_search_and_portable_request_share_one_canonical_search_snapshot() {
    let definition = scripted_definition();
    let context = context_for(&definition);
    let (_runtime, probe) = runtime_index(definition, Arc::new(RecordingBackend::new()));
    let rust_request = probe
        .search("fox")
        .search_fields(["title"])
        .filter(field("brand").eq("acme"))
        .filter("category = 'books'")
        .sort(Sort::desc("brand").missing_first())
        .facet(crate::Facet::new("category").name("categories").limit(3))
        .grouping(
            GroupingPlan::new().level(
                GroupingPlan::by("brand")
                    .output(crate::GroupingAggregation::count())
                    .nested(GroupingPlan::by("category")),
            ),
        )
        .ranking(Ranking::bm25())
        .rank_property("source", "catalog")
        .projection(["title", "brand"])
        .highlight(["title"])
        .language("EN-us")
        .limit(7)
        .offset(2);

    let direct = expected_search(&context, &rust_request);
    let bytes = searchkernel_ir::search_request_ir_to_api(&direct);
    let portable = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes)
        .expect("canonical search request should decode");
    let portable = context
        .search_to_ir(portable)
        .expect("canonical portable search should resolve");

    assert_eq!(direct, portable);
    assert_eq!(
        searchkernel_ir::snapshot::search_request(&direct),
        searchkernel_ir::snapshot::search_request(&portable)
    );
}
