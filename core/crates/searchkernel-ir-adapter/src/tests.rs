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
        RankingPolicyIr::DefaultRelevanceV1,
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
    assert_eq!(error.details[0].code, "grouping_not_supported");
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
        RankingPolicyIr::DefaultRelevanceV1,
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
        RankingPolicyIr::DefaultRelevanceV1,
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
    assert_eq!(
        ranking_profile(RankingPolicyIr::Bm25).unwrap_err().code,
        searchkernel_ir::ErrorCode::Unsupported
    );

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
        RankingPolicyIr::DefaultRelevanceV1,
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

    assert_eq!(
        ranking_profile(RankingPolicyIr::Bm25).unwrap_err().details[0].code,
        "bm25_ranking_not_supported"
    );
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
