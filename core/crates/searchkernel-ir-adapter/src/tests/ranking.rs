use super::*;

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
    definition.set_document_type("rank_profile_test");
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
    definition.set_document_type("two_phase_rank_profile_test");
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
