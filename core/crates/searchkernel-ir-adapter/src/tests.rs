use super::*;
use searchkernel_ir::*;
use searchkernel_vespa_engine::{
    ArrayValue, ConfigCompiler, Document, Engine, Field, FieldType, FieldValue, GroupingExpression,
    Query, Schema, SortField, Source, StructField, StructFieldType, Value as VespaValue,
};

fn field(name: &str) -> FieldName {
    FieldName::parse(name).unwrap()
}

fn request(
    search_fields: Vec<SearchFieldIr>,
    sort: Vec<SortIr>,
    language: Option<LanguageHint>,
) -> SearchRequestIr {
    SearchRequestIr::new(
        QueryIr::Text("wireless headphones".into()),
        search_fields,
        None,
        sort,
        20,
        0,
        searchkernel_ir::ProjectionIr::new(Vec::new()),
        HighlightingIr::Disabled,
        language,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    )
}

fn facet_grouping(
    field: &str,
    limit: u32,
    order: searchkernel_vespa_engine::GroupingOrder,
) -> GroupingRequestIr {
    let attribute = || GroupingExpressionIr::Attribute(field.to_owned());
    let count = || GroupingExpressionIr::Aggregation(Box::new(GroupingAggregationIr::Count));
    let order = match order {
        searchkernel_vespa_engine::GroupingOrder::CountDescending => {
            GroupingExpressionIr::Negative(Box::new(count()))
        }
        searchkernel_vespa_engine::GroupingOrder::CountAscending => count(),
        searchkernel_vespa_engine::GroupingOrder::ValueDescending => {
            GroupingExpressionIr::Negative(Box::new(attribute()))
        }
        searchkernel_vespa_engine::GroupingOrder::ValueAscending => attribute(),
        searchkernel_vespa_engine::GroupingOrder::None => {
            panic!("facet grouping requires an explicit order")
        }
    };
    GroupingRequestIr {
        root: GroupingAllIr {
            operations: vec![
                GroupingOperationIr::Group(attribute()),
                GroupingOperationIr::Each(GroupingAllIr {
                    operations: vec![
                        GroupingOperationIr::Max(Some(limit)),
                        GroupingOperationIr::Order(vec![order]),
                        GroupingOperationIr::Output(vec![GroupingOutputIr {
                            label: None,
                            expression: count(),
                        }]),
                    ],
                    label: None,
                }),
            ],
            label: None,
        },
        continuations: Vec::new(),
    }
}

fn native_grouping(levels: &[(&str, Vec<GroupingExpressionIr>)]) -> GroupingRequestIr {
    let mut nested = None;
    for (field, outputs) in levels.iter().rev() {
        let output = GroupingOperationIr::Output(
            outputs
                .iter()
                .cloned()
                .map(|expression| GroupingOutputIr {
                    label: None,
                    expression,
                })
                .collect(),
        );
        let mut each_operations = vec![output];
        if let Some(child) = nested {
            each_operations.push(GroupingOperationIr::All(child));
        }
        let each = GroupingAllIr {
            operations: each_operations,
            label: None,
        };
        nested = Some(GroupingAllIr {
            operations: vec![
                GroupingOperationIr::Group(GroupingExpressionIr::Attribute((*field).to_owned())),
                GroupingOperationIr::Each(each),
            ],
            label: None,
        });
    }
    GroupingRequestIr {
        root: nested.expect("native grouping requires at least one level"),
        continuations: Vec::new(),
    }
}

fn count_expression() -> GroupingExpressionIr {
    GroupingExpressionIr::Aggregation(Box::new(GroupingAggregationIr::Count))
}

fn average_expression(field: &str) -> GroupingExpressionIr {
    GroupingExpressionIr::Aggregation(Box::new(GroupingAggregationIr::Avg(
        GroupingExpressionIr::Attribute(field.to_owned()),
    )))
}

#[test]
fn canonical_facet_labels_preserve_fast_path_or_named_grouping_semantics() {
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
    let [crate::search::GroupingLowering::Native(native)] = lowered.as_slice() else {
        panic!("custom-named facet must use grouping plan to preserve its label");
    };
    assert_eq!(native.plan.root_label.as_deref(), Some("brands"));
    assert_eq!(native.plan.levels[0].label.as_deref(), Some("brands"));
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
                .aggregation(searchkernel_vespa_engine::GroupingAggregation::Count)
                .aggregation(
                    searchkernel_vespa_engine::GroupingAggregation::Average(
                        searchkernel_vespa_engine::GroupingExpression::attribute("price"),
                    )
                ),
            )
            .level(
                searchkernel_vespa_engine::GroupingLevel::new(
                    searchkernel_vespa_engine::GroupingExpression::attribute("qty"),
                )
                .aggregation(searchkernel_vespa_engine::GroupingAggregation::Count)
                .aggregation(
                    searchkernel_vespa_engine::GroupingAggregation::Average(
                        searchkernel_vespa_engine::GroupingExpression::attribute("price"),
                    )
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
fn semantically_different_all_each_trees_do_not_share_lowering() {
    let canonical = native_grouping(&[("category", vec![count_expression()])]);
    let mut all_materialization = canonical.clone();
    let each = all_materialization
        .root
        .operations
        .pop()
        .expect("canonical grouping has an each operation");
    let GroupingOperationIr::Each(each) = each else {
        panic!("canonical grouping must end with each");
    };
    all_materialization
        .root
        .operations
        .push(GroupingOperationIr::All(each));

    assert!(lower_groupings(&[canonical]).is_ok());
    let error =
        lower_groupings(&[all_materialization]).expect_err("All must not be treated as Each");
    assert_eq!(error.details[0].code, "grouping_all_topology_not_supported");
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
    let api_request =
        searchkernel_ir::__flatbuffers::root::<searchkernel_ir::api::SearchRequest>(&bytes)
            .expect("encoded search request");
    let round_tripped = IndexContext::from_definition_ir(definition)
        .search_to_ir(api_request)
        .expect("FlatBuffers request should resolve to IR");
    let lowered = lower_groupings(round_tripped.grouping()).expect("grouping should lower");
    assert!(matches!(
        lowered.as_slice(),
        [crate::search::GroupingLowering::Native(_)]
    ));
}

#[test]
fn vespa_coverage_maps_to_portable_coverage_without_issue_fabrication() {
    let full = portable_coverage(searchkernel_vespa_engine::SearchCoverage {
        active: 10,
        covered: 10,
        target_active: 10,
        degradation_flags: 0,
    });
    assert_eq!(full.coverage_percent, Some(100.0));
    assert_eq!(full.full, Some(true));
    assert_eq!(full.results_full, Some(1));

    let partial = portable_coverage(searchkernel_vespa_engine::SearchCoverage {
        active: 10,
        covered: 4,
        target_active: 20,
        degradation_flags: 0,
    });
    assert_eq!(partial.coverage_percent, Some(40.0));
    assert_eq!(partial.documents, Some(4));
    assert_eq!(partial.full, Some(false));
    assert_eq!(partial.results_full, Some(0));

    let match_phase = portable_coverage(searchkernel_vespa_engine::SearchCoverage {
        active: 10,
        covered: 10,
        target_active: 20,
        degradation_flags: 0x01,
    });
    assert_eq!(match_phase.coverage_percent, Some(100.0));
    assert_eq!(match_phase.full, Some(false));
    assert_eq!(match_phase.results_full, Some(0));
    assert_eq!(
        match_phase.degraded.as_ref().unwrap().match_phase,
        Some(true)
    );

    let timeout = portable_coverage(searchkernel_vespa_engine::SearchCoverage {
        active: 10,
        covered: 10,
        target_active: 20,
        degradation_flags: 0x02,
    });
    assert_eq!(timeout.coverage_percent, Some(100.0));
    assert_eq!(timeout.full, Some(false));
    assert_eq!(timeout.results_full, Some(0));
    assert_eq!(timeout.degraded.as_ref().unwrap().timeout, Some(true));

    let ann_timeout = portable_coverage(searchkernel_vespa_engine::SearchCoverage {
        active: 10,
        covered: 10,
        target_active: 20,
        degradation_flags: 0x04,
    });
    assert_eq!(ann_timeout.coverage_percent, Some(100.0));
    assert_eq!(ann_timeout.full, Some(false));
    assert_eq!(ann_timeout.results_full, Some(0));
    assert_eq!(
        ann_timeout.degraded.as_ref().unwrap().ann_timeout,
        Some(true)
    );

    let empty = portable_coverage(searchkernel_vespa_engine::SearchCoverage {
        active: 0,
        covered: 0,
        target_active: 10,
        degradation_flags: 0,
    });
    assert_eq!(empty.coverage_percent, Some(100.0));
    assert_eq!(empty.full, Some(true));

    let empty_timeout = portable_coverage(searchkernel_vespa_engine::SearchCoverage {
        active: 0,
        covered: 0,
        target_active: 0,
        degradation_flags: 0x02,
    });
    assert_eq!(empty_timeout.coverage_percent, Some(0.0));
    assert_eq!(empty_timeout.full, Some(false));

    let non_ideal_corpus = portable_coverage(searchkernel_vespa_engine::SearchCoverage {
        active: 8,
        covered: 8,
        target_active: 10,
        degradation_flags: 0,
    });
    assert_eq!(non_ideal_corpus.coverage_percent, Some(100.0));
    assert_eq!(non_ideal_corpus.full, Some(true));
    assert_eq!(
        non_ideal_corpus.degraded.as_ref().unwrap().non_ideal_state,
        None
    );

    assert_eq!(full.documents, Some(10));
    assert_eq!(full.nodes, Some(1));
    assert_eq!(full.results, Some(1));
    assert!(portable_errors_from_native_issues(&["advisory".to_owned()]).is_empty());
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
fn all_does_not_substitute_for_each() {
    let request = GroupingRequestIr {
        root: GroupingAllIr {
            operations: vec![
                GroupingOperationIr::Group(GroupingExpressionIr::Attribute("category".into())),
                GroupingOperationIr::All(GroupingAllIr {
                    operations: vec![GroupingOperationIr::Output(vec![GroupingOutputIr {
                        label: None,
                        expression: count_expression(),
                    }])],
                    label: None,
                }),
            ],
            label: None,
        },
        continuations: Vec::new(),
    };
    let error = lower_groupings(&[request]).expect_err("all must not materialize a level");
    assert_eq!(error.details[0].code, "grouping_all_topology_not_supported");
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
fn text_lowers_to_vespa_default_weak_and() {
    let search_field = || {
        vec![SearchFieldIr::new(
            field("title"),
            SearchFieldWeight::new(1.0).unwrap(),
        )]
    };
    for text in ["wireless headphones", "foo-bar", "can't"] {
        let query = lower_query(&QueryIr::Text(text.into()), &search_field()).unwrap();
        assert_eq!(
            query,
            Query::Text {
                field: "title".into(),
                text: text.into(),
                target_num_hits: 100,
            }
        );
    }

    let query = lower_query(&QueryIr::Text("headphones".into()), &search_field()).unwrap();
    assert_eq!(
        query,
        Query::Text {
            field: "title".into(),
            text: "headphones".into(),
            target_num_hits: 100,
        }
    );
}

fn typed_field(name: &str) -> QueryFieldRefIr {
    QueryFieldRefIr {
        path: field(name),
        element_index: None,
    }
}

fn typed_expression(kind: QueryExprKindIr) -> QueryIr {
    QueryIr::Typed(Box::new(QueryExprIr {
        kind,
        annotations: std::collections::BTreeMap::new(),
    }))
}

#[test]
fn typed_query_a_lowers_to_existing_native_query_nodes() {
    let title = typed_field("title");
    let term = typed_expression(QueryExprKindIr::Contains {
        field: title.clone(),
        item: Box::new(QueryExprIr {
            kind: QueryExprKindIr::Term {
                value: RequestValueIr::String("wireless".to_owned()),
            },
            annotations: std::collections::BTreeMap::new(),
        }),
    });
    assert_eq!(
        lower_query(
            &term,
            &[SearchFieldIr::new(
                field("title"),
                SearchFieldWeight::new(1.0).unwrap(),
            )],
        )
        .unwrap(),
        Query::term("title", "wireless")
    );

    let phrase = typed_expression(QueryExprKindIr::Contains {
        field: title.clone(),
        item: Box::new(QueryExprIr {
            kind: QueryExprKindIr::Phrase(vec![
                QueryExprIr {
                    kind: QueryExprKindIr::Term {
                        value: RequestValueIr::String("wireless".to_owned()),
                    },
                    annotations: std::collections::BTreeMap::new(),
                },
                QueryExprIr {
                    kind: QueryExprKindIr::Term {
                        value: RequestValueIr::String("headphones".to_owned()),
                    },
                    annotations: std::collections::BTreeMap::new(),
                },
            ]),
            annotations: std::collections::BTreeMap::new(),
        }),
    });
    assert_eq!(
        lower_query(
            &phrase,
            &[SearchFieldIr::new(
                field("title"),
                SearchFieldWeight::new(1.0).unwrap(),
            )],
        )
        .unwrap(),
        Query::phrase("title", "wireless headphones")
    );

    let text = typed_expression(QueryExprKindIr::Contains {
        field: title,
        item: Box::new(QueryExprIr {
            kind: QueryExprKindIr::Text {
                text: "wireless headphones".to_owned(),
            },
            annotations: std::collections::BTreeMap::new(),
        }),
    });
    assert_eq!(
        lower_query(
            &text,
            &[SearchFieldIr::new(
                field("title"),
                SearchFieldWeight::new(1.0).unwrap(),
            )],
        )
        .unwrap(),
        Query::Text {
            field: "title".into(),
            text: "wireless headphones".into(),
            target_num_hits: 100,
        }
    );

    assert_eq!(
        lower_query(
            &typed_expression(QueryExprKindIr::And(vec![
                QueryExprIr {
                    kind: QueryExprKindIr::True,
                    annotations: std::collections::BTreeMap::new(),
                },
                QueryExprIr {
                    kind: QueryExprKindIr::Not(Box::new(QueryExprIr {
                        kind: QueryExprKindIr::False,
                        annotations: std::collections::BTreeMap::new(),
                    })),
                    annotations: std::collections::BTreeMap::new(),
                },
            ])),
            &[],
        )
        .unwrap(),
        Query::And(vec![
            Query::True,
            Query::Not(Box::new(Query::Not(Box::new(Query::True)))),
        ])
    );

    assert_eq!(
        lower_query(
            &typed_expression(QueryExprKindIr::Range {
                field: typed_field("price"),
                lower: QueryRangeBoundIr::Finite(RequestValueIr::Int64(10)),
                upper: QueryRangeBoundIr::PositiveInfinity,
                include_lower: Some(true),
                include_upper: None,
            }),
            &[],
        )
        .unwrap(),
        Query::AttributeRange {
            field: "price".to_owned(),
            lower: searchkernel_vespa_engine::AttributeBound::Included(VespaValue::Integer(10)),
            upper: searchkernel_vespa_engine::AttributeBound::Unbounded,
        }
    );

    assert_eq!(
        lower_query(
            &typed_expression(QueryExprKindIr::Comparison {
                field: typed_field("price"),
                op: ComparisonOp::Gte,
                value: RequestValueIr::Int64(10),
            }),
            &[],
        )
        .unwrap(),
        Query::AttributeRange {
            field: "price".to_owned(),
            lower: searchkernel_vespa_engine::AttributeBound::Included(VespaValue::Integer(10)),
            upper: searchkernel_vespa_engine::AttributeBound::Unbounded,
        }
    );

    assert_eq!(
        lower_query(
            &typed_expression(QueryExprKindIr::In {
                field: typed_field("brand"),
                values: vec![
                    RequestValueIr::String("a".to_owned()),
                    RequestValueIr::String("b".to_owned()),
                ],
            }),
            &[],
        )
        .unwrap(),
        Query::In {
            field: "brand".to_owned(),
            values: vec![
                searchkernel_vespa_engine::InQueryValue::String("a".to_owned()),
                searchkernel_vespa_engine::InQueryValue::String("b".to_owned()),
            ],
        }
    );
}

#[test]
fn typed_query_c_lowers_specialized_nodes() {
    let same_element = typed_contains(
        "tags",
        QueryExprKindIr::SameElement(vec![typed_term("wireless"), typed_term("audio")]),
    );
    assert_eq!(
        lower_query(&same_element, &[]).unwrap(),
        Query::SameElement {
            field: "tags".to_owned(),
            children: vec![
                Query::term("tags", "wireless"),
                Query::term("tags", "audio")
            ],
            element_filter: Vec::new(),
        }
    );

    let nearest_neighbor = typed_expression(QueryExprKindIr::NearestNeighbor {
        document_field: typed_field("embedding"),
        query_tensor: "query_embedding".to_owned(),
    });
    assert_eq!(
        lower_query(&nearest_neighbor, &[]).unwrap(),
        Query::NearestNeighbor {
            document_field: "embedding".to_owned(),
            query_tensor: "query_embedding".to_owned(),
            target_num_hits: 100,
            approximate: true,
        }
    );

    assert_eq!(
        lower_query(
            &typed_expression(QueryExprKindIr::GeoLocation {
                field: typed_field("location"),
                latitude: 37.3,
                longitude: -121.9,
                radius: "10km".to_owned(),
            }),
            &[],
        )
        .unwrap(),
        Query::GeoLocation {
            field: "location".to_owned(),
            latitude: 37.3,
            longitude: -121.9,
            radius: "10km".to_owned(),
        }
    );

    assert_eq!(
        lower_query(
            &typed_expression(QueryExprKindIr::GeoBoundingBox {
                field: typed_field("location"),
                south: 37.0,
                west: -122.0,
                north: 38.0,
                east: -121.0,
            }),
            &[],
        )
        .unwrap(),
        Query::GeoBoundingBox {
            field: "location".to_owned(),
            south: 37.0,
            west: -122.0,
            north: 38.0,
            east: -121.0,
        }
    );

    let predicate = typed_expression(QueryExprKindIr::Predicate {
        field: typed_field("access"),
        features: std::collections::BTreeMap::from([(
            "hobby".to_owned(),
            RequestValueIr::List(vec![
                RequestValueIr::String("music".to_owned()),
                RequestValueIr::String("hiking".to_owned()),
            ]),
        )]),
        range_features: std::collections::BTreeMap::from([(
            "age".to_owned(),
            RequestValueIr::UInt64(21),
        )]),
    });
    assert_eq!(
        lower_query(&predicate, &[]).unwrap(),
        Query::Predicate {
            field: "access".to_owned(),
            features: vec![
                searchkernel_vespa_engine::PredicateFeature {
                    key: "hobby".to_owned(),
                    value: "music".to_owned(),
                    sub_query_bitmask: u64::MAX,
                },
                searchkernel_vespa_engine::PredicateFeature {
                    key: "hobby".to_owned(),
                    value: "hiking".to_owned(),
                    sub_query_bitmask: u64::MAX,
                },
            ],
            range_features: vec![searchkernel_vespa_engine::PredicateRangeFeature {
                key: "age".to_owned(),
                value: 21,
                sub_query_bitmask: u64::MAX,
            }],
        }
    );
}

fn typed_contains(field_name: &str, kind: QueryExprKindIr) -> QueryIr {
    typed_expression(QueryExprKindIr::Contains {
        field: typed_field(field_name),
        item: Box::new(QueryExprIr {
            kind,
            annotations: std::collections::BTreeMap::new(),
        }),
    })
}

fn typed_term(value: &str) -> QueryExprIr {
    QueryExprIr {
        kind: QueryExprKindIr::Term {
            value: RequestValueIr::String(value.to_owned()),
        },
        annotations: std::collections::BTreeMap::new(),
    }
}

#[test]
fn typed_query_b_lowers_to_vespa_query_nodes() {
    let terms = vec![typed_term("wireless"), typed_term("headphones")];
    assert_eq!(
        lower_query(
            &typed_contains(
                "title",
                QueryExprKindIr::Near {
                    children: terms.clone(),
                    distance: 7,
                },
            ),
            &[]
        )
        .unwrap(),
        Query::Near {
            field: "title".to_owned(),
            children: vec![
                Query::term("title", "wireless"),
                Query::term("title", "headphones")
            ],
            distance: 7,
        }
    );
    assert_eq!(
        lower_query(
            &typed_contains(
                "title",
                QueryExprKindIr::ONear {
                    children: terms.clone(),
                    distance: 6,
                },
            ),
            &[]
        )
        .unwrap(),
        Query::ONear {
            field: "title".to_owned(),
            children: vec![
                Query::term("title", "wireless"),
                Query::term("title", "headphones")
            ],
            distance: 6,
        }
    );
    assert_eq!(
        lower_query(
            &typed_contains(
                "title",
                QueryExprKindIr::Fuzzy {
                    value: RequestValueIr::String("wireles".to_owned()),
                    max_edit_distance: 4,
                    prefix_length: 3,
                    prefix: true,
                },
            ),
            &[],
        )
        .unwrap(),
        Query::Fuzzy {
            field: "title".to_owned(),
            text: "wireles".to_owned(),
            max_edit_distance: 4,
            prefix_lock_length: 3,
            prefix_match: true,
        }
    );
    assert_eq!(
        lower_query(
            &typed_contains(
                "title",
                QueryExprKindIr::WeakAnd {
                    children: terms.clone(),
                    target_hits: 11,
                },
            ),
            &[],
        )
        .unwrap(),
        Query::WeakAnd {
            children: vec![
                Query::term("title", "wireless"),
                Query::term("title", "headphones")
            ],
            target_num_hits: 11,
        }
    );
    assert_eq!(
        lower_query(
            &typed_contains("title", QueryExprKindIr::Equiv(terms.clone())),
            &[]
        )
        .unwrap(),
        Query::Equiv(vec![
            Query::term("title", "wireless"),
            Query::term("title", "headphones")
        ])
    );
    assert_eq!(
        lower_query(
            &typed_expression(QueryExprKindIr::WeakAnd {
                children: vec![
                    QueryExprIr {
                        kind: QueryExprKindIr::Contains {
                            field: typed_field("title"),
                            item: Box::new(typed_term("vespa")),
                        },
                        annotations: std::collections::BTreeMap::new(),
                    },
                    QueryExprIr {
                        kind: QueryExprKindIr::Contains {
                            field: typed_field("description"),
                            item: Box::new(typed_term("search")),
                        },
                        annotations: std::collections::BTreeMap::new(),
                    },
                ],
                target_hits: 100,
            }),
            &[],
        )
        .unwrap(),
        Query::WeakAnd {
            children: vec![
                Query::term("title", "vespa"),
                Query::term("description", "search"),
            ],
            target_num_hits: 100,
        }
    );
    assert_eq!(
        lower_query(
            &typed_contains(
                "title",
                QueryExprKindIr::Rank {
                    matching: Box::new(terms[0].clone()),
                    ranking: vec![terms[1].clone()],
                },
            ),
            &[],
        )
        .unwrap(),
        Query::Rank {
            matching: Box::new(Query::term("title", "wireless")),
            ranking: vec![Query::term("title", "headphones")],
        }
    );
    assert_eq!(
        lower_query(
            &typed_expression(QueryExprKindIr::WeightedSet {
                field: typed_field("title"),
                items: vec![
                    WeightedQueryItemIr {
                        key: RequestValueIr::String("wireless".to_owned()),
                        weight: 100,
                    },
                    WeightedQueryItemIr {
                        key: RequestValueIr::String("headphones".to_owned()),
                        weight: 50,
                    },
                ],
            }),
            &[],
        )
        .unwrap(),
        Query::WeightedSet {
            field: "title".to_owned(),
            items: vec![
                searchkernel_vespa_engine::WeightedQueryItem::String {
                    key: "wireless".to_owned(),
                    weight: 100,
                },
                searchkernel_vespa_engine::WeightedQueryItem::String {
                    key: "headphones".to_owned(),
                    weight: 50,
                },
            ],
        }
    );
    assert_eq!(
        lower_query(
            &typed_expression(QueryExprKindIr::Wand {
                field: typed_field("title"),
                items: vec![WeightedQueryItemIr {
                    key: RequestValueIr::Int64(7),
                    weight: 100,
                }],
            }),
            &[],
        )
        .unwrap(),
        Query::Wand {
            field: "title".to_owned(),
            items: vec![searchkernel_vespa_engine::WeightedQueryItem::Integer {
                key: 7,
                weight: 100,
            }],
        }
    );
}

#[test]
fn typed_query_b_rejects_unsupported_weighted_values() {
    let mixed = typed_expression(QueryExprKindIr::WeightedSet {
        field: typed_field("title"),
        items: vec![
            WeightedQueryItemIr {
                key: RequestValueIr::String("wireless".to_owned()),
                weight: 100,
            },
            WeightedQueryItemIr {
                key: RequestValueIr::Int64(1),
                weight: 100,
            },
        ],
    });
    let error = lower_query(&mixed, &[]).unwrap_err();
    assert_eq!(
        error.details[0].code,
        "typed_query_weighted_key_not_supported"
    );

    let tensor = typed_expression(QueryExprKindIr::WeightedSet {
        field: typed_field("title"),
        items: vec![WeightedQueryItemIr {
            key: RequestValueIr::Tensor(RequestTensorIr {
                type_spec: "tensor<float>(x[2])".to_owned(),
                payload: RequestTensorPayloadIr::Dense(vec![1.0, 2.0]),
            }),
            weight: 100,
        }],
    });
    let error = lower_query(&tensor, &[]).unwrap_err();
    assert_eq!(
        error.details[0].code,
        "typed_query_weighted_key_not_supported"
    );
}

#[test]
fn typed_query_b_rejects_unsupported_reduced_user_query_syntax() {
    for value in [
        r#"wire\ less"#,  // escaping
        "wireless^2",     // weights
        "price:[10,20]",  // ranges
        "wire*",          // prefix/suffix
        "*wire",          // suffix/prefix
        "wire*less",      // infix wildcard
        "wire?less",      // wildcard
        "wire~2",         // fuzzy parser syntax
        "rank(wireless)", // composite/parser mode syntax
        "weakAnd(wireless)",
        "wireless+headphones",
    ] {
        let query = typed_contains(
            "title",
            QueryExprKindIr::UserInput {
                value: value.to_owned(),
                default_index: None,
                target_hits: 100,
            },
        );
        let error = lower_query(&query, &[]).expect_err(value);
        assert!(
            matches!(
                error.details[0].code.as_str(),
                "user_query_syntax_not_supported" | "user_query_composite_syntax_not_supported"
            ),
            "{value} returned {}",
            error.details[0].code
        );
    }
}

#[test]
fn typed_query_b_non_empty_rejects_false_and_documents_alternatives_shape() {
    let error = lower_query(
        &typed_contains(
            "title",
            QueryExprKindIr::NonEmpty(Box::new(QueryExprIr {
                kind: QueryExprKindIr::False,
                annotations: std::collections::BTreeMap::new(),
            })),
        ),
        &[],
    )
    .expect_err("false is an empty native query");
    assert_eq!(error.details[0].code, "empty_typed_query");

    let unsupported = typed_contains(
        "title",
        QueryExprKindIr::Alternatives(vec![QueryAlternativeIr {
            query: QueryExprIr {
                kind: QueryExprKindIr::And(vec![typed_term("wireless"), typed_term("headphones")]),
                annotations: std::collections::BTreeMap::new(),
            },
            exactness: None,
        }]),
    );
    let error = lower_query(&unsupported, &[]).expect_err("alternatives are term-only");
    assert_eq!(
        error.details[0].code,
        "typed_query_alternative_not_supported"
    );
}

#[test]
fn typed_query_b_remaining_operators_lower_to_existing_vespa_nodes() {
    let alternatives = typed_contains(
        "title",
        QueryExprKindIr::Alternatives(vec![
            QueryAlternativeIr {
                query: typed_term("wireless"),
                exactness: Some(1.0),
            },
            QueryAlternativeIr {
                query: typed_term("cordless"),
                exactness: Some(0.7),
            },
        ]),
    );
    assert_eq!(
        lower_query(&alternatives, &[]).unwrap(),
        Query::Alternatives {
            field: "title".to_owned(),
            alternatives: vec![
                searchkernel_vespa_engine::QueryAlternative {
                    text: "wireless".to_owned(),
                    exactness: 1.0,
                },
                searchkernel_vespa_engine::QueryAlternative {
                    text: "cordless".to_owned(),
                    exactness: 0.7,
                },
            ],
        }
    );
    assert_eq!(
        lower_query(
            &typed_expression(QueryExprKindIr::Matches {
                field: typed_field("title"),
                pattern: "wire.*".to_owned(),
            }),
            &[],
        )
        .unwrap(),
        Query::Matches {
            field: "title".to_owned(),
            pattern: "wire.*".to_owned(),
        }
    );
    assert_eq!(
        lower_query(
            &typed_expression(QueryExprKindIr::DotProduct {
                field: typed_field("tags"),
                items: vec![WeightedQueryItemIr {
                    key: RequestValueIr::String("wireless".to_owned()),
                    weight: 4,
                }],
            }),
            &[],
        )
        .unwrap(),
        Query::DotProduct {
            field: "tags".to_owned(),
            items: vec![searchkernel_vespa_engine::WeightedQueryItem::String {
                key: "wireless".to_owned(),
                weight: 4,
            }],
        }
    );

    let labeled_request = request(Vec::new(), Vec::new(), None);
    let labeled_request = labeled_request.with_query(typed_contains(
        "title",
        QueryExprKindIr::Labeled {
            query: Box::new(typed_term("wireless")),
            label: "candidate".to_owned(),
            score: 2.5,
        },
    ));
    assert_eq!(
        lower_search_query(
            &labeled_request,
            searchkernel_vespa_engine::LinguisticsOptions::default(),
        )
        .unwrap(),
        Query::Labeled {
            query: Box::new(Query::term("title", "wireless")),
            unique_id: 1,
            score: 2.5,
        }
    );

    assert_eq!(
        lower_query(
            &typed_contains(
                "title",
                QueryExprKindIr::NonEmpty(Box::new(typed_term("wireless"))),
            ),
            &[],
        )
        .unwrap(),
        Query::term("title", "wireless")
    );
    assert_eq!(
        lower_query(
            &typed_contains(
                "title",
                QueryExprKindIr::UserInput {
                    value: "wireless headphones".to_owned(),
                    default_index: None,
                    target_hits: 100,
                },
            ),
            &[],
        )
        .unwrap(),
        Query::WeakAnd {
            children: vec![
                Query::term("title", "wireless"),
                Query::term("title", "headphones"),
            ],
            target_num_hits: 100,
        }
    );
    let user_query_request = request(Vec::new(), Vec::new(), None)
        .with_query(typed_expression(QueryExprKindIr::UserQuery {
            default_index: None,
            target_hits: 100,
        }))
        .with_query_model(QueryModelIr {
            query_string: Some("wireless headphones".to_owned()),
            default_index: Some("title".to_owned()),
            ..QueryModelIr::default()
        });
    assert_eq!(
        lower_search_query(
            &user_query_request,
            searchkernel_vespa_engine::LinguisticsOptions::default(),
        )
        .unwrap(),
        Query::WeakAnd {
            children: vec![
                Query::term("title", "wireless"),
                Query::term("title", "headphones"),
            ],
            target_num_hits: 100,
        }
    );
}

#[test]
fn typed_query_a_reaches_real_vespa_through_the_adapter() {
    let root = std::env::temp_dir().join(format!(
        "searchkernel-adapter-typed-query-a-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let data = root.join("data");
    let config = root.join("config");
    let schema = Schema::new("typed_query_a")
        .field(Field::new("title", FieldType::Text).indexed().summary())
        .field(
            Field::new("sku", FieldType::String)
                .attribute()
                .fast_search()
                .summary(),
        )
        .field(
            Field::new("category", FieldType::String)
                .attribute()
                .summary(),
        )
        .field(Field::new("qty", FieldType::Integer).attribute().summary())
        .field(
            Field::new("price", FieldType::Integer)
                .attribute()
                .summary(),
        );
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile typed-query schema");
    let backend = VespaBackend::open(&data, &config).expect("open Vespa backend");

    for (id, title, sku, category, qty, price) in [
        ("doc:wireless", "wireless headphones", "A", "audio", 1, 20),
        ("doc:wired", "wired headphones", "B", "audio", 2, 5),
    ] {
        backend
            .insert(InsertRequestIr::new(DocumentIr::new(
                DocumentId::parse(id).unwrap(),
                std::collections::BTreeMap::from([
                    (
                        field("title"),
                        FieldValueIr::Single(ScalarValueIr::String(title.to_owned())),
                    ),
                    (
                        field("price"),
                        FieldValueIr::Single(ScalarValueIr::Int64(price)),
                    ),
                    (
                        field("sku"),
                        FieldValueIr::Single(ScalarValueIr::String(sku.to_owned())),
                    ),
                    (
                        field("category"),
                        FieldValueIr::Single(ScalarValueIr::String(category.to_owned())),
                    ),
                    (
                        field("qty"),
                        FieldValueIr::Single(ScalarValueIr::Int64(qty)),
                    ),
                ]),
            )))
            .expect("insert document");
    }

    let contains = SearchRequestIr::new(
        typed_expression(QueryExprKindIr::Contains {
            field: typed_field("title"),
            item: Box::new(QueryExprIr {
                kind: QueryExprKindIr::Term {
                    value: RequestValueIr::String("wireless".to_owned()),
                },
                annotations: std::collections::BTreeMap::new(),
            }),
        }),
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
    );
    let result = backend.search(contains).expect("execute typed term");
    let ResultNode::Group(group) = result.root else {
        panic!("search root must be a group");
    };
    assert_eq!(
        group
            .children
            .iter()
            .map(|child| match child {
                ResultNode::Hit(hit) => hit.id.as_str(),
                ResultNode::Group(_) => panic!("unexpected nested group"),
            })
            .collect::<Vec<_>>(),
        vec!["doc:wireless"]
    );

    let in_query = SearchRequestIr::new(
        typed_expression(QueryExprKindIr::In {
            field: typed_field("sku"),
            values: vec![
                RequestValueIr::String("A".to_owned()),
                RequestValueIr::String("B".to_owned()),
            ],
        }),
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
    );
    let result = backend.search(in_query).expect("execute typed InTerm");
    let ResultNode::Group(group) = result.root else {
        panic!("search root must be a group");
    };
    assert_eq!(
        group
            .children
            .iter()
            .map(|child| match child {
                ResultNode::Hit(hit) => hit.id.as_str(),
                ResultNode::Group(_) => panic!("unexpected nested group"),
            })
            .collect::<Vec<_>>(),
        vec!["doc:wireless", "doc:wired"]
    );

    let text_query = SearchRequestIr::new(
        typed_contains(
            "title",
            QueryExprKindIr::Text {
                text: "wireless headphones".to_owned(),
            },
        ),
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
    );
    let result = backend.search(text_query).expect("execute typed text");
    let ResultNode::Group(group) = result.root else {
        panic!("search root must be a group");
    };
    assert_eq!(
        group
            .children
            .iter()
            .map(|child| match child {
                ResultNode::Hit(hit) => hit.id.as_str(),
                ResultNode::Group(_) => panic!("unexpected nested group"),
            })
            .collect::<Vec<_>>(),
        vec!["doc:wireless", "doc:wired"]
    );

    let comparison = SearchRequestIr::new(
        typed_expression(QueryExprKindIr::Comparison {
            field: typed_field("price"),
            op: ComparisonOp::Gte,
            value: RequestValueIr::Int64(10),
        }),
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
    );
    let result = backend
        .search(comparison.clone())
        .expect("execute typed comparison");
    let ResultNode::Group(group) = result.root else {
        panic!("search root must be a group");
    };
    assert_eq!(group.children.len(), 1);
    assert!(matches!(
        &group.children[0],
        ResultNode::Hit(hit) if hit.id == "doc:wireless"
    ));

    let grouped = comparison
        .clone()
        .with_query(QueryIr::MatchAll)
        .with_grouping(vec![native_grouping(&[
            (
                "category",
                vec![count_expression(), average_expression("price")],
            ),
            ("qty", vec![count_expression(), average_expression("price")]),
        ])]);
    let result = backend.search(grouped).expect("execute native grouping");
    let ResultNode::Group(group) = result.root else {
        panic!("search root must be a group");
    };
    let [ResultNode::Hit(_), ResultNode::Hit(_), ResultNode::Group(grouping)] =
        group.children.as_slice()
    else {
        panic!("expected the hit and native grouping result");
    };
    assert_eq!(grouping.children.len(), 1);
    let ResultNode::Group(bucket) = &grouping.children[0] else {
        panic!("grouping result child must be a group");
    };
    assert_eq!(bucket.value, Some(Value::String("audio".into())));
    assert_eq!(bucket.fields["count"], Value::Int64(2));
    assert_eq!(bucket.fields["average(price)"], Value::Float64(12.5));
    assert_eq!(bucket.children.len(), 2);
    let ResultNode::Group(qty_one) = &bucket.children[0] else {
        panic!("first nested grouping result child must be a group");
    };
    let ResultNode::Group(qty_two) = &bucket.children[1] else {
        panic!("second nested grouping result child must be a group");
    };
    assert_eq!(qty_one.value, Some(Value::Int64(1)));
    assert_eq!(qty_one.fields["count"], Value::Int64(1));
    assert_eq!(qty_one.fields["average(price)"], Value::Float64(20.0));
    assert_eq!(qty_two.value, Some(Value::Int64(2)));
    assert_eq!(qty_two.fields["count"], Value::Int64(1));
    assert_eq!(qty_two.fields["average(price)"], Value::Float64(5.0));

    let safe_integer_boundary = (1_i64 << 53) - 1;
    let adjacent_integer = 1_i64 << 53;
    for (id, price) in [
        ("doc:safe-boundary", safe_integer_boundary),
        ("doc:adjacent-boundary", adjacent_integer),
    ] {
        backend
            .insert(InsertRequestIr::new(DocumentIr::new(
                DocumentId::parse(id).unwrap(),
                std::collections::BTreeMap::from([
                    (
                        field("title"),
                        FieldValueIr::Single(ScalarValueIr::String(id.to_owned())),
                    ),
                    (
                        field("price"),
                        FieldValueIr::Single(ScalarValueIr::Int64(price)),
                    ),
                    (
                        field("sku"),
                        FieldValueIr::Single(ScalarValueIr::String(id.to_owned())),
                    ),
                    (
                        field("category"),
                        FieldValueIr::Single(ScalarValueIr::String("precision".to_owned())),
                    ),
                    (field("qty"), FieldValueIr::Single(ScalarValueIr::Int64(1))),
                ]),
            )))
            .expect("insert precision-boundary document");
    }

    let mut safe_precision_grouping = native_grouping(&[("price", vec![count_expression()])]);
    let each = safe_precision_grouping
        .root
        .operations
        .iter_mut()
        .find_map(|operation| match operation {
            GroupingOperationIr::Each(each) => Some(each),
            _ => None,
        })
        .expect("safe precision grouping has an each operation");
    each.operations.insert(
        0,
        GroupingOperationIr::Filter(GroupingFilterIr::Range {
            expression: Box::new(GroupingExpressionIr::Attribute("price".into())),
            lower: Box::new(GroupingExpressionIr::Literal(RequestValueIr::Int64(
                safe_integer_boundary,
            ))),
            upper: Box::new(GroupingExpressionIr::Literal(RequestValueIr::Int64(
                safe_integer_boundary,
            ))),
            include_lower: true,
            include_upper: true,
        }),
    );
    let result = backend
        .search(
            comparison
                .clone()
                .with_query(QueryIr::MatchAll)
                .with_grouping(vec![safe_precision_grouping]),
        )
        .expect("execute grouping at the last contiguous exact integer boundary");
    let ResultNode::Group(group) = result.root else {
        panic!("search root must be a group");
    };
    let grouping = group
        .children
        .iter()
        .find_map(|node| match node {
            ResultNode::Group(grouping) => Some(grouping),
            ResultNode::Hit(_) => None,
        })
        .expect("precision grouping result");
    assert_eq!(grouping.children.len(), 1);
    let ResultNode::Group(boundary_group) = &grouping.children[0] else {
        panic!("precision result child must be a group");
    };
    assert_eq!(
        boundary_group.value,
        Some(Value::Int64(safe_integer_boundary))
    );
    assert_eq!(boundary_group.fields["count"], Value::Int64(1));

    let mut precision_grouping = native_grouping(&[("price", vec![count_expression()])]);
    let each = precision_grouping
        .root
        .operations
        .iter_mut()
        .find_map(|operation| match operation {
            GroupingOperationIr::Each(each) => Some(each),
            _ => None,
        })
        .expect("precision grouping has an each operation");
    each.operations.insert(
        0,
        GroupingOperationIr::Filter(GroupingFilterIr::Range {
            expression: Box::new(GroupingExpressionIr::Attribute("price".into())),
            lower: Box::new(GroupingExpressionIr::Literal(RequestValueIr::UInt64(
                9_007_199_254_740_992,
            ))),
            upper: Box::new(GroupingExpressionIr::Literal(RequestValueIr::UInt64(
                9_007_199_254_740_993,
            ))),
            include_lower: true,
            include_upper: false,
        }),
    );
    let error = backend
        .search(
            comparison
                .clone()
                .with_query(QueryIr::MatchAll)
                .with_grouping(vec![precision_grouping]),
        )
        .expect_err("precision-losing grouping bounds must fail before native execution");
    assert_eq!(error.code, ErrorCode::Unsupported);
    assert_eq!(
        error.details[0].code,
        "grouping_filter_bound_precision_loss"
    );

    let mut invalid_topology = native_grouping(&[(
        "category",
        vec![count_expression(), average_expression("price")],
    )]);
    let each = invalid_topology
        .root
        .operations
        .pop()
        .expect("canonical grouping has an each operation");
    let GroupingOperationIr::Each(each) = each else {
        panic!("canonical grouping must end with each");
    };
    invalid_topology
        .root
        .operations
        .push(GroupingOperationIr::All(each));
    let error = backend
        .search(
            comparison
                .with_query(QueryIr::MatchAll)
                .with_grouping(vec![invalid_topology]),
        )
        .expect_err("an all-only materialization must be rejected");
    assert_eq!(error.details[0].code, "grouping_all_topology_not_supported");

    backend.engine.close().expect("close Vespa backend");
    std::fs::remove_dir_all(root).expect("remove typed-query test data");
}

#[test]
fn typed_query_b_reaches_real_vespa_query_nodes() {
    let root = std::env::temp_dir().join(format!(
        "searchkernel-adapter-typed-query-b-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let data = root.join("data");
    let config = root.join("config");
    let schema = Schema::new("typed_query_b")
        .field(Field::new("title", FieldType::Text).indexed().summary())
        .field(
            Field::new("description", FieldType::Text)
                .indexed()
                .summary(),
        )
        .field(Field::new("sku", FieldType::String).attribute().summary());
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile typed-query schema");
    let backend = VespaBackend::open(&data, &config).expect("open Vespa backend");

    for (id, title, description) in [
        ("doc:ordered", "wireless headphones", "audio equipment"),
        ("doc:reversed", "headphones wireless", "audio equipment"),
        (
            "doc:spaced",
            "wireless studio noise headphones",
            "audio equipment",
        ),
        ("doc:other", "wireless speaker", "portable equipment"),
        ("doc:multi-field", "wireless", "vespa search"),
    ] {
        backend
            .insert(InsertRequestIr::new(DocumentIr::new(
                DocumentId::parse(id).unwrap(),
                std::collections::BTreeMap::from([
                    (
                        field("title"),
                        FieldValueIr::Single(ScalarValueIr::String(title.to_owned())),
                    ),
                    (
                        field("description"),
                        FieldValueIr::Single(ScalarValueIr::String(description.to_owned())),
                    ),
                    (
                        field("sku"),
                        FieldValueIr::Single(ScalarValueIr::String(
                            (if matches!(id, "doc:other" | "doc:multi-field" | "doc:spaced") {
                                "speaker"
                            } else {
                                "wireless"
                            })
                            .to_owned(),
                        )),
                    ),
                ]),
            )))
            .expect("insert document");
    }

    let request = |query: QueryIr| {
        SearchRequestIr::new(
            query,
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
    };
    let ids = |result: SearchResult| {
        let ResultNode::Group(group) = result.root else {
            panic!("search root must be a group");
        };
        group
            .children
            .iter()
            .map(|child| match child {
                ResultNode::Hit(hit) => hit.id.as_str().to_owned(),
                ResultNode::Group(_) => panic!("unexpected nested group"),
            })
            .collect::<Vec<_>>()
    };

    let near_ids = ids(backend
        .search(request(typed_contains(
            "title",
            QueryExprKindIr::Near {
                children: vec![typed_term("wireless"), typed_term("headphones")],
                distance: 2,
            },
        )))
        .expect("execute near query"));
    assert!(near_ids.contains(&"doc:ordered".to_owned()));
    assert!(near_ids.contains(&"doc:reversed".to_owned()));
    assert!(!near_ids.contains(&"doc:spaced".to_owned()));

    let onear_ids = ids(backend
        .search(request(typed_contains(
            "title",
            QueryExprKindIr::ONear {
                children: vec![typed_term("wireless"), typed_term("headphones")],
                distance: 2,
            },
        )))
        .expect("execute onear query"));
    assert_eq!(onear_ids, vec!["doc:ordered"]);

    let weak_and_ids = ids(backend
        .search(request(typed_expression(QueryExprKindIr::WeakAnd {
            children: vec![
                QueryExprIr {
                    kind: QueryExprKindIr::Contains {
                        field: typed_field("title"),
                        item: Box::new(typed_term("wireless")),
                    },
                    annotations: std::collections::BTreeMap::new(),
                },
                QueryExprIr {
                    kind: QueryExprKindIr::Contains {
                        field: typed_field("description"),
                        item: Box::new(typed_term("search")),
                    },
                    annotations: std::collections::BTreeMap::new(),
                },
            ],
            target_hits: 100,
        })))
        .expect("execute cross-field weak-and query"));
    assert!(weak_and_ids.contains(&"doc:multi-field".to_owned()));

    let fuzzy_ids = ids(backend
        .search(request(typed_contains(
            "sku",
            QueryExprKindIr::Fuzzy {
                value: RequestValueIr::String("wireles".to_owned()),
                max_edit_distance: 2,
                prefix_length: 0,
                prefix: false,
            },
        )))
        .expect("execute fuzzy query"));
    assert!(fuzzy_ids.contains(&"doc:ordered".to_owned()));
    assert!(fuzzy_ids.contains(&"doc:reversed".to_owned()));

    for (name, kind) in [
        (
            "weak-and",
            QueryExprKindIr::WeakAnd {
                children: vec![typed_term("wireless"), typed_term("headphones")],
                target_hits: 100,
            },
        ),
        (
            "equiv",
            QueryExprKindIr::Equiv(vec![typed_term("wireless"), typed_term("headphones")]),
        ),
    ] {
        let result = backend
            .search(request(typed_contains("title", kind)))
            .unwrap_or_else(|error| panic!("execute {name} query: {error}"));
        assert!(!ids(result).is_empty(), "{name} query returned no hits");
    }

    let weighted_items = vec![
        WeightedQueryItemIr {
            key: RequestValueIr::String("wireless".to_owned()),
            weight: 100,
        },
        WeightedQueryItemIr {
            key: RequestValueIr::String("headphones".to_owned()),
            weight: 50,
        },
    ];
    let weighted_ids = ids(backend
        .search(request(typed_expression(QueryExprKindIr::WeightedSet {
            field: typed_field("title"),
            items: weighted_items.clone(),
        })))
        .expect("execute weighted-set query"));
    assert!(weighted_ids.contains(&"doc:ordered".to_owned()));

    let wand_ids = ids(backend
        .search(request(typed_expression(QueryExprKindIr::Wand {
            field: typed_field("title"),
            items: weighted_items,
        })))
        .expect("execute wand query"));
    assert!(wand_ids.contains(&"doc:ordered".to_owned()));

    let rank_ids = ids(backend
        .search(request(typed_contains(
            "title",
            QueryExprKindIr::Rank {
                matching: Box::new(typed_term("wireless")),
                ranking: vec![typed_term("headphones")],
            },
        )))
        .expect("execute rank query"));
    assert_eq!(rank_ids.first().map(String::as_str), Some("doc:ordered"));

    let alternatives_ids = ids(backend
        .search(request(typed_contains(
            "title",
            QueryExprKindIr::Alternatives(vec![
                QueryAlternativeIr {
                    query: typed_term("wireless"),
                    exactness: Some(1.0),
                },
                QueryAlternativeIr {
                    query: typed_term("cordless"),
                    exactness: Some(0.5),
                },
            ]),
        )))
        .expect("execute alternatives query"));
    assert!(alternatives_ids.contains(&"doc:ordered".to_owned()));

    let matches_ids = ids(backend
        .search(request(typed_expression(QueryExprKindIr::Matches {
            field: typed_field("sku"),
            pattern: "wire.*".to_owned(),
        })))
        .expect("execute matches query"));
    assert_eq!(
        matches_ids,
        vec!["doc:ordered".to_owned(), "doc:reversed".to_owned()]
    );

    let dot_product_ids = ids(backend
        .search(request(typed_expression(QueryExprKindIr::DotProduct {
            field: typed_field("sku"),
            items: vec![WeightedQueryItemIr {
                key: RequestValueIr::String("wireless".to_owned()),
                weight: 100,
            }],
        })))
        .expect("execute dot-product query"));
    assert!(dot_product_ids.contains(&"doc:ordered".to_owned()));

    let user_input_ids = ids(backend
        .search(request(typed_contains(
            "title",
            QueryExprKindIr::UserInput {
                value: "wireless headphones".to_owned(),
                default_index: None,
                target_hits: 100,
            },
        )))
        .expect("userInput query"));
    assert!(user_input_ids.contains(&"doc:ordered".to_owned()));
    assert!(user_input_ids.contains(&"doc:spaced".to_owned()));

    let user_input_phrase_ids = ids(backend
        .search(request(typed_contains(
            "title",
            QueryExprKindIr::UserInput {
                value: "\"wireless headphones\"".to_owned(),
                default_index: None,
                target_hits: 100,
            },
        )))
        .expect("quoted userInput query"));
    assert_eq!(user_input_phrase_ids, vec!["doc:ordered"]);

    let user_query_ids = ids(backend
        .search(
            request(typed_expression(QueryExprKindIr::UserQuery {
                default_index: None,
                target_hits: 100,
            }))
            .with_query_model(QueryModelIr {
                query_string: Some("wireless headphones".to_owned()),
                default_index: Some("title".to_owned()),
                ..QueryModelIr::default()
            }),
        )
        .expect("userQuery query"));
    assert!(user_query_ids.contains(&"doc:ordered".to_owned()));

    let labeled_ids = ids(backend
        .search(request(typed_contains(
            "title",
            QueryExprKindIr::Labeled {
                query: Box::new(typed_term("wireless")),
                label: "candidate".to_owned(),
                score: 2.5,
            },
        )))
        .expect("execute labeled query"));
    assert!(labeled_ids.contains(&"doc:ordered".to_owned()));

    let non_empty_ids = ids(backend
        .search(request(typed_contains(
            "title",
            QueryExprKindIr::NonEmpty(Box::new(typed_term("wireless"))),
        )))
        .expect("NonEmpty query"));
    assert!(non_empty_ids.contains(&"doc:ordered".to_owned()));

    backend.engine.close().expect("close Vespa backend");
    std::fs::remove_dir_all(root).expect("remove typed-query test data");
}

#[test]
fn unsupported_search_semantics_are_rejected_instead_of_dropped() {
    let weighted = request(
        vec![SearchFieldIr::new(
            field("title"),
            SearchFieldWeight::new(2.0).unwrap(),
        )],
        Vec::new(),
        None,
    );
    let error = validate_search_request(&weighted).unwrap_err();
    assert_eq!(error.code, searchkernel_ir::ErrorCode::Unsupported);

    let missing_first = request(
        vec![SearchFieldIr::new(
            field("title"),
            SearchFieldWeight::new(1.0).unwrap(),
        )],
        vec![SortIr::new(
            field("price"),
            SortDirection::Asc,
            MissingPlacement::First,
        )],
        None,
    );
    assert!(validate_search_request(&missing_first).is_ok());

    let language = request(
        vec![SearchFieldIr::new(
            field("title"),
            SearchFieldWeight::new(1.0).unwrap(),
        )],
        Vec::new(),
        Some(LanguageHint::parse("en").unwrap()),
    );
    assert!(validate_search_request(&language).is_ok());

    let french = request(
        vec![SearchFieldIr::new(
            field("title"),
            SearchFieldWeight::new(1.0).unwrap(),
        )],
        Vec::new(),
        Some(LanguageHint::parse("fr").unwrap()),
    );
    let error = validate_search_request(&french).unwrap_err();
    assert_eq!(error.details[0].code, "language_not_supported");
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
fn multi_valued_schema_is_rejected_before_m_configuration() {
    let definition = IndexDefinitionIr::new(
        None,
        vec![FieldIr::new(
            field("tags"),
            DataTypeIr::Array(Box::new(DataTypeIr::Primitive(PrimitiveDataTypeIr::Int64))),
            None,
            FieldCapabilities::new(false, false, true, false, false),
        )],
        Vec::new(),
    )
    .unwrap();
    let error = validate_index_definition(&definition).unwrap_err();
    assert_eq!(error.code, searchkernel_ir::ErrorCode::Unsupported);
}

#[test]
fn document_type_drives_native_schema_identity() {
    let definition = {
        let field = FieldIr::new(
            field("title"),
            DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
            None,
            FieldCapabilities::new(true, true, false, false, false),
        );
        let mut definition = IndexDefinitionIr::new(
            Some(IndexName::parse("index_name").unwrap()),
            vec![field],
            Vec::new(),
        )
        .unwrap();
        definition.set_document_type("document_type");
        definition
    };
    let root = std::env::temp_dir().join(format!(
        "searchkernel-adapter-document-type-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    compile_index_definition(&definition, &root).expect("schema compiles");
    let manifest =
        std::fs::read_to_string(root.join("searchkernel-schema.json")).expect("manifest");
    assert!(manifest.contains("\"document_type\":\"document_type\""));
    assert!(!manifest.contains("\"document_type\":\"index_name\""));
    std::fs::remove_dir_all(root).expect("remove temporary schema");
}

#[test]
fn portable_dense_tensor_definition_compiles_and_opens_through_adapter() {
    let definition = IndexDefinitionIr::new(
        None,
        vec![FieldIr::new(
            field("embedding"),
            DataTypeIr::Tensor {
                type_spec: "tensor<float>(x[2])".to_owned(),
            },
            None,
            FieldCapabilities::new(true, false, false, false, false),
        )],
        Vec::new(),
    )
    .unwrap();
    let root = std::env::temp_dir().join(format!(
        "searchkernel-adapter-portable-tensor-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let config = root.join("config");
    let data = root.join("data");

    compile_index_definition(&definition, &config).expect("tensor schema compiles");
    let backend = VespaBackend::open(&data, &config).expect("tensor schema opens");
    Box::new(backend).close().expect("tensor backend closes");
    std::fs::remove_dir_all(root).expect("remove temporary tensor schema");
}

#[test]
fn portable_hnsw_tensor_definition_lowers_to_native_attribute_config() {
    let mut embedding = FieldIr::new(
        field("embedding"),
        DataTypeIr::Tensor {
            type_spec: "tensor<float>(x[3])".to_owned(),
        },
        None,
        FieldCapabilities::new(true, true, false, false, false),
    );
    embedding.set_index_config(Some(IndexConfigIr {
        enabled: Some(true),
        hnsw: Some(HnswIndexConfigIr {
            max_links_per_node: Some(4),
            neighbors_to_explore_at_insert: Some(16),
            options: std::collections::BTreeMap::new(),
        }),
        options: std::collections::BTreeMap::new(),
    }));
    embedding.set_attribute_config(Some(AttributeConfigIr {
        enabled: Some(true),
        fast_search: None,
        fast_access: None,
        distance_metric: DistanceMetricIr::Euclidean,
        options: std::collections::BTreeMap::new(),
    }));
    let definition = IndexDefinitionIr::new(None, vec![embedding], Vec::new()).unwrap();
    let root = std::env::temp_dir().join(format!(
        "searchkernel-adapter-hnsw-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    compile_index_definition(&definition, &root).expect("HNSW schema compiles");
    let attributes = std::fs::read_to_string(root.join("attributes.cfg")).unwrap();
    assert!(attributes.contains("attribute[0].index.hnsw.enabled true"));
    assert!(attributes.contains("attribute[0].distancemetric EUCLIDEAN"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn typed_attribute_settings_are_lowered_and_contradictions_fail_closed() {
    let mut value_field = FieldIr::new(
        field("value"),
        DataTypeIr::Primitive(PrimitiveDataTypeIr::Int64),
        None,
        FieldCapabilities::new(true, false, true, false, false),
    );
    value_field.set_attribute_config(Some(AttributeConfigIr {
        enabled: Some(true),
        fast_search: Some(true),
        fast_access: None,
        distance_metric: searchkernel_ir::DistanceMetricIr::Unspecified,
        options: std::collections::BTreeMap::new(),
    }));
    let definition = IndexDefinitionIr::new(None, vec![value_field], Vec::new()).unwrap();
    validate_index_definition(&definition).expect("typed attribute settings are supported");
    let root = std::env::temp_dir().join(format!(
        "searchkernel-adapter-attribute-settings-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    compile_index_definition(&definition, &root).expect("schema compiles");
    let attributes =
        std::fs::read_to_string(root.join("attributes.cfg")).expect("attributes config");
    assert!(attributes.contains("fastsearch true"));
    std::fs::remove_dir_all(root).expect("remove temporary schema");

    let mut contradictory = FieldIr::new(
        field("value"),
        DataTypeIr::Primitive(PrimitiveDataTypeIr::Int64),
        None,
        FieldCapabilities::new(true, false, true, false, false),
    );
    contradictory.set_attribute_config(Some(AttributeConfigIr {
        enabled: Some(false),
        fast_search: None,
        fast_access: None,
        distance_metric: searchkernel_ir::DistanceMetricIr::Unspecified,
        options: std::collections::BTreeMap::new(),
    }));
    let definition = IndexDefinitionIr::new(None, vec![contradictory], Vec::new()).unwrap();
    let error = validate_index_definition(&definition).expect_err("contradiction rejected");
    assert_eq!(error.code, searchkernel_ir::ErrorCode::InvalidArgument);
    assert_eq!(error.details[0].code, "invalid_capability");
}

#[test]
fn typed_index_and_summary_settings_cannot_disagree_with_resolved_capabilities() {
    let mut index_field = FieldIr::new(
        field("value"),
        DataTypeIr::Primitive(PrimitiveDataTypeIr::Int64),
        None,
        FieldCapabilities::new(true, false, false, false, false),
    );
    index_field.set_index_config(Some(IndexConfigIr {
        enabled: Some(true),
        hnsw: None,
        options: std::collections::BTreeMap::new(),
    }));
    let definition = IndexDefinitionIr::new(None, vec![index_field], Vec::new()).unwrap();
    let error = validate_index_definition(&definition).expect_err("index contradiction");
    assert_eq!(error.details[0].code, "invalid_capability");

    let mut summary_field = FieldIr::new(
        field("value"),
        DataTypeIr::Primitive(PrimitiveDataTypeIr::Int64),
        None,
        FieldCapabilities::new(true, false, false, false, false),
    );
    summary_field.set_summary_config(Some(searchkernel_ir::SummaryConfigIr {
        enabled: Some(false),
        options: std::collections::BTreeMap::new(),
    }));
    let definition = IndexDefinitionIr::new(None, vec![summary_field], Vec::new()).unwrap();
    let error = validate_index_definition(&definition).expect_err("summary contradiction");
    assert_eq!(error.details[0].code, "invalid_capability");
}

#[test]
fn word_matching_is_rejected_instead_of_narrowed_to_exact() {
    let mut title = FieldIr::new(
        field("title"),
        DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
        None,
        FieldCapabilities::new(true, true, false, false, false),
    );
    title.set_matching_config(Some(MatchingConfigIr {
        mode: MatchingModeIr::Word,
        options: std::collections::BTreeMap::new(),
    }));
    let definition = IndexDefinitionIr::new(None, vec![title], Vec::new()).unwrap();
    let error = validate_index_definition(&definition).expect_err("word matching rejected");
    assert_eq!(error.code, searchkernel_ir::ErrorCode::Unsupported);
    assert_eq!(error.details[0].code, "matching_mode_not_supported");
}

#[test]
fn unsupported_schema_capabilities_are_rejected_before_m_configuration() {
    for (kind, capabilities, expected_path) in [
        (
            FieldKind::Text,
            FieldCapabilities::new(true, false, false, false, false),
            "fields[0].searchable",
        ),
        (
            FieldKind::Float64,
            FieldCapabilities::new(true, false, true, true, true),
            "fields[0].facetable",
        ),
    ] {
        let definition = IndexDefinitionIr::new(
            None,
            vec![FieldIr::new(
                field("value"),
                match kind {
                    FieldKind::Text | FieldKind::Keyword => {
                        DataTypeIr::Primitive(PrimitiveDataTypeIr::String)
                    }
                    FieldKind::Float64 => DataTypeIr::Primitive(PrimitiveDataTypeIr::Float64),
                    FieldKind::Int64 => DataTypeIr::Primitive(PrimitiveDataTypeIr::Int64),
                    FieldKind::Bool => DataTypeIr::Primitive(PrimitiveDataTypeIr::Bool),
                },
                None,
                capabilities,
            )],
            Vec::new(),
        )
        .unwrap();
        let error = validate_index_definition(&definition).unwrap_err();
        assert_eq!(error.code, searchkernel_ir::ErrorCode::Unsupported);
        assert_eq!(error.details[0].code, "unsupported_field_capability");
        assert_eq!(error.details[0].path.as_deref(), Some(expected_path));
    }

    let definition = IndexDefinitionIr::new(
        None,
        vec![FieldIr::new(
            field("active"),
            DataTypeIr::Primitive(PrimitiveDataTypeIr::Bool),
            None,
            FieldCapabilities::new(true, false, true, true, true),
        )],
        Vec::new(),
    )
    .unwrap();
    validate_index_definition(&definition)
        .expect("single-valued boolean capabilities are supported");
}

#[test]
fn field_value_lowering_covers_scalars_arrays_and_type_errors() {
    let scalars = [
        (
            ScalarValueIr::Bool(true),
            FieldValue::Scalar(ScalarValue::Boolean(true)),
        ),
        (
            ScalarValueIr::Int64(-7),
            FieldValue::Scalar(ScalarValue::Integer(-7)),
        ),
        (
            ScalarValueIr::Float64(2.5),
            FieldValue::Scalar(ScalarValue::Float(2.5)),
        ),
        (
            ScalarValueIr::String("title".into()),
            FieldValue::Scalar(ScalarValue::String("title".into())),
        ),
    ];
    for (input, expected) in scalars {
        assert_eq!(
            lower_field_value(&FieldValueIr::Single(input)),
            Ok(expected)
        );
    }
    let arrays = [
        (
            vec![
                ScalarValueIr::String("a".into()),
                ScalarValueIr::String("b".into()),
            ],
            FieldValue::Array(searchkernel_vespa_engine::ArrayValue::Strings(vec![
                "a".into(),
                "b".into(),
            ])),
        ),
        (
            vec![ScalarValueIr::Int64(1), ScalarValueIr::Int64(2)],
            FieldValue::Array(searchkernel_vespa_engine::ArrayValue::Integers(vec![1, 2])),
        ),
        (
            vec![ScalarValueIr::Float64(1.0), ScalarValueIr::Float64(2.0)],
            FieldValue::Array(searchkernel_vespa_engine::ArrayValue::Floats(vec![
                1.0, 2.0,
            ])),
        ),
        (
            vec![ScalarValueIr::Bool(true), ScalarValueIr::Bool(false)],
            FieldValue::Array(searchkernel_vespa_engine::ArrayValue::Booleans(vec![
                true, false,
            ])),
        ),
    ];
    for (input, expected) in arrays {
        assert_eq!(lower_field_value(&FieldValueIr::Multi(input)), Ok(expected));
    }
    assert_eq!(
        lower_field_value(&FieldValueIr::Multi(Vec::new())),
        Ok(FieldValue::Array(
            searchkernel_vespa_engine::ArrayValue::Strings(Vec::new())
        ))
    );
    let error = lower_field_value(&FieldValueIr::Multi(vec![
        ScalarValueIr::String("a".into()),
        ScalarValueIr::Int64(1),
    ]))
    .unwrap_err();
    assert_eq!(error.code, searchkernel_ir::ErrorCode::Unsupported);
}

#[test]
fn query_and_filter_lowering_covers_supported_variants() {
    let title = field("title");
    let price = field("price");
    assert_eq!(lower_query(&QueryIr::MatchAll, &[]).unwrap(), Query::True);
    assert_eq!(
        lower_query(
            &QueryIr::Text("fox".into()),
            &[SearchFieldIr::new(
                title.clone(),
                SearchFieldWeight::new(1.0).unwrap(),
            )]
        )
        .unwrap(),
        Query::Text {
            field: "title".into(),
            text: "fox".into(),
            target_num_hits: 100,
        }
    );
    assert_eq!(
        lower_query(
            &QueryIr::Text("fox hound".into()),
            &[
                SearchFieldIr::new(title.clone(), SearchFieldWeight::new(1.0).unwrap()),
                SearchFieldIr::new(field("body"), SearchFieldWeight::new(1.0).unwrap()),
            ],
        )
        .unwrap(),
        Query::Or(vec![
            Query::Text {
                field: "title".into(),
                text: "fox hound".into(),
                target_num_hits: 100,
            },
            Query::Text {
                field: "body".into(),
                text: "fox hound".into(),
                target_num_hits: 100,
            },
        ])
    );
    assert!(lower_query(&QueryIr::Text("fox".into()), &[]).is_err());

    let eq_int = FilterIr::Comparison(ComparisonIr::new(
        price.clone(),
        ComparisonOp::Eq,
        ScalarValueIr::Int64(10),
    ));
    let ne_string = FilterIr::Comparison(ComparisonIr::new(
        title.clone(),
        ComparisonOp::Ne,
        ScalarValueIr::String("fox".into()),
    ));
    assert_eq!(
        lower_filter(&FilterIr::Exists {
            field: title.clone()
        })
        .unwrap(),
        Query::AttributeRange {
            field: "title".into(),
            lower: searchkernel_vespa_engine::AttributeBound::Unbounded,
            upper: searchkernel_vespa_engine::AttributeBound::Unbounded,
        }
    );
    assert_eq!(
        lower_filter(&eq_int).unwrap(),
        Query::AttributeEquals {
            field: "price".into(),
            value: VespaValue::Integer(10),
        }
    );
    assert_eq!(
        lower_filter(&ne_string).unwrap(),
        Query::Not(Box::new(Query::AttributeEquals {
            field: "title".into(),
            value: VespaValue::String("fox".into()),
        }))
    );
    assert_eq!(
        lower_filter(&FilterIr::And(vec![
            eq_int.clone(),
            FilterIr::Not(Box::new(ne_string.clone()))
        ]))
        .unwrap(),
        Query::And(vec![
            Query::AttributeEquals {
                field: "price".into(),
                value: VespaValue::Integer(10),
            },
            Query::Not(Box::new(Query::Not(Box::new(Query::AttributeEquals {
                field: "title".into(),
                value: VespaValue::String("fox".into()),
            })))),
        ])
    );
    assert_eq!(
        lower_filter(&FilterIr::Range(RangeIr::new(
            price.clone(),
            Some(ScalarValueIr::Int64(1)),
            Some(ScalarValueIr::Int64(5)),
            true,
            false,
        )))
        .unwrap(),
        Query::AttributeRange {
            field: "price".into(),
            lower: searchkernel_vespa_engine::AttributeBound::Included(VespaValue::Integer(1),),
            upper: searchkernel_vespa_engine::AttributeBound::Excluded(VespaValue::Integer(5),),
        }
    );
    assert_eq!(
        lower_filter(&FilterIr::Comparison(ComparisonIr::new(
            price.clone(),
            ComparisonOp::Gt,
            ScalarValueIr::Float64(1.0),
        )))
        .unwrap(),
        Query::AttributeRange {
            field: "price".into(),
            lower: searchkernel_vespa_engine::AttributeBound::Excluded(VespaValue::Float(1.0),),
            upper: searchkernel_vespa_engine::AttributeBound::Unbounded,
        }
    );
    assert_eq!(
        lower_filter(&FilterIr::Comparison(ComparisonIr::new(
            title.clone(),
            ComparisonOp::Eq,
            ScalarValueIr::Bool(true),
        )))
        .unwrap(),
        Query::AttributeEquals {
            field: "title".into(),
            value: VespaValue::Boolean(true),
        }
    );
    assert!(lower_filter(&FilterIr::Comparison(ComparisonIr::new(
        price,
        ComparisonOp::Gt,
        ScalarValueIr::Float64(f64::NAN),
    )))
    .is_err());
}

#[test]
fn value_conversion_preserves_all_native_value_shapes() {
    use searchkernel_vespa_engine::ArrayValue;

    let values = [
        (
            FieldValue::Scalar(ScalarValue::String("a".into())),
            Value::String("a".into()),
        ),
        (FieldValue::Scalar(ScalarValue::Integer(1)), Value::Int64(1)),
        (
            FieldValue::Scalar(ScalarValue::Float(1.5)),
            Value::Float64(1.5),
        ),
        (
            FieldValue::Scalar(ScalarValue::Boolean(true)),
            Value::Bool(true),
        ),
        (
            FieldValue::Array(ArrayValue::Strings(vec!["a".into()])),
            Value::List(vec![Value::String("a".into())]),
        ),
        (
            FieldValue::Array(ArrayValue::Integers(vec![1])),
            Value::List(vec![Value::Int64(1)]),
        ),
        (
            FieldValue::Array(ArrayValue::Floats(vec![1.5])),
            Value::List(vec![Value::Float64(1.5)]),
        ),
        (
            FieldValue::Array(ArrayValue::Booleans(vec![true])),
            Value::List(vec![Value::Bool(true)]),
        ),
        (
            FieldValue::Tensor(searchkernel_vespa_engine::TensorValue::dense(
                "tensor<float>(x[2])",
                [1.0, 2.0],
            )),
            Value::Tensor(RequestTensorIr {
                type_spec: "tensor<float>(x[2])".into(),
                payload: RequestTensorPayloadIr::Dense(vec![1.0, 2.0]),
            }),
        ),
    ];
    for (native, expected) in values {
        assert_eq!(value_to_ir(&native), expected);
    }
}

#[test]
fn analyze_language_is_explicitly_supported_or_rejected() {
    let english = LanguageHint::parse("en-US").unwrap();
    assert!(analysis_options(Some(&english)).is_ok());

    let french = LanguageHint::parse("fr").unwrap();
    let error = analysis_options(Some(&french)).unwrap_err();
    assert_eq!(error.code, searchkernel_ir::ErrorCode::Unsupported);
    assert_eq!(error.details[0].code, "language_not_supported");
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

#[test]
fn highlighting_rejects_multi_valued_fields_at_the_vespa_boundary() {
    let tags = field("tags");
    let request = SearchRequestIr::new(
        QueryIr::Text("brown".to_owned()),
        vec![SearchFieldIr::new(
            tags.clone(),
            SearchFieldWeight::new(1.0).unwrap(),
        )],
        None,
        Vec::new(),
        10,
        0,
        searchkernel_ir::ProjectionIr::new(Vec::new()),
        HighlightingIr::Requested {
            fields: vec![tags.clone()],
            multi_valued_fields: vec![tags],
        },
        None,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    );
    let error = validate_search_request(&request)
        .expect_err("the current Vespa dynamic-teaser path is scalar-only");
    assert_eq!(error.code, ErrorCode::Unsupported);
    assert_eq!(
        error.details[0].code,
        "highlighting_multi_valued_field_not_supported"
    );
}

#[test]
fn ranking_and_ordered_comparisons_preserve_supported_boundaries() {
    assert_eq!(
        ranking_profile(RankingPolicyIr::DefaultRelevanceV1),
        Ok("default")
    );
    assert_eq!(ranking_profile(RankingPolicyIr::Bm25), Ok("bm25"));

    let price = field("price");
    for (operation, expected) in [
        (
            ComparisonOp::Lt,
            Query::AttributeRange {
                field: "price".into(),
                lower: searchkernel_vespa_engine::AttributeBound::Unbounded,
                upper: searchkernel_vespa_engine::AttributeBound::Excluded(VespaValue::Integer(10)),
            },
        ),
        (
            ComparisonOp::Lte,
            Query::AttributeRange {
                field: "price".into(),
                lower: searchkernel_vespa_engine::AttributeBound::Unbounded,
                upper: searchkernel_vespa_engine::AttributeBound::Included(VespaValue::Integer(10)),
            },
        ),
        (
            ComparisonOp::Gt,
            Query::AttributeRange {
                field: "price".into(),
                lower: searchkernel_vespa_engine::AttributeBound::Excluded(VespaValue::Integer(10)),
                upper: searchkernel_vespa_engine::AttributeBound::Unbounded,
            },
        ),
        (
            ComparisonOp::Gte,
            Query::AttributeRange {
                field: "price".into(),
                lower: searchkernel_vespa_engine::AttributeBound::Included(VespaValue::Integer(10)),
                upper: searchkernel_vespa_engine::AttributeBound::Unbounded,
            },
        ),
    ] {
        assert_eq!(
            lower_comparison(&ComparisonIr::new(
                price.clone(),
                operation,
                ScalarValueIr::Int64(10),
            ))
            .unwrap(),
            expected
        );
    }
}

#[test]
fn ranking_inputs_lower_to_the_existing_native_request_surface() {
    let request = request(
        vec![SearchFieldIr::new(
            field("title"),
            SearchFieldWeight::new(1.0).unwrap(),
        )],
        Vec::new(),
        None,
    )
    .with_ranking_request(
        "product-search",
        std::collections::BTreeMap::from([("source".to_owned(), "catalog".to_owned())]),
    )
    .with_inputs(std::collections::BTreeMap::from([
        (
            "freshness".to_owned(),
            QueryInputIr::Literal(RequestValueIr::Float64(0.8)),
        ),
        (
            "integer_feature".to_owned(),
            QueryInputIr::Literal(RequestValueIr::Int64(8)),
        ),
    ]));
    validate_search_request(&request).expect("supported ranking inputs validate");
    let query =
        lower_search_query(&request, analysis_options(request.language()).unwrap()).unwrap();
    let lowered = lower_search_options(
        &request,
        query,
        analysis_options(request.language()).unwrap(),
    )
    .unwrap();
    assert_eq!(lowered.rank_profile, "product-search");
    assert_eq!(
        lowered.query_features,
        vec![
            ("freshness".to_owned(), 0.8),
            ("integer_feature".to_owned(), 8.0)
        ]
    );
    assert_eq!(
        lowered.rank_properties,
        vec![("source".to_owned(), "catalog".to_owned())]
    );
}

#[test]
fn typed_query_c_nearest_neighbor_reaches_real_vespa_through_the_adapter() {
    let root = std::env::temp_dir().join(format!(
        "searchkernel-adapter-typed-query-c-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let data = root.join("data");
    let config = root.join("config");
    let schema = Schema::new("typed_query_c")
        .field(
            Field::new(
                "embedding",
                FieldType::Tensor("tensor<float>(x[2])".to_owned()),
            )
            .attribute(),
        )
        .field(
            Field::new("category", FieldType::String)
                .attribute()
                .summary(),
        )
        .field(Field::new("qty", FieldType::Integer).attribute().summary())
        .rank_profile(
            searchkernel_vespa_engine::RankProfile::new("semantic")
                .query_input("query_embedding", "tensor<float>(x[2])")
                .first_phase("closeness(embedding)"),
        );
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile nearest-neighbor schema");
    let backend = VespaBackend::open(&data, &config).expect("open nearest-neighbor backend");

    for (id, values, category, qty) in [
        ("doc:near", vec![1.0, 0.0], "near", 1),
        ("doc:far", vec![0.0, 1.0], "far", 2),
    ] {
        backend
            .insert(InsertRequestIr::new(DocumentIr::new(
                DocumentId::parse(id).unwrap(),
                std::collections::BTreeMap::from([
                    (
                        field("embedding"),
                        FieldValueIr::Tensor(RequestTensorIr {
                            type_spec: "tensor<float>(x[2])".to_owned(),
                            payload: RequestTensorPayloadIr::Dense(values),
                        }),
                    ),
                    (
                        field("category"),
                        FieldValueIr::Single(ScalarValueIr::String(category.to_owned())),
                    ),
                    (
                        field("qty"),
                        FieldValueIr::Single(ScalarValueIr::Int64(qty)),
                    ),
                ]),
            )))
            .expect("insert tensor document");
    }

    backend
        .engine
        .close()
        .expect("close tensor plus INT32 backend before reopen");
    let backend = VespaBackend::open(&data, &config)
        .expect("reopen tensor plus INT32 backend without legacy classification");

    let request = SearchRequestIr::new(
        typed_expression(QueryExprKindIr::NearestNeighbor {
            document_field: typed_field("embedding"),
            query_tensor: "query_embedding".to_owned(),
        }),
        Vec::new(),
        None,
        Vec::new(),
        2,
        0,
        searchkernel_ir::ProjectionIr::new(Vec::new()),
        HighlightingIr::Disabled,
        None,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    )
    .with_ranking_request("semantic", std::collections::BTreeMap::new())
    .with_inputs(std::collections::BTreeMap::from([(
        "query_embedding".to_owned(),
        QueryInputIr::Literal(RequestValueIr::Tensor(RequestTensorIr {
            type_spec: "tensor<float>(x[2])".to_owned(),
            payload: RequestTensorPayloadIr::Dense(vec![0.9, 0.1]),
        })),
    )]))
    .with_grouping(vec![native_grouping(&[(
        "category",
        vec![count_expression(), average_expression("qty")],
    )])]);
    let result = backend
        .search(request)
        .expect("execute nearest-neighbor query");
    let ResultNode::Group(group) = result.root else {
        panic!("search root must be a group");
    };
    let ids = group
        .children
        .iter()
        .filter_map(|child| match child {
            ResultNode::Hit(hit) => Some(hit.id.as_str()),
            ResultNode::Group(_) => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["doc:near", "doc:far"]);
    assert!(matches!(
        group.children.as_slice(),
        [ResultNode::Hit(first), ResultNode::Hit(second), ResultNode::Group(_)]
            if first.score > second.score
    ));
    let ResultNode::Group(grouping) = group.children.last().unwrap() else {
        unreachable!("nearest-neighbor grouping is the final result child");
    };
    assert_eq!(grouping.children.len(), 2);

    backend
        .engine
        .close()
        .expect("close nearest-neighbor backend");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn array_struct_same_element_requires_two_terms_on_one_element() {
    let root = std::env::temp_dir().join(format!(
        "searchkernel-array-struct-same-element-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let data = root.join("data");
    let config = root.join("config");
    let schema = Schema::new("array_struct_same_element")
        .field(Field::new("tags", FieldType::ArrayString).indexed())
        .field(
            Field::new(
                "items",
                FieldType::ArrayStruct(vec![
                    StructField {
                        name: "name".to_owned(),
                        field_type: StructFieldType::String,
                    },
                    StructField {
                        name: "color".to_owned(),
                        field_type: StructFieldType::String,
                    },
                ]),
            )
            .attribute(),
        );
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile array-struct schema");
    let engine = Engine::open(&data, &config).expect("open array-struct engine");
    let make_items = |first_color: &str, second_color: &str| {
        ArrayValue::Structs(vec![
            std::collections::BTreeMap::from([
                ("name".to_owned(), "alice".into()),
                ("color".to_owned(), first_color.into()),
            ]),
            std::collections::BTreeMap::from([
                ("name".to_owned(), "bob".into()),
                ("color".to_owned(), second_color.into()),
            ]),
        ])
    };
    engine
        .put(
            Document::new("doc:match")
                .field(
                    "tags",
                    FieldValue::Array(ArrayValue::Strings(vec![
                        "wireless headphones".into(),
                        "speaker".into(),
                    ])),
                )
                .field("items", FieldValue::Array(make_items("red", "blue"))),
        )
        .expect("put matching array-struct document");
    engine
        .put(
            Document::new("doc:cross")
                .field(
                    "tags",
                    FieldValue::Array(ArrayValue::Strings(vec![
                        "wireless".into(),
                        "headphones".into(),
                    ])),
                )
                .field("items", FieldValue::Array(make_items("blue", "red"))),
        )
        .expect("put cross-element array-struct document");
    let query = Query::SameElement {
        field: "tags".to_owned(),
        children: vec![
            Query::term("tags", "wireless"),
            Query::term("tags", "headphones"),
        ],
        element_filter: Vec::new(),
    };
    std::thread::sleep(std::time::Duration::from_millis(200));
    let result = engine
        .search(searchkernel_vespa_engine::SearchRequest::new(query).limit(10))
        .expect("same-element query");
    let ids = result
        .hits
        .into_iter()
        .map(|hit| hit.document_id)
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["doc:match"]);
    engine.close().expect("close array-struct engine");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn array_string_patch_and_update_reanalyze_each_element() {
    let root = std::env::temp_dir().join(format!(
        "searchkernel-array-update-analysis-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let data = root.join("data");
    let config = root.join("config");
    let schema = Schema::new("array_update_analysis")
        .field(Field::new("tags", FieldType::ArrayString).indexed());
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile array update schema");
    let backend = VespaBackend::open(&data, &config).expect("open array update backend");
    let tags = |values: &[&str]| {
        FieldValueIr::Multi(
            values
                .iter()
                .map(|value| ScalarValueIr::String((*value).to_owned()))
                .collect(),
        )
    };
    backend
        .insert(InsertRequestIr::new(DocumentIr::new(
            DocumentId::parse("doc:array-update").unwrap(),
            std::collections::BTreeMap::from([(field("tags"), tags(&["quick fox"]))]),
        )))
        .expect("insert array document");

    let request = |term| {
        SearchRequestIr::new(
            typed_expression(QueryExprKindIr::Contains {
                field: typed_field("tags"),
                item: Box::new(typed_term(term)),
            }),
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
    };
    let ids = |result: SearchResult| {
        let ResultNode::Group(group) = result.root else {
            panic!("search root must be a group");
        };
        group
            .children
            .into_iter()
            .filter_map(|child| match child {
                ResultNode::Hit(hit) => Some(hit.id),
                ResultNode::Group(_) => None,
            })
            .collect::<Vec<_>>()
    };

    backend
        .patch(PatchRequestIr::new(
            DocumentId::parse("doc:array-update").unwrap(),
            vec![PatchOperationIr::Set {
                field: field("tags"),
                value: tags(&["slow hare"]),
            }],
        ))
        .expect("patch array document");
    std::thread::sleep(std::time::Duration::from_secs(2));
    assert_eq!(
        ids(backend
            .search(request("hare"))
            .expect("search patched term")),
        ["doc:array-update"]
    );
    assert!(ids(backend
        .search(request("quick"))
        .expect("search removed patched term"))
    .is_empty());

    backend
        .update(UpdateRequestIr::new(
            DocumentId::parse("doc:array-update").unwrap(),
            vec![UpdateOperationIr::Assign {
                target: UpdateTargetIr::parse("tags").unwrap(),
                value: tags(&["bright owl"]),
            }],
        ))
        .expect("update array document");
    std::thread::sleep(std::time::Duration::from_secs(2));
    assert_eq!(
        ids(backend.search(request("owl")).expect("search updated term")),
        ["doc:array-update"]
    );
    assert!(ids(backend
        .search(request("hare"))
        .expect("search removed updated term"))
    .is_empty());

    backend.engine.close().expect("close array update backend");
    let reopened = VespaBackend::open(&data, &config).expect("reopen array update backend");
    std::thread::sleep(std::time::Duration::from_secs(2));
    assert_eq!(
        ids(reopened
            .search(request("owl"))
            .expect("search persisted updated term")),
        ["doc:array-update"]
    );
    assert!(ids(reopened
        .search(request("quick"))
        .expect("search persisted removed term"))
    .is_empty());
    reopened
        .engine
        .close()
        .expect("close reopened array update backend");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn typed_query_c_same_element_reaches_real_vespa_for_arrays_and_relative_members() {
    let root = std::env::temp_dir().join(format!(
        "searchkernel-adapter-same-element-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let data = root.join("data");
    let config = root.join("config");
    let schema = Schema::new("typed_same_element")
        .field(
            Field::new("tags", FieldType::ArrayString)
                .indexed()
                .attribute()
                .summary(),
        )
        .field(
            Field::new("name", FieldType::Text)
                .indexed()
                .attribute()
                .summary(),
        )
        .field(
            Field::new(
                "items",
                FieldType::ArrayStruct(vec![
                    StructField {
                        name: "name".to_owned(),
                        field_type: StructFieldType::String,
                    },
                    StructField {
                        name: "color".to_owned(),
                        field_type: StructFieldType::String,
                    },
                ]),
            )
            .attribute(),
        );
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile same-element schema");
    let backend = VespaBackend::open(&data, &config).expect("open same-element backend");

    // Build the same relative-member query through the canonical portable
    // FlatBuffers -> *_to_ir() path. The direct-IR test below cannot detect a
    // field resolver that eagerly looks up `name` and `color` as top-level
    // fields before the `items` context is available.
    let portable_query = {
        let relative = |name: &str, value: &str| QueryExprIr {
            kind: QueryExprKindIr::Contains {
                field: typed_field(name),
                item: Box::new(typed_term(value)),
            },
            annotations: std::collections::BTreeMap::new(),
        };
        let query = QueryIr::Typed(Box::new(QueryExprIr {
            kind: QueryExprKindIr::Contains {
                field: typed_field("items"),
                item: Box::new(QueryExprIr {
                    kind: QueryExprKindIr::SameElement(vec![
                        relative("name", "bob"),
                        relative("color", "blue"),
                    ]),
                    annotations: std::collections::BTreeMap::new(),
                }),
            },
            annotations: std::collections::BTreeMap::new(),
        }));
        let definition = IndexDefinitionIr::new(
            None,
            vec![
                FieldIr::new(
                    field("name"),
                    DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
                    None,
                    FieldCapabilities::new(true, true, true, false, false),
                ),
                FieldIr::new(
                    field("items"),
                    DataTypeIr::Array(Box::new(DataTypeIr::Struct {
                        fields: vec![
                            StructFieldIr {
                                name: "name".to_owned(),
                                data_type: DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
                            },
                            StructFieldIr {
                                name: "color".to_owned(),
                                data_type: DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
                            },
                        ],
                    })),
                    None,
                    FieldCapabilities::new(true, true, true, false, false),
                ),
            ],
            Vec::new(),
        )
        .expect("portable same-element definition");
        let request = SearchRequestIr::new(
            query,
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
        );
        let bytes = searchkernel_ir::search_request_ir_to_api(&request);
        let api_request =
            searchkernel_ir::__flatbuffers::root::<searchkernel_ir::api::SearchRequest>(&bytes)
                .expect("portable same-element request");
        IndexContext::from_definition_ir(definition)
            .search_to_ir(api_request)
            .expect("relative portable fields resolve after parent context")
            .query()
            .clone()
    };

    let document = |id: &str, tags: Vec<&str>, items: [(&str, &str); 2]| {
        DocumentIr::new(
            DocumentId::parse(id).unwrap(),
            std::collections::BTreeMap::from([
                (
                    field("tags"),
                    FieldValueIr::Multi(
                        tags.into_iter()
                            .map(|value| ScalarValueIr::String(value.to_owned()))
                            .collect(),
                    ),
                ),
                (
                    field("items"),
                    FieldValueIr::Structured(
                        items
                            .into_iter()
                            .map(|(name, color)| {
                                std::collections::BTreeMap::from([
                                    ("name".to_owned(), ScalarValueIr::String(name.to_owned())),
                                    ("color".to_owned(), ScalarValueIr::String(color.to_owned())),
                                ])
                            })
                            .collect(),
                    ),
                ),
            ]),
        )
    };
    backend
        .insert(InsertRequestIr::new(document(
            "doc:same",
            vec!["wireless headphones", "speaker"],
            [("alice", "red"), ("bob", "blue")],
        )))
        .expect("insert same-element document");
    backend
        .insert(InsertRequestIr::new(document(
            "doc:split",
            vec!["wireless", "headphones"],
            [("alice", "blue"), ("bob", "red")],
        )))
        .expect("insert split-element document");

    let request = |query| {
        SearchRequestIr::new(
            query,
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
    };
    let ids = |result: SearchResult| {
        let ResultNode::Group(group) = result.root else {
            panic!("search root must be a group");
        };
        group
            .children
            .into_iter()
            .filter_map(|child| match child {
                ResultNode::Hit(hit) => Some(hit.id),
                ResultNode::Group(_) => None,
            })
            .collect::<Vec<_>>()
    };

    let tags_query = typed_contains(
        "tags",
        QueryExprKindIr::SameElement(vec![typed_term("wireless"), typed_term("headphones")]),
    );
    std::thread::sleep(std::time::Duration::from_secs(2));
    assert_eq!(
        ids(backend
            .search(request(tags_query))
            .expect("array<string> sameElement")),
        vec!["doc:same"]
    );
    assert_eq!(
        ids(backend
            .search(request(portable_query))
            .expect("portable relative-member sameElement")),
        vec!["doc:same"]
    );
    let member_pattern_query = |pattern: &str| {
        typed_contains(
            "items",
            QueryExprKindIr::SameElement(vec![
                QueryExprIr {
                    kind: QueryExprKindIr::Matches {
                        field: typed_field("name"),
                        pattern: pattern.to_owned(),
                    },
                    annotations: std::collections::BTreeMap::new(),
                },
                QueryExprIr {
                    kind: QueryExprKindIr::Contains {
                        field: typed_field("color"),
                        item: Box::new(typed_term("blue")),
                    },
                    annotations: std::collections::BTreeMap::new(),
                },
            ]),
        )
    };
    for pattern in ["bo.*", ".*bo.*", ".*ob"] {
        assert_eq!(
            ids(backend
                .search(request(member_pattern_query(pattern)))
                .expect("relative Matches sameElement")),
            vec!["doc:same"]
        );
    }

    let member_query = |element_filter: Vec<u64>| {
        let mut annotations = std::collections::BTreeMap::new();
        annotations.insert(
            "elementFilter".to_owned(),
            searchkernel_ir::RequestValueIr::List(
                element_filter
                    .into_iter()
                    .map(searchkernel_ir::RequestValueIr::UInt64)
                    .collect(),
            ),
        );
        typed_expression(QueryExprKindIr::Contains {
            field: typed_field("items"),
            item: Box::new(QueryExprIr {
                kind: QueryExprKindIr::SameElement(vec![
                    QueryExprIr {
                        kind: QueryExprKindIr::Contains {
                            field: typed_field("name"),
                            item: Box::new(typed_term("bob")),
                        },
                        annotations: std::collections::BTreeMap::new(),
                    },
                    QueryExprIr {
                        kind: QueryExprKindIr::Contains {
                            field: typed_field("color"),
                            item: Box::new(typed_term("blue")),
                        },
                        annotations: std::collections::BTreeMap::new(),
                    },
                ]),
                annotations,
            }),
        })
    };
    assert_eq!(
        ids(backend
            .search(request(member_query(vec![1])))
            .expect("array<struct> relative sameElement")),
        vec!["doc:same"]
    );
    assert!(ids(backend
        .search(request(member_query(vec![0])))
        .expect("array<struct> elementFilter"))
    .is_empty());

    backend.engine.close().expect("close same-element backend");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn typed_query_c_geo_reaches_real_vespa_for_location_and_bounding_box() {
    let root = std::env::temp_dir().join(format!(
        "searchkernel-adapter-geo-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let data = root.join("data");
    let config = root.join("config");
    let schema =
        Schema::new("typed_geo").field(Field::new("location", FieldType::Position).attribute());
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile geo schema");
    let backend = VespaBackend::open(&data, &config).expect("open geo backend");
    for (id, latitude, longitude) in [
        ("doc:near", 60.0, 10.0),
        ("doc:far", 60.2, 10.0),
        ("doc:boundary", 61.0, 11.0),
        ("doc:outside", 62.0, 12.0),
    ] {
        backend
            .insert(InsertRequestIr::new(DocumentIr::new(
                DocumentId::parse(id).unwrap(),
                std::collections::BTreeMap::from([(
                    field("location"),
                    FieldValueIr::Position {
                        latitude,
                        longitude,
                    },
                )]),
            )))
            .expect("insert position document");
    }
    let request = |kind| {
        SearchRequestIr::new(
            typed_expression(kind),
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
    };
    let ids = |result: SearchResult| {
        let ResultNode::Group(group) = result.root else {
            panic!("search root must be a group");
        };
        group
            .children
            .into_iter()
            .filter_map(|child| match child {
                ResultNode::Hit(hit) => Some(hit.id),
                ResultNode::Group(_) => None,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        ids(backend
            .search(request(QueryExprKindIr::GeoLocation {
                field: typed_field("location"),
                latitude: 60.0,
                longitude: 10.0,
                radius: "15km".to_owned(),
            }))
            .expect("geo location query")),
        vec!["doc:near"]
    );
    assert_eq!(
        ids(backend
            .search(request(QueryExprKindIr::GeoBoundingBox {
                field: typed_field("location"),
                south: 59.0,
                west: 9.0,
                north: 61.0,
                east: 11.0,
            }))
            .expect("geo bounding-box query")),
        vec!["doc:near", "doc:far", "doc:boundary"]
    );

    backend
        .update(UpdateRequestIr::new(
            DocumentId::parse("doc:near").unwrap(),
            vec![UpdateOperationIr::Assign {
                target: UpdateTargetIr::parse("location").unwrap(),
                value: FieldValueIr::Position {
                    latitude: 70.0,
                    longitude: 20.0,
                },
            }],
        ))
        .expect("update position document");
    std::thread::sleep(std::time::Duration::from_secs(2));
    assert!(ids(backend
        .search(request(QueryExprKindIr::GeoLocation {
            field: typed_field("location"),
            latitude: 60.0,
            longitude: 10.0,
            radius: "15km".to_owned(),
        }))
        .expect("search old position after update"))
    .is_empty());
    assert_eq!(
        ids(backend
            .search(request(QueryExprKindIr::GeoLocation {
                field: typed_field("location"),
                latitude: 70.0,
                longitude: 20.0,
                radius: "15km".to_owned(),
            }))
            .expect("search new position after update")),
        vec!["doc:near"]
    );

    backend.engine.close().expect("close geo backend");
    let reopened = VespaBackend::open(&data, &config).expect("reopen geo backend");
    assert!(ids(reopened
        .search(request(QueryExprKindIr::GeoLocation {
            field: typed_field("location"),
            latitude: 60.0,
            longitude: 10.0,
            radius: "15km".to_owned(),
        }))
        .expect("search persisted old position"))
    .is_empty());
    assert_eq!(
        ids(reopened
            .search(request(QueryExprKindIr::GeoLocation {
                field: typed_field("location"),
                latitude: 70.0,
                longitude: 20.0,
                radius: "15km".to_owned(),
            }))
            .expect("search persisted new position")),
        vec!["doc:near"]
    );
    reopened
        .update(UpdateRequestIr::new(
            DocumentId::parse("doc:near").unwrap(),
            vec![UpdateOperationIr::Clear {
                target: UpdateTargetIr::parse("location").unwrap(),
            }],
        ))
        .expect("clear position document");
    std::thread::sleep(std::time::Duration::from_secs(2));
    assert!(ids(reopened
        .search(request(QueryExprKindIr::GeoLocation {
            field: typed_field("location"),
            latitude: 70.0,
            longitude: 20.0,
            radius: "15km".to_owned(),
        }))
        .expect("search cleared position"))
    .is_empty());
    reopened.engine.close().expect("close reopened geo backend");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn typed_query_c_special_fields_reach_real_vespa_through_the_adapter() {
    let root = std::env::temp_dir().join(format!(
        "searchkernel-adapter-typed-query-c-special-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let data = root.join("data");
    let config = root.join("config");
    let schema = Schema::new("typed_query_c_special").field(
        Field::new("access", FieldType::Predicate)
            .attribute()
            .predicate_arity(8),
    );
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile specialized-query schema");
    let backend = VespaBackend::open(&data, &config).expect("open specialized-query backend");

    backend
        .insert(InsertRequestIr::new(DocumentIr::new(
            DocumentId::parse("doc:legacy-feature").unwrap(),
            std::collections::BTreeMap::from([(
                field("access"),
                FieldValueIr::Predicate {
                    features: std::collections::BTreeMap::from([(
                        "hobby".to_owned(),
                        "music".to_owned(),
                    )]),
                },
            )]),
        )))
        .expect("insert legacy feature predicate document");
    backend
        .insert(InsertRequestIr::new(DocumentIr::new(
            DocumentId::parse("doc:composed").unwrap(),
            std::collections::BTreeMap::from([(
                field("access"),
                FieldValueIr::PredicateExpression {
                    expression: r#"{"type":1,"children":[{"type":4,"key":"hobby","feature_set":["music"]},{"type":5,"key":"age","range_min":18,"range_max":30}]}"#.to_owned(),
                },
            )]),
        )))
        .expect("insert composed predicate expression document");
    backend
        .insert(InsertRequestIr::new(DocumentIr::new(
            DocumentId::parse("doc:range").unwrap(),
            std::collections::BTreeMap::from([(
                field("access"),
                FieldValueIr::PredicateExpression {
                    expression: r#"{"type":5,"key":"age","range_min":18,"range_max":30}"#
                        .to_owned(),
                },
            )]),
        )))
        .expect("insert range predicate expression document");
    backend
        .insert(InsertRequestIr::new(DocumentIr::new(
            DocumentId::parse("doc:or").unwrap(),
            std::collections::BTreeMap::from([(
                field("access"),
                FieldValueIr::PredicateExpression {
                    expression: r#"{"type":2,"children":[{"type":4,"key":"hobby","feature_set":["music"]},{"type":4,"key":"hobby","feature_set":["sports"]}]}"#.to_owned(),
                },
            )]),
        )))
        .expect("insert disjunction predicate expression document");
    backend
        .insert(InsertRequestIr::new(DocumentIr::new(
            DocumentId::parse("doc:not").unwrap(),
            std::collections::BTreeMap::from([(
                field("access"),
                FieldValueIr::PredicateExpression {
                    expression: r#"{"type":3,"children":[{"type":5,"key":"age","range_min":18,"range_max":30}]}"#.to_owned(),
                },
            )]),
        )))
        .expect("insert negation predicate expression document");
    let search = |kind| {
        backend
            .search(SearchRequestIr::new(
                kind,
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
            ))
            .expect("execute specialized query")
    };
    let ids = |result: SearchResult| {
        let ResultNode::Group(group) = result.root else {
            panic!("search root must be a group");
        };
        group
            .children
            .iter()
            .filter_map(|child| match child {
                ResultNode::Hit(hit) => Some(hit.id.clone()),
                ResultNode::Group(_) => None,
            })
            .collect::<Vec<_>>()
    };

    let music = search(QueryIr::Typed(Box::new(QueryExprIr {
        kind: QueryExprKindIr::Predicate {
            field: typed_field("access"),
            features: std::collections::BTreeMap::from([(
                "hobby".to_owned(),
                RequestValueIr::String("music".to_owned()),
            )]),
            range_features: std::collections::BTreeMap::new(),
        },
        annotations: std::collections::BTreeMap::new(),
    })));
    let mut music_ids = ids(music);
    music_ids.sort();
    assert_eq!(music_ids, vec!["doc:legacy-feature", "doc:not", "doc:or"]);

    let sports = search(QueryIr::Typed(Box::new(QueryExprIr {
        kind: QueryExprKindIr::Predicate {
            field: typed_field("access"),
            features: std::collections::BTreeMap::from([(
                "hobby".to_owned(),
                RequestValueIr::String("sports".to_owned()),
            )]),
            range_features: std::collections::BTreeMap::new(),
        },
        annotations: std::collections::BTreeMap::new(),
    })));
    let mut sports_ids = ids(sports);
    sports_ids.sort();
    assert_eq!(sports_ids, vec!["doc:not", "doc:or"]);

    assert_eq!(
        ids(search(QueryIr::Typed(Box::new(QueryExprIr {
            kind: QueryExprKindIr::Predicate {
                field: typed_field("access"),
                features: std::collections::BTreeMap::new(),
                range_features: std::collections::BTreeMap::from([(
                    "age".to_owned(),
                    RequestValueIr::UInt64(21),
                )]),
            },
            annotations: std::collections::BTreeMap::new(),
        })))),
        vec!["doc:range"]
    );

    assert_eq!(
        ids(search(QueryIr::Typed(Box::new(QueryExprIr {
            kind: QueryExprKindIr::Predicate {
                field: typed_field("access"),
                features: std::collections::BTreeMap::from([(
                    "hobby".to_owned(),
                    RequestValueIr::String("music".to_owned()),
                )]),
                range_features: std::collections::BTreeMap::from([(
                    "age".to_owned(),
                    RequestValueIr::UInt64(21),
                )]),
            },
            annotations: std::collections::BTreeMap::new(),
        })))),
        vec!["doc:legacy-feature", "doc:composed", "doc:range", "doc:or"]
    );

    backend
        .engine
        .close()
        .expect("close specialized-query backend");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn predicate_assignment_updates_old_and_new_values_across_reopen() {
    let root = std::env::temp_dir().join(format!(
        "searchkernel-adapter-predicate-update-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let data = root.join("data");
    let config = root.join("config");
    let schema = Schema::new("predicate_update").field(
        Field::new("access", FieldType::Predicate)
            .attribute()
            .predicate_arity(8),
    );
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile predicate update schema");
    let backend = VespaBackend::open(&data, &config).expect("open predicate update backend");

    backend
        .insert(InsertRequestIr::new(DocumentIr::new(
            DocumentId::parse("doc:update-features").unwrap(),
            std::collections::BTreeMap::from([(
                field("access"),
                FieldValueIr::Predicate {
                    features: std::collections::BTreeMap::from([(
                        "hobby".to_owned(),
                        "stale".to_owned(),
                    )]),
                },
            )]),
        )))
        .expect("insert predicate feature update document");
    backend
        .insert(InsertRequestIr::new(DocumentIr::new(
            DocumentId::parse("doc:update-expression").unwrap(),
            std::collections::BTreeMap::from([(
                field("access"),
                FieldValueIr::PredicateExpression {
                    expression: r#"{"type":4,"key":"hobby","feature_set":["stale"]}"#.to_owned(),
                },
            )]),
        )))
        .expect("insert predicate expression update document");

    let predicate = |value: &str| {
        QueryIr::Typed(Box::new(QueryExprIr {
            kind: QueryExprKindIr::Predicate {
                field: typed_field("access"),
                features: std::collections::BTreeMap::from([(
                    "hobby".to_owned(),
                    RequestValueIr::String(value.to_owned()),
                )]),
                range_features: std::collections::BTreeMap::new(),
            },
            annotations: std::collections::BTreeMap::new(),
        }))
    };
    let request = |value: &str| {
        SearchRequestIr::new(
            predicate(value),
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
    };
    let ids = |result: SearchResult| {
        let ResultNode::Group(group) = result.root else {
            panic!("search root must be a group");
        };
        let mut ids = group
            .children
            .into_iter()
            .filter_map(|child| match child {
                ResultNode::Hit(hit) => Some(hit.id),
                ResultNode::Group(_) => None,
            })
            .collect::<Vec<_>>();
        ids.sort();
        ids
    };

    assert_eq!(
        ids(backend
            .search(request("stale"))
            .expect("search stale predicate")),
        vec!["doc:update-expression", "doc:update-features"]
    );
    backend
        .update(UpdateRequestIr::new(
            DocumentId::parse("doc:update-features").unwrap(),
            vec![UpdateOperationIr::Assign {
                target: UpdateTargetIr::parse("access").unwrap(),
                value: FieldValueIr::PredicateExpression {
                    expression: r#"{"type":4,"key":"hobby","feature_set":["fresh"]}"#.to_owned(),
                },
            }],
        ))
        .expect("update feature predicate with expression");
    backend
        .update(UpdateRequestIr::new(
            DocumentId::parse("doc:update-expression").unwrap(),
            vec![UpdateOperationIr::Assign {
                target: UpdateTargetIr::parse("access").unwrap(),
                value: FieldValueIr::Predicate {
                    features: std::collections::BTreeMap::from([(
                        "hobby".to_owned(),
                        "fresh".to_owned(),
                    )]),
                },
            }],
        ))
        .expect("update expression predicate with features");
    std::thread::sleep(std::time::Duration::from_secs(2));

    assert!(ids(backend
        .search(request("stale"))
        .expect("search stale predicate after update"))
    .is_empty());
    assert_eq!(
        ids(backend
            .search(request("fresh"))
            .expect("search fresh predicate after update")),
        vec!["doc:update-expression", "doc:update-features"]
    );

    backend
        .engine
        .close()
        .expect("close predicate update backend");
    let reopened = VespaBackend::open(&data, &config).expect("reopen predicate update backend");
    assert!(ids(reopened
        .search(request("stale"))
        .expect("search stale predicate after reopen"))
    .is_empty());
    assert_eq!(
        ids(reopened
            .search(request("fresh"))
            .expect("search fresh predicate after reopen")),
        vec!["doc:update-expression", "doc:update-features"]
    );
    reopened
        .engine
        .close()
        .expect("close reopened predicate update backend");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn nearest_neighbor_lowers_dense_tensor_input_to_native_request_input() {
    let request = request(Vec::new(), Vec::new(), None)
        .with_query(typed_expression(QueryExprKindIr::NearestNeighbor {
            document_field: typed_field("embedding"),
            query_tensor: "query_embedding".to_owned(),
        }))
        .with_inputs(std::collections::BTreeMap::from([(
            "query_embedding".to_owned(),
            QueryInputIr::Literal(RequestValueIr::Tensor(RequestTensorIr {
                type_spec: "tensor<float>(x[2])".to_owned(),
                payload: RequestTensorPayloadIr::Dense(vec![0.1, 0.2]),
            })),
        )]));
    validate_search_request(&request).expect("dense nearest-neighbor input validates");
    let query = lower_search_query(&request, analysis_options(request.language()).unwrap())
        .expect("nearest-neighbor query lowers");
    let lowered = lower_search_options(
        &request,
        query,
        analysis_options(request.language()).unwrap(),
    )
    .expect("nearest-neighbor request lowers");
    assert_eq!(
        lowered.query_inputs,
        vec![(
            "query_embedding".to_owned(),
            searchkernel_vespa_engine::QueryInput::Literal(
                searchkernel_vespa_engine::TensorValue::Dense {
                    type_spec: "tensor<float>(x[2])".to_owned(),
                    values: vec![0.1, 0.2],
                }
            )
        )]
    );
}

#[test]
fn nearest_neighbor_allows_zero_explore_additional_hits() {
    let query = QueryIr::Typed(Box::new(QueryExprIr {
        kind: QueryExprKindIr::NearestNeighbor {
            document_field: typed_field("embedding"),
            query_tensor: "query_embedding".to_owned(),
        },
        annotations: std::collections::BTreeMap::from([(
            "exploreAdditionalHits".to_owned(),
            RequestValueIr::UInt64(0),
        )]),
    }));
    assert_eq!(
        lower_query(&query, &[]).expect("zero exploreAdditionalHits is valid"),
        Query::NearestNeighbor {
            document_field: "embedding".to_owned(),
            query_tensor: "query_embedding".to_owned(),
            target_num_hits: 100,
            approximate: true,
        }
    );
}

#[test]
fn specialized_query_lowering_rejects_unsupported_values_explicitly() {
    let predicate = typed_expression(QueryExprKindIr::Predicate {
        field: typed_field("access"),
        features: std::collections::BTreeMap::from([(
            "hobby".to_owned(),
            RequestValueIr::Float64(1.0),
        )]),
        range_features: std::collections::BTreeMap::new(),
    });
    let error = lower_query(&predicate, &[]).unwrap_err();
    assert_eq!(
        error.details[0].code,
        "predicate_feature_value_not_supported"
    );
}

#[test]
fn integer_query_features_reject_float_saturation_at_i64_and_u64_boundaries() {
    for value in [i64::MAX, i64::MAX - 1] {
        let error = query_feature_value(
            "integer_feature",
            &QueryInputIr::Literal(RequestValueIr::Int64(value)),
        )
        .expect_err("values rounded to 2^63 must not be accepted as i64");
        assert_eq!(error.details[0].code, "query_input_precision_loss");
    }

    assert_eq!(
        query_feature_value(
            "integer_feature",
            &QueryInputIr::Literal(RequestValueIr::Int64(i64::MAX - 1023)),
        )
        .unwrap(),
        (i64::MAX - 1023) as f64
    );

    let error = query_feature_value(
        "unsigned_feature",
        &QueryInputIr::Literal(RequestValueIr::UInt64(u64::MAX)),
    )
    .expect_err("values rounded to 2^64 must not be accepted as u64");
    assert_eq!(error.details[0].code, "query_input_precision_loss");

    assert_eq!(
        query_feature_value(
            "unsigned_feature",
            &QueryInputIr::Literal(RequestValueIr::UInt64(u64::MAX - 2047)),
        )
        .unwrap(),
        (u64::MAX - 2047) as f64
    );
}

#[test]
fn unsupported_query_inputs_and_rank_property_collisions_are_explicit() {
    let expression =
        request(Vec::new(), Vec::new(), None).with_inputs(std::collections::BTreeMap::from([(
            "embedding".to_owned(),
            QueryInputIr::Expression("embed(@query)".to_owned()),
        )]));
    let error = validate_search_request(&expression).unwrap_err();
    assert_eq!(
        error.details[0].code,
        "query_input_expression_not_supported"
    );

    let complex =
        request(Vec::new(), Vec::new(), None).with_inputs(std::collections::BTreeMap::from([(
            "list".to_owned(),
            QueryInputIr::Literal(RequestValueIr::List(vec![RequestValueIr::Int64(1)])),
        )]));
    let error = validate_search_request(&complex).unwrap_err();
    assert_eq!(error.details[0].code, "query_input_value_not_supported");

    let collision = request(Vec::new(), Vec::new(), None)
        .with_inputs(std::collections::BTreeMap::from([(
            "boost".to_owned(),
            QueryInputIr::Literal(RequestValueIr::Float64(2.0)),
        )]))
        .with_ranking_request(
            "default",
            std::collections::BTreeMap::from([("boost".to_owned(), "2".to_owned())]),
        );
    let error = validate_search_request(&collision).unwrap_err();
    assert_eq!(error.details[0].code, "rank_property_collision");
}

#[test]
fn supported_rank_profile_definitions_lower_to_native_config() {
    let mut definition = IndexDefinitionIr::new(
        None,
        vec![FieldIr::new(
            field("title"),
            DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
            None,
            FieldCapabilities::new(true, true, false, false, false),
        )],
        Vec::new(),
    )
    .unwrap();
    definition.set_rank_profiles(vec![RankProfileDefinitionIr {
        name: "product_search".to_owned(),
        inherits: Vec::new(),
        inputs: Vec::new(),
        functions: Vec::new(),
        first_phase: Some(RankPhaseDefinitionIr {
            expression: Some("bm25(title) + query(boost)".to_owned()),
            rerank_count: None,
            rank_score_drop_limit: None,
            properties: std::collections::BTreeMap::new(),
            total_rerank_count: None,
            keep_rank_count: None,
            total_keep_rank_count: None,
        }),
        second_phase: None,
        global_phase: None,
        rank_features: Vec::new(),
        match_features: vec!["bm25(title)".to_owned(), "query(boost)".to_owned()],
        summary_features: Vec::new(),
        sort_features: Vec::new(),
        constants: Vec::new(),
        onnx_models: Vec::new(),
        rank_properties: std::collections::BTreeMap::new(),
        strict: None,
        properties: std::collections::BTreeMap::new(),
    }]);
    let root = std::env::temp_dir().join(format!(
        "searchkernel-adapter-rank-profile-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    compile_index_definition(&definition, &root).expect("supported profile compiles");
    let config = std::fs::read_to_string(root.join("rank-profiles.cfg")).unwrap();
    assert!(config.contains("rankprofile[2].name \"product_search\""));
    assert!(config.contains("rankingExpression(product_search).rankingScript"));
    assert!(config.contains("bm25(title) + query(boost)"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn second_phase_and_rerank_count_lower_to_native_config_independently() {
    let mut definition = IndexDefinitionIr::new(
        None,
        vec![
            FieldIr::new(
                field("title"),
                DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
                None,
                FieldCapabilities::new(true, true, false, false, false),
            ),
            FieldIr::new(
                field("rating"),
                DataTypeIr::Primitive(PrimitiveDataTypeIr::Int64),
                None,
                FieldCapabilities::new(true, false, true, true, false),
            ),
            FieldIr::new(
                field("inventory"),
                DataTypeIr::Primitive(PrimitiveDataTypeIr::Int64),
                None,
                FieldCapabilities::new(true, false, true, true, false),
            ),
        ],
        Vec::new(),
    )
    .unwrap();
    definition.set_rank_profiles(vec![RankProfileDefinitionIr {
        name: "two_phase".to_owned(),
        inherits: Vec::new(),
        inputs: Vec::new(),
        functions: Vec::new(),
        first_phase: Some(RankPhaseDefinitionIr {
            expression: Some("attribute(rating)".to_owned()),
            rerank_count: None,
            rank_score_drop_limit: None,
            properties: std::collections::BTreeMap::new(),
            total_rerank_count: None,
            keep_rank_count: None,
            total_keep_rank_count: None,
        }),
        second_phase: Some(RankPhaseDefinitionIr {
            expression: Some("attribute(inventory)".to_owned()),
            rerank_count: Some(3),
            rank_score_drop_limit: None,
            properties: std::collections::BTreeMap::new(),
            total_rerank_count: None,
            keep_rank_count: None,
            total_keep_rank_count: None,
        }),
        global_phase: None,
        rank_features: Vec::new(),
        match_features: Vec::new(),
        summary_features: Vec::new(),
        sort_features: Vec::new(),
        constants: Vec::new(),
        onnx_models: Vec::new(),
        rank_properties: std::collections::BTreeMap::new(),
        strict: None,
        properties: std::collections::BTreeMap::new(),
    }]);
    let root = std::env::temp_dir().join(format!(
        "searchkernel-adapter-two-phase-rank-profile-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    compile_index_definition(&definition, &root).expect("two-phase profile compiles");
    let config = std::fs::read_to_string(root.join("rank-profiles.cfg")).unwrap();
    assert!(config.contains("vespa.rank.secondphase"));
    assert!(config.contains("rankingExpression(secondphase).rankingScript"));
    assert!(config.contains("attribute(inventory)"));
    assert!(config.contains("vespa.hitcollector.heapsize"));
    assert!(config.contains("value \"3\""));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn explicit_empty_projection_returns_no_document_fields() {
    let document = Document::new("doc-1")
        .field(
            "title",
            FieldValue::Scalar(ScalarValue::String("fox".into())),
        )
        .field("price", FieldValue::Scalar(ScalarValue::Integer(10)));
    assert!(project_document(&document, &[]).is_empty());
    assert_eq!(
        project_document(&document, &[field("title")]),
        BTreeMap::from([("title".into(), Value::String("fox".into()))])
    );
}

#[test]
fn empty_projection_lowers_to_no_summary_fields() {
    let request = request(
        vec![SearchFieldIr::new(
            field("title"),
            SearchFieldWeight::new(1.0).unwrap(),
        )],
        Vec::new(),
        None,
    );
    let language = analysis_options(request.language()).unwrap();
    let query = lower_search_query(&request, language).unwrap();
    let lowered = lower_search_options(&request, query, language).unwrap();

    assert_eq!(
        lowered.summary,
        searchkernel_vespa_engine::SummaryRequest::None
    );
}

#[test]
fn summary_class_lowers_independently_from_projection() {
    let title = field("title");
    let base = SearchRequestIr::new(
        QueryIr::Text("wireless headphones".into()),
        vec![SearchFieldIr::new(
            title.clone(),
            SearchFieldWeight::new(1.0).unwrap(),
        )],
        None,
        Vec::new(),
        20,
        0,
        ProjectionIr::new(vec![title]),
        HighlightingIr::Disabled,
        None,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    );
    let language = analysis_options(base.language()).unwrap();
    let query = lower_search_query(&base, language).unwrap();

    let default_class = lower_search_options(&base, query.clone(), language).unwrap();
    assert_eq!(default_class.summary_class, None);
    assert_eq!(
        default_class.summary,
        searchkernel_vespa_engine::SummaryRequest::Fields(vec!["title".into()])
    );

    for name in ["default", "some-class"] {
        let request = base.clone().with_presentation(PresentationIr {
            summary: Some(name.to_owned()),
            ..PresentationIr::default()
        });
        let lowered = lower_search_options(&request, query.clone(), language).unwrap();
        assert_eq!(lowered.summary_class.as_deref(), Some(name));
        assert_eq!(
            lowered.summary,
            searchkernel_vespa_engine::SummaryRequest::Fields(vec!["title".into()])
        );
    }
}

#[test]
fn summary_class_survives_explicit_empty_projection_without_fetching_docsums() {
    let request = request(
        vec![SearchFieldIr::new(
            field("title"),
            SearchFieldWeight::new(1.0).unwrap(),
        )],
        Vec::new(),
        None,
    )
    .with_presentation(PresentationIr {
        summary: Some("some-class".to_owned()),
        ..PresentationIr::default()
    });
    let language = analysis_options(request.language()).unwrap();
    let query = lower_search_query(&request, language).unwrap();
    let lowered = lower_search_options(&request, query, language).unwrap();

    assert_eq!(lowered.summary_class.as_deref(), Some("some-class"));
    assert_eq!(
        lowered.summary,
        searchkernel_vespa_engine::SummaryRequest::None
    );
}

#[test]
fn named_summary_and_highlighting_lower_together() {
    let title = field("title");
    let request = SearchRequestIr::new(
        QueryIr::Text("brown fox".into()),
        vec![SearchFieldIr::new(
            title.clone(),
            SearchFieldWeight::new(1.0).unwrap(),
        )],
        None,
        Vec::new(),
        10,
        0,
        ProjectionIr::new(vec![title.clone()]),
        HighlightingIr::Requested {
            fields: vec![title.clone()],
            multi_valued_fields: Vec::new(),
        },
        None,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    )
    .with_presentation(PresentationIr {
        summary: Some("custom".to_owned()),
        ..PresentationIr::default()
    });
    let language = analysis_options(request.language()).unwrap();
    let query = lower_search_query(&request, language).unwrap();
    let lowered = lower_search_options(&request, query, language).unwrap();

    assert_eq!(lowered.summary_class.as_deref(), Some("custom"));
    assert_eq!(
        lowered.summary,
        searchkernel_vespa_engine::SummaryRequest::Fields(vec!["title".into()])
    );
    assert_eq!(lowered.highlight_fields, vec!["title"]);
}

#[test]
fn highlighting_rejects_unsupported_same_field_query_shapes() {
    let title = field("title");
    let request = |query| {
        SearchRequestIr::new(
            query,
            vec![SearchFieldIr::new(
                title.clone(),
                SearchFieldWeight::new(1.0).unwrap(),
            )],
            None,
            Vec::new(),
            10,
            0,
            ProjectionIr::new(Vec::new()),
            HighlightingIr::Requested {
                fields: vec![title.clone()],
                multi_valued_fields: Vec::new(),
            },
            None,
            PrefixPolicy::Disabled,
            TypoPolicy::Disabled,
        )
    };

    let fuzzy = typed_contains(
        "title",
        QueryExprKindIr::Fuzzy {
            value: RequestValueIr::String("broun".to_owned()),
            max_edit_distance: 1,
            prefix_length: 0,
            prefix: false,
        },
    );
    let error = validate_search_request(&request(fuzzy))
        .expect_err("same-field fuzzy highlighting must be rejected before native lowering");
    assert_eq!(error.code, ErrorCode::Unsupported);

    let matches = typed_expression(QueryExprKindIr::Matches {
        field: typed_field("title"),
        pattern: "bro.*".to_owned(),
    });
    let error = validate_search_request(&request(matches))
        .expect_err("same-field regexp highlighting must be rejected before native lowering");
    assert_eq!(error.code, ErrorCode::Unsupported);

    let mixed = typed_expression(QueryExprKindIr::And(vec![
        QueryExprIr {
            kind: QueryExprKindIr::Contains {
                field: typed_field("title"),
                item: Box::new(typed_term("brown")),
            },
            annotations: BTreeMap::new(),
        },
        QueryExprIr {
            kind: QueryExprKindIr::In {
                field: typed_field("category"),
                values: vec![RequestValueIr::String("animal".to_owned())],
            },
            annotations: BTreeMap::new(),
        },
    ]));
    validate_search_request(&request(mixed))
        .expect("an unrelated attribute branch must not disable title highlighting");

    let mixed_specialized = typed_expression(QueryExprKindIr::And(vec![
        QueryExprIr {
            kind: QueryExprKindIr::Contains {
                field: typed_field("title"),
                item: Box::new(typed_term("brown")),
            },
            annotations: BTreeMap::new(),
        },
        QueryExprIr {
            kind: QueryExprKindIr::Wand {
                field: typed_field("category"),
                items: vec![WeightedQueryItemIr {
                    key: RequestValueIr::String("animal".to_owned()),
                    weight: 1,
                }],
            },
            annotations: BTreeMap::new(),
        },
    ]));
    validate_search_request(&request(mixed_specialized))
        .expect("an unrelated specialized branch must not disable title highlighting");
}

#[test]
fn highlighting_compatibility_survives_portable_flatbuffers_resolution() {
    let title = field("title");
    let request = SearchRequestIr::new(
        typed_contains(
            "title",
            QueryExprKindIr::Fuzzy {
                value: RequestValueIr::String("runnng".to_owned()),
                max_edit_distance: 2,
                prefix_length: 0,
                prefix: false,
            },
        ),
        Vec::new(),
        None,
        Vec::new(),
        10,
        0,
        ProjectionIr::new(Vec::new()),
        HighlightingIr::Requested {
            fields: vec![title.clone()],
            multi_valued_fields: Vec::new(),
        },
        None,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    );
    let definition = IndexDefinitionIr::new(
        None,
        vec![FieldIr::new(
            title,
            DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
            None,
            FieldCapabilities::new(true, true, false, false, false),
        )],
        Vec::new(),
    )
    .expect("highlighting parity definition");

    let bytes = searchkernel_ir::search_request_ir_to_api(&request);
    let api_request =
        searchkernel_ir::__flatbuffers::root::<searchkernel_ir::api::SearchRequest>(&bytes)
            .expect("encoded highlighting request");
    let portable = IndexContext::from_definition_ir(definition)
        .search_to_ir(api_request)
        .expect("portable highlighting request should resolve");
    assert_eq!(portable, request);

    for candidate in [&request, &portable] {
        let error = validate_search_request(candidate)
            .expect_err("same-field fuzzy highlighting must be rejected consistently");
        assert_eq!(error.code, ErrorCode::Unsupported);
        assert_eq!(error.details[0].code, "highlighting_query_not_supported");
    }
}

#[test]
fn every_expressible_search_option_is_lowered_to_the_generic_m_api() {
    let title = field("title");
    let description = field("description");
    let brand = field("brand");
    let price = field("price");
    let search_fields = vec![
        SearchFieldIr::new(title.clone(), SearchFieldWeight::new(1.0).unwrap()),
        SearchFieldIr::new(description.clone(), SearchFieldWeight::new(1.0).unwrap()),
    ];
    let sorts = vec![
        SortIr::new(price.clone(), SortDirection::Asc, MissingPlacement::First),
        SortIr::new(brand.clone(), SortDirection::Desc, MissingPlacement::Last),
    ];
    let grouping = [
        facet_grouping(
            brand.as_str(),
            7,
            searchkernel_vespa_engine::GroupingOrder::CountDescending,
        ),
        facet_grouping(
            price.as_str(),
            6,
            searchkernel_vespa_engine::GroupingOrder::CountAscending,
        ),
        facet_grouping(
            title.as_str(),
            5,
            searchkernel_vespa_engine::GroupingOrder::ValueAscending,
        ),
        facet_grouping(
            description.as_str(),
            4,
            searchkernel_vespa_engine::GroupingOrder::ValueDescending,
        ),
    ];
    let request = SearchRequestIr::new(
        QueryIr::Text("wireless headphones".into()),
        search_fields,
        Some(FilterIr::Comparison(ComparisonIr::new(
            price.clone(),
            ComparisonOp::Gte,
            ScalarValueIr::Int64(10),
        ))),
        sorts,
        3,
        2,
        searchkernel_ir::ProjectionIr::new(vec![title.clone(), brand.clone()]),
        HighlightingIr::Disabled,
        Some(LanguageHint::parse("en-US").unwrap()),
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    )
    .with_grouping(grouping.to_vec())
    .with_timeout(Some(std::time::Duration::from_millis(123)));

    validate_search_request(&request).unwrap();
    let language = analysis_options(request.language()).unwrap();
    let query = lower_search_query(&request, language).unwrap();
    let lowered = lower_search_options(&request, query.clone(), language).unwrap();
    assert_eq!(lowered.query, query);
    assert_eq!(lowered.source, searchkernel_vespa_engine::Source::All);
    assert_eq!(lowered.rank_profile, "default");
    assert_eq!(lowered.limit, 3);
    assert_eq!(lowered.offset, 2);
    assert_eq!(lowered.timeout, std::time::Duration::from_millis(123));
    assert_eq!(lowered.language, language);
    assert_eq!(
        lowered.sort,
        vec![
            SortField::ascending("price").missing_first(),
            SortField::descending("brand").missing_last(),
        ]
    );
    assert_eq!(
        lowered.summary,
        searchkernel_vespa_engine::SummaryRequest::Fields(vec!["title".into(), "brand".into(),])
    );

    let expected_orders = [
        searchkernel_vespa_engine::GroupingOrder::CountDescending,
        searchkernel_vespa_engine::GroupingOrder::CountAscending,
        searchkernel_vespa_engine::GroupingOrder::ValueAscending,
        searchkernel_vespa_engine::GroupingOrder::ValueDescending,
    ];
    let facets = lower_facet_groupings(request.grouping()).unwrap();
    for (facet, expected_order) in facets.iter().zip(expected_orders) {
        assert_eq!(facet.order, expected_order);
    }
    let lowered = lower_facets_into_request(lowered, &facets);
    assert_eq!(lowered.facets.len(), 4);
    for ((facet, native), expected_order) in facets.iter().zip(&lowered.facets).zip(expected_orders)
    {
        assert_eq!(native.field, facet.field);
        assert_eq!(native.max_groups, facet.limit);
        assert_eq!(native.order, expected_order);
    }
}

#[test]
fn unsupported_search_options_have_stable_narrow_codes() {
    let weighted = request(
        vec![SearchFieldIr::new(
            field("title"),
            SearchFieldWeight::new(2.0).unwrap(),
        )],
        Vec::new(),
        None,
    );
    let error = validate_search_request(&weighted).unwrap_err();
    assert_eq!(error.details[0].code, "ranking_weights_not_supported");

    assert_eq!(ranking_profile(RankingPolicyIr::Bm25), Ok("bm25"));
}

#[test]
fn insert_and_upsert_are_linearizable_for_one_document_id() {
    let root = std::env::temp_dir().join(format!(
        "searchkernel-adapter-linearizable-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos()
    ));
    let data = root.join("data");
    let config = root.join("config");
    std::fs::create_dir_all(&data).expect("create test data directory");
    ConfigCompiler
        .compile_to(
            &Schema::new("searchkernel").field(
                Field::new("value", FieldType::Integer)
                    .attribute()
                    .summary(),
            ),
            &config,
        )
        .expect("compile test schema");
    let engine = Engine::open(&data, &config).expect("open test engine");
    let probe = std::sync::Arc::new(MutationProbe {
        rendezvous: std::sync::Barrier::new(3),
        release: std::sync::Barrier::new(1),
        update_completed: None,
    });
    let backend = std::sync::Arc::new(VespaBackend::with_mutation_probe(engine, probe.clone()));

    let insert_backend = std::sync::Arc::clone(&backend);
    let insert =
        std::thread::spawn(move || insert_backend.insert(InsertRequestIr::new(test_document(1))));
    let upsert_backend = std::sync::Arc::clone(&backend);
    let upsert =
        std::thread::spawn(move || upsert_backend.upsert(UpsertRequestIr::new(test_document(2))));

    probe.rendezvous.wait();
    assert_eq!(
        insert
            .join()
            .expect("insert thread")
            .map(|result| result.id),
        Ok("doc-1".into())
    );
    assert_eq!(
        upsert
            .join()
            .expect("upsert thread")
            .map(|result| result.id),
        Ok("doc-1".into())
    );

    let stored = backend
        .engine
        .get("doc-1")
        .expect("read final document")
        .expect("final document exists");
    assert_eq!(
        stored.fields().get("value"),
        Some(&searchkernel_vespa_engine::FieldValue::Scalar(
            searchkernel_vespa_engine::ScalarValue::Integer(2),
        ))
    );

    let backend = match std::sync::Arc::try_unwrap(backend) {
        Ok(backend) => backend,
        Err(_) => panic!("test backend still has owners"),
    };
    backend.engine.close().expect("close test engine");
    std::fs::remove_dir_all(root).expect("remove test index");
}

#[test]
fn insert_does_not_overwrite_a_concurrent_create_if_missing_update() {
    let root = std::env::temp_dir().join(format!(
        "searchkernel-adapter-insert-update-linearizable-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos()
    ));
    let data = root.join("data");
    let config = root.join("config");
    std::fs::create_dir_all(&data).expect("create test data directory");
    ConfigCompiler
        .compile_to(
            &Schema::new("searchkernel").field(
                Field::new("value", FieldType::Integer)
                    .attribute()
                    .summary(),
            ),
            &config,
        )
        .expect("compile test schema");
    let engine = Engine::open(&data, &config).expect("open test engine");
    let (update_completed_sender, update_completed_receiver) = std::sync::mpsc::channel();
    let probe = std::sync::Arc::new(MutationProbe {
        rendezvous: std::sync::Barrier::new(2),
        release: std::sync::Barrier::new(2),
        update_completed: Some(update_completed_sender),
    });
    let backend = std::sync::Arc::new(VespaBackend::with_mutation_probe(engine, probe.clone()));

    let insert_backend = std::sync::Arc::clone(&backend);
    let insert =
        std::thread::spawn(move || insert_backend.insert(InsertRequestIr::new(test_document(1))));
    probe.rendezvous.wait();

    let update_backend = std::sync::Arc::clone(&backend);
    let update = std::thread::spawn(move || {
        update_backend.update(
            UpdateRequestIr::new(
                DocumentId::parse("doc-1").expect("valid document ID"),
                vec![UpdateOperationIr::Assign {
                    target: UpdateTargetIr::parse("value").expect("valid update target"),
                    value: FieldValueIr::Single(ScalarValueIr::Int64(2)),
                }],
            )
            .with_create(true),
        )
    });

    // On the buggy implementation the create-if-missing update completes while
    // insert is paused, after which insert overwrites it. With the fix the
    // update is blocked by the same existence-mutation lock. The timeout is
    // only a bounded observation of this forced interleaving, not a stress race.
    let update_completed_before_insert_release = update_completed_receiver
        .recv_timeout(std::time::Duration::from_secs(5))
        .is_ok();
    probe.release.wait();

    let insert_result = insert.join().expect("insert thread");
    let update_result = update.join().expect("update thread");
    let stored = backend
        .engine
        .get("doc-1")
        .expect("read final document")
        .expect("final document exists");
    let stored_value = stored.fields().get("value").cloned();

    if update_completed_before_insert_release {
        assert!(
            insert_result.is_err(),
            "insert succeeded after a create-if-missing update completed"
        );
    } else {
        assert!(
            insert_result.is_ok(),
            "serialized insert should succeed before the create-if-missing update"
        );
    }
    assert!(update_result.is_ok(), "create-if-missing update failed");
    assert_eq!(
        stored_value,
        Some(FieldValue::Scalar(
            searchkernel_vespa_engine::ScalarValue::Integer(2),
        ))
    );

    let backend = match std::sync::Arc::try_unwrap(backend) {
        Ok(backend) => backend,
        Err(_) => panic!("test backend still has owners"),
    };
    backend.engine.close().expect("close test engine");
    std::fs::remove_dir_all(root).expect("remove test index");
}

#[test]
fn non_creating_mutations_leave_a_missing_document_missing() {
    let root = std::env::temp_dir().join(format!(
        "searchkernel-adapter-noncreating-mutations-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos()
    ));
    let data = root.join("data");
    let config = root.join("config");
    std::fs::create_dir_all(&data).expect("create test data directory");
    ConfigCompiler
        .compile_to(
            &Schema::new("searchkernel").field(
                Field::new("value", FieldType::Integer)
                    .attribute()
                    .summary(),
            ),
            &config,
        )
        .expect("compile test schema");
    let backend =
        VespaBackend::open(&data, &config).expect("open non-creating mutation test backend");
    let id = DocumentId::parse("missing-doc").expect("valid document ID");

    let _patch_result = backend.patch(PatchRequestIr::new(
        id.clone(),
        vec![PatchOperationIr::Set {
            field: field("value"),
            value: FieldValueIr::Single(ScalarValueIr::Int64(1)),
        }],
    ));
    assert!(
        backend
            .engine
            .get(id.as_str())
            .expect("get after patch")
            .is_none(),
        "patch unexpectedly created a missing document"
    );

    let _update_result = backend.update(UpdateRequestIr::new(
        id.clone(),
        vec![UpdateOperationIr::Assign {
            target: UpdateTargetIr::parse("value").expect("valid update target"),
            value: FieldValueIr::Single(ScalarValueIr::Int64(2)),
        }],
    ));
    assert!(
        backend
            .engine
            .get(id.as_str())
            .expect("get after ordinary update")
            .is_none(),
        "ordinary update unexpectedly created a missing document"
    );

    let _create_false_result = backend.update(
        UpdateRequestIr::new(
            id.clone(),
            vec![UpdateOperationIr::Assign {
                target: UpdateTargetIr::parse("value").expect("valid update target"),
                value: FieldValueIr::Single(ScalarValueIr::Int64(3)),
            }],
        )
        .with_create(false),
    );
    assert!(
        backend
            .engine
            .get(id.as_str())
            .expect("get after create=false update")
            .is_none(),
        "create=false update unexpectedly created a missing document"
    );

    let deleted = backend
        .delete(DeleteRequestIr::new(id.clone()))
        .expect("delete missing document");
    assert!(
        !deleted.deleted,
        "delete unexpectedly reported a missing document as deleted"
    );
    assert!(
        backend
            .engine
            .get(id.as_str())
            .expect("get after delete")
            .is_none(),
        "delete unexpectedly created a missing document"
    );

    backend.engine.close().expect("close test engine");
    std::fs::remove_dir_all(root).expect("remove test index");
}

#[test]
fn session_close_drains_serialized_existence_mutations_without_deadlock() {
    let root = std::env::temp_dir().join(format!(
        "searchkernel-adapter-existence-close-drain-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos()
    ));
    let data = root.join("data");
    let config = root.join("config");
    std::fs::create_dir_all(&data).expect("create test data directory");
    ConfigCompiler
        .compile_to(
            &Schema::new("searchkernel").field(
                Field::new("value", FieldType::Integer)
                    .attribute()
                    .summary(),
            ),
            &config,
        )
        .expect("compile test schema");
    let engine = Engine::open(&data, &config).expect("open test engine");
    let probe = std::sync::Arc::new(MutationProbe {
        rendezvous: std::sync::Barrier::new(2),
        release: std::sync::Barrier::new(2),
        update_completed: None,
    });
    let backend = VespaBackend::with_mutation_probe(engine, probe.clone());
    let session = BackendSession::new(Box::new(backend));

    let insert_session = std::sync::Arc::clone(&session);
    let insert = std::thread::spawn(move || {
        insert_session.execute(|backend| backend.insert(InsertRequestIr::new(test_document(1))))
    });
    probe.rendezvous.wait();

    let (update_admitted_sender, update_admitted_receiver) = std::sync::mpsc::channel();
    let update_session = std::sync::Arc::clone(&session);
    let update = std::thread::spawn(move || {
        update_session.execute(|backend| {
            update_admitted_sender
                .send(())
                .expect("signal admitted update");
            backend.update(
                UpdateRequestIr::new(
                    DocumentId::parse("doc-1").expect("valid document ID"),
                    vec![UpdateOperationIr::Assign {
                        target: UpdateTargetIr::parse("value").expect("valid update target"),
                        value: FieldValueIr::Single(ScalarValueIr::Int64(2)),
                    }],
                )
                .with_create(true),
            )
        })
    });
    update_admitted_receiver
        .recv()
        .expect("create-if-missing update must be admitted before close");

    let close_session = std::sync::Arc::clone(&session);
    let closer = std::thread::spawn(move || close_session.close());

    // Observe the CLOSING admission boundary without reaching into session
    // internals. Once new work is rejected, close is definitely draining the
    // two already-admitted operations.
    loop {
        match session.execute(|_| Ok::<(), ()>(())) {
            Err(SessionError::Closed) => break,
            Ok(Ok(())) => std::thread::yield_now(),
            Ok(Err(())) => unreachable!("no-op callback cannot fail"),
        }
    }

    probe.release.wait();

    assert!(matches!(insert.join().expect("insert thread"), Ok(Ok(_))));
    assert!(matches!(update.join().expect("update thread"), Ok(Ok(_))));
    closer
        .join()
        .expect("close thread")
        .expect("session close must drain without deadlock");

    drop(session);
    std::fs::remove_dir_all(root).expect("remove test index");
}

fn test_document(value: i64) -> searchkernel_ir::DocumentIr {
    searchkernel_ir::DocumentIr::new(
        searchkernel_ir::DocumentId::parse("doc-1").expect("valid document ID"),
        std::collections::BTreeMap::from([(
            field("value"),
            FieldValueIr::Single(ScalarValueIr::Int64(value)),
        )]),
    )
}

#[test]
fn yql_lowering_uses_shared_parser_and_native_order_by() {
    let request = request(Vec::new(), Vec::new(), None).with_query(QueryIr::Yql {
        source:
            "select * from products where title contains \"fox\" order by price desc, title asc"
                .to_owned(),
        parameters: std::collections::BTreeMap::new(),
    });
    let language = searchkernel_vespa_engine::LinguisticsOptions::default();

    assert_eq!(
        lower_query(request.query(), &[]).unwrap(),
        Query::Term {
            field: "title".to_owned(),
            text: "fox".to_owned(),
        }
    );
    let lowered = lower_search_options(
        &request,
        lower_search_query(&request, language).unwrap(),
        language,
    )
    .unwrap();
    assert_eq!(lowered.source, Source::Named("products".to_owned()));
    assert_eq!(
        lowered.sort,
        vec![
            SortField::descending("price"),
            SortField::ascending("title")
        ]
    );
}

#[test]
fn yql_controls_conflict_with_separately_supplied_filter() {
    let request = SearchRequestIr::new(
        QueryIr::Yql {
            source: "select * from products where title contains \"fox\"".to_owned(),
            parameters: std::collections::BTreeMap::new(),
        },
        Vec::new(),
        Some(FilterIr::Exists {
            field: field("title"),
        }),
        Vec::new(),
        10,
        0,
        ProjectionIr::new(Vec::new()),
        HighlightingIr::Disabled,
        None,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    );

    let error = validate_search_request(&request).unwrap_err();
    assert_eq!(error.details[0].code, "yql_conflict");
    assert_eq!(error.details[0].path.as_deref(), Some("filter"));
}
