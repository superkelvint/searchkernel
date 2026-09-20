//! Portable schema compilation and validation for the native Vespa adapter.

use crate::native_error;
use searchkernel_ir::{PortableError, Result};
use searchkernel_vespa_engine::{
    ConfigCompiler, Field, FieldType, RankProfile, Schema, StructField, StructFieldType,
};

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
    for profile in definition.rank_profiles().iter().map(lower_rank_profile) {
        schema = schema.rank_profile(profile?);
    }
    for field in definition.fields() {
        let kind = match field.data_type() {
            searchkernel_ir::DataTypeIr::Tensor { type_spec } => {
                FieldType::Tensor(type_spec.clone())
            }
            searchkernel_ir::DataTypeIr::Array(element)
                if matches!(
                    element.as_ref(),
                    searchkernel_ir::DataTypeIr::Primitive(
                        searchkernel_ir::PrimitiveDataTypeIr::String
                    )
                ) =>
            {
                FieldType::ArrayString
            }
            searchkernel_ir::DataTypeIr::Array(element)
                if matches!(
                    element.as_ref(),
                    searchkernel_ir::DataTypeIr::Struct { .. }
                        | searchkernel_ir::DataTypeIr::StructReference { .. }
                ) =>
            {
                FieldType::ArrayStruct(struct_fields(definition, element)?)
            }
            searchkernel_ir::DataTypeIr::Position => FieldType::Position,
            searchkernel_ir::DataTypeIr::Predicate => FieldType::Predicate,
            _ => match field.kind() {
                searchkernel_ir::FieldKind::Text => FieldType::Text,
                searchkernel_ir::FieldKind::Keyword => FieldType::String,
                searchkernel_ir::FieldKind::Int64 => FieldType::Long,
                searchkernel_ir::FieldKind::Float64 => FieldType::Float,
                searchkernel_ir::FieldKind::Bool => FieldType::Boolean,
            },
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
        // Vespa cannot expose tensor fields through the default document
        // summary, but its document retriever still returns the complete
        // TensorFieldValue used by portable GET. Keep the portable stored
        // capability without manufacturing an invalid tensor summary field.
        if capabilities.stored()
            && !matches!(
                field.data_type(),
                searchkernel_ir::DataTypeIr::Position | searchkernel_ir::DataTypeIr::Tensor { .. }
            )
        {
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
        || !definition.fieldsets().is_empty()
        || !definition.schema_fields().is_empty()
        || !definition.imported_fields().is_empty()
        || !definition.document_summaries().is_empty()
        || !definition.constants().is_empty()
        || !definition.onnx_models().is_empty()
    {
        return Err(PortableError::unsupported(
            "schema_feature_not_supported",
            Some("schema"),
            "the current native adapter cannot lower rich schema-level configuration",
        ));
    }
    for profile in definition.rank_profiles() {
        lower_rank_profile(profile)?;
    }
    for (index, field) in definition.fields().iter().enumerate() {
        if matches!(
            field.data_type(),
            searchkernel_ir::DataTypeIr::WeightedSet(_)
        ) || matches!(
            field.data_type(),
            searchkernel_ir::DataTypeIr::Array(element)
                if !matches!(
                    element.as_ref(),
                    searchkernel_ir::DataTypeIr::Primitive(
                        searchkernel_ir::PrimitiveDataTypeIr::String
                    ) | searchkernel_ir::DataTypeIr::Struct { .. }
                        | searchkernel_ir::DataTypeIr::StructReference { .. }
                )
        ) {
            return Err(PortableError::unsupported(
                "multi_valued_fields_not_supported",
                Some("fields"),
                format!(
                    "field `{}` requires a collection type outside the supported array<string> path",
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
                searchkernel_ir::PrimitiveDataTypeIr::String,
            )
            | searchkernel_ir::DataTypeIr::Primitive(searchkernel_ir::PrimitiveDataTypeIr::Bool)
            | searchkernel_ir::DataTypeIr::Primitive(searchkernel_ir::PrimitiveDataTypeIr::Int64)
            | searchkernel_ir::DataTypeIr::Primitive(
                searchkernel_ir::PrimitiveDataTypeIr::Float64,
            )
            | searchkernel_ir::DataTypeIr::Position
            | searchkernel_ir::DataTypeIr::Tensor { .. } => {}
            searchkernel_ir::DataTypeIr::Array(element)
                if matches!(
                    element.as_ref(),
                    searchkernel_ir::DataTypeIr::Primitive(
                        searchkernel_ir::PrimitiveDataTypeIr::String
                    )
                ) => {}
            searchkernel_ir::DataTypeIr::Array(element)
                if matches!(
                    element.as_ref(),
                    searchkernel_ir::DataTypeIr::Struct { .. }
                        | searchkernel_ir::DataTypeIr::StructReference { .. }
                ) =>
            {
                struct_fields(definition, element)?;
            }
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

fn struct_fields(
    definition: &searchkernel_ir::IndexDefinitionIr,
    data_type: &searchkernel_ir::DataTypeIr,
) -> Result<Vec<StructField>> {
    let fields = match data_type {
        searchkernel_ir::DataTypeIr::Struct { fields } => fields.clone(),
        searchkernel_ir::DataTypeIr::StructReference { name } => definition
            .structs()
            .iter()
            .find(|value| value.name == *name)
            .map(|value| value.fields.clone())
            .ok_or_else(|| {
                PortableError::invalid(
                    "unknown_struct",
                    Some("fields"),
                    format!("struct `{name}` is not declared"),
                )
            })?,
        _ => unreachable!("struct_fields called for a non-struct type"),
    };
    fields
        .into_iter()
        .map(|field| {
            let field_type = match field.data_type {
                searchkernel_ir::DataTypeIr::Primitive(
                    searchkernel_ir::PrimitiveDataTypeIr::String,
                ) => StructFieldType::String,
                searchkernel_ir::DataTypeIr::Primitive(
                    searchkernel_ir::PrimitiveDataTypeIr::Int64,
                ) => StructFieldType::Long,
                searchkernel_ir::DataTypeIr::Primitive(
                    searchkernel_ir::PrimitiveDataTypeIr::Int32,
                ) => StructFieldType::Integer,
                searchkernel_ir::DataTypeIr::Primitive(
                    searchkernel_ir::PrimitiveDataTypeIr::Float64
                    | searchkernel_ir::PrimitiveDataTypeIr::Float32,
                ) => StructFieldType::Float,
                searchkernel_ir::DataTypeIr::Primitive(
                    searchkernel_ir::PrimitiveDataTypeIr::Bool,
                ) => StructFieldType::Boolean,
                _ => {
                    return Err(PortableError::unsupported(
                        "nested_struct_value_not_supported",
                        Some("structs"),
                        format!(
                            "struct member `{}` must be a supported primitive",
                            field.name
                        ),
                    ));
                }
            };
            Ok(StructField {
                name: field.name,
                field_type,
            })
        })
        .collect()
}

/// Lower the deliberately small portable rank-profile subset understood by
/// the current native configuration compiler.  Ranking expressions remain
/// opaque Vespa expressions; this function only wires their declared shape
/// into M's existing `RankProfile` model.
fn lower_rank_profile(profile: &searchkernel_ir::RankProfileDefinitionIr) -> Result<RankProfile> {
    let unsupported = |path: &str, message: &str| {
        Err(PortableError::unsupported(
            "rank_profile_feature_not_supported",
            Some(format!("rank_profiles.{}.{}", profile.name, path)),
            message,
        ))
    };

    if !profile.inherits.is_empty() {
        return unsupported(
            "inherits",
            "rank-profile inheritance is not representable by the native schema model",
        );
    }
    if !profile.functions.is_empty() {
        return unsupported(
            "functions",
            "rank-profile functions are not representable by the native schema model",
        );
    }
    if profile.second_phase.is_some() {
        return unsupported(
            "second_phase",
            "second-phase ranking is outside the supported native schema subset",
        );
    }
    if profile.global_phase.is_some() {
        return unsupported(
            "global_phase",
            "global-phase ranking is outside the supported native schema subset",
        );
    }
    if !profile.rank_features.is_empty() {
        return unsupported(
            "rank_features",
            "declared rank features are not representable by the native schema model",
        );
    }
    if !profile.summary_features.is_empty() {
        return unsupported(
            "summary_features",
            "declared summary features are not representable by the native schema model",
        );
    }
    if !profile.sort_features.is_empty() {
        return unsupported(
            "sort_features",
            "declared sort features are not representable by the native schema model",
        );
    }
    if !profile.constants.is_empty() {
        return unsupported(
            "constants",
            "ranking constants are outside the supported native schema subset",
        );
    }
    if !profile.onnx_models.is_empty() {
        return unsupported(
            "onnx_models",
            "ranking models are outside the supported native schema subset",
        );
    }
    if !profile.rank_properties.is_empty() {
        return unsupported(
            "rank_properties",
            "profile-default rank properties are not representable by the native schema model",
        );
    }
    if profile.strict.is_some() || !profile.properties.is_empty() {
        return unsupported(
            "properties",
            "rank-profile policy properties are not representable by the native schema model",
        );
    }

    let first_phase = profile.first_phase.as_ref().ok_or_else(|| {
        PortableError::invalid(
            "missing_rank_profile_first_phase",
            Some(format!("rank_profiles.{}.first_phase", profile.name)),
            "a supported rank profile requires a first-phase ranking expression",
        )
    })?;
    if first_phase.expression.is_none()
        || first_phase.rerank_count.is_some()
        || first_phase.rank_score_drop_limit.is_some()
        || first_phase.total_rerank_count.is_some()
        || first_phase.keep_rank_count.is_some()
        || first_phase.total_keep_rank_count.is_some()
        || !first_phase.properties.is_empty()
    {
        return unsupported(
            "first_phase",
            "only an opaque first-phase expression and match features are supported",
        );
    }

    let mut native = RankProfile::new(profile.name.clone())
        .first_phase(first_phase.expression.as_deref().unwrap_or_default());
    for input in &profile.inputs {
        let searchkernel_ir::DataTypeIr::Tensor { type_spec } = &input.data_type else {
            return unsupported(
                "inputs",
                "only dense tensor rank-profile inputs are representable by the native schema model",
            );
        };
        if !matches!(
            input.default_value,
            searchkernel_ir::RequestValueIr::Tensor(_)
        ) {
            return unsupported(
                "inputs",
                "tensor rank-profile inputs require tensor default values",
            );
        }
        native = native.query_input(input.name.clone(), type_spec.clone());
    }
    for feature in &profile.match_features {
        native = native.match_feature(feature);
    }
    Ok(native)
}
