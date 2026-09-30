use super::*;

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
fn analyze_language_is_explicitly_supported_or_rejected() {
    let english = LanguageHint::parse("en-US").unwrap();
    assert!(analysis_options(Some(&english)).is_ok());

    let french = LanguageHint::parse("fr").unwrap();
    let error = analysis_options(Some(&french)).unwrap_err();
    assert_eq!(error.code, searchkernel_ir::ErrorCode::Unsupported);
    assert_eq!(error.details[0].code, "language_not_supported");
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
