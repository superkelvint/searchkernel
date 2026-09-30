use super::*;
use crate::schema::FieldKind;

#[test]
fn public_field_data_type_cannot_override_its_declared_kind() {
    let mut field = Field::int64("price");
    assert!(matches!(field.kind, FieldKind::Int64));
    field.data_type = DataType::Primitive(PrimitiveDataType::Float64);

    let error = definition_to_ir(&IndexDefinition::new().field(field))
        .expect_err("a public data-type mutation must be canonically validated");
    assert_eq!(error.details()[0].code, "invalid_type_combination");
}

#[test]
fn fresh_indexing_script_preserves_explicit_capability_for_validation() {
    let mut field = Field::int64("price").stored(true);
    field.indexing_script = Some("input price | attribute price".to_owned());

    let error = definition_to_ir(&IndexDefinition::new().field(field))
        .expect_err("explicit capability must remain visible to canonical validation");
    assert_eq!(error.details()[0].code, "ambiguous_indexing_script");
    assert_eq!(
        error.details()[0].path.as_deref(),
        Some("fields[0].indexing_script")
    );
}

#[test]
fn rich_schema_negative_semantics_match_portable_ingress() {
    use searchkernel_ir::{
        AnalyzerIr, DataTypeIr, FieldCapabilities, FieldIr, FieldName, IndexConfigIr,
        PrimitiveDataTypeIr, SchemaPropertiesIr, SummaryConfigIr,
    };

    let text_field = || {
        FieldIr::new(
            FieldName::parse("title").expect("valid field name"),
            DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
            Some(AnalyzerIr {
                name: "standard".to_owned(),
                version: 1,
                options: SchemaPropertiesIr::new(),
            }),
            FieldCapabilities::new(true, true, false, false, false),
        )
    };

    let mut stored_summary_conflict = text_field();
    stored_summary_conflict.set_summary_config(Some(SummaryConfigIr {
        enabled: Some(false),
        options: SchemaPropertiesIr::new(),
    }));
    assert_rust_and_portable_schema_error_match(
        invalid_schema_with_field(stored_summary_conflict),
        "invalid_capability",
    );

    let mut searchable_index_conflict = text_field();
    searchable_index_conflict.set_index_config(Some(IndexConfigIr {
        enabled: Some(false),
        hnsw: None,
        options: SchemaPropertiesIr::new(),
    }));
    assert_rust_and_portable_schema_error_match(
        invalid_schema_with_field(searchable_index_conflict),
        "invalid_capability",
    );

    let mut invalid_analyzer = text_field();
    invalid_analyzer.set_analyzer(Some(AnalyzerIr {
        name: String::new(),
        version: 1,
        options: SchemaPropertiesIr::new(),
    }));
    assert_rust_and_portable_schema_error_match(
        invalid_schema_with_field(invalid_analyzer),
        "invalid_analyzer",
    );

    let mut ambiguous_indexing_script = text_field();
    ambiguous_indexing_script.set_indexing_script(Some("input title | index title".to_owned()));
    assert_rust_and_portable_schema_error_match(
        invalid_schema_with_field(ambiguous_indexing_script),
        "ambiguous_indexing_script",
    );

    let missing_struct = FieldIr::new(
        FieldName::parse("payload").expect("valid field name"),
        DataTypeIr::StructReference {
            name: "MissingStruct".to_owned(),
        },
        None,
        FieldCapabilities::new(true, false, true, true, false),
    );
    assert_rust_and_portable_schema_error_match(
        invalid_schema_with_field(missing_struct),
        "unknown_struct",
    );
}
