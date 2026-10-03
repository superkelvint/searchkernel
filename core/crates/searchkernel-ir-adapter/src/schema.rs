//! Portable schema compilation and validation for the native Vespa adapter.

use crate::native_error;
use searchkernel_ir::{PortableError, Result};
use searchkernel_vespa_engine::{
    ConfigCompiler, DistanceMetric, Field, FieldType, HnswIndexConfig, RankProfile, Schema,
    StructField, StructFieldType,
};

const PORTABLE_PREDICATE_ARITY: u32 = 8;

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
    let mut schema = Schema::new(definition.resolved_document_type());
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
        if matches!(field.data_type(), searchkernel_ir::DataTypeIr::Predicate) {
            // Predicate arity is a Vespa physical-index parameter rather than
            // a portable value-semantic. The portable schema currently has no
            // arity knob, so keep one deterministic backend default instead of
            // making Predicate syntactically accepted but impossible to compile.
            native = native.predicate_arity(PORTABLE_PREDICATE_ARITY);
        }
        let is_tensor = matches!(
            field.data_type(),
            searchkernel_ir::DataTypeIr::Tensor { .. }
        );
        if capabilities.searchable() && !is_tensor {
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
        if let Some(config) = field.index().and_then(|value| value.hnsw.as_ref()) {
            let mut hnsw = HnswIndexConfig::new();
            if let Some(value) = config.max_links_per_node {
                hnsw = hnsw.max_links_per_node(value);
            }
            if let Some(value) = config.neighbors_to_explore_at_insert {
                hnsw = hnsw.neighbors_to_explore_at_insert(value);
            }
            native = native.hnsw(hnsw);
        }
        if let Some(attribute) = field.attribute() {
            if let Some(metric) = lower_distance_metric(attribute.distance_metric)? {
                native = native.distance_metric(metric);
            }
        }
        // Search projection support is semantic authority: native schema
        // lowering must use the same classification as IR's default/explicit
        // projection resolver. GET remains able to retrieve richer stored types.
        if capabilities.stored()
            && field.data_type().search_projection_support()
                == searchkernel_ir::SearchProjectionSupport::Supported
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
        if capabilities.searchable()
            && !matches!(field.kind(), searchkernel_ir::FieldKind::Text)
            && !matches!(
                field.data_type(),
                searchkernel_ir::DataTypeIr::Tensor { .. }
            )
        {
            return Err(PortableError::unsupported(
                "unsupported_field_capability",
                Some(format!("{field_path}.searchable")),
                "the current native schema indexes text fields and HNSW tensors only",
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
            || field.index().is_some_and(|value| {
                !value.options.is_empty()
                    || value
                        .hnsw
                        .as_ref()
                        .is_some_and(|hnsw| !hnsw.options.is_empty())
            })
            || field
                .attribute()
                .is_some_and(|value| value.fast_access.is_some() || !value.options.is_empty())
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
            | searchkernel_ir::DataTypeIr::Predicate
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
                    "the current native adapter cannot lower this data type",
                ))
            }
        }
    }
    Ok(())
}

fn lower_distance_metric(
    metric: searchkernel_ir::DistanceMetricIr,
) -> Result<Option<DistanceMetric>> {
    Ok(match metric {
        searchkernel_ir::DistanceMetricIr::Unspecified => None,
        searchkernel_ir::DistanceMetricIr::Euclidean => Some(DistanceMetric::Euclidean),
        searchkernel_ir::DistanceMetricIr::Angular => Some(DistanceMetric::Angular),
        searchkernel_ir::DistanceMetricIr::DotProduct => Some(DistanceMetric::DotProduct),
        searchkernel_ir::DistanceMetricIr::PrenormalizedAngular => {
            Some(DistanceMetric::PrenormalizedAngular)
        }
        searchkernel_ir::DistanceMetricIr::GeoDegrees => Some(DistanceMetric::GeoDegrees),
        searchkernel_ir::DistanceMetricIr::Hamming => Some(DistanceMetric::Hamming),
    })
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

fn validate_rank_profile_input_default(
    profile: &searchkernel_ir::RankProfileDefinitionIr,
    input: &searchkernel_ir::RankProfileInputIr,
    type_spec: &str,
) -> Result<()> {
    let path = format!(
        "rank_profiles.{}.inputs.{}.default_value",
        profile.name, input.name
    );
    let searchkernel_ir::RequestValueIr::Tensor(tensor) = &input.default_value else {
        return Err(PortableError::unsupported(
            "rank_profile_input_default_not_supported",
            Some(path),
            "rank-profile input defaults must be tensor values",
        ));
    };
    if tensor.type_spec != type_spec {
        return Err(PortableError::invalid(
            "rank_profile_input_default_type_mismatch",
            Some(path),
            format!(
                "rank-profile input type is `{type_spec}` but the default is `{}`",
                tensor.type_spec
            ),
        ));
    }
    let searchkernel_ir::RequestTensorPayloadIr::Dense(values) = &tensor.payload else {
        return Err(PortableError::unsupported(
            "rank_profile_input_default_not_supported",
            Some(path),
            "only dense tensor rank-profile input declarations are supported by the native schema model",
        ));
    };

    // The ergonomic Rust facade uses an empty dense tensor as a
    // declaration-only placeholder. It means "no explicit default" and
    // preserves the established query_input() behavior.
    if values.is_empty() {
        return Ok(());
    }

    // Native rank-profile defaults are deliberately rejected until the
    // embedded Vespa boundary has a proven shape-faithful tensor literal
    // representation and real-engine execution coverage. Failing here keeps
    // represented canonical IR from being silently dropped or approximated.
    Err(PortableError::unsupported(
        "rank_profile_input_default_not_supported",
        Some(path),
        "explicit rank-profile input defaults are not supported by native schema compilation",
    ))
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
    if let Some(second_phase) = &profile.second_phase {
        if second_phase.expression.is_none() {
            return unsupported(
                "second_phase.expression",
                "a second-phase ranking expression is required when second-phase controls are set",
            );
        }
        if second_phase.rank_score_drop_limit.is_some()
            || second_phase.total_rerank_count.is_some()
            || second_phase.keep_rank_count.is_some()
            || second_phase.total_keep_rank_count.is_some()
            || !second_phase.properties.is_empty()
        {
            return unsupported(
                "second_phase",
                "only a second-phase expression and per-node rerank count are supported",
            );
        }
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
    if let Some(second_phase) = &profile.second_phase {
        native = native.second_phase(second_phase.expression.as_deref().unwrap_or_default());
        if let Some(rerank_count) = second_phase.rerank_count {
            native = native.rerank_count(rerank_count);
        }
    }
    for input in &profile.inputs {
        let searchkernel_ir::DataTypeIr::Tensor { type_spec } = &input.data_type else {
            return unsupported(
                "inputs",
                "only dense tensor rank-profile inputs are representable by the native schema model",
            );
        };
        validate_rank_profile_input_default(profile, input, type_spec)?;
        native = native.query_input(input.name.clone(), type_spec.clone());
    }
    for feature in &profile.match_features {
        native = native.match_feature(feature);
    }
    Ok(native)
}

#[cfg(test)]
mod tests {
    use super::compile_index_definition;
    use searchkernel_ir::{
        DataTypeIr, ErrorCode, FieldCapabilities, FieldIr, FieldName, IndexDefinitionIr,
        PrimitiveDataTypeIr, RankPhaseDefinitionIr, RankProfileDefinitionIr, RankProfileInputIr,
        RequestTensorIr, RequestTensorPayloadIr, RequestValueIr,
    };

    fn field(name: &str) -> FieldName {
        FieldName::parse(name).unwrap()
    }

    fn definition_with_rank_input_default(type_spec: &str, values: Vec<f64>) -> IndexDefinitionIr {
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
        definition.set_rank_profiles(vec![RankProfileDefinitionIr {
            name: "with_default".to_owned(),
            inherits: Vec::new(),
            inputs: vec![RankProfileInputIr {
                name: "embedding".to_owned(),
                data_type: DataTypeIr::Tensor {
                    type_spec: type_spec.to_owned(),
                },
                default_value: RequestValueIr::Tensor(RequestTensorIr {
                    type_spec: type_spec.to_owned(),
                    payload: RequestTensorPayloadIr::Dense(values),
                }),
            }],
            functions: Vec::new(),
            first_phase: Some(RankPhaseDefinitionIr {
                expression: Some("query(embedding)".to_owned()),
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
            match_features: Vec::new(),
            summary_features: Vec::new(),
            sort_features: Vec::new(),
            constants: Vec::new(),
            onnx_models: Vec::new(),
            rank_properties: std::collections::BTreeMap::new(),
            strict: None,
            properties: std::collections::BTreeMap::new(),
        }]);
        definition
    }

    #[test]
    fn explicit_rank_profile_input_defaults_fail_closed_before_native_config() {
        let cases = [
            // Same declared tensor type, distinct explicit defaults: neither
            // may be silently collapsed into the declaration-only path.
            ("tensor<float>(x[2])", vec![1.0, 0.0]),
            ("tensor<float>(x[2])", vec![0.0, 1.0]),
            ("tensor<float>(a[1],b[4])", vec![1.0, 2.0, 3.0, 4.0]),
            // Deliberately malformed cell count. The adapter rejects all
            // explicit defaults before native config, so malformed dense
            // shapes cannot be flattened into a misleading Vespa literal.
            ("tensor<float>(a[1],b[4])", vec![1.0, 2.0, 3.0]),
        ];

        for (case_index, (type_spec, values)) in cases.into_iter().enumerate() {
            let definition = definition_with_rank_input_default(type_spec, values);
            let root = std::env::temp_dir().join(format!(
                "searchkernel-adapter-rank-profile-default-{}-{case_index}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&root);

            let error = compile_index_definition(&definition, &root)
                .expect_err("explicit rank-profile defaults must fail closed");
            assert_eq!(error.code, ErrorCode::Unsupported);
            assert_eq!(error.details.len(), 1);
            assert_eq!(
                error.details[0].code,
                "rank_profile_input_default_not_supported"
            );
            assert_eq!(
                error.details[0].path.as_deref(),
                Some("rank_profiles.with_default.inputs.embedding.default_value")
            );
            assert!(
                !root.exists(),
                "validation must fail before native config output is created"
            );
        }
    }

    #[test]
    fn declaration_only_rank_profile_input_remains_supported() {
        let mut definition = definition_with_rank_input_default("tensor<float>(x[2])", Vec::new());
        definition.set_document_type("rank_profile_test");
        let root = std::env::temp_dir().join(format!(
            "searchkernel-adapter-rank-profile-declaration-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);

        compile_index_definition(&definition, &root)
            .expect("empty dense payload is the declaration-only placeholder");
        let config = std::fs::read_to_string(root.join("rank-profiles.cfg")).unwrap();
        assert!(config.contains("vespa.type.query.embedding"));
        assert!(!config.contains(".name \"query(embedding)\""));
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod predicate_schema_tests {
    use super::{compile_index_definition, PORTABLE_PREDICATE_ARITY};
    use searchkernel_ir::{DataTypeIr, FieldCapabilities, FieldIr, FieldName, IndexDefinitionIr};

    #[test]
    fn predicate_schema_compiles_with_the_portable_native_arity_default() {
        let mut definition = IndexDefinitionIr::new(
            None,
            vec![FieldIr::new(
                FieldName::parse("access").unwrap(),
                DataTypeIr::Predicate,
                None,
                FieldCapabilities::new(true, false, false, false, false),
            )],
            Vec::new(),
        )
        .unwrap();
        definition.set_document_type("predicate_test");
        let root = std::env::temp_dir().join(format!(
            "searchkernel-adapter-predicate-schema-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);

        compile_index_definition(&definition, &root)
            .expect("portable Predicate schema should compile");
        let attributes = std::fs::read_to_string(root.join("attributes.cfg")).unwrap();
        assert!(attributes.contains(&format!("attribute[0].arity {PORTABLE_PREDICATE_ARITY}")));
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod struct_reference_schema_tests {
    use super::compile_index_definition;
    use searchkernel_ir::{
        DataTypeIr, FieldCapabilities, FieldIr, FieldName, IndexDefinitionIr,
        NamedStructDefinitionIr, PrimitiveDataTypeIr, StructFieldIr,
    };
    use std::collections::BTreeMap;

    #[test]
    fn stored_struct_reference_array_omits_unsupported_native_summary() {
        let mut definition = IndexDefinitionIr::new(
            None,
            vec![FieldIr::new(
                FieldName::parse("people").unwrap(),
                DataTypeIr::Array(Box::new(DataTypeIr::StructReference {
                    name: "person".to_owned(),
                })),
                None,
                FieldCapabilities::new(true, false, true, false, false),
            )],
            Vec::new(),
        )
        .unwrap();
        definition.set_document_type("struct_reference_test");
        definition.set_structs(vec![NamedStructDefinitionIr {
            name: "person".to_owned(),
            fields: vec![
                StructFieldIr {
                    name: "name".to_owned(),
                    data_type: DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
                },
                StructFieldIr {
                    name: "score".to_owned(),
                    data_type: DataTypeIr::Primitive(PrimitiveDataTypeIr::Float64),
                },
            ],
            properties: BTreeMap::new(),
        }]);

        let root = std::env::temp_dir().join(format!(
            "searchkernel-adapter-struct-reference-summary-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);

        compile_index_definition(&definition, &root)
            .expect("stored struct-reference arrays should compile");
        let summary = std::fs::read_to_string(root.join("summary.cfg")).unwrap();
        assert!(
            !summary.contains("people"),
            "array<struct> is GET-readable from the document store but is not a native summary type"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
