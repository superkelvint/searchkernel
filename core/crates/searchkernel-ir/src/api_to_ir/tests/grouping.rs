use super::*;

#[test]
fn search_resolver_rejects_duplicate_grouping_aliases_at_semantic_boundary() {
    let mut context = context();
    context.definition.fields[0].capabilities.groupable = true;

    let grouping = GroupingRequestIr {
        root: GroupingAllIr {
            operations: vec![
                GroupingOperationIr::Group(GroupingExpressionIr::Attribute("brand".to_owned())),
                GroupingOperationIr::Each(GroupingAllIr {
                    operations: vec![
                        GroupingOperationIr::Alias {
                            name: "count".to_owned(),
                            expression: GroupingExpressionIr::Aggregation(Box::new(
                                GroupingAggregationIr::Count,
                            )),
                        },
                        GroupingOperationIr::Alias {
                            name: "count".to_owned(),
                            expression: GroupingExpressionIr::Aggregation(Box::new(
                                GroupingAggregationIr::Count,
                            )),
                        },
                    ],
                    label: None,
                }),
            ],
            label: None,
        },
        continuations: Vec::new(),
    };

    let error = resolve_search(
        &context,
        match_all_search(vec![GroupingInput::Structured(grouping)]),
    )
    .expect_err("duplicate grouping aliases must fail in the canonical resolver");
    assert_eq!(error.details[0].code, "duplicate_grouping_alias");
}

#[test]
fn flatbuffer_grouping_ingress_uses_canonical_alias_validation() {
    let grouping = GroupingRequestIr {
        root: GroupingAllIr {
            operations: vec![
                GroupingOperationIr::Group(GroupingExpressionIr::Attribute("brand".to_owned())),
                GroupingOperationIr::Each(GroupingAllIr {
                    operations: vec![
                        GroupingOperationIr::Alias {
                            name: "count".to_owned(),
                            expression: GroupingExpressionIr::Aggregation(Box::new(
                                GroupingAggregationIr::Count,
                            )),
                        },
                        GroupingOperationIr::Alias {
                            name: "count".to_owned(),
                            expression: GroupingExpressionIr::Aggregation(Box::new(
                                GroupingAggregationIr::Count,
                            )),
                        },
                    ],
                    label: None,
                }),
            ],
            label: None,
        },
        continuations: Vec::new(),
    };
    let request = SearchRequestIr::new(
        QueryIr::MatchAll,
        Vec::new(),
        None,
        Vec::new(),
        20,
        0,
        ProjectionIr::new(Vec::new()),
        HighlightingIr::Disabled,
        None,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    )
    .with_grouping(vec![grouping]);
    let bytes = crate::wire::v1::search_request_ir_to_api(&request);
    let request =
        crate::__flatbuffers::root::<api::SearchRequest>(&bytes).expect("encoded search request");

    let mut context = context();
    context.definition.fields[0].capabilities.groupable = true;
    let error = context
        .search_to_ir(request)
        .expect_err("portable grouping must use canonical alias validation");
    assert_eq!(error.details[0].code, "duplicate_grouping_alias");
}

#[test]
fn grouping_range_integer_bounds_round_trip_through_portable_ir() {
    let lower = 9_007_199_254_740_993;
    let upper = 9_007_199_254_740_994;
    let grouping = GroupingRequestIr {
        root: GroupingAllIr {
            operations: vec![
                GroupingOperationIr::Group(GroupingExpressionIr::Attribute("price".into())),
                GroupingOperationIr::Each(GroupingAllIr {
                    operations: vec![
                        GroupingOperationIr::Filter(GroupingFilterIr::Range {
                            expression: Box::new(GroupingExpressionIr::Attribute("price".into())),
                            lower: Box::new(GroupingExpressionIr::Literal(RequestValueIr::UInt64(
                                lower,
                            ))),
                            upper: Box::new(GroupingExpressionIr::Literal(RequestValueIr::UInt64(
                                upper,
                            ))),
                            include_lower: true,
                            include_upper: false,
                        }),
                        GroupingOperationIr::Output(vec![GroupingOutputIr {
                            label: None,
                            expression: GroupingExpressionIr::Aggregation(Box::new(
                                GroupingAggregationIr::Count,
                            )),
                        }]),
                    ],
                    label: None,
                }),
            ],
            label: None,
        },
        continuations: Vec::new(),
    };
    let request = SearchRequestIr::new(
        QueryIr::MatchAll,
        Vec::new(),
        None,
        Vec::new(),
        20,
        0,
        ProjectionIr::new(Vec::new()),
        HighlightingIr::Disabled,
        None,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    )
    .with_grouping(vec![grouping]);
    let bytes = crate::wire::v1::search_request_ir_to_api(&request);
    let api_request =
        crate::__flatbuffers::root::<api::SearchRequest>(&bytes).expect("encoded search request");

    let mut context = context();
    context.definition.fields[1].capabilities.groupable = true;
    let round_trip = context
        .search_to_ir(api_request)
        .expect("portable grouping should resolve");

    assert_eq!(round_trip.grouping(), request.grouping());
}

#[test]
fn search_resolver_rejects_unknown_grouping_alias_references() {
    let mut context = context();
    context.definition.fields[0].capabilities.groupable = true;

    let grouping = GroupingRequestIr {
        root: GroupingAllIr {
            operations: vec![
                GroupingOperationIr::Group(GroupingExpressionIr::Attribute("brand".to_owned())),
                GroupingOperationIr::Each(GroupingAllIr {
                    operations: vec![GroupingOperationIr::Output(vec![GroupingOutputIr {
                        label: Some("count".to_owned()),
                        expression: GroupingExpressionIr::Alias("missing".to_owned()),
                    }])],
                    label: None,
                }),
            ],
            label: None,
        },
        continuations: Vec::new(),
    };

    let error = resolve_search(
        &context,
        match_all_search(vec![GroupingInput::Structured(grouping)]),
    )
    .expect_err("unknown grouping aliases must fail in the canonical resolver");
    assert_eq!(error.details[0].code, "grouping_alias_unknown");
}

#[test]
fn search_resolver_rejects_grouping_output_name_collisions() {
    let mut context = context();
    context.definition.fields[0].capabilities.groupable = true;

    let grouping = GroupingRequestIr {
        root: GroupingAllIr {
            operations: vec![
                GroupingOperationIr::Group(GroupingExpressionIr::Attribute("brand".to_owned())),
                GroupingOperationIr::Each(GroupingAllIr {
                    operations: vec![GroupingOperationIr::Output(vec![
                        GroupingOutputIr {
                            label: None,
                            expression: GroupingExpressionIr::Aggregation(Box::new(
                                GroupingAggregationIr::Count,
                            )),
                        },
                        GroupingOutputIr {
                            label: None,
                            expression: GroupingExpressionIr::Aggregation(Box::new(
                                GroupingAggregationIr::Count,
                            )),
                        },
                    ])],
                    label: None,
                }),
            ],
            label: None,
        },
        continuations: Vec::new(),
    };

    let error = resolve_search(
        &context,
        match_all_search(vec![GroupingInput::Structured(grouping)]),
    )
    .expect_err("duplicate grouping output names must fail in the canonical resolver");
    assert_eq!(error.details[0].code, "grouping_output_name_collision");
}

#[test]
fn search_resolver_rejects_root_output_name_collisions_across_groupings() {
    let grouping = || root_output_grouping(Some("metric"), GroupingAggregationIr::Count);

    let error = resolve_search(
        &context(),
        match_all_search(vec![
            GroupingInput::Structured(grouping()),
            GroupingInput::Structured(grouping()),
        ]),
    )
    .expect_err("root output names must be unique across grouping requests");

    assert_eq!(error.details[0].code, "grouping_output_name_collision");
    assert_eq!(
        error.details[0].path.as_deref(),
        Some("grouping[1].root.operations[0].outputs[0]")
    );
}

#[test]
fn flatbuffer_grouping_ingress_rejects_the_same_cross_request_collision() {
    let request = SearchRequestIr::new(
        QueryIr::MatchAll,
        Vec::new(),
        None,
        Vec::new(),
        20,
        0,
        ProjectionIr::new(Vec::new()),
        HighlightingIr::Disabled,
        None,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    )
    .with_grouping(vec![
        root_output_grouping(Some("metric"), GroupingAggregationIr::Count),
        root_output_grouping(Some("metric"), GroupingAggregationIr::Count),
    ]);
    let bytes = crate::wire::v1::search_request_ir_to_api(&request);
    let request =
        crate::__flatbuffers::root::<api::SearchRequest>(&bytes).expect("encoded grouping request");

    let error = context()
        .search_to_ir(request)
        .expect_err("portable ingress must use cross-request root validation");
    assert_eq!(error.details[0].code, "grouping_output_name_collision");
    assert_eq!(
        error.details[0].path.as_deref(),
        Some("grouping[1].root.operations[0].outputs[0]")
    );
}

#[test]
fn search_resolver_accepts_distinct_root_output_names_across_groupings() {
    let request = resolve_search(
        &context(),
        match_all_search(vec![
            GroupingInput::Structured(root_output_grouping(
                Some("total_count"),
                GroupingAggregationIr::Count,
            )),
            GroupingInput::Structured(root_output_grouping(
                Some("average_price"),
                GroupingAggregationIr::Avg(GroupingExpressionIr::Attribute("price".to_owned())),
            )),
        ]),
    )
    .expect("distinct root names must remain valid");

    assert_eq!(request.grouping().len(), 2);
}

#[test]
fn search_resolver_does_not_compare_nested_group_outputs_across_groupings() {
    let mut context = context();
    context.definition.fields[0].capabilities.groupable = true;
    let grouping = || GroupingRequestIr {
        root: GroupingAllIr {
            operations: vec![
                GroupingOperationIr::Group(GroupingExpressionIr::Attribute("brand".to_owned())),
                GroupingOperationIr::Each(GroupingAllIr {
                    operations: vec![GroupingOperationIr::Output(vec![GroupingOutputIr {
                        label: None,
                        expression: GroupingExpressionIr::Aggregation(Box::new(
                            GroupingAggregationIr::Count,
                        )),
                    }])],
                    label: None,
                }),
            ],
            label: None,
        },
        continuations: Vec::new(),
    };

    resolve_search(
        &context,
        match_all_search(vec![
            GroupingInput::Structured(grouping()),
            GroupingInput::Structured(grouping()),
        ]),
    )
    .expect("nested group output names do not share the flattened root namespace");
}

#[test]
fn search_resolver_does_not_compare_facet_outputs_with_root_outputs() {
    let mut context = context();
    context.definition.fields[0].capabilities.groupable = true;
    let request = resolve_search(
        &context,
        match_all_search(vec![
            GroupingInput::Facet(FacetInput {
                field: "brand".to_owned(),
                name: None,
                limit: None,
                sort: None,
            }),
            GroupingInput::Structured(root_output_grouping(
                Some("count"),
                GroupingAggregationIr::Count,
            )),
        ]),
    )
    .expect("facet output names do not share the flattened root namespace");

    assert_eq!(request.grouping().len(), 2);
}

#[test]
fn search_resolver_rejects_summary_and_max_in_the_same_grouping_scope() {
    let mut context = context();
    context.definition.fields[0].capabilities.groupable = true;

    let grouping = GroupingRequestIr {
        root: GroupingAllIr {
            operations: vec![
                GroupingOperationIr::Group(GroupingExpressionIr::Attribute("brand".to_owned())),
                GroupingOperationIr::Each(GroupingAllIr {
                    operations: vec![
                        GroupingOperationIr::Max(Some(3)),
                        GroupingOperationIr::Output(vec![GroupingOutputIr {
                            label: Some("hits".to_owned()),
                            expression: GroupingExpressionIr::Aggregation(Box::new(
                                GroupingAggregationIr::Summary(Some("default".to_owned())),
                            )),
                        }]),
                    ],
                    label: None,
                }),
            ],
            label: None,
        },
        continuations: Vec::new(),
    };

    let error = resolve_search(
        &context,
        match_all_search(vec![GroupingInput::Structured(grouping)]),
    )
    .expect_err("summary and max must fail at the portable semantic boundary");
    assert_eq!(error.details[0].code, "grouping_summary_max_ambiguous");
}

#[test]
fn search_resolver_treats_group_and_each_as_one_semantic_scope() {
    let mut context = context();
    context.definition.fields[0].capabilities.groupable = true;

    let grouping = GroupingRequestIr {
        root: GroupingAllIr {
            operations: vec![
                GroupingOperationIr::Group(GroupingExpressionIr::Attribute("brand".to_owned())),
                GroupingOperationIr::Max(Some(3)),
                GroupingOperationIr::Each(GroupingAllIr {
                    operations: vec![GroupingOperationIr::Output(vec![GroupingOutputIr {
                        label: Some("hits".to_owned()),
                        expression: GroupingExpressionIr::Aggregation(Box::new(
                            GroupingAggregationIr::Summary(Some("default".to_owned())),
                        )),
                    }])],
                    label: None,
                }),
            ],
            label: None,
        },
        continuations: Vec::new(),
    };

    let error = resolve_search(
        &context,
        match_all_search(vec![GroupingInput::Structured(grouping)]),
    )
    .expect_err("group and each operations at one level must share max/summary semantics");
    assert_eq!(error.details[0].code, "grouping_summary_max_ambiguous");
}

#[test]
fn search_resolver_keeps_parallel_grouping_branches_semantically_valid() {
    let mut context = context();
    context.definition.fields[0].capabilities.groupable = true;

    let branch = || GroupingAllIr {
        operations: vec![
            GroupingOperationIr::Group(GroupingExpressionIr::Attribute("brand".to_owned())),
            GroupingOperationIr::Each(GroupingAllIr {
                operations: vec![GroupingOperationIr::Output(vec![GroupingOutputIr {
                    label: None,
                    expression: GroupingExpressionIr::Aggregation(Box::new(
                        GroupingAggregationIr::Count,
                    )),
                }])],
                label: None,
            }),
        ],
        label: None,
    };
    let grouping = GroupingRequestIr {
        root: GroupingAllIr {
            operations: vec![
                GroupingOperationIr::All(branch()),
                GroupingOperationIr::All(branch()),
            ],
            label: None,
        },
        continuations: Vec::new(),
    };

    resolve_search(
        &context,
        match_all_search(vec![GroupingInput::Structured(grouping)]),
    )
    .expect("parallel portable grouping branches remain semantically valid");
}

#[test]
fn facet_expansion_is_canonical_for_defaults_names_limits_and_sorts() {
    let mut facet_context = context();
    facet_context.definition.fields[0].capabilities.groupable = true;
    let cases = [
        (None, FacetSortInput::CountDesc),
        (Some(FacetSortInput::CountDesc), FacetSortInput::CountDesc),
        (Some(FacetSortInput::CountAsc), FacetSortInput::CountAsc),
        (Some(FacetSortInput::ValueAsc), FacetSortInput::ValueAsc),
        (Some(FacetSortInput::ValueDesc), FacetSortInput::ValueDesc),
    ];

    for (requested, expected) in cases {
        let input = FacetInput {
            field: "brand".to_owned(),
            name: Some("brands".to_owned()),
            limit: Some(0),
            sort: requested,
        };
        let grouping = facet_grouping_input(&input);
        assert_eq!(
            resolve_facets(&facet_context, vec![input.clone()]).unwrap(),
            vec![grouping.clone()],
            "facet shorthand must enter the same canonical grouping IR"
        );
        assert_eq!(grouping.root.label.as_deref(), Some("brands"));
        let [GroupingOperationIr::Group(GroupingExpressionIr::Attribute(field)), GroupingOperationIr::Each(each)] =
            grouping.root.operations.as_slice()
        else {
            panic!("facet expansion must use group + each");
        };
        assert_eq!(field, "brand");
        let [GroupingOperationIr::Max(Some(limit)), GroupingOperationIr::Order(order), GroupingOperationIr::Output(outputs)] =
            each.operations.as_slice()
        else {
            panic!("facet expansion must use max + order + output");
        };
        assert_eq!(*limit, 0);
        assert_eq!(outputs.len(), 1);
        assert!(matches!(
            outputs[0].expression,
            GroupingExpressionIr::Aggregation(ref aggregation)
                if matches!(aggregation.as_ref(), GroupingAggregationIr::Count)
        ));
        let [order] = order.as_slice() else {
            panic!("facet expansion must emit one order expression");
        };
        match expected {
            FacetSortInput::CountDesc => assert!(matches!(
                order,
                GroupingExpressionIr::Negative(value)
                    if matches!(
                        value.as_ref(),
                        GroupingExpressionIr::Aggregation(aggregation)
                            if matches!(aggregation.as_ref(), GroupingAggregationIr::Count)
                    )
            )),
            FacetSortInput::CountAsc => assert!(matches!(
                order,
                GroupingExpressionIr::Aggregation(aggregation)
                    if matches!(aggregation.as_ref(), GroupingAggregationIr::Count)
            )),
            FacetSortInput::ValueAsc => assert!(matches!(
                order,
                GroupingExpressionIr::Attribute(value) if value == "brand"
            )),
            FacetSortInput::ValueDesc => assert!(matches!(
                order,
                GroupingExpressionIr::Negative(value)
                    if matches!(
                        value.as_ref(),
                        GroupingExpressionIr::Attribute(field) if field == "brand"
                    )
            )),
        }
    }

    let defaults = facet_grouping_input(&FacetInput {
        field: "brand".to_owned(),
        name: None,
        limit: None,
        sort: None,
    });
    assert_eq!(
        resolve_facets(
            &facet_context,
            vec![FacetInput {
                field: "brand".to_owned(),
                name: None,
                limit: None,
                sort: None,
            }]
        )
        .unwrap(),
        vec![defaults.clone()]
    );
    assert_eq!(defaults.root.label.as_deref(), Some("brand"));
    let GroupingOperationIr::Each(each) = &defaults.root.operations[1] else {
        panic!("default facet expansion must contain each");
    };
    assert!(matches!(
        each.operations.first(),
        Some(GroupingOperationIr::Max(Some(10)))
    ));
}

#[test]
fn structured_facet_duplicates_share_the_canonical_duplicate_rule() {
    let mut context = context();
    context.definition.fields[0].capabilities.groupable = true;
    let facet = facet_grouping_input(&FacetInput {
        field: "brand".to_owned(),
        name: Some("brands".to_owned()),
        limit: None,
        sort: None,
    });

    let error = resolve_search(
        &context,
        match_all_search(vec![
            GroupingInput::Structured(facet.clone()),
            GroupingInput::Structured(facet),
        ]),
    )
    .expect_err("duplicate structured facets must fail like Rust facet shorthand");
    assert_eq!(error.details[0].code, "duplicate_facet_field");
}

#[test]
fn search_resolver_rejects_non_numeric_grouping_aggregations() {
    let context = context();
    let grouping = GroupingRequestIr {
        root: GroupingAllIr {
            operations: vec![GroupingOperationIr::Output(vec![GroupingOutputIr {
                label: None,
                expression: GroupingExpressionIr::Aggregation(Box::new(
                    GroupingAggregationIr::Avg(GroupingExpressionIr::Attribute("brand".to_owned())),
                )),
            }])],
            label: None,
        },
        continuations: Vec::new(),
    };

    let error = resolve_search(
        &context,
        match_all_search(vec![GroupingInput::Structured(grouping)]),
    )
    .expect_err("string aggregation expressions must be rejected in the canonical resolver");
    assert_eq!(error.details[0].code, "field_not_numeric");
}

#[test]
fn search_resolver_owns_portable_grouping_function_and_quantile_validation() {
    let context = context();
    let invalid_groupings = [
        (
            GroupingExpressionIr::Function {
                name: "not_supported".to_owned(),
                arguments: Vec::new(),
            },
            "unsupported_grouping_function",
        ),
        (
            GroupingExpressionIr::Aggregation(Box::new(GroupingAggregationIr::Quantiles {
                quantiles: vec![-12.0, f64::NAN, 500.0],
                expression: GroupingExpressionIr::Attribute("price".to_owned()),
            })),
            "invalid_quantile",
        ),
    ];

    for (expression, detail_code) in invalid_groupings {
        let grouping = GroupingRequestIr {
            root: GroupingAllIr {
                operations: vec![GroupingOperationIr::Group(expression)],
                label: None,
            },
            continuations: Vec::new(),
        };
        let error = resolve_search(
            &context,
            match_all_search(vec![GroupingInput::Structured(grouping)]),
        )
        .expect_err("portable grouping invariants must be enforced centrally");
        assert_eq!(error.details[0].code, detail_code);
    }
}
