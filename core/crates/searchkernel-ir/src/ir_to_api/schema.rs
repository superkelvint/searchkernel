use super::*;

pub fn describe_result_to_api(definition: &crate::model::IndexDefinitionIr) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let index = index_definition_offset(&mut builder, definition);
    let root = api::DescribeResult::create(
        &mut builder,
        &api::DescribeResultArgs { index: Some(index) },
    );
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

/// Encode a Describe result into an existing transport builder.
pub fn describe_result_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    definition: &crate::model::IndexDefinitionIr,
) -> flatbuffers::WIPOffset<api::DescribeResult<'a>> {
    let index = index_definition_offset(builder, definition);
    api::DescribeResult::create(builder, &api::DescribeResultArgs { index: Some(index) })
}

/// Encode only the normalized `IndexDefinition` table.
pub fn index_definition_ir_to_api(definition: &crate::model::IndexDefinitionIr) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let root = index_definition_offset(&mut builder, definition);
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

/// Encode unresolved schema intent without applying resolver defaults.
pub fn index_definition_input_to_api(
    definition: &crate::api_to_ir::IndexDefinitionInput,
) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let root = index_definition_input_offset(&mut builder, definition);
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

pub(super) fn index_definition_input_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    definition: &crate::api_to_ir::IndexDefinitionInput,
) -> flatbuffers::WIPOffset<api::IndexDefinition<'a>> {
    let name = definition
        .name
        .as_ref()
        .map(|value| builder.create_string(value));
    let fields = definition
        .fields
        .iter()
        .map(|field| unresolved_field_definition_offset(builder, field))
        .collect::<Vec<_>>();
    let fields = builder.create_vector(&fields);
    let schema_fields = (!definition.schema_fields.is_empty()).then(|| {
        let fields = definition
            .schema_fields
            .iter()
            .map(|field| {
                let field = unresolved_field_definition_offset(builder, field);
                api::SearchOnlyFieldDefinition::create(
                    builder,
                    &api::SearchOnlyFieldDefinitionArgs { field: Some(field) },
                )
            })
            .collect::<Vec<_>>();
        builder.create_vector(&fields)
    });
    let default_search_fields = definition.default_search_fields.as_ref().map(|values| {
        let values = values
            .iter()
            .map(|value| {
                let field = builder.create_string(&value.field);
                let weight = value.weight.map(api::OptionalDouble::new);
                api::SearchField::create(
                    builder,
                    &api::SearchFieldArgs {
                        field: Some(field),
                        weight: weight.as_ref(),
                    },
                )
            })
            .collect::<Vec<_>>();
        let values = builder.create_vector(&values);
        api::SearchFieldSelection::create(
            builder,
            &api::SearchFieldSelectionArgs {
                fields: Some(values),
            },
        )
    });
    let properties = request_value_entries_offset(builder, &definition.properties);
    let document_type = definition
        .document_type
        .as_ref()
        .map(|value| builder.create_string(value));
    let schema_inherits = string_vector_offset(builder, &definition.schema_inherits);
    let document_inherits = string_vector_offset(builder, &definition.document_inherits);
    let structs = named_struct_offsets(builder, &definition.structs);
    let fieldsets = fieldset_offsets(builder, &definition.fieldsets);
    let imported_fields = imported_field_offsets(builder, &definition.imported_fields);
    let document_summaries = document_summary_offsets(builder, &definition.document_summaries);
    let rank_profiles = unresolved_rank_profile_offsets(builder, &definition.rank_profiles);
    let constants = constant_offsets(builder, &definition.constants);
    let onnx_models = onnx_model_offsets(builder, &definition.onnx_models);
    api::IndexDefinition::create(
        builder,
        &api::IndexDefinitionArgs {
            name,
            fields: Some(fields),
            default_search_fields,
            properties,
            document_type,
            schema_inherits,
            document_inherits,
            structs,
            fieldsets,
            schema_fields,
            imported_fields,
            document_summaries,
            rank_profiles,
            constants,
            onnx_models,
        },
    )
}

fn unresolved_field_definition_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    field: &crate::api_to_ir::UnresolvedFieldInput,
) -> flatbuffers::WIPOffset<api::FieldDefinition<'a>> {
    let name = builder.create_string(field.name.as_str());
    let data_type = data_type_offset(builder, &field.data_type);
    let stored = field.stored.map(api::OptionalBool::new);
    let searchable = field.searchable.map(api::OptionalBool::new);
    let filterable = field.filterable.map(api::OptionalBool::new);
    let sortable = field.sortable.map(api::OptionalBool::new);
    let facetable = field.groupable.map(api::OptionalBool::new);
    let analyzer = field
        .analyzer
        .as_ref()
        .map(|value| analyzer_offset(builder, value));
    let matching = field
        .matching
        .as_ref()
        .map(|value| matching_offset(builder, value));
    let index = field
        .index
        .as_ref()
        .map(|value| index_config_offset(builder, value));
    let attribute = field
        .attribute
        .as_ref()
        .map(|value| attribute_config_offset(builder, value));
    let summary = field
        .summary
        .as_ref()
        .map(|value| summary_config_offset(builder, value));
    let indexing_script = field
        .indexing_script
        .as_ref()
        .map(|value| builder.create_string(value));
    let properties = request_value_entries_offset(builder, &field.properties);
    let struct_fields = struct_field_config_offsets(builder, &field.struct_fields);
    api::FieldDefinition::create(
        builder,
        &api::FieldDefinitionArgs {
            name: Some(name),
            data_type: Some(data_type),
            stored: stored.as_ref(),
            searchable: searchable.as_ref(),
            filterable: filterable.as_ref(),
            sortable: sortable.as_ref(),
            facetable: facetable.as_ref(),
            analyzer,
            matching,
            index,
            attribute,
            summary,
            indexing_script,
            properties,
            struct_fields,
        },
    )
}

fn unresolved_rank_profile_offsets<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    values: &[crate::api_to_ir::UnresolvedRankProfileInput],
) -> Option<
    flatbuffers::WIPOffset<
        flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<api::RankProfileDefinition<'a>>>,
    >,
> {
    (!values.is_empty()).then(|| {
        let values = values
            .iter()
            .map(|value| {
                let name = builder.create_string(&value.name);
                let inherits = string_vector_offset(builder, &value.inherits);
                let inputs = rank_input_offsets(builder, &value.inputs);
                let functions = rank_function_offsets(builder, &value.functions);
                let first_phase = value
                    .first_phase
                    .as_ref()
                    .map(|value| rank_phase_offset(builder, value));
                let second_phase = value
                    .second_phase
                    .as_ref()
                    .map(|value| rank_phase_offset(builder, value));
                let global_phase = value
                    .global_phase
                    .as_ref()
                    .map(|value| rank_phase_offset(builder, value));
                let rank_features = string_vector_offset(builder, &value.rank_features);
                let match_features = string_vector_offset(builder, &value.match_features);
                let summary_features = string_vector_offset(builder, &value.summary_features);
                let sort_features = string_vector_offset(builder, &value.sort_features);
                let constants = constant_offsets(builder, &value.constants);
                let onnx_models = onnx_model_offsets(builder, &value.onnx_models);
                let rank_properties = string_entry_offsets(builder, &value.rank_properties);
                let strict = value.strict.map(api::OptionalBool::new);
                let properties = request_value_entries_offset(builder, &value.properties);
                api::RankProfileDefinition::create(
                    builder,
                    &api::RankProfileDefinitionArgs {
                        name: Some(name),
                        inherits,
                        inputs,
                        functions,
                        first_phase,
                        second_phase,
                        global_phase,
                        rank_features,
                        match_features,
                        summary_features,
                        sort_features,
                        constants,
                        onnx_models,
                        rank_properties,
                        strict: strict.as_ref(),
                        properties,
                    },
                )
            })
            .collect::<Vec<_>>();
        builder.create_vector(&values)
    })
}

pub fn index_definition_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    definition: &crate::model::IndexDefinitionIr,
) -> flatbuffers::WIPOffset<api::IndexDefinition<'a>> {
    let name = definition
        .name
        .as_ref()
        .map(|v| builder.create_string(v.as_str()));
    let mut field_offsets = Vec::with_capacity(definition.fields.len());
    for field in &definition.fields {
        field_offsets.push(field_definition_offset(builder, field));
    }
    let fields = builder.create_vector(&field_offsets);
    let mut search_offsets = Vec::with_capacity(definition.default_search_fields.len());
    for search_field in &definition.default_search_fields {
        let field_name = builder.create_string(search_field.field.as_str());
        let weight = api::OptionalDouble::new(search_field.weight.get());
        search_offsets.push(api::SearchField::create(
            builder,
            &api::SearchFieldArgs {
                field: Some(field_name),
                weight: Some(&weight),
            },
        ));
    }
    let search_vector = builder.create_vector(&search_offsets);
    let selection = api::SearchFieldSelection::create(
        builder,
        &api::SearchFieldSelectionArgs {
            fields: Some(search_vector),
        },
    );
    let properties = request_value_entries_offset(builder, &definition.properties);
    let document_type = definition
        .document_type
        .as_ref()
        .map(|value| builder.create_string(value));
    let schema_inherits = string_vector_offset(builder, &definition.schema_inherits);
    let document_inherits = string_vector_offset(builder, &definition.document_inherits);
    let structs = named_struct_offsets(builder, &definition.structs);
    let fieldsets = fieldset_offsets(builder, &definition.fieldsets);
    let schema_fields = schema_field_offsets(builder, &definition.schema_fields);
    let imported_fields = imported_field_offsets(builder, &definition.imported_fields);
    let document_summaries = document_summary_offsets(builder, &definition.document_summaries);
    let rank_profiles = rank_profile_offsets(builder, &definition.rank_profiles);
    let constants = constant_offsets(builder, &definition.constants);
    let onnx_models = onnx_model_offsets(builder, &definition.onnx_models);
    api::IndexDefinition::create(
        builder,
        &api::IndexDefinitionArgs {
            name,
            fields: Some(fields),
            default_search_fields: Some(selection),
            properties,
            document_type,
            schema_inherits,
            document_inherits,
            structs,
            fieldsets,
            schema_fields,
            imported_fields,
            document_summaries,
            rank_profiles,
            constants,
            onnx_models,
        },
    )
}

pub(super) fn matching_mode_to_api(value: ir::MatchingModeIr) -> api::MatchingMode {
    match value {
        ir::MatchingModeIr::Unspecified => api::MatchingMode::Unspecified,
        ir::MatchingModeIr::Text => api::MatchingMode::Text,
        ir::MatchingModeIr::Word => api::MatchingMode::Word,
        ir::MatchingModeIr::Exact => api::MatchingMode::Exact,
        ir::MatchingModeIr::Gram => api::MatchingMode::Gram,
    }
}

pub(super) fn primitive_type_to_api(value: ir::PrimitiveDataTypeIr) -> api::PrimitiveDataTypeKind {
    match value {
        ir::PrimitiveDataTypeIr::String => api::PrimitiveDataTypeKind::String,
        ir::PrimitiveDataTypeIr::Bool => api::PrimitiveDataTypeKind::Bool,
        ir::PrimitiveDataTypeIr::Byte => api::PrimitiveDataTypeKind::Byte,
        ir::PrimitiveDataTypeIr::Int32 => api::PrimitiveDataTypeKind::Int32,
        ir::PrimitiveDataTypeIr::Int64 => api::PrimitiveDataTypeKind::Int64,
        ir::PrimitiveDataTypeIr::Float32 => api::PrimitiveDataTypeKind::Float32,
        ir::PrimitiveDataTypeIr::Float64 => api::PrimitiveDataTypeKind::Float64,
        ir::PrimitiveDataTypeIr::Raw => api::PrimitiveDataTypeKind::Raw,
    }
}

pub(super) fn data_type_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &ir::DataTypeIr,
) -> flatbuffers::WIPOffset<api::DataType<'a>> {
    let (kind_type, kind) = match value {
        ir::DataTypeIr::Primitive(value) => {
            let value = api::PrimitiveDataType::create(
                builder,
                &api::PrimitiveDataTypeArgs {
                    kind: primitive_type_to_api(*value),
                },
            );
            (api::DataTypeKind::PrimitiveDataType, value.as_union_value())
        }
        ir::DataTypeIr::Tensor { type_spec } => {
            let type_spec = builder.create_string(type_spec);
            let value = api::TensorDataType::create(
                builder,
                &api::TensorDataTypeArgs {
                    type_spec: Some(type_spec),
                },
            );
            (api::DataTypeKind::TensorDataType, value.as_union_value())
        }
        ir::DataTypeIr::Array(element_type) => {
            let element_type = data_type_offset(builder, element_type);
            let value = api::ArrayDataType::create(
                builder,
                &api::ArrayDataTypeArgs {
                    element_type: Some(element_type),
                },
            );
            (api::DataTypeKind::ArrayDataType, value.as_union_value())
        }
        ir::DataTypeIr::Map {
            key_type,
            value_type,
        } => {
            let key_type = data_type_offset(builder, key_type);
            let value_type = data_type_offset(builder, value_type);
            let value = api::MapDataType::create(
                builder,
                &api::MapDataTypeArgs {
                    key_type: Some(key_type),
                    value_type: Some(value_type),
                },
            );
            (api::DataTypeKind::MapDataType, value.as_union_value())
        }
        ir::DataTypeIr::WeightedSet(element_type) => {
            let element_type = data_type_offset(builder, element_type);
            let value = api::WeightedSetDataType::create(
                builder,
                &api::WeightedSetDataTypeArgs {
                    element_type: Some(element_type),
                },
            );
            (
                api::DataTypeKind::WeightedSetDataType,
                value.as_union_value(),
            )
        }
        ir::DataTypeIr::Struct { fields } => {
            let fields = struct_field_offsets(builder, fields);
            let value = api::StructDataType::create(
                builder,
                &api::StructDataTypeArgs {
                    fields: Some(fields),
                },
            );
            (api::DataTypeKind::StructDataType, value.as_union_value())
        }
        ir::DataTypeIr::StructReference { name } => {
            let name = builder.create_string(name);
            let value = api::StructReferenceDataType::create(
                builder,
                &api::StructReferenceDataTypeArgs { name: Some(name) },
            );
            (
                api::DataTypeKind::StructReferenceDataType,
                value.as_union_value(),
            )
        }
        ir::DataTypeIr::Predicate => {
            let value = api::PredicateDataType::create(builder, &api::PredicateDataTypeArgs {});
            (api::DataTypeKind::PredicateDataType, value.as_union_value())
        }
        ir::DataTypeIr::Position => {
            let value = api::PositionDataType::create(builder, &api::PositionDataTypeArgs {});
            (api::DataTypeKind::PositionDataType, value.as_union_value())
        }
        ir::DataTypeIr::Reference { document_type } => {
            let document_type = builder.create_string(document_type);
            let value = api::ReferenceDataType::create(
                builder,
                &api::ReferenceDataTypeArgs {
                    document_type: Some(document_type),
                },
            );
            (api::DataTypeKind::ReferenceDataType, value.as_union_value())
        }
    };
    api::DataType::create(
        builder,
        &api::DataTypeArgs {
            kind_type,
            kind: Some(kind),
        },
    )
}

pub(super) fn struct_field_offsets<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    fields: &[ir::StructFieldIr],
) -> flatbuffers::WIPOffset<
    flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<api::StructFieldDefinition<'a>>>,
> {
    let fields = fields
        .iter()
        .map(|field| {
            let name = builder.create_string(&field.name);
            let data_type = data_type_offset(builder, &field.data_type);
            api::StructFieldDefinition::create(
                builder,
                &api::StructFieldDefinitionArgs {
                    name: Some(name),
                    data_type: Some(data_type),
                },
            )
        })
        .collect::<Vec<_>>();
    builder.create_vector(&fields)
}

pub(super) fn string_vector_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    values: &[String],
) -> Option<flatbuffers::WIPOffset<flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<&'a str>>>>
{
    (!values.is_empty()).then(|| {
        let values = values
            .iter()
            .map(|value| builder.create_string(value))
            .collect::<Vec<_>>();
        builder.create_vector(&values)
    })
}

pub(super) fn matching_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &ir::MatchingConfigIr,
) -> flatbuffers::WIPOffset<api::MatchingConfig<'a>> {
    let options = request_value_entries_offset(builder, &value.options);
    api::MatchingConfig::create(
        builder,
        &api::MatchingConfigArgs {
            mode: matching_mode_to_api(value.mode),
            options,
        },
    )
}

pub(super) fn index_config_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &ir::IndexConfigIr,
) -> flatbuffers::WIPOffset<api::IndexConfig<'a>> {
    let enabled = value.enabled.map(api::OptionalBool::new);
    let hnsw = value.hnsw.as_ref().map(|value| {
        let max_links_per_node = value.max_links_per_node.map(api::OptionalUInt32::new);
        let neighbors_to_explore_at_insert = value
            .neighbors_to_explore_at_insert
            .map(api::OptionalUInt32::new);
        let options = request_value_entries_offset(builder, &value.options);
        api::HnswIndexConfig::create(
            builder,
            &api::HnswIndexConfigArgs {
                max_links_per_node: max_links_per_node.as_ref(),
                neighbors_to_explore_at_insert: neighbors_to_explore_at_insert.as_ref(),
                options,
            },
        )
    });
    let options = request_value_entries_offset(builder, &value.options);
    api::IndexConfig::create(
        builder,
        &api::IndexConfigArgs {
            enabled: enabled.as_ref(),
            hnsw,
            options,
        },
    )
}

pub(super) fn distance_metric_to_api(value: ir::DistanceMetricIr) -> api::DistanceMetric {
    match value {
        ir::DistanceMetricIr::Unspecified => api::DistanceMetric::Unspecified,
        ir::DistanceMetricIr::Euclidean => api::DistanceMetric::Euclidean,
        ir::DistanceMetricIr::Angular => api::DistanceMetric::Angular,
        ir::DistanceMetricIr::DotProduct => api::DistanceMetric::DotProduct,
        ir::DistanceMetricIr::PrenormalizedAngular => api::DistanceMetric::PrenormalizedAngular,
        ir::DistanceMetricIr::GeoDegrees => api::DistanceMetric::GeoDegrees,
        ir::DistanceMetricIr::Hamming => api::DistanceMetric::Hamming,
    }
}

pub(super) fn attribute_config_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &ir::AttributeConfigIr,
) -> flatbuffers::WIPOffset<api::AttributeConfig<'a>> {
    let enabled = value.enabled.map(api::OptionalBool::new);
    let fast_search = value.fast_search.map(api::OptionalBool::new);
    let fast_access = value.fast_access.map(api::OptionalBool::new);
    let options = request_value_entries_offset(builder, &value.options);
    api::AttributeConfig::create(
        builder,
        &api::AttributeConfigArgs {
            enabled: enabled.as_ref(),
            fast_search: fast_search.as_ref(),
            fast_access: fast_access.as_ref(),
            distance_metric: distance_metric_to_api(value.distance_metric),
            options,
        },
    )
}

pub(super) fn summary_config_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &ir::SummaryConfigIr,
) -> flatbuffers::WIPOffset<api::SummaryConfig<'a>> {
    let enabled = value.enabled.map(api::OptionalBool::new);
    let options = request_value_entries_offset(builder, &value.options);
    api::SummaryConfig::create(
        builder,
        &api::SummaryConfigArgs {
            enabled: enabled.as_ref(),
            options,
        },
    )
}

pub(super) fn analyzer_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &ir::AnalyzerIr,
) -> flatbuffers::WIPOffset<api::Analyzer<'a>> {
    let name = builder.create_string(&value.name);
    let options = request_value_entries_offset(builder, &value.options);
    api::Analyzer::create(
        builder,
        &api::AnalyzerArgs {
            name: Some(name),
            version: value.version,
            options,
        },
    )
}

pub(super) fn struct_field_config_offsets<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    values: &[ir::StructFieldConfigIr],
) -> Option<
    flatbuffers::WIPOffset<
        flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<api::StructFieldConfig<'a>>>,
    >,
> {
    (!values.is_empty()).then(|| {
        let values = values
            .iter()
            .map(|value| {
                let path = builder.create_string(&value.path);
                let indexing_script = value
                    .indexing_script
                    .as_ref()
                    .map(|value| builder.create_string(value));
                let matching = value
                    .matching
                    .as_ref()
                    .map(|value| matching_offset(builder, value));
                let index = value
                    .index
                    .as_ref()
                    .map(|value| index_config_offset(builder, value));
                let attribute = value
                    .attribute
                    .as_ref()
                    .map(|value| attribute_config_offset(builder, value));
                let summary = value
                    .summary
                    .as_ref()
                    .map(|value| summary_config_offset(builder, value));
                let rank = value
                    .rank
                    .as_ref()
                    .map(|value| builder.create_string(value));
                let query_commands = string_vector_offset(builder, &value.query_commands);
                let properties = request_value_entries_offset(builder, &value.properties);
                api::StructFieldConfig::create(
                    builder,
                    &api::StructFieldConfigArgs {
                        path: Some(path),
                        indexing_script,
                        matching,
                        index,
                        attribute,
                        summary,
                        rank,
                        query_commands,
                        properties,
                    },
                )
            })
            .collect::<Vec<_>>();
        builder.create_vector(&values)
    })
}

pub(super) fn field_definition_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    field: &ir::FieldIr,
) -> flatbuffers::WIPOffset<api::FieldDefinition<'a>> {
    let name = builder.create_string(field.name.as_str());
    let data_type = data_type_offset(builder, &field.data_type);
    // An indexing script owns the field's resolved capabilities in the
    // canonical V2 form.  Long-tail properties and nested struct-field
    // configuration are independent metadata and must not force inferred
    // capability booleans back into the wire representation.
    let script_only = field.indexing_script.is_some();
    let stored = (!script_only).then(|| api::OptionalBool::new(field.capabilities.stored));
    let searchable = (!script_only).then(|| api::OptionalBool::new(field.capabilities.searchable));
    let filterable = (!script_only).then(|| api::OptionalBool::new(field.capabilities.filterable));
    let sortable = (!script_only).then(|| api::OptionalBool::new(field.capabilities.sortable));
    let facetable = (!script_only).then(|| api::OptionalBool::new(field.capabilities.groupable));
    let indexing_script = field
        .indexing_script
        .as_ref()
        .map(|value| builder.create_string(value));
    let analyzer = field
        .analyzer
        .as_ref()
        .map(|value| analyzer_offset(builder, value));
    let matching = field
        .matching
        .as_ref()
        .map(|value| matching_offset(builder, value));
    let index = field
        .index
        .as_ref()
        .map(|value| index_config_offset(builder, value));
    let attribute = field
        .attribute
        .as_ref()
        .map(|value| attribute_config_offset(builder, value));
    let summary = field
        .summary
        .as_ref()
        .map(|value| summary_config_offset(builder, value));
    let properties = request_value_entries_offset(builder, &field.properties);
    let struct_fields = struct_field_config_offsets(builder, &field.struct_fields);
    api::FieldDefinition::create(
        builder,
        &api::FieldDefinitionArgs {
            name: Some(name),
            data_type: Some(data_type),
            stored: stored.as_ref(),
            searchable: searchable.as_ref(),
            filterable: filterable.as_ref(),
            sortable: sortable.as_ref(),
            facetable: facetable.as_ref(),
            analyzer,
            matching,
            index,
            attribute,
            summary,
            indexing_script,
            properties,
            struct_fields,
        },
    )
}

pub(super) fn named_struct_offsets<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    values: &[ir::NamedStructDefinitionIr],
) -> Option<
    flatbuffers::WIPOffset<
        flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<api::NamedStructDefinition<'a>>>,
    >,
> {
    (!values.is_empty()).then(|| {
        let values = values
            .iter()
            .map(|value| {
                let name = builder.create_string(&value.name);
                let fields = struct_field_offsets(builder, &value.fields);
                let properties = request_value_entries_offset(builder, &value.properties);
                api::NamedStructDefinition::create(
                    builder,
                    &api::NamedStructDefinitionArgs {
                        name: Some(name),
                        fields: Some(fields),
                        properties,
                    },
                )
            })
            .collect::<Vec<_>>();
        builder.create_vector(&values)
    })
}

pub(super) fn fieldset_offsets<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    values: &[ir::FieldSetDefinitionIr],
) -> Option<
    flatbuffers::WIPOffset<
        flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<api::FieldSetDefinition<'a>>>,
    >,
> {
    (!values.is_empty()).then(|| {
        let values = values
            .iter()
            .map(|value| {
                let name = builder.create_string(&value.name);
                let fields = value
                    .fields
                    .iter()
                    .map(|field| builder.create_string(field.as_str()))
                    .collect::<Vec<_>>();
                let fields = builder.create_vector(&fields);
                let matching = value
                    .matching
                    .as_ref()
                    .map(|value| matching_offset(builder, value));
                let query_commands = string_vector_offset(builder, &value.query_commands);
                let properties = request_value_entries_offset(builder, &value.properties);
                api::FieldSetDefinition::create(
                    builder,
                    &api::FieldSetDefinitionArgs {
                        name: Some(name),
                        fields: Some(fields),
                        matching,
                        query_commands,
                        properties,
                    },
                )
            })
            .collect::<Vec<_>>();
        builder.create_vector(&values)
    })
}

pub(super) fn schema_field_offsets<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    values: &[ir::FieldIr],
) -> Option<
    flatbuffers::WIPOffset<
        flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<api::SearchOnlyFieldDefinition<'a>>>,
    >,
> {
    (!values.is_empty()).then(|| {
        let values = values
            .iter()
            .map(|value| {
                let field = field_definition_offset(builder, value);
                api::SearchOnlyFieldDefinition::create(
                    builder,
                    &api::SearchOnlyFieldDefinitionArgs { field: Some(field) },
                )
            })
            .collect::<Vec<_>>();
        builder.create_vector(&values)
    })
}

pub(super) fn imported_field_offsets<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    values: &[ir::ImportedFieldDefinitionIr],
) -> Option<
    flatbuffers::WIPOffset<
        flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<api::ImportedFieldDefinition<'a>>>,
    >,
> {
    (!values.is_empty()).then(|| {
        let values = values
            .iter()
            .map(|value| {
                let reference_field = builder.create_string(value.reference_field.as_str());
                let target_field = builder.create_string(&value.target_field);
                let alias = builder.create_string(value.alias.as_str());
                let properties = request_value_entries_offset(builder, &value.properties);
                api::ImportedFieldDefinition::create(
                    builder,
                    &api::ImportedFieldDefinitionArgs {
                        reference_field: Some(reference_field),
                        target_field: Some(target_field),
                        alias: Some(alias),
                        properties,
                    },
                )
            })
            .collect::<Vec<_>>();
        builder.create_vector(&values)
    })
}

pub(super) fn document_summary_offsets<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    values: &[ir::DocumentSummaryDefinitionIr],
) -> Option<
    flatbuffers::WIPOffset<
        flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<api::DocumentSummaryDefinition<'a>>>,
    >,
> {
    (!values.is_empty()).then(|| {
        let values = values
            .iter()
            .map(|value| {
                let name = builder.create_string(&value.name);
                let inherits = string_vector_offset(builder, &value.inherits);
                let fields = document_summary_field_offsets(builder, &value.fields);
                let from_disk = value.from_disk.map(api::OptionalBool::new);
                let omit_summary_features = value.omit_summary_features.map(api::OptionalBool::new);
                let properties = request_value_entries_offset(builder, &value.properties);
                api::DocumentSummaryDefinition::create(
                    builder,
                    &api::DocumentSummaryDefinitionArgs {
                        name: Some(name),
                        inherits,
                        fields,
                        from_disk: from_disk.as_ref(),
                        omit_summary_features: omit_summary_features.as_ref(),
                        properties,
                    },
                )
            })
            .collect::<Vec<_>>();
        builder.create_vector(&values)
    })
}

pub(super) fn document_summary_field_offsets<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    values: &[ir::DocumentSummaryFieldDefinitionIr],
) -> Option<
    flatbuffers::WIPOffset<
        flatbuffers::Vector<
            'a,
            flatbuffers::ForwardsUOffset<api::DocumentSummaryFieldDefinition<'a>>,
        >,
    >,
> {
    (!values.is_empty()).then(|| {
        let values = values
            .iter()
            .map(|value| {
                let name = builder.create_string(&value.name);
                let source = builder.create_string(&value.source);
                let data_type = data_type_offset(builder, &value.data_type);
                let struct_fields = string_vector_offset(builder, &value.struct_fields);
                let properties = request_value_entries_offset(builder, &value.properties);
                api::DocumentSummaryFieldDefinition::create(
                    builder,
                    &api::DocumentSummaryFieldDefinitionArgs {
                        name: Some(name),
                        source: Some(source),
                        data_type: Some(data_type),
                        struct_fields,
                        properties,
                    },
                )
            })
            .collect::<Vec<_>>();
        builder.create_vector(&values)
    })
}

pub(super) fn string_entry_offsets<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    values: &std::collections::BTreeMap<String, String>,
) -> Option<
    flatbuffers::WIPOffset<
        flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<api::StringEntry<'a>>>,
    >,
> {
    (!values.is_empty()).then(|| {
        let values = values
            .iter()
            .map(|(key, value)| {
                let key = builder.create_string(key);
                let value = builder.create_string(value);
                api::StringEntry::create(
                    builder,
                    &api::StringEntryArgs {
                        key: Some(key),
                        value: Some(value),
                    },
                )
            })
            .collect::<Vec<_>>();
        builder.create_vector(&values)
    })
}

pub(super) fn constant_offsets<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    values: &[ir::ConstantDefinitionIr],
) -> Option<
    flatbuffers::WIPOffset<
        flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<api::ConstantDefinition<'a>>>,
    >,
> {
    (!values.is_empty()).then(|| {
        let values = values
            .iter()
            .map(|value| {
                let name = builder.create_string(&value.name);
                let data_type = data_type_offset(builder, &value.data_type);
                let (source_type, source) = match &value.source {
                    ir::ConstantSourceIr::Literal(value) => {
                        let value = request_value_offset(builder, value);
                        let source = api::LiteralConstantSource::create(
                            builder,
                            &api::LiteralConstantSourceArgs { value: Some(value) },
                        );
                        (
                            api::ConstantSourceKind::LiteralConstantSource,
                            source.as_union_value(),
                        )
                    }
                    ir::ConstantSourceIr::File { path } => {
                        let path = builder.create_string(path);
                        let source = api::FileConstantSource::create(
                            builder,
                            &api::FileConstantSourceArgs { path: Some(path) },
                        );
                        (
                            api::ConstantSourceKind::FileConstantSource,
                            source.as_union_value(),
                        )
                    }
                };
                let properties = request_value_entries_offset(builder, &value.properties);
                api::ConstantDefinition::create(
                    builder,
                    &api::ConstantDefinitionArgs {
                        name: Some(name),
                        data_type: Some(data_type),
                        source_type,
                        source: Some(source),
                        properties,
                    },
                )
            })
            .collect::<Vec<_>>();
        builder.create_vector(&values)
    })
}

pub(super) fn onnx_model_offsets<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    values: &[ir::OnnxModelDefinitionIr],
) -> Option<
    flatbuffers::WIPOffset<
        flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<api::OnnxModelDefinition<'a>>>,
    >,
> {
    (!values.is_empty()).then(|| {
        let values = values
            .iter()
            .map(|value| {
                let name = builder.create_string(&value.name);
                let file = builder.create_string(&value.file);
                let inputs = string_entry_offsets(builder, &value.inputs);
                let outputs = string_entry_offsets(builder, &value.outputs);
                let gpu_device = value.gpu_device.map(api::OptionalUInt32::new);
                let intraop_threads = value.intraop_threads.map(api::OptionalUInt32::new);
                let interop_threads = value.interop_threads.map(api::OptionalUInt32::new);
                let properties = request_value_entries_offset(builder, &value.properties);
                api::OnnxModelDefinition::create(
                    builder,
                    &api::OnnxModelDefinitionArgs {
                        name: Some(name),
                        file: Some(file),
                        inputs,
                        outputs,
                        gpu_device: gpu_device.as_ref(),
                        intraop_threads: intraop_threads.as_ref(),
                        interop_threads: interop_threads.as_ref(),
                        properties,
                    },
                )
            })
            .collect::<Vec<_>>();
        builder.create_vector(&values)
    })
}

pub(super) fn rank_phase_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &ir::RankPhaseDefinitionIr,
) -> flatbuffers::WIPOffset<api::RankPhaseDefinition<'a>> {
    let expression = value
        .expression
        .as_ref()
        .map(|value| builder.create_string(value));
    let rerank_count = value.rerank_count.map(api::OptionalUInt32::new);
    let rank_score_drop_limit = value.rank_score_drop_limit.map(api::OptionalDouble::new);
    let total_rerank_count = value.total_rerank_count.map(api::OptionalUInt32::new);
    let keep_rank_count = value.keep_rank_count.map(api::OptionalUInt32::new);
    let total_keep_rank_count = value.total_keep_rank_count.map(api::OptionalUInt32::new);
    let properties = request_value_entries_offset(builder, &value.properties);
    api::RankPhaseDefinition::create(
        builder,
        &api::RankPhaseDefinitionArgs {
            expression,
            rerank_count: rerank_count.as_ref(),
            rank_score_drop_limit: rank_score_drop_limit.as_ref(),
            properties,
            total_rerank_count: total_rerank_count.as_ref(),
            keep_rank_count: keep_rank_count.as_ref(),
            total_keep_rank_count: total_keep_rank_count.as_ref(),
        },
    )
}

pub(super) fn rank_profile_offsets<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    values: &[ir::RankProfileDefinitionIr],
) -> Option<
    flatbuffers::WIPOffset<
        flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<api::RankProfileDefinition<'a>>>,
    >,
> {
    (!values.is_empty()).then(|| {
        let values = values
            .iter()
            .map(|value| {
                let name = builder.create_string(&value.name);
                let inherits = string_vector_offset(builder, &value.inherits);
                let inputs = rank_input_offsets(builder, &value.inputs);
                let functions = rank_function_offsets(builder, &value.functions);
                let first_phase = value
                    .first_phase
                    .as_ref()
                    .map(|value| rank_phase_offset(builder, value));
                let second_phase = value
                    .second_phase
                    .as_ref()
                    .map(|value| rank_phase_offset(builder, value));
                let global_phase = value
                    .global_phase
                    .as_ref()
                    .map(|value| rank_phase_offset(builder, value));
                let rank_features = string_vector_offset(builder, &value.rank_features);
                let match_features = string_vector_offset(builder, &value.match_features);
                let summary_features = string_vector_offset(builder, &value.summary_features);
                let sort_features = string_vector_offset(builder, &value.sort_features);
                let constants = constant_offsets(builder, &value.constants);
                let onnx_models = onnx_model_offsets(builder, &value.onnx_models);
                let rank_properties = string_entry_offsets(builder, &value.rank_properties);
                let strict = value.strict.map(api::OptionalBool::new);
                let properties = request_value_entries_offset(builder, &value.properties);
                api::RankProfileDefinition::create(
                    builder,
                    &api::RankProfileDefinitionArgs {
                        name: Some(name),
                        inherits,
                        inputs,
                        functions,
                        first_phase,
                        second_phase,
                        global_phase,
                        rank_features,
                        match_features,
                        summary_features,
                        sort_features,
                        constants,
                        onnx_models,
                        rank_properties,
                        strict: strict.as_ref(),
                        properties,
                    },
                )
            })
            .collect::<Vec<_>>();
        builder.create_vector(&values)
    })
}

pub(super) fn rank_input_offsets<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    values: &[ir::RankProfileInputIr],
) -> Option<
    flatbuffers::WIPOffset<
        flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<api::RankProfileInput<'a>>>,
    >,
> {
    (!values.is_empty()).then(|| {
        let values = values
            .iter()
            .map(|value| {
                let name = builder.create_string(&value.name);
                let data_type = data_type_offset(builder, &value.data_type);
                let default_value = request_value_offset(builder, &value.default_value);
                api::RankProfileInput::create(
                    builder,
                    &api::RankProfileInputArgs {
                        name: Some(name),
                        data_type: Some(data_type),
                        default_value: Some(default_value),
                    },
                )
            })
            .collect::<Vec<_>>();
        builder.create_vector(&values)
    })
}

pub(super) fn rank_function_offsets<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    values: &[ir::RankFunctionDefinitionIr],
) -> Option<
    flatbuffers::WIPOffset<
        flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<api::RankFunctionDefinition<'a>>>,
    >,
> {
    (!values.is_empty()).then(|| {
        let values = values
            .iter()
            .map(|value| {
                let name = builder.create_string(&value.name);
                let arguments = string_vector_offset(builder, &value.arguments);
                let expression = builder.create_string(&value.expression);
                let inline = value.inline.map(api::OptionalBool::new);
                api::RankFunctionDefinition::create(
                    builder,
                    &api::RankFunctionDefinitionArgs {
                        name: Some(name),
                        arguments,
                        expression: Some(expression),
                        inline: inline.as_ref(),
                    },
                )
            })
            .collect::<Vec<_>>();
        builder.create_vector(&values)
    })
}
