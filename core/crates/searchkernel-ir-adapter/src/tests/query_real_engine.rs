use super::*;

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
        .field(Field::new("qty", FieldType::Long).attribute().summary())
        .field(Field::new("price", FieldType::Long).attribute().summary());
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
        let bytes = searchkernel_ir::wire::v1::search_request_ir_to_api(&request);
        let api_request = searchkernel_ir::__flatbuffers::root::<
            searchkernel_ir::wire::v1::schema::SearchRequest,
        >(&bytes)
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
