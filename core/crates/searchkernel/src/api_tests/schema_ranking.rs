use super::*;

#[test]
fn describe_preserves_configured_rank_profiles() {
    let definition = IndexDefinition::new()
        .field(Field::text("title"))
        .rank_profile(
            crate::RankProfile::new("two_phase")
                .first_phase("bm25(title)")
                .second_phase("attribute(score)")
                .rerank_count(7)
                .match_feature("bm25(title)")
                .query_input("query_vector", "tensor<float>(x[3])"),
        );
    let (_runtime, index) = runtime_index(definition, Arc::new(RecordingBackend::new()));

    let described = index.describe().unwrap();
    assert_eq!(described.rank_profiles().len(), 1);
    let profile = &described.rank_profiles()[0];
    assert_eq!(profile.name(), "two_phase");
    assert_eq!(
        profile
            .first_phase
            .as_ref()
            .and_then(|phase| phase.expression.as_deref()),
        Some("bm25(title)")
    );
    assert_eq!(
        profile
            .second_phase
            .as_ref()
            .and_then(|phase| phase.expression.as_deref()),
        Some("attribute(score)")
    );
    assert_eq!(profile.rerank_count, Some(7));
    assert_eq!(profile.match_features, vec!["bm25(title)"]);
    assert_eq!(
        profile.query_inputs,
        vec![("query_vector".to_owned(), "tensor<float>(x[3])".to_owned())]
    );
}

#[test]
fn rank_profile_input_default_converges_across_rust_and_flatbuffers_ingress() {
    let canonical = canonical_fixture_ir();
    let expected = canonical.rank_profiles()[0].inputs[1].default_value.clone();

    let rust_definition = definition_from_ir(&canonical);
    let rust_ir = definition_to_ir(&rust_definition)
        .expect("Rust schema ingress should preserve canonical rank-profile defaults");

    let bytes = searchkernel_ir::wire::v1::index_definition_ir_to_api(&canonical);
    let root = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(&bytes)
        .expect("portable schema should decode");
    let portable_ir = searchkernel_ir::wire::v1::index_definition_to_ir(root)
        .expect("portable schema ingress should preserve canonical rank-profile defaults");

    assert_eq!(rust_ir.rank_profiles()[0].inputs[1].default_value, expected);
    assert_eq!(
        portable_ir.rank_profiles()[0].inputs[1].default_value,
        rust_ir.rank_profiles()[0].inputs[1].default_value
    );
}

#[test]
fn describe_preserves_rank_profile_inheritance() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(
        canonical.rank_profiles()[0].inherits,
        described.rank_profiles()[0].inherits
    );
}

#[test]
fn describe_preserves_rank_profile_inputs_and_defaults() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(
        canonical.rank_profiles()[0].inputs,
        described.rank_profiles()[0].inputs
    );
}

#[test]
fn describe_preserves_rank_profile_functions() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(
        canonical.rank_profiles()[0].functions,
        described.rank_profiles()[0].functions
    );
}

#[test]
fn describe_preserves_rank_profile_constants() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(
        canonical.rank_profiles()[0].constants,
        described.rank_profiles()[0].constants
    );
}

#[test]
fn describe_preserves_rank_profile_onnx_models() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(
        canonical.rank_profiles()[0].onnx_models,
        described.rank_profiles()[0].onnx_models
    );
}

#[test]
fn described_rank_profile_exposes_phase_presence_and_controls() {
    let (canonical, described) = public_described_canonical_fixture();
    let expected = &canonical.rank_profiles()[0];
    let actual = &described.rank_profiles()[0];

    assert_eq!(
        actual
            .first_phase
            .as_ref()
            .map(crate::schema::RankPhase::to_ir),
        expected.first_phase
    );
    assert_eq!(
        actual
            .second_phase
            .as_ref()
            .map(crate::schema::RankPhase::to_ir),
        expected.second_phase
    );
    assert_eq!(
        actual
            .global_phase
            .as_ref()
            .map(crate::schema::RankPhase::to_ir),
        expected.global_phase
    );
    assert!(described.rank_profiles()[2].first_phase.is_none());
    assert!(described.rank_profiles()[2].second_phase.is_none());
    assert!(described.rank_profiles()[2].global_phase.is_none());
}

#[test]
fn described_rank_profile_exposes_feature_and_property_metadata() {
    let (canonical, described) = public_described_canonical_fixture();
    let expected = &canonical.rank_profiles()[0];
    let actual = &described.rank_profiles()[0];

    assert_eq!(actual.rank_features, expected.rank_features);
    assert_eq!(actual.match_features, expected.match_features);
    assert_eq!(actual.summary_features, expected.summary_features);
    assert_eq!(actual.sort_features, expected.sort_features);
    assert_eq!(actual.rank_properties, expected.rank_properties);
    assert_eq!(actual.strict, expected.strict);
    assert_eq!(
        crate::schema::properties_to_ir(&actual.properties),
        expected.properties
    );
}

#[test]
fn public_rank_profile_phase_state_round_trips_to_ir() {
    let phase = |expression, rerank_count, rank_score_drop_limit, total_rerank_count| {
        crate::schema::RankPhase {
            expression,
            rerank_count,
            rank_score_drop_limit,
            properties: BTreeMap::new(),
            total_rerank_count,
            keep_rank_count: None,
            total_keep_rank_count: None,
        }
    };
    let mut profile = RankProfile::new("phases");
    profile.first_phase = Some(phase(Some("first".to_owned()), None, None, None));
    profile.second_phase = Some(phase(None, None, None, Some(11)));
    profile.global_phase = Some(phase(None, None, Some(0.5), None));
    let expected_first = profile.first_phase.clone();
    let expected_second = profile.second_phase.clone();
    let expected_global = profile.global_phase.clone();

    let resolved = definition_to_ir(
        &IndexDefinition::new()
            .field(Field::text("title"))
            .rank_profile(profile),
    )
    .expect("public rank-profile phases should resolve");
    let resolved = &resolved.rank_profiles()[0];

    assert_eq!(
        resolved.first_phase,
        expected_first.as_ref().map(crate::schema::RankPhase::to_ir)
    );
    assert_eq!(
        resolved.second_phase,
        expected_second
            .as_ref()
            .map(crate::schema::RankPhase::to_ir)
    );
    assert_eq!(
        resolved.global_phase,
        expected_global
            .as_ref()
            .map(crate::schema::RankPhase::to_ir)
    );
}

#[test]
fn public_rank_profile_metadata_round_trips_to_ir() {
    let mut profile = RankProfile::new("edited");
    profile.rank_features = vec!["rank_feature".to_owned()];
    profile.match_features = vec!["match_feature".to_owned()];
    profile.summary_features = vec!["summary_feature".to_owned()];
    profile.sort_features = vec!["sort_feature".to_owned()];
    profile.rank_properties = BTreeMap::from([("rank.property".to_owned(), "value".to_owned())]);
    profile.strict = Some(true);
    profile.properties = BTreeMap::from([(
        "generic.property".to_owned(),
        crate::schema::SchemaValue::String("value".to_owned()),
    )]);

    let resolved = definition_to_ir(
        &IndexDefinition::new()
            .field(Field::text("title"))
            .rank_profile(profile),
    )
    .expect("public rank-profile metadata should resolve");
    let resolved = &resolved.rank_profiles()[0];

    assert_eq!(resolved.rank_features, ["rank_feature"]);
    assert_eq!(resolved.match_features, ["match_feature"]);
    assert_eq!(resolved.summary_features, ["summary_feature"]);
    assert_eq!(resolved.sort_features, ["sort_feature"]);
    assert_eq!(resolved.rank_properties["rank.property"], "value");
    assert_eq!(resolved.strict, Some(true));
    assert_eq!(
        resolved.properties["generic.property"],
        searchkernel_ir::RequestValueIr::String("value".to_owned())
    );
}

#[test]
fn described_rank_profile_exposes_inheritance_and_functions_publicly() {
    let (canonical, described) = public_described_canonical_fixture();
    let actual = &described.rank_profiles()[0];
    let expected = &canonical.rank_profiles()[0];
    assert_eq!(actual.inherits, expected.inherits);
    assert_eq!(
        actual
            .functions
            .iter()
            .map(crate::schema::RankFunction::to_ir)
            .collect::<Vec<_>>(),
        expected.functions
    );
}

#[test]
fn public_rank_profile_configuration_round_trips_without_canonical_sidecar() {
    let (canonical, described) = public_described_canonical_fixture();
    let profile = described.rank_profiles()[0].clone();
    let resolved = definition_to_ir(
        &IndexDefinition::new()
            .field(Field::text("title"))
            .rank_profile(profile),
    )
    .expect("profile should resolve");
    assert_eq!(
        resolved.rank_profiles()[0].inherits,
        canonical.rank_profiles()[0].inherits
    );
    assert_eq!(
        resolved.rank_profiles()[0].functions,
        canonical.rank_profiles()[0].functions
    );
}

#[test]
fn described_rank_profile_exposes_model_state_publicly() {
    let (canonical, described) = public_described_canonical_fixture();
    let expected = &canonical.rank_profiles()[0];
    let actual = &described.rank_profiles()[0];
    assert_eq!(
        actual
            .inputs
            .iter()
            .map(crate::schema::RankProfileInput::to_ir)
            .collect::<Vec<_>>(),
        expected.inputs
    );
    assert_eq!(
        actual
            .constants
            .iter()
            .map(crate::schema::ConstantDefinition::to_ir)
            .collect::<Vec<_>>(),
        expected.constants
    );
    assert_eq!(
        actual
            .onnx_models
            .iter()
            .map(crate::schema::OnnxModel::to_ir)
            .collect::<Vec<_>>(),
        expected.onnx_models
    );
}

#[test]
fn public_rank_profile_model_state_round_trips_without_sidecar() {
    let (canonical, described) = public_described_canonical_fixture();
    let profile = described.rank_profiles()[0].clone();
    let resolved = definition_to_ir(
        &IndexDefinition::new()
            .field(Field::text("title"))
            .rank_profile(profile),
    )
    .expect("profile");
    assert_eq!(
        resolved.rank_profiles()[0].inputs,
        canonical.rank_profiles()[0].inputs
    );
    assert_eq!(
        resolved.rank_profiles()[0].constants,
        canonical.rank_profiles()[0].constants
    );
    assert_eq!(
        resolved.rank_profiles()[0].onnx_models,
        canonical.rank_profiles()[0].onnx_models
    );
}

#[test]
fn cloning_described_rich_rank_profile_preserves_its_public_schema_state() {
    let (canonical, described) = public_described_canonical_fixture();
    let extracted = described.rank_profiles()[0].clone();
    let resolved = definition_to_ir(
        &IndexDefinition::new()
            .field(Field::text("title"))
            .rank_profile(extracted),
    )
    .expect("cloned rank profile should resolve");

    assert_eq!(canonical.rank_profiles()[0], resolved.rank_profiles()[0]);
}

#[test]
fn mutating_a_described_rank_profile_preserves_untouched_canonical_state() {
    let (canonical, described) = public_described_canonical_fixture();
    let modified = described.rank_profiles()[0]
        .clone()
        .first_phase("new_rank_expression")
        .match_feature("new_feature");
    assert_eq!(modified.inherits, canonical.rank_profiles()[0].inherits);
    assert_eq!(
        modified
            .functions
            .iter()
            .map(crate::schema::RankFunction::to_ir)
            .collect::<Vec<_>>(),
        canonical.rank_profiles()[0].functions
    );
    assert_eq!(
        modified
            .constants
            .iter()
            .map(crate::schema::ConstantDefinition::to_ir)
            .collect::<Vec<_>>(),
        canonical.rank_profiles()[0].constants
    );
    assert_eq!(
        modified
            .first_phase
            .as_ref()
            .and_then(|phase| phase.expression.as_deref()),
        Some("new_rank_expression")
    );
    assert!(modified
        .match_features
        .iter()
        .any(|feature| feature == "new_feature"));

    let resolved = definition_to_ir(
        &IndexDefinition::new()
            .field(Field::text("title"))
            .rank_profile(modified),
    )
    .expect("mutated profile should resolve");
    assert_eq!(
        resolved.rank_profiles()[0]
            .first_phase
            .as_ref()
            .and_then(|phase| phase.expression.as_deref()),
        Some("new_rank_expression")
    );
    assert!(resolved.rank_profiles()[0]
        .match_features
        .iter()
        .any(|feature| feature == "new_feature"));
}

#[test]
fn empty_named_ranking_has_identical_rust_and_portable_semantics() {
    let definition = IndexDefinition::new().field(Field::text("title"));
    let context = context_for(&definition);
    let (_runtime, index) = runtime_index(definition, Arc::new(RecordingBackend::new()));

    let rust_error = index
        .search("x")
        .ranking(Ranking::named(""))
        .inspect()
        .expect_err("empty Rust ranking profile should be invalid");
    let bytes = portable_text_request_with_empty_controls(Some(""), None, None);
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let portable_error = searchkernel_ir::wire::v1::search_to_ir(&context, request)
        .expect_err("empty portable ranking profile must not become the default");

    assert_eq!(rust_error.details()[0].code, "invalid_ranking");
    assert_eq!(portable_error.details[0].code, "invalid_ranking");
}

#[cfg(feature = "native")]
#[cfg(feature = "native")]
#[test]
fn public_rank_profile_preserves_second_phase_and_rerank_count() {
    let definition = IndexDefinition::new()
        .field(Field::int64("rating"))
        .field(Field::int64("inventory"))
        .rank_profile(
            RankProfile::new("two_phase")
                .first_phase("attribute(rating)")
                .second_phase("attribute(inventory)")
                .rerank_count(3),
        );

    let resolved = definition_to_ir(&definition).unwrap();
    let profile = &resolved.rank_profiles()[0];
    assert_eq!(
        profile.first_phase.as_ref().unwrap().expression.as_deref(),
        Some("attribute(rating)")
    );
    let second_phase = profile.second_phase.as_ref().unwrap();
    assert_eq!(
        second_phase.expression.as_deref(),
        Some("attribute(inventory)")
    );
    assert_eq!(second_phase.rerank_count, Some(3));
}
#[cfg(feature = "native")]
#[cfg(feature = "native")]
#[test]
fn public_describe_preserves_rank_profiles_from_resolved_ir() {
    let definition = IndexDefinition::new()
        .field(Field::text("title"))
        .rank_profile(
            RankProfile::new("two_phase")
                .first_phase("bm25(title)")
                .second_phase("attribute(score)")
                .rerank_count(7)
                .match_feature("bm25(title)")
                .query_input("query_vector", "tensor<float>(x[3])"),
        );
    let index = IndexRuntime::from_backend(definition, Arc::new(RecordingBackend::new())).unwrap();

    let described = index.describe().unwrap();
    assert_eq!(described.rank_profiles().len(), 1);
    let profile = &described.rank_profiles()[0];
    assert_eq!(profile.name(), "two_phase");
    assert_eq!(
        profile
            .first_phase
            .as_ref()
            .and_then(|phase| phase.expression.as_deref()),
        Some("bm25(title)")
    );
    assert_eq!(
        profile
            .second_phase
            .as_ref()
            .and_then(|phase| phase.expression.as_deref()),
        Some("attribute(score)")
    );
    assert_eq!(profile.rerank_count, Some(7));
    assert_eq!(profile.match_features, vec!["bm25(title)"]);
    assert_eq!(
        profile.query_inputs,
        vec![("query_vector".to_owned(), "tensor<float>(x[3])".to_owned())]
    );
}
