use super::*;

#[test]
fn public_schema_fields_round_trip_to_ir() {
    let (canonical, described) = public_described_canonical_fixture();
    let resolved = definition_to_ir(&described).expect("public schema fields should resolve");

    assert_eq!(resolved.schema_fields(), canonical.schema_fields());
}

#[test]
fn schema_field_builder_adds_schema_only_field_to_ir() {
    let definition = IndexDefinition::new()
        .field(Field::text("title"))
        .schema_field(Field::text("derived"));
    let resolved = definition_to_ir(&definition).expect("schema field should resolve");

    assert_eq!(
        resolved
            .schema_fields()
            .iter()
            .map(|field| field.name().as_str())
            .collect::<Vec<_>>(),
        ["derived"]
    );
}

#[test]
fn canonical_index_definition_round_trips_through_flatbuffers_without_loss() {
    let canonical = canonical_fixture_ir();
    let bytes = searchkernel_ir::index_definition_ir_to_api(&canonical);
    let root = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(&bytes)
        .expect("encoded canonical fixture should parse");
    let round_tripped = searchkernel_ir::wire::v1::index_definition_to_ir(root)
        .expect("encoded canonical fixture should resolve");
    assert_eq!(canonical, round_tripped);
}

#[cfg(feature = "native")]
#[test]
fn portable_definition_encoding_uses_public_state_without_a_canonical_sidecar() {
    let (_canonical, mut described) = public_described_canonical_fixture();
    described.default_search_fields = Some(vec![SearchField::new("title").weight(0.125)]);
    let described_bytes = crate::portable_definition::encode_portable_definition(&described);
    let root = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(&described_bytes)
        .expect("portable definition should parse");
    let encoded = searchkernel_ir::wire::v1::index_definition_to_ir(root)
        .expect("portable definition should resolve");

    assert_eq!(encoded.default_search_fields()[0].weight().get(), 0.125);
}

#[test]
fn portable_definition_round_trips_all_public_schema_state() {
    let (canonical, described) = public_described_canonical_fixture();
    let bytes = crate::portable_definition::encode_portable_definition(&described);
    let root = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(&bytes)
        .expect("portable definition should parse");
    let encoded = searchkernel_ir::wire::v1::index_definition_to_ir(root)
        .expect("portable definition should resolve");

    assert_eq!(encoded, canonical);
}

#[test]
fn cloned_described_fields_round_trip_data_types_without_sidecar_state() {
    let (canonical, described) = public_described_canonical_fixture();
    for name in [
        "byte_value",
        "count32",
        "ratio32",
        "payload",
        "scores",
        "parent",
    ] {
        let field = described
            .fields()
            .iter()
            .find(|field| field.name() == name)
            .expect("described field should exist")
            .clone();
        let resolved = definition_to_ir(&IndexDefinition::new().field(field))
            .expect("cloned public field should resolve");
        let expected = canonical
            .fields()
            .iter()
            .find(|field| field.name().as_str() == name)
            .expect("fixture field should exist");
        assert_eq!(
            resolved.fields()[0].data_type(),
            expected.data_type(),
            "field {name}"
        );
    }
}

#[test]
fn cloned_described_field_round_trips_capabilities_without_sidecar_state() {
    let (canonical, described) = public_described_canonical_fixture();
    let expected = canonical
        .fields()
        .iter()
        .find(|field| field.name().as_str() == "location")
        .expect("canonical location field should exist");
    let field = described
        .fields()
        .iter()
        .find(|field| field.name() == "location")
        .expect("described location field should exist")
        .clone();

    let resolved = definition_to_ir(&IndexDefinition::new().field(field))
        .expect("public capabilities should resolve without sidecar state");
    let actual = &resolved.fields()[0];

    assert_eq!(actual.capabilities(), expected.capabilities());
}

#[test]
fn public_field_configurations_round_trip_without_canonical_sidecar() {
    let (canonical, described) = public_described_canonical_fixture();
    let field = described
        .fields()
        .iter()
        .find(|field| field.name() == "embedding")
        .expect("embedding field should be described")
        .clone();

    let resolved = definition_to_ir(&IndexDefinition::new().field(field))
        .expect("public field configuration should resolve");
    let expected = canonical
        .fields()
        .iter()
        .find(|field| field.name().as_str() == "embedding")
        .expect("canonical embedding field should exist");
    let actual = &resolved.fields()[0];

    assert_eq!(actual.index(), expected.index());
    assert_eq!(actual.attribute(), expected.attribute());
}

#[test]
fn public_document_summaries_round_trip_without_canonical_sidecar() {
    let (canonical, described) = public_described_canonical_fixture();
    let resolved = definition_to_ir(&described).expect("public document summaries should resolve");
    assert_eq!(
        resolved.document_summaries(),
        canonical.document_summaries()
    );
}

#[test]
fn field_to_ir_preserves_struct_field_configuration_without_canonical_sidecar() {
    let (canonical, described) = public_described_canonical_fixture();
    let field = described.fields()[0].clone();
    let resolved =
        definition_to_ir(&IndexDefinition::new().field(field)).expect("field should resolve");
    assert_eq!(
        resolved.fields()[0].struct_fields(),
        canonical.fields()[0].struct_fields()
    );
}

#[test]
fn public_field_rich_state_round_trips_without_sidecar() {
    let (canonical, described) = public_described_canonical_fixture();
    let field = described.fields()[0].clone();
    let resolved = definition_to_ir(&IndexDefinition::new().field(field)).expect("field");
    assert_eq!(
        resolved.fields()[0].summary(),
        canonical.fields()[0].summary()
    );
    assert_eq!(
        resolved.fields()[0].indexing_script(),
        canonical.fields()[0].indexing_script()
    );
    assert_eq!(
        resolved.fields()[0].properties(),
        canonical.fields()[0].properties()
    );
}

#[test]
fn cloning_described_rich_field_preserves_its_public_schema_state() {
    let (canonical, described) = public_described_canonical_fixture();
    let extracted = described.fields()[0].clone();
    let resolved = definition_to_ir(&IndexDefinition::new().field(extracted))
        .expect("cloned field should resolve");

    assert_eq!(canonical.fields()[0], resolved.fields()[0]);
}

#[test]
fn described_fields_round_trip_from_public_state_without_field_sidecars() {
    let (canonical, described) = public_described_canonical_fixture();

    for (index, expected) in canonical.fields().iter().enumerate() {
        let public = described.fields()[index].clone();

        let mut standalone = IndexDefinition::new().field(public);
        // A StructReference is not a self-contained field type: its named
        // struct declaration is schema context, not hidden Field sidecar state.
        standalone.structs = described.structs().to_vec();
        let resolved = definition_to_ir(&standalone)
            .expect("public field state should resolve without a canonical sidecar");

        assert_eq!(
            expected,
            &resolved.fields()[0],
            "field {} must round-trip from public Field state plus required schema context",
            expected.name().as_str()
        );
    }
}

#[test]
fn mutating_a_described_field_is_revalidated_against_untouched_rich_state() {
    let (_, described) = public_described_canonical_fixture();
    let modified = described.fields()[0].clone().stored(false);

    let error = definition_to_ir(&IndexDefinition::new().field(modified))
        .expect_err("capability mutation must be checked against retained summary configuration");
    assert_eq!(error.details()[0].code, "invalid_capability");
    assert_eq!(error.details()[0].path.as_deref(), Some("fields[0].stored"));
}

#[test]
fn modifying_described_definition_re_resolves_public_builder_state() {
    let definition = IndexDefinition::new().field(Field::text("title"));
    let (_runtime, index) = runtime_index(definition, Arc::new(RecordingBackend::new()));

    let described = index.describe().expect("describe should succeed");
    let modified = described.name("renamed");
    let resolved = definition_to_ir(&modified).expect("modified schema should resolve");

    assert_eq!(resolved.name().map(|name| name.as_str()), Some("renamed"));
}

#[test]
fn modifying_rich_described_definition_preserves_untouched_canonical_state() {
    let canonical = canonical_fixture_ir();
    let index = index_from_canonical_ir(canonical);
    let described = index.describe().expect("describe should succeed");
    let modified = described.name("renamed");
    let resolved = definition_to_ir(&modified).expect("modified schema should resolve");
    let expected = canonical_fixture_ir_named("renamed", 0.75, 0.25);

    assert_eq!(expected, resolved);
}

#[test]
fn anti_sidecar_acceptance_guard_uses_public_definition_state() {
    let (canonical, mut described) = public_described_canonical_fixture();
    described.default_search_fields = Some(vec![SearchField::new("title").weight(0.125)]);

    let resolved = definition_to_ir(&described).expect("public definition should resolve");

    assert_ne!(
        resolved.default_search_fields(),
        canonical.default_search_fields(),
        "Rust-to-IR conversion must not restore default search fields from the hidden definition sidecar"
    );
    assert_eq!(resolved.default_search_fields()[0].weight().get(), 0.125);
}

#[test]
fn describe_round_trips_complete_index_definition_without_semantic_loss() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(canonical, described);
}

#[test]
fn rich_public_field_state_is_validated_by_shared_resolver() {
    let (_, mut described) = public_described_canonical_fixture();
    described.fields[0]
        .attribute_config
        .as_mut()
        .expect("fixture field should have attribute configuration")
        .fast_search = Some(true);

    let error = definition_to_ir(&described)
        .expect_err("rich field state must be validated before final IR construction");
    assert_eq!(error.code(), crate::ErrorCode::InvalidArgument);
}

#[test]
fn rich_public_definition_matches_portable_without_canonical_sidecar() {
    let (canonical, described) = public_described_canonical_fixture();

    let resolved = definition_to_ir(&described)
        .expect("complete public schema state should use the shared resolver");
    assert_eq!(canonical, resolved);
}
