use super::*;

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
fn position_lowering_rejects_precision_vespa_cannot_round_trip() {
    let exact = FieldValueIr::Position {
        latitude: 37.774929,
        longitude: -122.419416,
    };
    assert_eq!(
        lower_field_value(&exact).expect("microdegree position should be representable"),
        FieldValue::Position(PositionValue::new(37.774929, -122.419416))
    );

    for (name, value) in [
        (
            "latitude",
            FieldValueIr::Position {
                latitude: 37.7749294,
                longitude: -122.419416,
            },
        ),
        (
            "longitude",
            FieldValueIr::Position {
                latitude: 37.774929,
                longitude: -122.4194164,
            },
        ),
    ] {
        let error = lower_field_value(&value)
            .expect_err("sub-microdegree position must fail before native mutation");
        assert_eq!(error.code, searchkernel_ir::ErrorCode::Unsupported);
        assert_eq!(error.details[0].code, "position_precision_not_supported");
        let expected_path = format!("document.value.{name}");
        assert_eq!(
            error.details[0].path.as_deref(),
            Some(expected_path.as_str())
        );
    }
}

#[test]
fn predicate_feature_map_lowering_rejects_unrepresentable_multi_key_values() {
    let single = FieldValueIr::Predicate {
        features: std::collections::BTreeMap::from([("hobby".to_owned(), "music".to_owned())]),
    };
    assert!(matches!(
        lower_field_value(&single),
        Ok(FieldValue::Predicate(_))
    ));

    for features in [
        std::collections::BTreeMap::new(),
        std::collections::BTreeMap::from([
            ("hobby".to_owned(), "music".to_owned()),
            ("region".to_owned(), "us".to_owned()),
        ]),
    ] {
        let error = lower_field_value(&FieldValueIr::Predicate { features })
            .expect_err("unrepresentable predicate feature maps must fail before native mutation");
        assert_eq!(error.code, searchkernel_ir::ErrorCode::Unsupported);
        assert_eq!(error.details[0].code, "predicate_feature_map_not_supported");
        assert_eq!(
            error.details[0].path.as_deref(),
            Some("document.value.features")
        );
    }

    assert!(lower_field_value(&FieldValueIr::PredicateExpression {
        expression:
            r#"{"type":1,"children":[{"type":4,"key":"hobby","feature_set":["music"]},{"type":4,"key":"region","feature_set":["us"]}]}"#
                .to_owned(),
    })
    .is_ok());
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
        assert_eq!(
            value_to_ir(&native).expect("supported native value"),
            expected
        );
    }
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
