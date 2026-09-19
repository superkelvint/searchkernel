use super::*;
use searchkernel_ir::*;
use searchkernel_vespa_engine::{
    ConfigCompiler, Engine, Field, FieldType, Query, Schema, SortField, Value as VespaValue,
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
        let mut operations = vec![GroupingOperationIr::Group(GroupingExpressionIr::Attribute(
            (*field).to_owned(),
        ))];
        if !outputs.is_empty() {
            operations.push(GroupingOperationIr::Output(
                outputs
                    .iter()
                    .cloned()
                    .map(|expression| GroupingOutputIr {
                        label: None,
                        expression,
                    })
                    .collect(),
            ));
        }
        if let Some(child) = nested {
            operations.push(GroupingOperationIr::Each(child));
        }
        nested = Some(GroupingAllIr {
            operations,
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
fn native_grouping_ir_lowers_to_linear_attribute_plan() {
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
fn unsupported_grouping_ir_is_not_approximated() {
    let request = native_grouping(&[(
        "category",
        vec![GroupingExpressionIr::Aggregation(Box::new(
            GroupingAggregationIr::Sum(GroupingExpressionIr::Attribute("price".into())),
        ))],
    )]);
    let error = lower_groupings(&[request]).expect_err("sum must remain unsupported");
    assert_eq!(error.code, searchkernel_ir::ErrorCode::Unsupported);
    assert_eq!(error.details[0].code, "grouping_aggregation_not_supported");

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
}

#[test]
fn analyzed_multi_term_text_lowers_to_a_phrase() {
    let search_field = || {
        vec![SearchFieldIr::new(
            field("title"),
            SearchFieldWeight::new(1.0).unwrap(),
        )]
    };
    for text in ["wireless headphones", "foo-bar", "can't"] {
        let query = lower_query(&QueryIr::Text(text.into()), &search_field()).unwrap();
        assert_eq!(query, Query::phrase("title", text));
    }

    let query = lower_query(&QueryIr::Text("headphones".into()), &search_field()).unwrap();
    assert_eq!(query, Query::term("title", "headphones"));
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
        Query::phrase("title", "wireless headphones")
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
        Query::Or(vec![
            Query::AttributeEquals {
                field: "brand".to_owned(),
                value: VespaValue::String("a".to_owned()),
            },
            Query::AttributeEquals {
                field: "brand".to_owned(),
                value: VespaValue::String("b".to_owned()),
            },
        ])
    );
}

#[test]
fn typed_query_c_lowers_specialized_nodes_and_retains_annotations_guard() {
    let annotated = QueryIr::Typed(Box::new(QueryExprIr {
        kind: QueryExprKindIr::True,
        annotations: std::collections::BTreeMap::from([(
            "ranked".to_owned(),
            RequestValueIr::Bool(true),
        )]),
    }));
    let error = lower_query(&annotated, &[]).unwrap_err();
    assert_eq!(
        error.details[0].code,
        "typed_query_annotations_not_supported"
    );

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

    let labeled_request = request(Vec::new(), Vec::new(), None).with_ranking_request(
        "default",
        std::collections::BTreeMap::from([("vespa.label.candidate.id".to_owned(), "7".to_owned())]),
    );
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
            unique_id: 7,
            score: 2.5,
        }
    );

    assert_eq!(
        lower_query(
            &typed_contains(
                "title",
                QueryExprKindIr::NonEmpty(Box::new(typed_term("wireless")))
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
                Query::term("title", "headphone"),
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
    assert!(matches!(
        lower_search_query(
            &user_query_request,
            searchkernel_vespa_engine::LinguisticsOptions::default(),
        )
        .unwrap(),
        Query::WeakAnd { children, .. }
            if children
                .iter()
                .all(|query| matches!(query, Query::Term { field, .. } if field == "title"))
    ));
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
            Field::new("price", FieldType::Integer)
                .attribute()
                .summary(),
        );
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile typed-query schema");
    let backend = VespaBackend::open(&data, &config).expect("open Vespa backend");

    for (id, title, price) in [
        ("doc:wireless", "wireless headphones", 20),
        ("doc:wired", "wired headphones", 5),
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

    let grouped = comparison.with_grouping(vec![native_grouping(&[(
        "price",
        vec![count_expression(), average_expression("price")],
    )])]);
    let result = backend.search(grouped).expect("execute native grouping");
    let ResultNode::Group(group) = result.root else {
        panic!("search root must be a group");
    };
    let [ResultNode::Hit(_), ResultNode::Group(grouping)] = group.children.as_slice() else {
        panic!("expected the hit and native grouping result");
    };
    assert_eq!(grouping.children.len(), 1);
    let ResultNode::Group(bucket) = &grouping.children[0] else {
        panic!("grouping result child must be a group");
    };
    assert_eq!(bucket.value, Some(Value::Int64(20)));
    assert_eq!(bucket.fields["count"], Value::Int64(1));
    assert_eq!(bucket.fields["average(price)"], Value::Float64(20.0));

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
        .expect("execute user-input query"));
    assert!(user_input_ids.contains(&"doc:ordered".to_owned()));

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
        .expect("execute user-query query"));
    assert!(user_query_ids.contains(&"doc:ordered".to_owned()));

    let labeled_ids = ids(backend
        .search(
            request(typed_contains(
                "title",
                QueryExprKindIr::Labeled {
                    query: Box::new(typed_term("wireless")),
                    label: "candidate".to_owned(),
                    score: 2.5,
                },
            ))
            .with_ranking_request(
                "default",
                std::collections::BTreeMap::from([(
                    "vespa.label.candidate.id".to_owned(),
                    "7".to_owned(),
                )]),
            ),
        )
        .expect("execute labeled query"));
    assert!(labeled_ids.contains(&"doc:ordered".to_owned()));

    let non_empty_ids = ids(backend
        .search(request(typed_contains(
            "title",
            QueryExprKindIr::NonEmpty(Box::new(typed_term("wireless"))),
        )))
        .expect("execute non-empty query"));
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
            DataTypeIr::Array(Box::new(DataTypeIr::Primitive(PrimitiveDataTypeIr::String))),
            None,
            FieldCapabilities::new(false, false, true, false, false),
        )],
        Vec::new(),
        RankingPolicyIr::DefaultRelevanceV1,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
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
            RankingPolicyIr::DefaultRelevanceV1,
            PrefixPolicy::Disabled,
            TypoPolicy::Disabled,
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
    let definition = IndexDefinitionIr::new(
        None,
        vec![value_field],
        Vec::new(),
        RankingPolicyIr::DefaultRelevanceV1,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    )
    .unwrap();
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
    let definition = IndexDefinitionIr::new(
        None,
        vec![contradictory],
        Vec::new(),
        RankingPolicyIr::DefaultRelevanceV1,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    )
    .unwrap();
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
    let definition = IndexDefinitionIr::new(
        None,
        vec![index_field],
        Vec::new(),
        RankingPolicyIr::DefaultRelevanceV1,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    )
    .unwrap();
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
    let definition = IndexDefinitionIr::new(
        None,
        vec![summary_field],
        Vec::new(),
        RankingPolicyIr::DefaultRelevanceV1,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    )
    .unwrap();
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
    let definition = IndexDefinitionIr::new(
        None,
        vec![title],
        Vec::new(),
        RankingPolicyIr::DefaultRelevanceV1,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    )
    .unwrap();
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
            RankingPolicyIr::DefaultRelevanceV1,
            PrefixPolicy::Disabled,
            TypoPolicy::Disabled,
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
        RankingPolicyIr::DefaultRelevanceV1,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
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
        Query::term("title", "fox")
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
            Query::phrase("title", "fox hound"),
            Query::phrase("body", "fox hound"),
        ])
    );
    assert!(lower_query(&QueryIr::Text("fox".into()), &[]).is_err());
    assert_eq!(
        lower_query(
            &QueryIr::Term {
                field: price.clone(),
                value: ScalarValueIr::Int64(10),
            },
            &[],
        )
        .unwrap(),
        Query::equals("price", 10)
    );
    assert_eq!(
        lower_query(
            &QueryIr::Term {
                field: title.clone(),
                value: ScalarValueIr::String("fox".into()),
            },
            &[],
        )
        .unwrap(),
        Query::term("title", "fox")
    );
    assert!(lower_query(
        &QueryIr::Term {
            field: price.clone(),
            value: ScalarValueIr::Float64(1.0),
        },
        &[],
    )
    .is_err());
    assert_eq!(
        lower_query(
            &QueryIr::And(vec![
                QueryIr::MatchAll,
                QueryIr::Not(Box::new(QueryIr::MatchAll,))
            ]),
            &[],
        )
        .unwrap(),
        Query::And(vec![Query::True, Query::Not(Box::new(Query::True))])
    );

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
fn search_validation_supports_facet_shaped_grouping_and_rejects_highlighting_precisely() {
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
        },
        None,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    );
    let error = validate_search_request(&highlighting).unwrap_err();
    assert_eq!(error.code, searchkernel_ir::ErrorCode::Unsupported);
    assert_eq!(
        error.details[0].code,
        "highlighting_native_result_unavailable"
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
        .rank_profile(
            searchkernel_vespa_engine::RankProfile::new("semantic")
                .query_input("query_embedding", "tensor<float>(x[2])")
                .first_phase("closeness(embedding)"),
        );
    ConfigCompiler
        .compile_to(&schema, &config)
        .expect("compile nearest-neighbor schema");
    let backend = VespaBackend::open(&data, &config).expect("open nearest-neighbor backend");

    for (id, values) in [("doc:near", vec![1.0, 0.0]), ("doc:far", vec![0.0, 1.0])] {
        backend
            .insert(InsertRequestIr::new(DocumentIr::new(
                DocumentId::parse(id).unwrap(),
                std::collections::BTreeMap::from([(
                    field("embedding"),
                    FieldValueIr::Tensor(RequestTensorIr {
                        type_spec: "tensor<float>(x[2])".to_owned(),
                        payload: RequestTensorPayloadIr::Dense(values),
                    }),
                )]),
            )))
            .expect("insert tensor document");
    }

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
    )]));
    let result = backend
        .search(request)
        .expect("execute nearest-neighbor query");
    let ResultNode::Group(group) = result.root else {
        panic!("search root must be a group");
    };
    let ids = group
        .children
        .iter()
        .map(|child| match child {
            ResultNode::Hit(hit) => hit.id.as_str(),
            ResultNode::Group(_) => panic!("unexpected nested group"),
        })
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["doc:near", "doc:far"]);
    assert!(matches!(
        group.children.as_slice(),
        [ResultNode::Hit(first), ResultNode::Hit(second)] if first.score > second.score
    ));

    backend
        .engine
        .close()
        .expect("close nearest-neighbor backend");
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
fn specialized_query_inputs_reject_unsupported_shapes_explicitly() {
    let nearest = typed_expression(QueryExprKindIr::NearestNeighbor {
        document_field: typed_field("embedding"),
        query_tensor: "query_embedding".to_owned(),
    });
    let missing = request(Vec::new(), Vec::new(), None).with_query(nearest.clone());
    let error = validate_search_request(&missing).unwrap_err();
    assert_eq!(error.details[0].code, "missing_query_input");

    let scalar = missing.with_inputs(std::collections::BTreeMap::from([(
        "query_embedding".to_owned(),
        QueryInputIr::Literal(RequestValueIr::Float64(1.0)),
    )]));
    let error = validate_search_request(&scalar).unwrap_err();
    assert_eq!(error.details[0].code, "nearest_neighbor_tensor_required");

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
        RankingPolicyIr::DefaultRelevanceV1,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
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
    assert!(config.contains("rankprofile[1].name \"product_search\""));
    assert!(config.contains("rankingExpression(product_search).rankingScript"));
    assert!(config.contains("bm25(title) + query(boost)"));
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

fn test_document(value: i64) -> searchkernel_ir::DocumentIr {
    searchkernel_ir::DocumentIr::new(
        searchkernel_ir::DocumentId::parse("doc-1").expect("valid document ID"),
        std::collections::BTreeMap::from([(
            field("value"),
            FieldValueIr::Single(ScalarValueIr::Int64(value)),
        )]),
    )
}
