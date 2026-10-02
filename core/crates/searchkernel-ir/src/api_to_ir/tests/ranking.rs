use super::*;

#[test]
fn rust_rank_profiles_preserve_phase_presence_and_second_phase_expression() {
    let definition = resolve_rust_index_definition_with_profiles(
        RustIndexDefinitionInput {
            name: Some("ranked".to_owned()),
            fields: vec![RustFieldInput {
                name: "title".to_owned(),
                kind: RustFieldKind::Text,
                multi: false,
                stored: None,
                searchable: None,
                filterable: None,
                sortable: None,
                facetable: None,
                analyzer: None,
                distance_metric: None,
                hnsw: None,
            }],
            default_search_fields: None,
        },
        vec![RustRankProfileInput {
            name: "custom".to_owned(),
            first_phase: None,
            second_phase: Some("attribute(score)".to_owned()),
            rerank_count: Some(7),
            match_features: Vec::new(),
            query_inputs: Vec::new(),
        }],
    )
    .expect("rank profile should resolve");

    let profile = &definition.rank_profiles()[0];
    assert_eq!(profile.first_phase, None);
    assert_eq!(
        profile
            .second_phase
            .as_ref()
            .and_then(|phase| phase.expression.as_deref()),
        Some("attribute(score)")
    );
    assert_eq!(
        profile
            .second_phase
            .as_ref()
            .and_then(|phase| phase.rerank_count),
        Some(7)
    );
}

#[test]
fn rust_rank_profile_empty_phase_matches_portable_normalization() {
    let rust_definition = resolve_rust_index_definition_with_profiles(
        RustIndexDefinitionInput {
            name: Some("ranked".to_owned()),
            fields: vec![RustFieldInput {
                name: "title".to_owned(),
                kind: RustFieldKind::Text,
                multi: false,
                stored: None,
                searchable: None,
                filterable: None,
                sortable: None,
                facetable: None,
                analyzer: None,
                distance_metric: None,
                hnsw: None,
            }],
            default_search_fields: None,
        },
        vec![RustRankProfileInput {
            name: "custom".to_owned(),
            first_phase: Some(String::new()),
            second_phase: None,
            rerank_count: None,
            match_features: Vec::new(),
            query_inputs: Vec::new(),
        }],
    )
    .expect("Rust rank profile should resolve");
    let bytes = crate::wire::v1::index_definition_ir_to_api(&rust_definition);
    let portable_definition = crate::wire::v1::index_definition_to_ir(
        crate::__flatbuffers::root::<api::IndexDefinition>(&bytes)
            .expect("encoded Rust definition should parse"),
    )
    .expect("portable rank profile should resolve");

    assert_eq!(rust_definition, portable_definition);
}

#[test]
fn rust_rank_profile_resolution_matches_portable_resolution() {
    let rust_definition = resolve_rust_index_definition_with_profiles(
        RustIndexDefinitionInput {
            name: Some("ranked".to_owned()),
            fields: vec![RustFieldInput {
                name: "title".to_owned(),
                kind: RustFieldKind::Text,
                multi: false,
                stored: None,
                searchable: None,
                filterable: None,
                sortable: None,
                facetable: None,
                analyzer: None,
                distance_metric: None,
                hnsw: None,
            }],
            default_search_fields: None,
        },
        vec![RustRankProfileInput {
            name: "custom".to_owned(),
            first_phase: Some("nativeRank(title)".to_owned()),
            second_phase: Some("attribute(score)".to_owned()),
            rerank_count: Some(7),
            match_features: vec!["nativeRank(title)".to_owned()],
            query_inputs: vec![("query_vector".to_owned(), "tensor<float>(x[3])".to_owned())],
        }],
    )
    .expect("Rust rank profile should resolve");
    let bytes = crate::wire::v1::index_definition_ir_to_api(&rust_definition);
    let portable_definition = crate::wire::v1::index_definition_to_ir(
        crate::__flatbuffers::root::<api::IndexDefinition>(&bytes)
            .expect("encoded Rust definition should parse"),
    )
    .expect("portable rank profile should resolve");

    assert_eq!(rust_definition, portable_definition);
}

#[test]
fn rust_rank_profile_rerank_count_without_second_phase_matches_portable_resolution() {
    let rust_definition = resolve_rust_index_definition_with_profiles(
        RustIndexDefinitionInput {
            name: Some("ranked".to_owned()),
            fields: vec![RustFieldInput {
                name: "title".to_owned(),
                kind: RustFieldKind::Text,
                multi: false,
                stored: None,
                searchable: None,
                filterable: None,
                sortable: None,
                facetable: None,
                analyzer: None,
                distance_metric: None,
                hnsw: None,
            }],
            default_search_fields: None,
        },
        vec![RustRankProfileInput {
            name: "custom".to_owned(),
            first_phase: None,
            second_phase: None,
            rerank_count: Some(7),
            match_features: Vec::new(),
            query_inputs: Vec::new(),
        }],
    )
    .expect("Rust rank profile should resolve");
    let bytes = crate::wire::v1::index_definition_ir_to_api(&rust_definition);
    let portable_definition = crate::wire::v1::index_definition_to_ir(
        crate::__flatbuffers::root::<api::IndexDefinition>(&bytes)
            .expect("encoded Rust definition should parse"),
    )
    .expect("portable rank profile should resolve");

    assert_eq!(rust_definition, portable_definition);
    let phase = portable_definition.rank_profiles()[0]
        .second_phase
        .as_ref()
        .expect("rerank count should retain second-phase presence");
    assert_eq!(phase.expression, None);
    assert_eq!(phase.rerank_count, Some(7));
}

#[test]
fn rust_rank_profile_validation_matches_portable_errors() {
    let schema = || RustIndexDefinitionInput {
        name: Some("ranked".to_owned()),
        fields: vec![RustFieldInput {
            name: "title".to_owned(),
            kind: RustFieldKind::Text,
            multi: false,
            stored: None,
            searchable: None,
            filterable: None,
            sortable: None,
            facetable: None,
            analyzer: None,
            distance_metric: None,
            hnsw: None,
        }],
        default_search_fields: None,
    };
    let profile = || RustRankProfileInput {
        name: "custom".to_owned(),
        first_phase: None,
        second_phase: None,
        rerank_count: None,
        match_features: Vec::new(),
        query_inputs: vec![("query_vector".to_owned(), "tensor<float>(x[3])".to_owned())],
    };
    let portable_error_code = |definition: IndexDefinitionIr| {
        let bytes = crate::wire::v1::index_definition_ir_to_api(&definition);
        crate::wire::v1::index_definition_to_ir(
            crate::__flatbuffers::root::<api::IndexDefinition>(&bytes)
                .expect("encoded definition should parse"),
        )
        .expect_err("portable definition should be rejected")
        .details[0]
            .code
            .clone()
    };

    let duplicate_profiles = vec![profile(), profile()];
    let rust_error = resolve_rust_index_definition_with_profiles(schema(), duplicate_profiles)
        .expect_err("duplicate Rust profiles should be rejected");
    let valid_profile = resolve_rust_index_definition_with_profiles(schema(), vec![profile()])
        .expect("single Rust profile should resolve")
        .rank_profiles()[0]
        .clone();
    let mut portable_definition = resolve_rust_index_definition(schema()).unwrap();
    portable_definition.set_rank_profiles(vec![valid_profile.clone(), valid_profile]);
    assert_eq!(rust_error.details[0].code, "duplicate_rank_profile");
    assert_eq!(
        rust_error.details[0].code,
        portable_error_code(portable_definition)
    );

    let duplicate_inputs = RustRankProfileInput {
        query_inputs: vec![
            ("query_vector".to_owned(), "tensor<float>(x[3])".to_owned()),
            ("query_vector".to_owned(), "tensor<float>(x[3])".to_owned()),
        ],
        ..profile()
    };
    let rust_error = resolve_rust_index_definition_with_profiles(schema(), vec![duplicate_inputs])
        .expect_err("duplicate Rust inputs should be rejected");
    let mut portable_profile =
        resolve_rust_index_definition_with_profiles(schema(), vec![profile()])
            .expect("single Rust profile should resolve")
            .rank_profiles()[0]
            .clone();
    portable_profile
        .inputs
        .push(portable_profile.inputs[0].clone());
    let mut portable_definition = resolve_rust_index_definition(schema()).unwrap();
    portable_definition.set_rank_profiles(vec![portable_profile]);
    assert_eq!(rust_error.details[0].code, "duplicate_rank_input");
    assert_eq!(
        rust_error.details[0].code,
        portable_error_code(portable_definition)
    );
}

#[test]
fn unknown_ranking_profile_is_rejected_before_backend_lowering() {
    let error = resolve_search(
        &context(),
        SearchIntent {
            query: QuerySourceInput::MatchAll,
            search_fields: None,
            filters: Vec::new(),
            textual_filters: Vec::new(),
            sorts: Vec::new(),
            grouping: Vec::new(),
            ranking: Some(RankingInput::Named("missing".to_owned())),
            query_inputs: BTreeMap::new(),
            rank_properties: BTreeMap::new(),
            query_model: QueryModelInput::default(),
            projection: None,
            highlighting: None,
            presentation: PresentationInput::default(),
            properties: BTreeMap::new(),
            query_profile: None,
            search_chain: None,
            timeout: None,
            limit: None,
            offset: None,
        },
    )
    .expect_err("unknown ranking profile must not reach Vespa fallback");

    assert_eq!(error.code, crate::ErrorCode::InvalidArgument);
    assert_eq!(error.details[0].code, "invalid_rank_profile");
}

#[test]
fn component_ranking_resolution_does_not_bypass_search_semantics() {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let profile = builder.create_string("");
    let ranking = api::RankingRequest::create(
        &mut builder,
        &api::RankingRequestArgs {
            profile: Some(profile),
            ..Default::default()
        },
    );
    builder.finish(ranking, None);
    let ranking_bytes = builder.finished_data().to_vec();
    let ranking = crate::__flatbuffers::root::<api::RankingRequest>(&ranking_bytes).unwrap();
    let direct = context()
        .ranking_request_to_ir(Some(ranking))
        .expect_err("empty named profiles must be rejected by component conversion");
    assert_eq!(direct.details[0].code, "invalid_ranking");

    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let match_all = api::MatchAllQuery::create(&mut builder, &api::MatchAllQueryArgs {});
    let source = api::QuerySource::create(
        &mut builder,
        &api::QuerySourceArgs {
            source_type: api::QuerySourceKind::MatchAllQuery,
            source: Some(match_all.as_union_value()),
        },
    );
    let profile = builder.create_string("");
    let ranking = api::RankingRequest::create(
        &mut builder,
        &api::RankingRequestArgs {
            profile: Some(profile),
            ..Default::default()
        },
    );
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(source),
            ranking: Some(ranking),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let request = crate::__flatbuffers::root::<api::SearchRequest>(builder.finished_data())
        .expect("valid search request");
    let error = context()
        .search_to_ir(request)
        .expect_err("full search resolution must reject an empty named profile");
    assert_eq!(error.details[0].code, "invalid_ranking");
}

#[test]
fn ranking_component_matches_full_search_for_empty_profile() {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let profile = builder.create_string("");
    let ranking = api::RankingRequest::create(
        &mut builder,
        &api::RankingRequestArgs {
            profile: Some(profile),
            ..Default::default()
        },
    );
    let ranking_bytes = {
        builder.finish(ranking, None);
        builder.finished_data().to_vec()
    };
    let ranking = crate::__flatbuffers::root::<api::RankingRequest>(&ranking_bytes).unwrap();

    let component_error = context()
        .ranking_request_to_ir(Some(ranking))
        .expect_err("empty named profiles must be rejected by component conversion");

    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let match_all = api::MatchAllQuery::create(&mut builder, &api::MatchAllQueryArgs {});
    let source = api::QuerySource::create(
        &mut builder,
        &api::QuerySourceArgs {
            source_type: api::QuerySourceKind::MatchAllQuery,
            source: Some(match_all.as_union_value()),
        },
    );
    let profile = builder.create_string("");
    let ranking = api::RankingRequest::create(
        &mut builder,
        &api::RankingRequestArgs {
            profile: Some(profile),
            ..Default::default()
        },
    );
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(source),
            ranking: Some(ranking),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let request = crate::__flatbuffers::root::<api::SearchRequest>(builder.finished_data())
        .expect("valid search request");
    let search_error = context()
        .search_to_ir(request)
        .expect_err("full search resolution must reject the same empty profile");

    assert_eq!(
        component_error.details[0].code,
        search_error.details[0].code
    );
    assert_eq!(
        component_error.details[0].path,
        search_error.details[0].path
    );
}
