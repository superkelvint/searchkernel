use super::*;

#[test]
fn canonical_and_custom_facet_labels_preserve_the_facet_fast_path() {
    let mut default = facet_grouping(
        "brand",
        10,
        searchkernel_vespa_engine::GroupingOrder::CountDescending,
    );
    default.root.label = Some("brand".to_owned());
    let lowered = lower_groupings(&[default]).expect("default-named facet should lower");
    assert!(matches!(
        lowered.as_slice(),
        [crate::search::GroupingLowering::Facet(_)]
    ));

    let mut named = facet_grouping(
        "brand",
        10,
        searchkernel_vespa_engine::GroupingOrder::CountDescending,
    );
    named.root.label = Some("brands".to_owned());
    let lowered = lower_groupings(&[named]).expect("custom-named facet should lower");
    let [crate::search::GroupingLowering::Facet(facet)] = lowered.as_slice() else {
        panic!("custom-named facet should retain the facet lowering");
    };
    assert_eq!(facet.field, "brand");
    assert_eq!(facet.name, "brands");
}

#[test]
fn canonical_one_level_grouping_lowers_to_linear_attribute_plan() {
    let request = native_grouping(&[(
        "category",
        vec![count_expression(), average_expression("price")],
    )]);
    let lowered = lower_groupings(&[request]).expect("native grouping should lower");
    let [crate::search::GroupingLowering::Native(native)] = lowered.as_slice() else {
        panic!("expected a native grouping lowering");
    };
    assert_eq!(native.plan.levels.len(), 1);
    assert_eq!(
        native.plan.levels[0].classifier,
        GroupingExpression::attribute("category")
    );
    assert_eq!(native.plan.levels[0].aggregations.len(), 2);
}

#[test]
fn distinct_grouping_output_names_are_accepted() {
    let request = native_grouping(&[(
        "category",
        vec![average_expression("price"), average_expression("qty")],
    )]);
    let lowered = lower_groupings(&[request]).expect("distinct averages must be accepted");
    assert!(matches!(
        lowered.as_slice(),
        [crate::search::GroupingLowering::Native(_)]
    ));
}

#[test]
fn canonical_two_level_grouping_lowers_to_linear_attribute_plan() {
    let request = native_grouping(&[
        (
            "category",
            vec![count_expression(), average_expression("price")],
        ),
        ("qty", vec![count_expression(), average_expression("price")]),
    ]);
    let lowered = lower_groupings(&[request]).expect("native grouping should lower");
    let [crate::search::GroupingLowering::Native(native)] = lowered.as_slice() else {
        panic!("expected a native grouping lowering");
    };
    assert_eq!(
        native.plan,
        searchkernel_vespa_engine::GroupingPlan::new()
            .level(
                searchkernel_vespa_engine::GroupingLevel::new(
                    searchkernel_vespa_engine::GroupingExpression::attribute("category"),
                )
                .aggregation_label(
                    searchkernel_vespa_engine::GroupingAggregation::Count,
                    "count",
                )
                .aggregation_label(
                    searchkernel_vespa_engine::GroupingAggregation::Average(
                        searchkernel_vespa_engine::GroupingExpression::attribute("price"),
                    ),
                    "average(price)",
                ),
            )
            .level(
                searchkernel_vespa_engine::GroupingLevel::new(
                    searchkernel_vespa_engine::GroupingExpression::attribute("qty"),
                )
                .aggregation_label(
                    searchkernel_vespa_engine::GroupingAggregation::Count,
                    "count",
                )
                .aggregation_label(
                    searchkernel_vespa_engine::GroupingAggregation::Average(
                        searchkernel_vespa_engine::GroupingExpression::attribute("price"),
                    ),
                    "average(price)",
                ),
            )
    );
}

#[test]
fn grouping_range_rejects_integer_bounds_that_round_as_f64() {
    let request = GroupingRequestIr {
        root: GroupingAllIr {
            operations: vec![
                GroupingOperationIr::Group(GroupingExpressionIr::Attribute("category".into())),
                GroupingOperationIr::Each(GroupingAllIr {
                    operations: vec![
                        GroupingOperationIr::Filter(GroupingFilterIr::Range {
                            expression: Box::new(GroupingExpressionIr::Attribute("price".into())),
                            lower: Box::new(GroupingExpressionIr::Literal(RequestValueIr::UInt64(
                                9_007_199_254_740_993,
                            ))),
                            upper: Box::new(GroupingExpressionIr::Literal(RequestValueIr::UInt64(
                                9_007_199_254_740_994,
                            ))),
                            include_lower: true,
                            include_upper: false,
                        }),
                        GroupingOperationIr::Output(vec![GroupingOutputIr {
                            label: None,
                            expression: count_expression(),
                        }]),
                    ],
                    label: None,
                }),
            ],
            label: None,
        },
        continuations: Vec::new(),
    };

    let error = lower_groupings(&[request])
        .expect_err("integer grouping bounds must not be silently rounded to f64");
    assert_eq!(error.code, searchkernel_ir::ErrorCode::Unsupported);
    assert_eq!(
        error.details[0].code,
        "grouping_filter_bound_precision_loss"
    );
}

#[test]
fn canonical_grouping_round_trips_through_flatbuffers_and_ir() {
    let request = SearchRequestIr::new(
        QueryIr::MatchAll,
        Vec::new(),
        None,
        Vec::new(),
        10,
        0,
        ProjectionIr::new(Vec::new()),
        HighlightingIr::Disabled,
        None,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    )
    .with_grouping(vec![native_grouping(&[
        (
            "category",
            vec![count_expression(), average_expression("price")],
        ),
        ("qty", vec![count_expression(), average_expression("price")]),
    ])]);
    let definition = IndexDefinitionIr::new(
        None,
        vec![
            FieldIr::new(
                field("category"),
                DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
                None,
                FieldCapabilities::new(true, false, true, true, true),
            ),
            FieldIr::new(
                field("qty"),
                DataTypeIr::Primitive(PrimitiveDataTypeIr::Int64),
                None,
                FieldCapabilities::new(true, false, true, true, true),
            ),
            FieldIr::new(
                field("price"),
                DataTypeIr::Primitive(PrimitiveDataTypeIr::Float64),
                None,
                FieldCapabilities::new(true, false, true, true, true),
            ),
        ],
        Vec::new(),
    )
    .expect("grouping test definition");
    let bytes = searchkernel_ir::search_request_ir_to_api(&request);
    let api_request = searchkernel_ir::__flatbuffers::root::<
        searchkernel_ir::wire::v1::schema::SearchRequest,
    >(&bytes)
    .expect("encoded search request");
    let round_tripped = searchkernel_ir::wire::v1::search_to_ir(
        &IndexContext::from_definition_ir(definition),
        api_request,
    )
    .expect("FlatBuffers request should resolve to IR");
    let lowered = lower_groupings(round_tripped.grouping()).expect("grouping should lower");
    assert!(matches!(
        lowered.as_slice(),
        [crate::search::GroupingLowering::Native(_)]
    ));
}

#[test]
fn grouping_deadline_remaining_duration_is_deterministic() {
    let started = std::time::Instant::now();
    let deadline = started + std::time::Duration::from_millis(100);
    assert_eq!(
        super::backend::remaining_search_timeout(
            deadline,
            started + std::time::Duration::from_millis(40),
        )
        .unwrap(),
        std::time::Duration::from_millis(60)
    );
    let error = super::backend::remaining_search_timeout(
        deadline,
        started + std::time::Duration::from_millis(100),
    )
    .expect_err("an exhausted deadline must reject the next grouping");
    assert_eq!(error.code, ErrorCode::DeadlineExceeded);
}

#[test]
fn output_on_parent_all_is_rejected_instead_of_moved_to_group() {
    let request = GroupingRequestIr {
        root: GroupingAllIr {
            operations: vec![
                GroupingOperationIr::Group(GroupingExpressionIr::Attribute("category".into())),
                GroupingOperationIr::Output(vec![GroupingOutputIr {
                    label: None,
                    expression: count_expression(),
                }]),
                GroupingOperationIr::Each(GroupingAllIr {
                    operations: Vec::new(),
                    label: None,
                }),
            ],
            label: None,
        },
        continuations: Vec::new(),
    };
    let error = lower_groupings(&[request]).expect_err("parent all output must be rejected");
    assert_eq!(error.details[0].code, "grouping_output_before_each");
}

#[test]
fn parallel_grouping_branches_are_rejected() {
    let request = GroupingRequestIr {
        root: GroupingAllIr {
            operations: vec![
                GroupingOperationIr::Group(GroupingExpressionIr::Attribute("category".into())),
                GroupingOperationIr::Each(GroupingAllIr {
                    operations: vec![
                        GroupingOperationIr::All(
                            native_grouping(&[("qty", vec![count_expression()])]).root,
                        ),
                        GroupingOperationIr::All(
                            native_grouping(&[("price", vec![count_expression()])]).root,
                        ),
                    ],
                    label: None,
                }),
            ],
            label: None,
        },
        continuations: Vec::new(),
    };
    let error = lower_groupings(&[request]).expect_err("parallel grouping must be rejected");
    assert_eq!(error.details[0].code, "parallel_grouping_not_supported");
}

#[test]
fn unsupported_grouping_ir_is_not_approximated() {
    let request = native_grouping(&[(
        "category",
        vec![GroupingExpressionIr::Aggregation(Box::new(
            GroupingAggregationIr::Sum(GroupingExpressionIr::Attribute("price".into())),
        ))],
    )]);
    lower_groupings(&[request]).expect("sum is a supported phase9 aggregation");

    let request = native_grouping(&[(
        "category",
        vec![GroupingExpressionIr::Aggregation(Box::new(
            GroupingAggregationIr::Avg(GroupingExpressionIr::Literal(RequestValueIr::String(
                "price".into(),
            ))),
        ))],
    )]);
    let error = lower_groupings(&[request]).expect_err("average literal must remain unsupported");
    assert_eq!(error.code, searchkernel_ir::ErrorCode::Unsupported);
    assert_eq!(error.details[0].code, "grouping_classifier_not_supported");

    let request = GroupingRequestIr {
        root: GroupingAllIr {
            operations: vec![
                GroupingOperationIr::Output(vec![GroupingOutputIr {
                    label: None,
                    expression: count_expression(),
                }]),
                GroupingOperationIr::Each(
                    native_grouping(&[("category", vec![count_expression()])]).root,
                ),
            ],
            label: None,
        },
        continuations: Vec::new(),
    };
    let error = lower_groupings(&[request])
        .expect_err("outputs on a structural wrapper must not be dropped");
    assert_eq!(error.code, searchkernel_ir::ErrorCode::Unsupported);
    assert_eq!(error.details[0].code, "grouping_output_before_each");
}

#[test]
fn structured_groupings_share_the_primary_search_snapshot() {
    let root = std::env::temp_dir().join(format!(
        "searchkernel-adapter-grouping-snapshot-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos()
    ));
    let data = root.join("data");
    let config = root.join("config");
    let schema = Schema::new("grouping_snapshot")
        .field(
            Field::new("category", FieldType::String)
                .attribute()
                .summary(),
        )
        .field(Field::new("qty", FieldType::Integer).attribute().summary());
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile grouping snapshot schema");
    let engine = Engine::open(&data, &config).expect("open grouping snapshot engine");
    let document = |id: &str, category: &str, qty: i64| {
        InsertRequestIr::new(DocumentIr::new(
            DocumentId::parse(id).expect("valid document ID"),
            std::collections::BTreeMap::from([
                (
                    field("category"),
                    FieldValueIr::Single(ScalarValueIr::String(category.to_owned())),
                ),
                (
                    field("qty"),
                    FieldValueIr::Single(ScalarValueIr::Int64(qty)),
                ),
            ]),
        ))
    };
    let baseline = VespaBackend::from_engine(engine);
    baseline
        .insert(document("doc:old", "old", 1))
        .expect("insert baseline document");
    let probe = std::sync::Arc::new(SearchGroupingProbe {
        after_primary: std::sync::Barrier::new(2),
        continue_search: std::sync::Barrier::new(2),
    });
    let backend = std::sync::Arc::new(VespaBackend::with_search_grouping_probe(
        baseline.engine,
        probe.clone(),
    ));
    let request = SearchRequestIr::new(
        QueryIr::MatchAll,
        Vec::new(),
        None,
        Vec::new(),
        10,
        0,
        searchkernel_ir::ProjectionIr::new(Vec::new()),
        HighlightingIr::Disabled,
        None,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    )
    .with_grouping(vec![
        native_grouping(&[("category", vec![count_expression()])]),
        native_grouping(&[("qty", vec![count_expression()])]),
    ]);
    let searching = std::sync::Arc::clone(&backend);
    let search = std::thread::spawn(move || searching.search(request));
    probe.after_primary.wait();
    backend
        .insert(document("doc:new", "new", 99))
        .expect("insert interleaved document");
    probe.continue_search.wait();
    let result = search
        .join()
        .expect("search thread")
        .expect("search with structured groupings");
    let ResultNode::Group(root_group) = result.root else {
        panic!("search result root must be a group");
    };
    assert_eq!(
        root_group
            .children
            .iter()
            .filter_map(|child| match child {
                ResultNode::Hit(hit) => Some(hit.id.as_str()),
                ResultNode::Group(_) => None,
            })
            .collect::<Vec<_>>(),
        vec!["doc:old"]
    );
    let grouping_nodes = root_group
        .children
        .iter()
        .filter(|child| matches!(child, ResultNode::Group(_)))
        .collect::<Vec<_>>();
    assert_eq!(grouping_nodes.len(), 2);
    assert!(grouping_nodes
        .iter()
        .all(|node| !contains_group_value(node, &Value::String("new".to_owned()))));
    assert!(grouping_nodes
        .iter()
        .all(|node| !contains_group_value(node, &Value::Int64(99))));

    let backend = std::sync::Arc::try_unwrap(backend)
        .unwrap_or_else(|_| panic!("no outstanding backend owners"));
    backend
        .engine
        .close()
        .expect("close grouping snapshot engine");
    std::fs::remove_dir_all(root).expect("remove grouping snapshot index");
}

#[test]
fn unsupported_structured_grouping_is_explicitly_rejected() {
    let request = request(Vec::new(), Vec::new(), None).with_grouping(vec![GroupingRequestIr {
        root: GroupingAllIr {
            operations: Vec::new(),
            label: None,
        },
        continuations: Vec::new(),
    }]);
    let error = validate_search_request(&request).expect_err("grouping must not be dropped");
    assert_eq!(error.code, searchkernel_ir::ErrorCode::Unsupported);
    assert_eq!(error.details[0].code, "grouping_level_missing_classifier");
}

#[test]
fn search_validation_supports_facet_shaped_grouping_and_lowers_highlighting_precisely() {
    let title = field("title");
    let search_fields = vec![SearchFieldIr::new(
        title.clone(),
        SearchFieldWeight::new(1.0).unwrap(),
    )];
    let facets = SearchRequestIr::new(
        QueryIr::Text("fox".into()),
        search_fields.clone(),
        None,
        Vec::new(),
        10,
        0,
        searchkernel_ir::ProjectionIr::new(Vec::new()),
        HighlightingIr::Disabled,
        None,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    )
    .with_grouping(vec![facet_grouping(
        title.as_str(),
        10,
        searchkernel_vespa_engine::GroupingOrder::CountDescending,
    )]);
    assert!(validate_search_request(&facets).is_ok());

    let highlighting = SearchRequestIr::new(
        QueryIr::Text("fox".into()),
        search_fields,
        None,
        Vec::new(),
        10,
        0,
        searchkernel_ir::ProjectionIr::new(Vec::new()),
        HighlightingIr::Requested {
            fields: vec![title],
            multi_valued_fields: Vec::new(),
        },
        None,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    );
    validate_search_request(&highlighting).unwrap();
    let language = analysis_options(highlighting.language()).unwrap();
    let query = lower_search_query(&highlighting, language).unwrap();
    let lowered = lower_search_options(&highlighting, query, language).unwrap();
    assert_eq!(
        lowered.summary,
        searchkernel_vespa_engine::SummaryRequest::None
    );
    assert_eq!(lowered.highlight_fields, vec!["title"]);
}
