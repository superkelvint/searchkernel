use super::*;

#[test]
fn filter_and_typed_membership_share_unsupported_value_and_sentinel_errors() {
    for (scalar, typed) in [
        (ScalarValueIr::Bool(true), RequestValueIr::Bool(true)),
        (ScalarValueIr::Float64(1.5), RequestValueIr::Float64(1.5)),
        (
            ScalarValueIr::Int64(i64::MIN),
            RequestValueIr::Int64(i64::MIN),
        ),
    ] {
        let filter_error = lower_filter(&FilterIr::In {
            field: field("member"),
            values: vec![scalar],
        })
        .unwrap_err();
        let typed_error = lower_query(
            &typed_expression(QueryExprKindIr::In {
                field: typed_field("member"),
                values: vec![typed],
            }),
            &[],
        )
        .unwrap_err();
        assert_eq!(filter_error, typed_error);
        assert_eq!(filter_error.code, ErrorCode::Unsupported);
        assert_eq!(filter_error.details[0].code, "m_lowering");
    }
}

#[test]
fn membership_validation_remains_at_the_canonical_boundary() {
    let definition = IndexDefinitionIr::new(
        None,
        vec![
            FieldIr::new(
                field("qty"),
                DataTypeIr::Primitive(PrimitiveDataTypeIr::Int64),
                None,
                FieldCapabilities::new(false, false, true, false, false),
            ),
            FieldIr::new(
                field("price"),
                DataTypeIr::Primitive(PrimitiveDataTypeIr::Float64),
                None,
                FieldCapabilities::new(false, false, true, false, false),
            ),
            FieldIr::new(
                field("active"),
                DataTypeIr::Primitive(PrimitiveDataTypeIr::Bool),
                None,
                FieldCapabilities::new(false, false, true, false, false),
            ),
        ],
        Vec::new(),
    )
    .unwrap();
    let context = IndexContext::from_definition_ir(definition);
    for (name, values, detail) in [
        ("qty", Vec::new(), "empty_in"),
        (
            "qty",
            vec![ScalarValueIr::Int64(1), ScalarValueIr::String("two".into())],
            "type_mismatch",
        ),
        ("price", vec![ScalarValueIr::Float64(1.0)], "invalid_filter"),
        ("active", vec![ScalarValueIr::Bool(true)], "invalid_filter"),
    ] {
        let request = SearchRequestIr::new(
            QueryIr::MatchAll,
            Vec::new(),
            Some(FilterIr::In {
                field: field(name),
                values,
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
        let bytes = searchkernel_ir::wire::v1::search_request_ir_to_api(&request);
        let decoded = searchkernel_ir::__flatbuffers::root::<
            searchkernel_ir::wire::v1::schema::SearchRequest,
        >(&bytes)
        .unwrap();
        let error = searchkernel_ir::wire::v1::search_to_ir(&context, decoded).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidArgument);
        assert_eq!(error.details[0].code, detail);
        assert!(error.details[0].path.is_some());
    }
}

#[test]
fn filter_membership_preserves_the_same_native_in_node_as_typed_queries() {
    use searchkernel_vespa_engine::InQueryValue;

    for (kind, source, values, typed_values, native_values) in [
        (
            PrimitiveDataTypeIr::String,
            "member IN ('Acme', 'TrailCo')",
            vec![
                ScalarValueIr::String("Acme".into()),
                ScalarValueIr::String("TrailCo".into()),
            ],
            vec![
                RequestValueIr::String("Acme".into()),
                RequestValueIr::String("TrailCo".into()),
            ],
            vec![
                InQueryValue::String("Acme".into()),
                InQueryValue::String("TrailCo".into()),
            ],
        ),
        (
            PrimitiveDataTypeIr::Int64,
            "member IN (10, 20)",
            vec![ScalarValueIr::Int64(10), ScalarValueIr::Int64(20)],
            vec![RequestValueIr::Int64(10), RequestValueIr::Int64(20)],
            vec![InQueryValue::Integer(10), InQueryValue::Integer(20)],
        ),
    ] {
        let mut member = FieldIr::new(
            field("member"),
            DataTypeIr::Primitive(kind),
            None,
            FieldCapabilities::new(false, false, true, false, false),
        );
        if kind == PrimitiveDataTypeIr::String {
            member.set_matching_config(Some(MatchingConfigIr {
                mode: MatchingModeIr::Exact,
                options: BTreeMap::new(),
            }));
        }
        let definition = IndexDefinitionIr::new(None, vec![member], Vec::new()).unwrap();
        let structured = FilterIr::In {
            field: field("member"),
            values,
        };
        let textual =
            bind_filter_expression(source, &definition, &FilterParameters::new()).unwrap();
        assert_eq!(textual, structured);
        let expected = Query::In {
            field: "member".into(),
            values: native_values,
        };
        let typed = typed_expression(QueryExprKindIr::In {
            field: typed_field("member"),
            values: typed_values,
        });
        assert_eq!(lower_query(&typed, &[]).unwrap(), expected);
        assert_eq!(lower_filter(&structured).unwrap(), expected);
        assert_eq!(lower_filter(&textual).unwrap(), expected);
    }
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
