//! Portable schema compilation and validation for the native Vespa adapter.

use crate::native_error;
use searchkernel_ir::{PortableError, Result};
use searchkernel_vespa_engine::{ConfigCompiler, Field, FieldType, Schema};

/// Compile the resolved portable definition into the M-layer configuration.
///
/// This is deliberately owned by the adapter: the product layer must resolve
/// portable defaults into IR before the Vespa schema is generated.  M does not
/// currently expose collection fields through its generic document ABI, so a
/// multi-valued definition is rejected before any on-disk state is created.
pub fn compile_index_definition(
    definition: &searchkernel_ir::IndexDefinitionIr,
    config_dir: impl AsRef<std::path::Path>,
) -> Result<()> {
    validate_index_definition(definition)?;
    let mut schema = Schema::new(
        definition
            .document_type()
            .or_else(|| definition.name().map(searchkernel_ir::IndexName::as_str))
            .unwrap_or("searchkernel"),
    );
    for field in definition.fields() {
        if matches!(
            field.cardinality(),
            searchkernel_ir::FieldCardinality::Multi
        ) {
            return Err(PortableError::unsupported(
                "multi_valued_fields_not_supported",
                Some("fields"),
                format!(
                    "multi-valued field `{}` requires collection-valued M document/configuration and grouping support",
                    field.name().as_str()
                ),
            ));
        }
        let kind = match field.kind() {
            searchkernel_ir::FieldKind::Text => FieldType::Text,
            searchkernel_ir::FieldKind::Keyword => FieldType::String,
            searchkernel_ir::FieldKind::Int64 => FieldType::Integer,
            searchkernel_ir::FieldKind::Float64 => FieldType::Float,
            searchkernel_ir::FieldKind::Bool => FieldType::Boolean,
        };
        let capabilities = field.capabilities();
        let mut native = Field::new(field.name().as_str(), kind);
        if capabilities.searchable() {
            native = native.indexed();
        }
        let attribute = field.attribute();
        let attribute_enabled = attribute
            .and_then(|value| value.enabled)
            .unwrap_or_else(|| {
                capabilities.filterable()
                    || capabilities.sortable()
                    || capabilities.groupable()
                    || !matches!(field.kind(), searchkernel_ir::FieldKind::Text)
            });
        if attribute_enabled {
            native = native.attribute();
        }
        let fast_search = attribute.and_then(|value| value.fast_search).unwrap_or(
            matches!(field.kind(), searchkernel_ir::FieldKind::Keyword)
                && capabilities.filterable(),
        );
        if fast_search {
            native = native.fast_search();
        }
        if capabilities.stored() {
            native = native.summary();
        }
        schema = schema.field(native);
    }
    ConfigCompiler
        .compile_to(&schema, config_dir)
        .map_err(native_error)
}

/// Validate the subset of the portable schema that the current M ABI can
/// execute without losing document values.
pub fn validate_index_definition(definition: &searchkernel_ir::IndexDefinitionIr) -> Result<()> {
    if !definition.properties().is_empty()
        || !definition.schema_inherits().is_empty()
        || !definition.document_inherits().is_empty()
        || !definition.structs().is_empty()
        || !definition.fieldsets().is_empty()
        || !definition.schema_fields().is_empty()
        || !definition.imported_fields().is_empty()
        || !definition.document_summaries().is_empty()
        || !definition.rank_profiles().is_empty()
        || !definition.constants().is_empty()
        || !definition.onnx_models().is_empty()
    {
        return Err(PortableError::unsupported(
            "schema_feature_not_supported",
            Some("schema"),
            "the current native adapter cannot lower rich schema-level configuration",
        ));
    }
    for (index, field) in definition.fields().iter().enumerate() {
        if matches!(
            field.data_type(),
            searchkernel_ir::DataTypeIr::Array(_) | searchkernel_ir::DataTypeIr::WeightedSet(_)
        ) {
            return Err(PortableError::unsupported(
                "multi_valued_fields_not_supported",
                Some("fields"),
                format!(
                    "multi-valued field `{}` requires collection-valued M document/configuration and grouping support",
                    field.name().as_str()
                ),
            ));
        }

        let field_path = format!("fields[{index}]");
        let capabilities = field.capabilities();
        let attribute_enabled = field.attribute().and_then(|value| value.enabled);
        let capability_requires_attribute =
            capabilities.filterable() || capabilities.sortable() || capabilities.groupable();
        if attribute_enabled == Some(false) && capability_requires_attribute {
            return Err(PortableError::invalid(
                "invalid_capability",
                Some(format!("{field_path}.attribute.enabled")),
                "attribute.enabled=false conflicts with an attribute-backed field capability",
            ));
        }
        if attribute_enabled == Some(false)
            && !matches!(field.kind(), searchkernel_ir::FieldKind::Text)
        {
            return Err(PortableError::unsupported(
                "attribute_disabled_not_supported",
                Some(format!("{field_path}.attribute.enabled")),
                "the current native schema requires non-text fields to be attributes",
            ));
        }
        if field
            .attribute()
            .and_then(|value| value.fast_search)
            .is_some_and(|value| value)
            && !capabilities.filterable()
        {
            return Err(PortableError::invalid(
                "invalid_capability",
                Some(format!("{field_path}.attribute.fast_search")),
                "attribute.fast_search requires a filterable field",
            ));
        }
        if field
            .attribute()
            .and_then(|value| value.fast_search)
            .is_some_and(|value| value)
            && attribute_enabled == Some(false)
        {
            return Err(PortableError::invalid(
                "invalid_capability",
                Some(format!("{field_path}.attribute.fast_search")),
                "attribute.fast_search cannot be enabled when attribute.enabled is false",
            ));
        }
        if field
            .index()
            .and_then(|value| value.enabled)
            .zip(Some(capabilities.searchable()))
            .is_some_and(|(configured, resolved)| configured != resolved)
        {
            return Err(PortableError::invalid(
                "invalid_capability",
                Some(format!("{field_path}.index.enabled")),
                "index.enabled conflicts with the resolved searchable capability",
            ));
        }
        if field
            .summary()
            .and_then(|value| value.enabled)
            .zip(Some(capabilities.stored()))
            .is_some_and(|(configured, resolved)| configured != resolved)
        {
            return Err(PortableError::invalid(
                "invalid_capability",
                Some(format!("{field_path}.summary.enabled")),
                "summary.enabled conflicts with the resolved stored capability",
            ));
        }
        if field
            .matching()
            .is_some_and(|value| value.mode == searchkernel_ir::MatchingModeIr::Word)
        {
            return Err(PortableError::unsupported(
                "matching_mode_not_supported",
                Some(format!("{field_path}.matching.mode")),
                "the current native schema has no word-matching representation",
            ));
        }
        if capabilities.searchable() && !matches!(field.kind(), searchkernel_ir::FieldKind::Text) {
            return Err(PortableError::unsupported(
                "unsupported_field_capability",
                Some(format!("{field_path}.searchable")),
                "the current native schema indexes text fields only",
            ));
        }
        if matches!(
            field.data_type(),
            searchkernel_ir::DataTypeIr::Primitive(searchkernel_ir::PrimitiveDataTypeIr::String)
        ) && !matches!(
            field.matching().map(|value| value.mode),
            Some(searchkernel_ir::MatchingModeIr::Exact | searchkernel_ir::MatchingModeIr::Word)
        ) && !field.capabilities().searchable()
        {
            return Err(PortableError::unsupported(
                "unsupported_field_capability",
                Some(format!("{field_path}.searchable")),
                format!(
                    "text field `{}` must be searchable in the current native schema",
                    field.name().as_str()
                ),
            ));
        }
        if matches!(
            field.data_type(),
            searchkernel_ir::DataTypeIr::Primitive(searchkernel_ir::PrimitiveDataTypeIr::Float64)
        ) && field.capabilities().groupable()
        {
            return Err(PortableError::unsupported(
                "unsupported_field_capability",
                Some(format!("{field_path}.facetable")),
                format!(
                    "float field `{}` cannot be facetable in the current native schema",
                    field.name().as_str()
                ),
            ));
        }
        if !field.properties().is_empty()
            || !field.struct_fields().is_empty()
            || field.indexing_script().is_some()
            || field.matching().is_some_and(|value| {
                !value.options.is_empty()
                    || !matches!(
                        value.mode,
                        searchkernel_ir::MatchingModeIr::Unspecified
                            | searchkernel_ir::MatchingModeIr::Text
                            | searchkernel_ir::MatchingModeIr::Exact
                    )
            })
            || field
                .index()
                .is_some_and(|value| value.hnsw.is_some() || !value.options.is_empty())
            || field.attribute().is_some_and(|value| {
                value.fast_access.is_some()
                    || value.distance_metric != searchkernel_ir::DistanceMetricIr::Unspecified
                    || !value.options.is_empty()
            })
            || field
                .summary()
                .is_some_and(|value| !value.options.is_empty())
            || field.analyzer().is_some_and(|value| {
                value.name != "standard" || value.version != 1 || !value.options.is_empty()
            })
        {
            return Err(PortableError::unsupported(
                "schema_feature_not_supported",
                Some(format!("fields[{index}]")),
                "the current native adapter cannot lower this rich field configuration",
            ));
        }
        match field.data_type() {
            searchkernel_ir::DataTypeIr::Primitive(
                searchkernel_ir::PrimitiveDataTypeIr::String
                | searchkernel_ir::PrimitiveDataTypeIr::Bool
                | searchkernel_ir::PrimitiveDataTypeIr::Int64
                | searchkernel_ir::PrimitiveDataTypeIr::Float64,
            ) => {}
            _ => {
                return Err(PortableError::unsupported(
                    "data_type_not_supported",
                    Some(format!("fields[{index}].data_type")),
                    "the current native adapter cannot lower this V2 data type",
                ))
            }
        }
    }
    Ok(())
}
