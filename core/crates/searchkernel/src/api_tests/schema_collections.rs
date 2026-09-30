use super::*;

#[test]
fn described_public_values_expose_fieldset_definitions() {
    let (canonical, described) = public_described_canonical_fixture();
    let actual = described
        .fieldsets()
        .iter()
        .map(crate::schema::FieldSet::to_ir)
        .collect::<Result<Vec<_>>>()
        .expect("public fieldsets should convert");
    assert_eq!(actual, canonical.fieldsets());
}

#[test]
fn public_fieldsets_round_trip_without_canonical_sidecar() {
    let (canonical, mut described) = public_described_canonical_fixture();
    described.fieldsets = canonical
        .fieldsets()
        .iter()
        .map(crate::schema::FieldSet::from_ir)
        .collect();
    let resolved = definition_to_ir(&described).expect("public fieldsets should resolve");
    assert_eq!(resolved.fieldsets(), canonical.fieldsets());
}

#[test]
fn described_public_values_expose_onnx_models() {
    let (canonical, described) = public_described_canonical_fixture();
    let actual = described
        .onnx_models()
        .iter()
        .map(crate::schema::OnnxModel::to_ir)
        .collect::<Vec<_>>();
    assert_eq!(actual, canonical.onnx_models());
}

#[test]
fn public_onnx_models_round_trip_without_canonical_sidecar() {
    let (canonical, mut described) = public_described_canonical_fixture();
    described.onnx_models = canonical
        .onnx_models()
        .iter()
        .map(crate::schema::OnnxModel::from_ir)
        .collect();
    let resolved = definition_to_ir(&described).expect("public ONNX models should resolve");
    assert_eq!(resolved.onnx_models(), canonical.onnx_models());
}

#[test]
fn described_public_values_expose_imported_fields() {
    let (canonical, described) = public_described_canonical_fixture();
    let actual = described
        .imported_fields()
        .iter()
        .map(crate::schema::ImportedField::to_ir)
        .collect::<Result<Vec<_>>>()
        .expect("public imported fields should convert");
    assert_eq!(actual, canonical.imported_fields());
}

#[test]
fn public_imported_fields_round_trip_without_canonical_sidecar() {
    let (canonical, mut described) = public_described_canonical_fixture();
    described.imported_fields = canonical
        .imported_fields()
        .iter()
        .map(crate::schema::ImportedField::from_ir)
        .collect();
    let resolved = definition_to_ir(&described).expect("public imported fields should resolve");
    assert_eq!(resolved.imported_fields(), canonical.imported_fields());
}

#[test]
fn described_index_definition_exposes_document_metadata() {
    let (canonical, described) = public_described_canonical_fixture();
    assert_eq!(
        described.document_type,
        canonical.document_type().map(str::to_owned)
    );
    assert_eq!(described.schema_inherits, canonical.schema_inherits());
    assert_eq!(described.document_inherits, canonical.document_inherits());
    let resolved = definition_to_ir(&described).expect("described schema should resolve");
    assert_eq!(resolved.document_type(), canonical.document_type());
    assert_eq!(resolved.schema_inherits(), canonical.schema_inherits());
    assert_eq!(resolved.document_inherits(), canonical.document_inherits());
}

#[test]
fn modifying_rich_described_definition_merges_collection_state() {
    let canonical = canonical_fixture_ir();
    let index = index_from_canonical_ir(canonical);
    let described = index.describe().expect("describe should succeed");
    let modified = described.default_search_fields([
        SearchField::new("title").weight(0.9),
        SearchField::new("tags").weight(0.1),
    ]);
    let resolved = definition_to_ir(&modified).expect("modified schema should resolve");
    let expected = canonical_fixture_ir_named("complete_schema", 0.9, 0.1);

    assert_eq!(expected, resolved);
}

#[test]
fn modifying_rich_described_definition_merges_append_operations() {
    let canonical = canonical_fixture_ir();
    let index = index_from_canonical_ir(canonical.clone());
    let described = index.describe().expect("describe should succeed");
    let added_field = Field::keyword("added_keyword");
    let added_profile = RankProfile::new("added_profile").first_phase("nativeRank");
    let modified = described
        .field(added_field.clone())
        .rank_profile(added_profile.clone());
    let resolved = definition_to_ir(&modified).expect("modified schema should resolve");

    let added_field_ir = definition_to_ir(&IndexDefinition::new().field(added_field))
        .expect("added field should resolve")
        .fields()[0]
        .clone();
    let added_profile_ir = definition_to_ir(
        &IndexDefinition::new()
            .field(Field::keyword("placeholder"))
            .rank_profile(added_profile),
    )
    .expect("added profile should resolve")
    .rank_profiles()[0]
        .clone();
    let mut expected = canonical;
    assert!(expected.push_field(added_field_ir));
    assert!(expected.push_rank_profile(added_profile_ir));

    assert_eq!(expected, resolved);
}

#[test]
fn describe_preserves_schema_properties() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(canonical.properties(), described.properties());
}

#[test]
fn describe_exposes_schema_properties_publicly() {
    let (canonical, described) = public_described_canonical_fixture();
    assert_eq!(
        crate::schema::properties_to_ir(described.properties()),
        canonical.properties().clone()
    );
}

#[test]
fn public_schema_properties_round_trip_through_definition_to_ir() {
    let (canonical, mut described) = public_described_canonical_fixture();
    described.properties.insert(
        "schema.property".to_owned(),
        crate::schema::SchemaValue::String("public-edit".to_owned()),
    );
    let resolved = definition_to_ir(&described).expect("described schema should resolve");
    assert_eq!(
        resolved.properties(),
        &crate::schema::properties_to_ir(described.properties())
    );
    assert_ne!(resolved.properties(), canonical.properties());
}

#[test]
fn describe_preserves_document_type() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(canonical.document_type(), described.document_type());
}

#[test]
fn describe_preserves_schema_inheritance() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(canonical.schema_inherits(), described.schema_inherits());
}

#[test]
fn describe_preserves_document_inheritance() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(canonical.document_inherits(), described.document_inherits());
}

#[test]
fn describe_preserves_named_structs() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(canonical.structs(), described.structs());
}

#[test]
fn describe_exposes_named_structs_publicly() {
    let (canonical, described) = public_described_canonical_fixture();
    let actual = described
        .structs()
        .iter()
        .map(crate::schema::NamedStruct::to_ir)
        .collect::<Vec<_>>();
    assert_eq!(actual, canonical.structs());
}

#[test]
fn public_named_structs_round_trip_without_sidecar_state() {
    let (canonical, mut described) = public_described_canonical_fixture();
    described.structs = canonical
        .structs()
        .iter()
        .map(crate::schema::NamedStruct::from_ir)
        .collect();

    let resolved = definition_to_ir(&described).expect("public structs should resolve");
    assert_eq!(resolved.structs(), canonical.structs());
}

#[test]
fn describe_preserves_fieldsets() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(canonical.fieldsets(), described.fieldsets());
}

#[test]
fn describe_preserves_imported_fields() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(canonical.imported_fields(), described.imported_fields());
}

#[test]
fn describe_preserves_schema_constants() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(canonical.constants(), described.constants());
}

#[test]
fn described_schema_constants_are_publicly_represented() {
    let (canonical, described) = public_described_canonical_fixture();
    assert_eq!(described.constants().len(), canonical.constants().len());
    for (public, expected) in described.constants().iter().zip(canonical.constants()) {
        assert_eq!(public.name(), expected.name);
        assert_eq!(public.data_type().to_ir(), expected.data_type);
        assert_eq!(
            crate::schema::properties_to_ir(public.properties()),
            expected.properties
        );
        match (public.source(), &expected.source) {
            (
                ConstantSource::Literal(public_value),
                searchkernel_ir::ConstantSourceIr::Literal(expected_value),
            ) => assert_eq!(public_value.to_ir(), *expected_value),
            (
                ConstantSource::File { path: public_path },
                searchkernel_ir::ConstantSourceIr::File {
                    path: expected_path,
                },
            ) => {
                assert_eq!(public_path, expected_path)
            }
            _ => panic!("constant source variant changed across describe"),
        }
    }
    assert_eq!(
        definition_to_ir(&described)
            .expect("public constants should convert back to IR")
            .constants(),
        canonical.constants()
    );
    let rebuilt = described.constants().iter().cloned().fold(
        IndexDefinition::new().field(Field::text("title")),
        |definition, constant| definition.constant(constant),
    );
    assert_eq!(
        definition_to_ir(&rebuilt)
            .expect("cloned public constants should convert back to IR")
            .constants(),
        canonical.constants()
    );
}

#[test]
fn describe_preserves_schema_onnx_models() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(canonical.onnx_models(), described.onnx_models());
}
