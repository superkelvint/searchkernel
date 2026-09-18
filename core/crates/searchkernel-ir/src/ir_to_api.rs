//! Owned Search Kernel IR to portable FlatBuffers encoding.

use crate::error::{ErrorCode, PortableError};
use crate::model as ir;
use searchkernel_protocol::v2 as api;

/// Encode a portable error as a standalone FlatBuffers `Error` table.
pub fn error_to_api(error: &PortableError) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let root = error_offset(&mut builder, error);
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

/// Encode a portable error into an existing FlatBuffers builder.
pub fn error_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    error: &PortableError,
) -> flatbuffers::WIPOffset<api::Error<'a>> {
    let message = builder.create_string(&error.message);
    let mut detail_offsets = Vec::with_capacity(error.details.len());
    for detail in &error.details {
        let code = builder.create_string(&detail.code);
        let path = detail.path.as_deref().map(|v| builder.create_string(v));
        let text = builder.create_string(&detail.message);
        let metadata = if detail.metadata.is_empty() {
            None
        } else {
            let mut entries = Vec::with_capacity(detail.metadata.len());
            for (key, value) in &detail.metadata {
                let key = builder.create_string(key);
                let value = builder.create_string(value);
                entries.push(api::StringEntry::create(
                    builder,
                    &api::StringEntryArgs {
                        key: Some(key),
                        value: Some(value),
                    },
                ));
            }
            Some(builder.create_vector(&entries))
        };
        detail_offsets.push(api::ErrorDetail::create(
            builder,
            &api::ErrorDetailArgs {
                code: Some(code),
                path,
                message: Some(text),
                metadata,
            },
        ));
    }
    let details = (!detail_offsets.is_empty()).then(|| builder.create_vector(&detail_offsets));
    api::Error::create(
        builder,
        &api::ErrorArgs {
            code: error_code(error.code),
            message: Some(message),
            details,
            retryable: error.retryable,
        },
    )
}

fn error_code(code: ErrorCode) -> api::ErrorCode {
    match code {
        ErrorCode::InvalidArgument => api::ErrorCode::InvalidArgument,
        ErrorCode::NotFound => api::ErrorCode::NotFound,
        ErrorCode::AlreadyExists => api::ErrorCode::AlreadyExists,
        ErrorCode::FailedPrecondition => api::ErrorCode::FailedPrecondition,
        ErrorCode::Conflict => api::ErrorCode::Conflict,
        ErrorCode::Unsupported => api::ErrorCode::Unsupported,
        ErrorCode::ResourceExhausted => api::ErrorCode::ResourceExhausted,
        ErrorCode::Internal => api::ErrorCode::Internal,
        ErrorCode::Unavailable => api::ErrorCode::Unavailable,
        ErrorCode::DeadlineExceeded => api::ErrorCode::DeadlineExceeded,
    }
}

/// Encode the complete normalized request inside an `InspectQueryResult`.
pub fn inspect_query_result_to_api(request: &crate::model::SearchRequestIr) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let request = resolved_request_offset(&mut builder, request);
    let root = api::InspectQueryResult::create(
        &mut builder,
        &api::InspectQueryResultArgs {
            resolved_request: Some(request),
        },
    );
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

/// Encode an InspectQuery result into an existing transport builder.
pub fn inspect_query_result_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    request: &crate::model::SearchRequestIr,
) -> flatbuffers::WIPOffset<api::InspectQueryResult<'a>> {
    let resolved_request = resolved_request_offset(builder, request);
    api::InspectQueryResult::create(
        builder,
        &api::InspectQueryResultArgs {
            resolved_request: Some(resolved_request),
        },
    )
}

/// Encode only the normalized `SearchRequest` table.
pub fn search_request_ir_to_api(request: &crate::model::SearchRequestIr) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let root = search_request_offset(&mut builder, request);
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

/// Encode the effective index definition inside a `DescribeResult`.
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

fn index_definition_offset<'a>(
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

fn matching_mode_to_api(value: ir::MatchingModeIr) -> api::MatchingMode {
    match value {
        ir::MatchingModeIr::Unspecified => api::MatchingMode::Unspecified,
        ir::MatchingModeIr::Text => api::MatchingMode::Text,
        ir::MatchingModeIr::Word => api::MatchingMode::Word,
        ir::MatchingModeIr::Exact => api::MatchingMode::Exact,
        ir::MatchingModeIr::Gram => api::MatchingMode::Gram,
    }
}

fn primitive_type_to_api(value: ir::PrimitiveDataTypeIr) -> api::PrimitiveDataTypeKind {
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

fn data_type_offset<'a>(
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

fn struct_field_offsets<'a>(
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

fn string_vector_offset<'a>(
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

fn matching_offset<'a>(
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

fn index_config_offset<'a>(
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

fn distance_metric_to_api(value: ir::DistanceMetricIr) -> api::DistanceMetric {
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

fn attribute_config_offset<'a>(
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

fn summary_config_offset<'a>(
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

fn analyzer_offset<'a>(
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

fn struct_field_config_offsets<'a>(
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

fn field_definition_offset<'a>(
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

fn named_struct_offsets<'a>(
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

fn fieldset_offsets<'a>(
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

fn schema_field_offsets<'a>(
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

fn imported_field_offsets<'a>(
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

fn document_summary_offsets<'a>(
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

fn document_summary_field_offsets<'a>(
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

fn string_entry_offsets<'a>(
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

fn constant_offsets<'a>(
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

fn onnx_model_offsets<'a>(
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

fn rank_phase_offset<'a>(
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

fn rank_profile_offsets<'a>(
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

fn rank_input_offsets<'a>(
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

fn rank_function_offsets<'a>(
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

fn search_request_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    request: &ir::SearchRequestIr,
) -> flatbuffers::WIPOffset<api::SearchRequest<'a>> {
    let query = query_source_offset(builder, &request.query);

    let search_fields = search_field_selection_offset(builder, &request.search_fields);
    let filter = request.filter.as_ref().map(|value| {
        let expression = filter_offset(builder, value);
        let expressions = builder.create_vector(&[expression]);
        api::FilterRequest::create(
            builder,
            &api::FilterRequestArgs {
                structured: Some(expressions),
                sources: None,
                parameters: None,
            },
        )
    });
    let sorts = request
        .sort
        .iter()
        .map(|value| sort_offset(builder, value))
        .collect::<Vec<_>>();
    let sort = builder.create_vector(&sorts);
    let ranking = ranking_request_offset(builder, &request.ranking_request);
    let limit = api::OptionalUInt32::new(request.limit);
    let offset = api::OptionalUInt32::new(request.offset);
    let presentation =
        presentation_offset(builder, request).expect("presentation is explicit in normalized IR");
    let timeout = request
        .timeout
        .map(|value| api::OptionalUInt64::new(value.as_millis() as u64));
    let inputs = query_inputs_offset(builder, &request.inputs);
    let query_model = query_model_offset(builder, &request.query_model);
    let properties = request_value_entries_offset(builder, &request.properties);
    let query_profile = request
        .query_profile
        .as_ref()
        .map(|value| builder.create_string(value));
    let search_chain = request
        .search_chain
        .as_ref()
        .map(|value| builder.create_string(value));
    let grouping = grouping_offsets(builder, &request.grouping);

    api::SearchRequest::create(
        builder,
        &api::SearchRequestArgs {
            query: Some(query),
            search_fields: Some(search_fields),
            filters: filter,
            sort: Some(sort),
            ranking: Some(ranking),
            limit: Some(&limit),
            offset: Some(&offset),
            timeout_ms: timeout.as_ref(),
            grouping,
            inputs,
            query_model,
            presentation: Some(presentation),
            properties,
            query_profile,
            search_chain,
        },
    )
}

fn resolved_request_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    request: &ir::SearchRequestIr,
) -> flatbuffers::WIPOffset<api::ResolvedSearchRequest<'a>> {
    let query = query_source_offset(builder, &request.query);
    let search_fields = search_fields_as_vector(builder, &request.search_fields);
    let filters = request.filter.as_ref().map(|value| {
        let expression = filter_offset(builder, value);
        builder.create_vector(&[expression])
    });
    let sorts = request
        .sort
        .iter()
        .map(|value| resolved_sort_offset(builder, value))
        .collect::<Vec<_>>();
    let sort = builder.create_vector(&sorts);
    let ranking = ranking_request_offset(builder, &request.ranking_request);
    let limit = api::OptionalUInt32::new(request.limit);
    let offset = api::OptionalUInt32::new(request.offset);
    let timeout = request
        .timeout
        .map(|value| api::OptionalUInt64::new(value.as_millis() as u64));
    let inputs = query_inputs_offset(builder, &request.inputs);
    let query_model = query_model_offset(builder, &request.query_model);
    let presentation = presentation_offset(builder, request);
    let properties = request_value_entries_offset(builder, &request.properties);
    let query_profile = request
        .query_profile
        .as_ref()
        .map(|value| builder.create_string(value));
    let search_chain = request
        .search_chain
        .as_ref()
        .map(|value| builder.create_string(value));
    let grouping = resolved_grouping_offsets(builder, &request.grouping);
    api::ResolvedSearchRequest::create(
        builder,
        &api::ResolvedSearchRequestArgs {
            query: Some(query),
            search_fields: Some(search_fields),
            filters,
            sort: Some(sort),
            grouping,
            ranking: Some(ranking),
            inputs,
            limit: limit.value(),
            offset: offset.value(),
            timeout_ms: timeout.as_ref(),
            query_model,
            presentation,
            properties,
            query_profile,
            search_chain,
        },
    )
}

fn search_fields_as_vector<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    fields: &[ir::SearchFieldIr],
) -> flatbuffers::WIPOffset<
    flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<api::SearchField<'a>>>,
> {
    let offsets = fields
        .iter()
        .map(|field| {
            let name = builder.create_string(field.field.as_str());
            let weight = api::OptionalDouble::new(field.weight.get());
            api::SearchField::create(
                builder,
                &api::SearchFieldArgs {
                    field: Some(name),
                    weight: Some(&weight),
                },
            )
        })
        .collect::<Vec<_>>();
    builder.create_vector(&offsets)
}

fn ranking_request_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    ranking: &ir::RankingRequestIr,
) -> flatbuffers::WIPOffset<api::RankingRequest<'a>> {
    let properties = (!ranking.properties.is_empty()).then(|| {
        let entries = ranking
            .properties
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
        builder.create_vector(&entries)
    });
    let profile = builder.create_string(&ranking.profile);
    api::RankingRequest::create(
        builder,
        &api::RankingRequestArgs {
            profile: Some(profile),
            properties,
        },
    )
}

fn request_value_entries_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    values: &std::collections::BTreeMap<String, ir::RequestValueIr>,
) -> Option<
    flatbuffers::WIPOffset<
        flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<api::ValueEntry<'a>>>,
    >,
> {
    (!values.is_empty()).then(|| {
        let entries = values
            .iter()
            .map(|(key, value)| {
                let key = builder.create_string(key);
                let value = request_value_offset(builder, value);
                api::ValueEntry::create(
                    builder,
                    &api::ValueEntryArgs {
                        key: Some(key),
                        value: Some(value),
                    },
                )
            })
            .collect::<Vec<_>>();
        builder.create_vector(&entries)
    })
}

fn request_value_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &ir::RequestValueIr,
) -> flatbuffers::WIPOffset<api::Value<'a>> {
    let (kind_type, kind) = match value {
        ir::RequestValueIr::Null => (
            api::ValueKind::NullValueValue,
            api::NullValueValue::create(
                builder,
                &api::NullValueValueArgs {
                    value: api::NullValue::Null,
                },
            )
            .as_union_value(),
        ),
        ir::RequestValueIr::Bool(value) => (
            api::ValueKind::BoolValue,
            api::BoolValue::create(builder, &api::BoolValueArgs { value: *value }).as_union_value(),
        ),
        ir::RequestValueIr::Int64(value) => (
            api::ValueKind::Int64Value,
            api::Int64Value::create(builder, &api::Int64ValueArgs { value: *value })
                .as_union_value(),
        ),
        ir::RequestValueIr::UInt64(value) => (
            api::ValueKind::UInt64Value,
            api::UInt64Value::create(builder, &api::UInt64ValueArgs { value: *value })
                .as_union_value(),
        ),
        ir::RequestValueIr::Float64(value) => (
            api::ValueKind::Float64Value,
            api::Float64Value::create(builder, &api::Float64ValueArgs { value: *value })
                .as_union_value(),
        ),
        ir::RequestValueIr::NonFiniteFloat(value) => {
            let kind = match value {
                ir::RequestNonFiniteFloatIr::NaN => api::NonFiniteFloatKind::NaN,
                ir::RequestNonFiniteFloatIr::PositiveInfinity => {
                    api::NonFiniteFloatKind::PositiveInfinity
                }
                ir::RequestNonFiniteFloatIr::NegativeInfinity => {
                    api::NonFiniteFloatKind::NegativeInfinity
                }
            };
            (
                api::ValueKind::NonFiniteFloatValue,
                api::NonFiniteFloatValue::create(builder, &api::NonFiniteFloatValueArgs { kind })
                    .as_union_value(),
            )
        }
        ir::RequestValueIr::String(value) => {
            let value = builder.create_string(value);
            (
                api::ValueKind::StringValue,
                api::StringValue::create(builder, &api::StringValueArgs { value: Some(value) })
                    .as_union_value(),
            )
        }
        ir::RequestValueIr::Bytes(value) => {
            let value = builder.create_vector(value);
            (
                api::ValueKind::BytesValue,
                api::BytesValue::create(builder, &api::BytesValueArgs { value: Some(value) })
                    .as_union_value(),
            )
        }
        ir::RequestValueIr::List(values) => {
            let values = values
                .iter()
                .map(|value| request_value_offset(builder, value))
                .collect::<Vec<_>>();
            let values = builder.create_vector(&values);
            (
                api::ValueKind::ValueList,
                api::ValueList::create(
                    builder,
                    &api::ValueListArgs {
                        values: Some(values),
                    },
                )
                .as_union_value(),
            )
        }
        ir::RequestValueIr::Object(values) => {
            let entries = values
                .iter()
                .map(|(key, value)| {
                    let key = builder.create_string(key);
                    let value = request_value_offset(builder, value);
                    api::ValueEntry::create(
                        builder,
                        &api::ValueEntryArgs {
                            key: Some(key),
                            value: Some(value),
                        },
                    )
                })
                .collect::<Vec<_>>();
            let values = builder.create_vector(&entries);
            (
                api::ValueKind::ValueObject,
                api::ValueObject::create(
                    builder,
                    &api::ValueObjectArgs {
                        fields: Some(values),
                    },
                )
                .as_union_value(),
            )
        }
        ir::RequestValueIr::Tensor(value) => (
            api::ValueKind::TensorValue,
            request_tensor_offset(builder, value).as_union_value(),
        ),
    };
    api::Value::create(
        builder,
        &api::ValueArgs {
            kind_type,
            kind: Some(kind),
        },
    )
}

fn tensor_address_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    address: &std::collections::BTreeMap<String, String>,
) -> flatbuffers::WIPOffset<
    flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<api::TensorAddressEntry<'a>>>,
> {
    let values = address
        .iter()
        .map(|(dimension, label)| {
            let dimension = builder.create_string(dimension);
            let label = builder.create_string(label);
            api::TensorAddressEntry::create(
                builder,
                &api::TensorAddressEntryArgs {
                    dimension: Some(dimension),
                    label: Some(label),
                },
            )
        })
        .collect::<Vec<_>>();
    builder.create_vector(&values)
}

fn request_tensor_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    tensor: &ir::RequestTensorIr,
) -> flatbuffers::WIPOffset<api::TensorValue<'a>> {
    let type_spec = builder.create_string(&tensor.type_spec);
    let (payload_type, payload) = match &tensor.payload {
        ir::RequestTensorPayloadIr::Dense(values) => {
            let values = builder.create_vector(values);
            let value = api::DenseTensorValues::create(
                builder,
                &api::DenseTensorValuesArgs {
                    values: Some(values),
                },
            );
            (
                api::TensorPayloadKind::DenseTensorValues,
                value.as_union_value(),
            )
        }
        ir::RequestTensorPayloadIr::Sparse(cells) => {
            let cells = cells
                .iter()
                .map(|cell| {
                    let address = tensor_address_offset(builder, &cell.address);
                    api::TensorCell::create(
                        builder,
                        &api::TensorCellArgs {
                            address: Some(address),
                            value: cell.value,
                        },
                    )
                })
                .collect::<Vec<_>>();
            let cells = builder.create_vector(&cells);
            let value = api::SparseTensorCells::create(
                builder,
                &api::SparseTensorCellsArgs { cells: Some(cells) },
            );
            (
                api::TensorPayloadKind::SparseTensorCells,
                value.as_union_value(),
            )
        }
        ir::RequestTensorPayloadIr::Mixed(blocks) => {
            let blocks = blocks
                .iter()
                .map(|block| {
                    let address = tensor_address_offset(builder, &block.address);
                    let values = builder.create_vector(&block.values);
                    api::TensorBlock::create(
                        builder,
                        &api::TensorBlockArgs {
                            address: Some(address),
                            values: Some(values),
                        },
                    )
                })
                .collect::<Vec<_>>();
            let blocks = builder.create_vector(&blocks);
            let value = api::MixedTensorBlocks::create(
                builder,
                &api::MixedTensorBlocksArgs {
                    blocks: Some(blocks),
                },
            );
            (
                api::TensorPayloadKind::MixedTensorBlocks,
                value.as_union_value(),
            )
        }
    };
    api::TensorValue::create(
        builder,
        &api::TensorValueArgs {
            type_spec: Some(type_spec),
            payload_type,
            payload: Some(payload),
        },
    )
}

fn query_inputs_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    inputs: &std::collections::BTreeMap<String, ir::QueryInputIr>,
) -> Option<
    flatbuffers::WIPOffset<
        flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<api::QueryInput<'a>>>,
    >,
> {
    (!inputs.is_empty()).then(|| {
        let values = inputs
            .iter()
            .map(|(name, input)| {
                let name = builder.create_string(name);
                let (input_type, input) = match input {
                    ir::QueryInputIr::Literal(value) => {
                        let value = request_value_offset(builder, value);
                        let value = api::LiteralQueryInput::create(
                            builder,
                            &api::LiteralQueryInputArgs { value: Some(value) },
                        );
                        (
                            api::QueryInputKind::LiteralQueryInput,
                            value.as_union_value(),
                        )
                    }
                    ir::QueryInputIr::Expression(value) => {
                        let value = builder.create_string(value);
                        let value = api::ExpressionQueryInput::create(
                            builder,
                            &api::ExpressionQueryInputArgs {
                                expression: Some(value),
                            },
                        );
                        (
                            api::QueryInputKind::ExpressionQueryInput,
                            value.as_union_value(),
                        )
                    }
                };
                api::QueryInput::create(
                    builder,
                    &api::QueryInputArgs {
                        name: Some(name),
                        input_type,
                        input: Some(input),
                    },
                )
            })
            .collect::<Vec<_>>();
        builder.create_vector(&values)
    })
}

fn query_model_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    model: &ir::QueryModelIr,
) -> Option<flatbuffers::WIPOffset<api::QueryModel<'a>>> {
    let populated = model.query_string.is_some()
        || model.default_index.is_some()
        || model.encoding.is_some()
        || model.filter.is_some()
        || model.language.is_some()
        || model.locale.is_some()
        || model.query_type.is_some()
        || !model.sources.is_empty()
        || !model.restrict.is_empty()
        || model.search_path.is_some()
        || model.search_group.is_some();
    populated.then(|| {
        let query_string = model
            .query_string
            .as_ref()
            .map(|value| builder.create_string(value));
        let default_index = model
            .default_index
            .as_ref()
            .map(|value| builder.create_string(value));
        let encoding = model
            .encoding
            .as_ref()
            .map(|value| builder.create_string(value));
        let filter = model
            .filter
            .as_ref()
            .map(|value| builder.create_string(value));
        let language = model
            .language
            .as_ref()
            .map(|value| builder.create_string(value.as_str()));
        let locale = model
            .locale
            .as_ref()
            .map(|value| builder.create_string(value));
        let source_offsets = model
            .sources
            .iter()
            .map(|value| builder.create_string(value))
            .collect::<Vec<_>>();
        let sources = (!source_offsets.is_empty()).then(|| builder.create_vector(&source_offsets));
        let restrict_offsets = model
            .restrict
            .iter()
            .map(|value| builder.create_string(value))
            .collect::<Vec<_>>();
        let restrict =
            (!restrict_offsets.is_empty()).then(|| builder.create_vector(&restrict_offsets));
        let search_path = model
            .search_path
            .as_ref()
            .map(|value| builder.create_string(value));
        let query_type = model.query_type.as_ref().map(|value| {
            let composite = value
                .composite
                .as_ref()
                .map(|value| builder.create_string(value));
            let tokenization = value
                .tokenization
                .as_ref()
                .map(|value| builder.create_string(value));
            let syntax = value
                .syntax
                .as_ref()
                .map(|value| builder.create_string(value));
            let profile = value
                .profile
                .as_ref()
                .map(|value| builder.create_string(value));
            let is_yql_default = value.is_yql_default.map(api::OptionalBool::new);
            api::QueryModelType::create(
                builder,
                &api::QueryModelTypeArgs {
                    composite,
                    tokenization,
                    syntax,
                    profile,
                    is_yql_default: is_yql_default.as_ref(),
                },
            )
        });
        let group = model.search_group.map(api::OptionalUInt32::new);
        api::QueryModel::create(
            builder,
            &api::QueryModelArgs {
                query_string,
                default_index,
                encoding,
                filter,
                language,
                locale,
                type_: query_type,
                sources,
                restrict,
                search_path,
                search_group: group.as_ref(),
            },
        )
    })
}

fn presentation_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    request: &ir::SearchRequestIr,
) -> Option<flatbuffers::WIPOffset<api::PresentationRequest<'a>>> {
    let projection = projection_offset(builder, &request.projection);
    let highlighting = match &request.highlighting {
        ir::HighlightingIr::Disabled => None,
        ir::HighlightingIr::Requested { fields } => Some(highlighting_offset(builder, fields)),
    };
    let summary = request
        .presentation
        .summary
        .as_ref()
        .map(|value| builder.create_string(value));
    let format = request
        .presentation
        .format
        .as_ref()
        .map(|value| builder.create_string(value));
    let bolding = request.presentation.bolding.map(api::OptionalBool::new);
    let timing = request.presentation.timing.map(api::OptionalBool::new);
    Some(api::PresentationRequest::create(
        builder,
        &api::PresentationRequestArgs {
            summary,
            fields: Some(projection),
            highlighting,
            format,
            bolding: bolding.as_ref(),
            timing: timing.as_ref(),
        },
    ))
}

fn grouping_offsets<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    grouping: &[ir::GroupingRequestIr],
) -> Option<
    flatbuffers::WIPOffset<
        flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<api::GroupingRequest<'a>>>,
    >,
> {
    (!grouping.is_empty()).then(|| {
        let values = grouping
            .iter()
            .map(|value| grouping_request_offset(builder, value))
            .collect::<Vec<_>>();
        builder.create_vector(&values)
    })
}

fn resolved_grouping_offsets<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    grouping: &[ir::GroupingRequestIr],
) -> Option<
    flatbuffers::WIPOffset<
        flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<api::ResolvedGroupingRequest<'a>>>,
    >,
> {
    (!grouping.is_empty()).then(|| {
        let values = grouping
            .iter()
            .map(|value| {
                let root = grouping_all_offset(builder, &value.root);
                let continuations = (!value.continuations.is_empty()).then(|| {
                    let values = value
                        .continuations
                        .iter()
                        .map(|value| builder.create_string(value))
                        .collect::<Vec<_>>();
                    builder.create_vector(&values)
                });
                api::ResolvedGroupingRequest::create(
                    builder,
                    &api::ResolvedGroupingRequestArgs {
                        root: Some(root),
                        continuations,
                    },
                )
            })
            .collect::<Vec<_>>();
        builder.create_vector(&values)
    })
}

fn grouping_request_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &ir::GroupingRequestIr,
) -> flatbuffers::WIPOffset<api::GroupingRequest<'a>> {
    let root = grouping_all_offset(builder, &value.root);
    let source = api::StructuredGroupingSource::create(
        builder,
        &api::StructuredGroupingSourceArgs { root: Some(root) },
    );
    let continuations = (!value.continuations.is_empty()).then(|| {
        let values = value
            .continuations
            .iter()
            .map(|value| builder.create_string(value))
            .collect::<Vec<_>>();
        builder.create_vector(&values)
    });
    api::GroupingRequest::create(
        builder,
        &api::GroupingRequestArgs {
            source_type: api::GroupingSourceKind::StructuredGroupingSource,
            source: Some(source.as_union_value()),
            continuations,
        },
    )
}

fn grouping_all_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &ir::GroupingAllIr,
) -> flatbuffers::WIPOffset<api::GroupingAllOperation<'a>> {
    let operations = value
        .operations
        .iter()
        .map(|value| grouping_operation_offset(builder, value))
        .collect::<Vec<_>>();
    let operations = (!operations.is_empty()).then(|| builder.create_vector(&operations));
    let label = value
        .label
        .as_ref()
        .map(|value| builder.create_string(value));
    api::GroupingAllOperation::create(
        builder,
        &api::GroupingAllOperationArgs { operations, label },
    )
}

fn grouping_operation_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &ir::GroupingOperationIr,
) -> flatbuffers::WIPOffset<api::GroupingOperation<'a>> {
    let (operation_type, operation) = match value {
        ir::GroupingOperationIr::All(value) => {
            let value = grouping_all_offset(builder, value);
            (
                api::GroupingOperationKind::GroupingAllOperation,
                value.as_union_value(),
            )
        }
        ir::GroupingOperationIr::Each(value) => {
            let operations = value
                .operations
                .iter()
                .map(|value| grouping_operation_offset(builder, value))
                .collect::<Vec<_>>();
            let operations = (!operations.is_empty()).then(|| builder.create_vector(&operations));
            let label = value
                .label
                .as_ref()
                .map(|value| builder.create_string(value));
            let value = api::GroupingEachOperation::create(
                builder,
                &api::GroupingEachOperationArgs { operations, label },
            );
            (
                api::GroupingOperationKind::GroupingEachOperation,
                value.as_union_value(),
            )
        }
        ir::GroupingOperationIr::Group(value) => {
            let expression = grouping_expression_offset(builder, value);
            let value = api::GroupingGroupOperation::create(
                builder,
                &api::GroupingGroupOperationArgs {
                    expression: Some(expression),
                },
            );
            (
                api::GroupingOperationKind::GroupingGroupOperation,
                value.as_union_value(),
            )
        }
        ir::GroupingOperationIr::Output(values) => {
            let values = values
                .iter()
                .map(|value| {
                    let label = value
                        .label
                        .as_ref()
                        .map(|value| builder.create_string(value));
                    let expression = grouping_expression_offset(builder, &value.expression);
                    api::GroupingOutput::create(
                        builder,
                        &api::GroupingOutputArgs {
                            label,
                            expression: Some(expression),
                        },
                    )
                })
                .collect::<Vec<_>>();
            let outputs = (!values.is_empty()).then(|| builder.create_vector(&values));
            let value = api::GroupingOutputOperation::create(
                builder,
                &api::GroupingOutputOperationArgs { outputs },
            );
            (
                api::GroupingOperationKind::GroupingOutputOperation,
                value.as_union_value(),
            )
        }
        ir::GroupingOperationIr::Max(value) => {
            let (value_type, value) = match value {
                Some(value) => {
                    let value = api::FiniteGroupingMax::create(
                        builder,
                        &api::FiniteGroupingMaxArgs { value: *value },
                    );
                    (
                        api::GroupingMaxKind::FiniteGroupingMax,
                        value.as_union_value(),
                    )
                }
                None => {
                    let value =
                        api::InfiniteGroupingMax::create(builder, &api::InfiniteGroupingMaxArgs {});
                    (
                        api::GroupingMaxKind::InfiniteGroupingMax,
                        value.as_union_value(),
                    )
                }
            };
            let value = api::GroupingMaxOperation::create(
                builder,
                &api::GroupingMaxOperationArgs {
                    value_type,
                    value: Some(value),
                },
            );
            (
                api::GroupingOperationKind::GroupingMaxOperation,
                value.as_union_value(),
            )
        }
        ir::GroupingOperationIr::Precision(number) => {
            let value = api::GroupingPrecisionOperation::create(
                builder,
                &api::GroupingPrecisionOperationArgs { value: *number },
            );
            (
                api::GroupingOperationKind::GroupingPrecisionOperation,
                value.as_union_value(),
            )
        }
        ir::GroupingOperationIr::Order(values) => {
            let values = values
                .iter()
                .map(|value| {
                    let expression = grouping_expression_offset(builder, value);
                    api::GroupingOrderExpr::create(
                        builder,
                        &api::GroupingOrderExprArgs {
                            expression: Some(expression),
                        },
                    )
                })
                .collect::<Vec<_>>();
            let expressions = (!values.is_empty()).then(|| builder.create_vector(&values));
            let value = api::GroupingOrderOperation::create(
                builder,
                &api::GroupingOrderOperationArgs { expressions },
            );
            (
                api::GroupingOperationKind::GroupingOrderOperation,
                value.as_union_value(),
            )
        }
        ir::GroupingOperationIr::Alias { name, expression } => {
            let name = builder.create_string(name);
            let expression = grouping_expression_offset(builder, expression);
            let value = api::GroupingAliasOperation::create(
                builder,
                &api::GroupingAliasOperationArgs {
                    name: Some(name),
                    expression: Some(expression),
                },
            );
            (
                api::GroupingOperationKind::GroupingAliasOperation,
                value.as_union_value(),
            )
        }
        ir::GroupingOperationIr::Filter(value) => {
            let filter = grouping_filter_offset(builder, value);
            let value = api::GroupingFilterOperation::create(
                builder,
                &api::GroupingFilterOperationArgs {
                    filter: Some(filter),
                },
            );
            (
                api::GroupingOperationKind::GroupingFilterOperation,
                value.as_union_value(),
            )
        }
        ir::GroupingOperationIr::Keep(value) => {
            let filter = grouping_filter_offset(builder, value);
            let value = api::GroupingKeepOperation::create(
                builder,
                &api::GroupingKeepOperationArgs {
                    filter: Some(filter),
                },
            );
            (
                api::GroupingOperationKind::GroupingKeepOperation,
                value.as_union_value(),
            )
        }
        ir::GroupingOperationIr::Where(value) => {
            let expression = grouping_expression_offset(builder, value);
            let value = api::GroupingWhereOperation::create(
                builder,
                &api::GroupingWhereOperationArgs {
                    expression: Some(expression),
                },
            );
            (
                api::GroupingOperationKind::GroupingWhereOperation,
                value.as_union_value(),
            )
        }
    };
    api::GroupingOperation::create(
        builder,
        &api::GroupingOperationArgs {
            operation_type,
            operation: Some(operation),
        },
    )
}

fn grouping_expression_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &ir::GroupingExpressionIr,
) -> flatbuffers::WIPOffset<api::GroupingExpression<'a>> {
    let (expr_type, expr) = match value {
        ir::GroupingExpressionIr::Attribute(value) => {
            let field = builder.create_string(value);
            let value = api::GroupingAttributeExpression::create(
                builder,
                &api::GroupingAttributeExpressionArgs { field: Some(field) },
            );
            (
                api::GroupingExpressionKind::GroupingAttributeExpression,
                value.as_union_value(),
            )
        }
        ir::GroupingExpressionIr::Literal(value) => {
            let value = request_value_offset(builder, value);
            let value = api::GroupingLiteralExpression::create(
                builder,
                &api::GroupingLiteralExpressionArgs { value: Some(value) },
            );
            (
                api::GroupingExpressionKind::GroupingLiteralExpression,
                value.as_union_value(),
            )
        }
        ir::GroupingExpressionIr::Function { name, arguments } => {
            let name = builder.create_string(name);
            let arguments = arguments
                .iter()
                .map(|value| grouping_expression_offset(builder, value))
                .collect::<Vec<_>>();
            let arguments = (!arguments.is_empty()).then(|| builder.create_vector(&arguments));
            let value = api::GroupingFunctionExpression::create(
                builder,
                &api::GroupingFunctionExpressionArgs {
                    name: Some(name),
                    arguments,
                },
            );
            (
                api::GroupingExpressionKind::GroupingFunctionExpression,
                value.as_union_value(),
            )
        }
        ir::GroupingExpressionIr::Positive(value) => {
            let expression = grouping_expression_offset(builder, value);
            let value = api::GroupingUnaryExpression::create(
                builder,
                &api::GroupingUnaryExpressionArgs {
                    op: api::GroupingUnaryOperator::Positive,
                    expression: Some(expression),
                },
            );
            (
                api::GroupingExpressionKind::GroupingUnaryExpression,
                value.as_union_value(),
            )
        }
        ir::GroupingExpressionIr::Negative(value) => {
            let expression = grouping_expression_offset(builder, value);
            let value = api::GroupingUnaryExpression::create(
                builder,
                &api::GroupingUnaryExpressionArgs {
                    op: api::GroupingUnaryOperator::Negative,
                    expression: Some(expression),
                },
            );
            (
                api::GroupingExpressionKind::GroupingUnaryExpression,
                value.as_union_value(),
            )
        }
        ir::GroupingExpressionIr::Aggregation(value) => match value.as_ref() {
            ir::GroupingAggregationIr::Count => {
                let aggregation =
                    api::CountAggregation::create(builder, &api::CountAggregationArgs {});
                let value = api::GroupingAggregationExpression::create(
                    builder,
                    &api::GroupingAggregationExpressionArgs {
                        aggregation_type: api::AggregationKind::CountAggregation,
                        aggregation: Some(aggregation.as_union_value()),
                    },
                );
                (
                    api::GroupingExpressionKind::GroupingAggregationExpression,
                    value.as_union_value(),
                )
            }
            ir::GroupingAggregationIr::Sum(expression) => {
                let expression = grouping_expression_offset(builder, expression);
                let aggregation = api::SumAggregation::create(
                    builder,
                    &api::SumAggregationArgs {
                        expression: Some(expression),
                    },
                );
                let value = api::GroupingAggregationExpression::create(
                    builder,
                    &api::GroupingAggregationExpressionArgs {
                        aggregation_type: api::AggregationKind::SumAggregation,
                        aggregation: Some(aggregation.as_union_value()),
                    },
                );
                (
                    api::GroupingExpressionKind::GroupingAggregationExpression,
                    value.as_union_value(),
                )
            }
            ir::GroupingAggregationIr::Avg(expression) => {
                let expression = grouping_expression_offset(builder, expression);
                let aggregation = api::AvgAggregation::create(
                    builder,
                    &api::AvgAggregationArgs {
                        expression: Some(expression),
                    },
                );
                let value = api::GroupingAggregationExpression::create(
                    builder,
                    &api::GroupingAggregationExpressionArgs {
                        aggregation_type: api::AggregationKind::AvgAggregation,
                        aggregation: Some(aggregation.as_union_value()),
                    },
                );
                (
                    api::GroupingExpressionKind::GroupingAggregationExpression,
                    value.as_union_value(),
                )
            }
            ir::GroupingAggregationIr::Min(expression) => {
                let expression = grouping_expression_offset(builder, expression);
                let aggregation = api::MinAggregation::create(
                    builder,
                    &api::MinAggregationArgs {
                        expression: Some(expression),
                    },
                );
                let value = api::GroupingAggregationExpression::create(
                    builder,
                    &api::GroupingAggregationExpressionArgs {
                        aggregation_type: api::AggregationKind::MinAggregation,
                        aggregation: Some(aggregation.as_union_value()),
                    },
                );
                (
                    api::GroupingExpressionKind::GroupingAggregationExpression,
                    value.as_union_value(),
                )
            }
            ir::GroupingAggregationIr::Max(expression) => {
                let expression = grouping_expression_offset(builder, expression);
                let aggregation = api::MaxAggregation::create(
                    builder,
                    &api::MaxAggregationArgs {
                        expression: Some(expression),
                    },
                );
                let value = api::GroupingAggregationExpression::create(
                    builder,
                    &api::GroupingAggregationExpressionArgs {
                        aggregation_type: api::AggregationKind::MaxAggregation,
                        aggregation: Some(aggregation.as_union_value()),
                    },
                );
                (
                    api::GroupingExpressionKind::GroupingAggregationExpression,
                    value.as_union_value(),
                )
            }
            ir::GroupingAggregationIr::Xor(expression) => {
                let expression = grouping_expression_offset(builder, expression);
                let aggregation = api::XorAggregation::create(
                    builder,
                    &api::XorAggregationArgs {
                        expression: Some(expression),
                    },
                );
                let value = api::GroupingAggregationExpression::create(
                    builder,
                    &api::GroupingAggregationExpressionArgs {
                        aggregation_type: api::AggregationKind::XorAggregation,
                        aggregation: Some(aggregation.as_union_value()),
                    },
                );
                (
                    api::GroupingExpressionKind::GroupingAggregationExpression,
                    value.as_union_value(),
                )
            }
            ir::GroupingAggregationIr::StdDev(expression) => {
                let expression = grouping_expression_offset(builder, expression);
                let aggregation = api::StdDevAggregation::create(
                    builder,
                    &api::StdDevAggregationArgs {
                        expression: Some(expression),
                    },
                );
                let value = api::GroupingAggregationExpression::create(
                    builder,
                    &api::GroupingAggregationExpressionArgs {
                        aggregation_type: api::AggregationKind::StdDevAggregation,
                        aggregation: Some(aggregation.as_union_value()),
                    },
                );
                (
                    api::GroupingExpressionKind::GroupingAggregationExpression,
                    value.as_union_value(),
                )
            }
            ir::GroupingAggregationIr::Quantiles {
                quantiles,
                expression,
            } => {
                let quantiles = builder.create_vector(quantiles);
                let expression = grouping_expression_offset(builder, expression);
                let aggregation = api::QuantilesAggregation::create(
                    builder,
                    &api::QuantilesAggregationArgs {
                        quantiles: Some(quantiles),
                        expression: Some(expression),
                    },
                );
                let value = api::GroupingAggregationExpression::create(
                    builder,
                    &api::GroupingAggregationExpressionArgs {
                        aggregation_type: api::AggregationKind::QuantilesAggregation,
                        aggregation: Some(aggregation.as_union_value()),
                    },
                );
                (
                    api::GroupingExpressionKind::GroupingAggregationExpression,
                    value.as_union_value(),
                )
            }
            ir::GroupingAggregationIr::Summary(summary) => {
                let summary = summary.as_ref().map(|value| builder.create_string(value));
                let aggregation = api::SummaryAggregation::create(
                    builder,
                    &api::SummaryAggregationArgs { summary },
                );
                let value = api::GroupingAggregationExpression::create(
                    builder,
                    &api::GroupingAggregationExpressionArgs {
                        aggregation_type: api::AggregationKind::SummaryAggregation,
                        aggregation: Some(aggregation.as_union_value()),
                    },
                );
                (
                    api::GroupingExpressionKind::GroupingAggregationExpression,
                    value.as_union_value(),
                )
            }
        },
        ir::GroupingExpressionIr::Binary {
            operator,
            left,
            right,
        } => {
            let op = match operator {
                ir::GroupingBinaryOperatorIr::Add => api::GroupingBinaryOperator::Add,
                ir::GroupingBinaryOperatorIr::Subtract => api::GroupingBinaryOperator::Subtract,
                ir::GroupingBinaryOperatorIr::Multiply => api::GroupingBinaryOperator::Multiply,
                ir::GroupingBinaryOperatorIr::Divide => api::GroupingBinaryOperator::Divide,
                ir::GroupingBinaryOperatorIr::Modulo => api::GroupingBinaryOperator::Modulo,
            };
            let left = grouping_expression_offset(builder, left);
            let right = grouping_expression_offset(builder, right);
            let value = api::GroupingBinaryExpression::create(
                builder,
                &api::GroupingBinaryExpressionArgs {
                    op,
                    left: Some(left),
                    right: Some(right),
                },
            );
            (
                api::GroupingExpressionKind::GroupingBinaryExpression,
                value.as_union_value(),
            )
        }
        ir::GroupingExpressionIr::Alias(name) => {
            let name = builder.create_string(name);
            let value = api::GroupingAliasReference::create(
                builder,
                &api::GroupingAliasReferenceArgs { name: Some(name) },
            );
            (
                api::GroupingExpressionKind::GroupingAliasReference,
                value.as_union_value(),
            )
        }
        ir::GroupingExpressionIr::Member { base, member } => {
            let base = grouping_expression_offset(builder, base);
            let member = builder.create_string(member);
            let value = api::GroupingMemberExpression::create(
                builder,
                &api::GroupingMemberExpressionArgs {
                    base: Some(base),
                    member: Some(member),
                },
            );
            (
                api::GroupingExpressionKind::GroupingMemberExpression,
                value.as_union_value(),
            )
        }
        ir::GroupingExpressionIr::MapLookup { base, key } => {
            let base = grouping_expression_offset(builder, base);
            let key = grouping_expression_offset(builder, key);
            let value = api::GroupingMapLookupExpression::create(
                builder,
                &api::GroupingMapLookupExpressionArgs {
                    base: Some(base),
                    key: Some(key),
                },
            );
            (
                api::GroupingExpressionKind::GroupingMapLookupExpression,
                value.as_union_value(),
            )
        }
        ir::GroupingExpressionIr::Bucket(value) => {
            let lower = grouping_range_bound_offset(builder, &value.lower);
            let upper = grouping_range_bound_offset(builder, &value.upper);
            let include_lower = api::OptionalBool::new(value.include_lower);
            let include_upper = api::OptionalBool::new(value.include_upper);
            let value = api::GroupingBucketExpression::create(
                builder,
                &api::GroupingBucketExpressionArgs {
                    lower: Some(lower),
                    upper: Some(upper),
                    include_lower: Some(&include_lower),
                    include_upper: Some(&include_upper),
                },
            );
            (
                api::GroupingExpressionKind::GroupingBucketExpression,
                value.as_union_value(),
            )
        }
    };
    api::GroupingExpression::create(
        builder,
        &api::GroupingExpressionArgs {
            expr_type,
            expr: Some(expr),
        },
    )
}

fn grouping_filter_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &ir::GroupingFilterIr,
) -> flatbuffers::WIPOffset<api::GroupingFilterExpr<'a>> {
    let (expr_type, expr) = match value {
        ir::GroupingFilterIr::And(values) => {
            let values = values
                .iter()
                .map(|value| grouping_filter_offset(builder, value))
                .collect::<Vec<_>>();
            let clauses = (!values.is_empty()).then(|| builder.create_vector(&values));
            let value =
                api::GroupingFilterAnd::create(builder, &api::GroupingFilterAndArgs { clauses });
            (
                api::GroupingFilterExprKind::GroupingFilterAnd,
                value.as_union_value(),
            )
        }
        ir::GroupingFilterIr::Or(values) => {
            let values = values
                .iter()
                .map(|value| grouping_filter_offset(builder, value))
                .collect::<Vec<_>>();
            let clauses = (!values.is_empty()).then(|| builder.create_vector(&values));
            let value =
                api::GroupingFilterOr::create(builder, &api::GroupingFilterOrArgs { clauses });
            (
                api::GroupingFilterExprKind::GroupingFilterOr,
                value.as_union_value(),
            )
        }
        ir::GroupingFilterIr::Not(value) => {
            let clause = grouping_filter_offset(builder, value);
            let value = api::GroupingFilterNot::create(
                builder,
                &api::GroupingFilterNotArgs {
                    clause: Some(clause),
                },
            );
            (
                api::GroupingFilterExprKind::GroupingFilterNot,
                value.as_union_value(),
            )
        }
        ir::GroupingFilterIr::Regex {
            expression,
            pattern,
        } => {
            let expression = grouping_expression_offset(builder, expression);
            let pattern = builder.create_string(pattern);
            let value = api::GroupingFilterRegex::create(
                builder,
                &api::GroupingFilterRegexArgs {
                    expression: Some(expression),
                    pattern: Some(pattern),
                },
            );
            (
                api::GroupingFilterExprKind::GroupingFilterRegex,
                value.as_union_value(),
            )
        }
        ir::GroupingFilterIr::Range {
            expression,
            lower,
            upper,
            include_lower,
            include_upper,
        } => {
            let expression = grouping_expression_offset(builder, expression);
            let lower = grouping_expression_offset(builder, lower);
            let upper = grouping_expression_offset(builder, upper);
            let include_lower = api::OptionalBool::new(*include_lower);
            let include_upper = api::OptionalBool::new(*include_upper);
            let value = api::GroupingFilterRange::create(
                builder,
                &api::GroupingFilterRangeArgs {
                    expression: Some(expression),
                    lower: Some(lower),
                    upper: Some(upper),
                    include_lower: Some(&include_lower),
                    include_upper: Some(&include_upper),
                },
            );
            (
                api::GroupingFilterExprKind::GroupingFilterRange,
                value.as_union_value(),
            )
        }
        ir::GroupingFilterIr::IsTrue(expression) => {
            let expression = grouping_expression_offset(builder, expression);
            let value = api::GroupingFilterIsTrue::create(
                builder,
                &api::GroupingFilterIsTrueArgs {
                    expression: Some(expression),
                },
            );
            (
                api::GroupingFilterExprKind::GroupingFilterIsTrue,
                value.as_union_value(),
            )
        }
    };
    api::GroupingFilterExpr::create(
        builder,
        &api::GroupingFilterExprArgs {
            expr_type,
            expr: Some(expr),
        },
    )
}

fn grouping_range_bound_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &ir::GroupingRangeBoundIr,
) -> flatbuffers::WIPOffset<api::RangeBound<'a>> {
    let (bound_type, bound) = match value {
        ir::GroupingRangeBoundIr::Finite(value) => {
            let value = request_value_offset(builder, value);
            let value = api::FiniteRangeBound::create(
                builder,
                &api::FiniteRangeBoundArgs { value: Some(value) },
            );
            (
                api::RangeBoundKind::FiniteRangeBound,
                value.as_union_value(),
            )
        }
        ir::GroupingRangeBoundIr::NegativeInfinity => {
            let value =
                api::NegativeInfinityBound::create(builder, &api::NegativeInfinityBoundArgs {});
            (
                api::RangeBoundKind::NegativeInfinityBound,
                value.as_union_value(),
            )
        }
        ir::GroupingRangeBoundIr::PositiveInfinity => {
            let value =
                api::PositiveInfinityBound::create(builder, &api::PositiveInfinityBoundArgs {});
            (
                api::RangeBoundKind::PositiveInfinityBound,
                value.as_union_value(),
            )
        }
        ir::GroupingRangeBoundIr::Unbounded => {
            let value = api::UnboundedRangeBound::create(builder, &api::UnboundedRangeBoundArgs {});
            (
                api::RangeBoundKind::UnboundedRangeBound,
                value.as_union_value(),
            )
        }
    };
    api::RangeBound::create(
        builder,
        &api::RangeBoundArgs {
            bound_type,
            bound: Some(bound),
        },
    )
}

fn query_source_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    query: &ir::QueryIr,
) -> flatbuffers::WIPOffset<api::QuerySource<'a>> {
    let (source_type, source) = match query {
        ir::QueryIr::Text(text) => {
            let text = builder.create_string(text);
            let value = api::SearchKernelTextQuery::create(
                builder,
                &api::SearchKernelTextQueryArgs { text: Some(text) },
            );
            (
                api::QuerySourceKind::SearchKernelTextQuery,
                value.as_union_value(),
            )
        }
        ir::QueryIr::MatchAll => {
            let value = api::MatchAllQuery::create(builder, &api::MatchAllQueryArgs {});
            (api::QuerySourceKind::MatchAllQuery, value.as_union_value())
        }
        ir::QueryIr::Yql { source, parameters } => {
            let source = builder.create_string(source);
            let parameters = request_value_entries_offset(builder, parameters);
            let value = api::YqlQuery::create(
                builder,
                &api::YqlQueryArgs {
                    source: Some(source),
                    parameters,
                },
            );
            (api::QuerySourceKind::YqlQuery, value.as_union_value())
        }
        _ => {
            let value = query_expr_offset(builder, query);
            let value =
                api::TypedQuery::create(builder, &api::TypedQueryArgs { query: Some(value) });
            (api::QuerySourceKind::TypedQuery, value.as_union_value())
        }
    };
    api::QuerySource::create(
        builder,
        &api::QuerySourceArgs {
            source_type,
            source: Some(source),
        },
    )
}

fn search_field_selection_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    fields: &[ir::SearchFieldIr],
) -> flatbuffers::WIPOffset<api::SearchFieldSelection<'a>> {
    let fields = fields
        .iter()
        .map(|field| {
            let name = builder.create_string(field.field.as_str());
            let weight = api::OptionalDouble::new(field.weight.get());
            api::SearchField::create(
                builder,
                &api::SearchFieldArgs {
                    field: Some(name),
                    weight: Some(&weight),
                },
            )
        })
        .collect::<Vec<_>>();
    let fields = builder.create_vector(&fields);
    api::SearchFieldSelection::create(
        builder,
        &api::SearchFieldSelectionArgs {
            fields: Some(fields),
        },
    )
}

fn projection_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    projection: &ir::ProjectionIr,
) -> flatbuffers::WIPOffset<api::FieldProjection<'a>> {
    let fields = projection
        .fields
        .iter()
        .map(|field| builder.create_string(field.as_str()))
        .collect::<Vec<_>>();
    let fields = builder.create_vector(&fields);
    api::FieldProjection::create(
        builder,
        &api::FieldProjectionArgs {
            fields: Some(fields),
        },
    )
}

fn highlighting_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    fields: &[ir::FieldName],
) -> flatbuffers::WIPOffset<api::HighlightRequest<'a>> {
    let fields = fields
        .iter()
        .map(|field| builder.create_string(field.as_str()))
        .collect::<Vec<_>>();
    let fields = builder.create_vector(&fields);
    api::HighlightRequest::create(
        builder,
        &api::HighlightRequestArgs {
            fields: Some(fields),
        },
    )
}

fn query_expr_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    query: &ir::QueryIr,
) -> flatbuffers::WIPOffset<api::QueryExpr<'a>> {
    let (expr_type, expr) = match query {
        ir::QueryIr::Term { field, value } => {
            let field = query_field_ref_offset(builder, field.as_str());
            let value = scalar_value_offset(builder, value);
            let term = api::TermQuery::create(builder, &api::TermQueryArgs { value: Some(value) });
            let item = api::QueryExpr::create(
                builder,
                &api::QueryExprArgs {
                    expr_type: api::QueryExprKind::TermQuery,
                    expr: Some(term.as_union_value()),
                    annotations: None,
                },
            );
            let contains = api::QueryContains::create(
                builder,
                &api::QueryContainsArgs {
                    field: Some(field),
                    item: Some(item),
                },
            );
            (api::QueryExprKind::QueryContains, contains.as_union_value())
        }
        ir::QueryIr::And(clauses) => {
            let clauses = clauses
                .iter()
                .map(|clause| query_expr_offset(builder, clause))
                .collect::<Vec<_>>();
            let clauses = builder.create_vector(&clauses);
            let value = api::QueryAnd::create(
                builder,
                &api::QueryAndArgs {
                    clauses: Some(clauses),
                },
            );
            (api::QueryExprKind::QueryAnd, value.as_union_value())
        }
        ir::QueryIr::Or(clauses) => {
            let clauses = clauses
                .iter()
                .map(|clause| query_expr_offset(builder, clause))
                .collect::<Vec<_>>();
            let clauses = builder.create_vector(&clauses);
            let value = api::QueryOr::create(
                builder,
                &api::QueryOrArgs {
                    clauses: Some(clauses),
                },
            );
            (api::QueryExprKind::QueryOr, value.as_union_value())
        }
        ir::QueryIr::Not(clause) => {
            let clause = query_expr_offset(builder, clause);
            let value = api::QueryNot::create(
                builder,
                &api::QueryNotArgs {
                    clause: Some(clause),
                },
            );
            (api::QueryExprKind::QueryNot, value.as_union_value())
        }
        ir::QueryIr::MatchAll | ir::QueryIr::Text(_) | ir::QueryIr::Yql { .. } => unreachable!(),
    };
    api::QueryExpr::create(
        builder,
        &api::QueryExprArgs {
            expr_type,
            expr: Some(expr),
            annotations: None,
        },
    )
}

fn filter_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    filter: &ir::FilterIr,
) -> flatbuffers::WIPOffset<api::FilterExpr<'a>> {
    let (expr_type, expr) = match filter {
        ir::FilterIr::And(clauses) => {
            let clauses = clauses
                .iter()
                .map(|clause| filter_offset(builder, clause))
                .collect::<Vec<_>>();
            let clauses = builder.create_vector(&clauses);
            let value = api::FilterAnd::create(
                builder,
                &api::FilterAndArgs {
                    clauses: Some(clauses),
                },
            );
            (api::FilterExprKind::FilterAnd, value.as_union_value())
        }
        ir::FilterIr::Or(clauses) => {
            let clauses = clauses
                .iter()
                .map(|clause| filter_offset(builder, clause))
                .collect::<Vec<_>>();
            let clauses = builder.create_vector(&clauses);
            let value = api::FilterOr::create(
                builder,
                &api::FilterOrArgs {
                    clauses: Some(clauses),
                },
            );
            (api::FilterExprKind::FilterOr, value.as_union_value())
        }
        ir::FilterIr::Not(clause) => {
            let clause = filter_offset(builder, clause);
            let value = api::FilterNot::create(
                builder,
                &api::FilterNotArgs {
                    clause: Some(clause),
                },
            );
            (api::FilterExprKind::FilterNot, value.as_union_value())
        }
        ir::FilterIr::Comparison(comparison) => {
            let field = query_field_ref_offset(builder, comparison.field.as_str());
            let value = scalar_value_offset(builder, &comparison.value);
            let value = api::FilterComparison::create(
                builder,
                &api::FilterComparisonArgs {
                    field: Some(field),
                    op: comparison_op(comparison.op),
                    value: Some(value),
                },
            );
            (
                api::FilterExprKind::FilterComparison,
                value.as_union_value(),
            )
        }
        ir::FilterIr::Range(range) => {
            let field = query_field_ref_offset(builder, range.field.as_str());
            let lower = range_bound_offset(builder, range.lower.as_ref());
            let upper = range_bound_offset(builder, range.upper.as_ref());
            let include_lower = api::OptionalBool::new(range.include_lower);
            let include_upper = api::OptionalBool::new(range.include_upper);
            let value = api::FilterRange::create(
                builder,
                &api::FilterRangeArgs {
                    field: Some(field),
                    lower: Some(lower),
                    upper: Some(upper),
                    include_lower: Some(&include_lower),
                    include_upper: Some(&include_upper),
                },
            );
            (api::FilterExprKind::FilterRange, value.as_union_value())
        }
        ir::FilterIr::In { field, values } => {
            let field = query_field_ref_offset(builder, field.as_str());
            let values = values
                .iter()
                .map(|value| scalar_value_offset(builder, value))
                .collect::<Vec<_>>();
            let values = builder.create_vector(&values);
            let value = api::FilterIn::create(
                builder,
                &api::FilterInArgs {
                    field: Some(field),
                    values: Some(values),
                },
            );
            (api::FilterExprKind::FilterIn, value.as_union_value())
        }
        ir::FilterIr::Exists { field } => {
            let field = query_field_ref_offset(builder, field.as_str());
            let value =
                api::FilterExists::create(builder, &api::FilterExistsArgs { field: Some(field) });
            (api::FilterExprKind::FilterExists, value.as_union_value())
        }
    };
    api::FilterExpr::create(
        builder,
        &api::FilterExprArgs {
            expr_type,
            expr: Some(expr),
        },
    )
}

fn range_bound_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: Option<&ir::ScalarValueIr>,
) -> flatbuffers::WIPOffset<api::RangeBound<'a>> {
    let (bound_type, bound) = match value {
        Some(value) => (api::RangeBoundKind::FiniteRangeBound, {
            let value = scalar_value_offset(builder, value);
            api::FiniteRangeBound::create(
                builder,
                &api::FiniteRangeBoundArgs { value: Some(value) },
            )
            .as_union_value()
        }),
        None => (
            api::RangeBoundKind::UnboundedRangeBound,
            api::UnboundedRangeBound::create(builder, &api::UnboundedRangeBoundArgs {})
                .as_union_value(),
        ),
    };
    api::RangeBound::create(
        builder,
        &api::RangeBoundArgs {
            bound_type,
            bound: Some(bound),
        },
    )
}

fn scalar_value_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &ir::ScalarValueIr,
) -> flatbuffers::WIPOffset<api::Value<'a>> {
    let (kind_type, kind) = match value {
        ir::ScalarValueIr::Bool(value) => (
            api::ValueKind::BoolValue,
            api::BoolValue::create(builder, &api::BoolValueArgs { value: *value }).as_union_value(),
        ),
        ir::ScalarValueIr::Int64(value) => (
            api::ValueKind::Int64Value,
            api::Int64Value::create(builder, &api::Int64ValueArgs { value: *value })
                .as_union_value(),
        ),
        ir::ScalarValueIr::Float64(value) => (
            api::ValueKind::Float64Value,
            api::Float64Value::create(builder, &api::Float64ValueArgs { value: *value })
                .as_union_value(),
        ),
        ir::ScalarValueIr::String(value) => {
            let value = builder.create_string(value);
            let value =
                api::StringValue::create(builder, &api::StringValueArgs { value: Some(value) });
            (api::ValueKind::StringValue, value.as_union_value())
        }
    };
    api::Value::create(
        builder,
        &api::ValueArgs {
            kind_type,
            kind: Some(kind),
        },
    )
}

fn comparison_op(op: ir::ComparisonOp) -> api::ComparisonOp {
    match op {
        ir::ComparisonOp::Eq => api::ComparisonOp::Eq,
        ir::ComparisonOp::Ne => api::ComparisonOp::Ne,
        ir::ComparisonOp::Lt => api::ComparisonOp::Lt,
        ir::ComparisonOp::Lte => api::ComparisonOp::Lte,
        ir::ComparisonOp::Gt => api::ComparisonOp::Gt,
        ir::ComparisonOp::Gte => api::ComparisonOp::Gte,
    }
}

fn query_field_ref_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    path: &str,
) -> flatbuffers::WIPOffset<api::QueryFieldRef<'a>> {
    let path = builder.create_string(path);
    api::QueryFieldRef::create(
        builder,
        &api::QueryFieldRefArgs {
            path: Some(path),
            element_index: None,
        },
    )
}

fn missing_policy_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    missing: ir::MissingPlacement,
) -> flatbuffers::WIPOffset<api::MissingPolicy<'a>> {
    let (policy_type, policy) = match missing {
        ir::MissingPlacement::First => {
            let policy = api::MissingFirst::create(builder, &api::MissingFirstArgs {});
            (
                api::MissingPolicyKind::MissingFirst,
                policy.as_union_value(),
            )
        }
        ir::MissingPlacement::Last => {
            let policy = api::MissingLast::create(builder, &api::MissingLastArgs {});
            (api::MissingPolicyKind::MissingLast, policy.as_union_value())
        }
    };
    api::MissingPolicy::create(
        builder,
        &api::MissingPolicyArgs {
            policy_type,
            policy: Some(policy),
        },
    )
}

fn sort_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    sort: &ir::SortIr,
) -> flatbuffers::WIPOffset<api::SortExpr<'a>> {
    let field = builder.create_string(sort.field.as_str());
    let field =
        api::FieldSortTarget::create(builder, &api::FieldSortTargetArgs { field: Some(field) });
    let target = api::SortTarget::create(
        builder,
        &api::SortTargetArgs {
            target_type: api::SortTargetKind::FieldSortTarget,
            target: Some(field.as_union_value()),
        },
    );
    let missing = missing_policy_offset(builder, sort.missing);
    api::SortExpr::create(
        builder,
        &api::SortExprArgs {
            target: Some(target),
            direction: match sort.direction {
                ir::SortDirection::Asc => api::SortDirection::Asc,
                ir::SortDirection::Desc => api::SortDirection::Desc,
            },
            missing: Some(missing),
            annotations: None,
        },
    )
}

fn resolved_sort_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    sort: &ir::SortIr,
) -> flatbuffers::WIPOffset<api::ResolvedSortExpr<'a>> {
    let field = builder.create_string(sort.field.as_str());
    let field =
        api::FieldSortTarget::create(builder, &api::FieldSortTargetArgs { field: Some(field) });
    let target = api::ResolvedSortTarget::create(
        builder,
        &api::ResolvedSortTargetArgs {
            target_type: api::ResolvedSortTargetKind::FieldSortTarget,
            target: Some(field.as_union_value()),
        },
    );
    let missing = missing_policy_offset(builder, sort.missing);
    api::ResolvedSortExpr::create(
        builder,
        &api::ResolvedSortExprArgs {
            target: Some(target),
            direction: match sort.direction {
                ir::SortDirection::Asc => api::SortDirection::Asc,
                ir::SortDirection::Desc => api::SortDirection::Desc,
            },
            missing: Some(missing),
            annotations: None,
        },
    )
}

pub fn insert_result_to_api(result: &ir::InsertResult) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let root = insert_result_offset(&mut builder, result);
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

pub fn insert_result_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    result: &ir::InsertResult,
) -> flatbuffers::WIPOffset<api::InsertResult<'a>> {
    let id = builder.create_string(&result.id);
    api::InsertResult::create(builder, &api::InsertResultArgs { id: Some(id) })
}

pub fn upsert_result_to_api(result: &ir::UpsertResult) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let root = upsert_result_offset(&mut builder, result);
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

pub fn upsert_result_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    result: &ir::UpsertResult,
) -> flatbuffers::WIPOffset<api::UpsertResult<'a>> {
    let id = builder.create_string(&result.id);
    api::UpsertResult::create(builder, &api::UpsertResultArgs { id: Some(id) })
}

pub fn patch_result_to_api(result: &ir::PatchResult) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let root = patch_result_offset(&mut builder, result);
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

pub fn patch_result_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    result: &ir::PatchResult,
) -> flatbuffers::WIPOffset<api::PatchResult<'a>> {
    let id = builder.create_string(&result.id);
    api::PatchResult::create(builder, &api::PatchResultArgs { id: Some(id) })
}

pub fn delete_result_to_api(result: &ir::DeleteResult) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let root = delete_result_offset(&mut builder, result);
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

pub fn delete_result_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    result: &ir::DeleteResult,
) -> flatbuffers::WIPOffset<api::DeleteResult<'a>> {
    let id = builder.create_string(&result.id);
    api::DeleteResult::create(
        builder,
        &api::DeleteResultArgs {
            id: Some(id),
            deleted: result.deleted,
        },
    )
}

pub fn get_result_to_api(result: &ir::GetResult) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let root = get_result_offset(&mut builder, result);
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

pub fn get_result_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    result: &ir::GetResult,
) -> flatbuffers::WIPOffset<api::GetResult<'a>> {
    let document = result
        .document
        .as_ref()
        .map(|document| document_offset(builder, document));
    api::GetResult::create(builder, &api::GetResultArgs { document })
}

pub fn search_result_to_api(result: &ir::SearchResult) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let root = search_result_offset(&mut builder, result);
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

pub fn search_result_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    result: &ir::SearchResult,
) -> flatbuffers::WIPOffset<api::SearchResult<'a>> {
    let hit_nodes = result
        .hits
        .iter()
        .map(|hit| {
            let hit = hit_offset(builder, hit);
            api::ResultNode::create(
                builder,
                &api::ResultNodeArgs {
                    node_type: api::ResultNodeKind::Hit,
                    node: Some(hit.as_union_value()),
                },
            )
        })
        .collect::<Vec<_>>();
    let hit_nodes = builder.create_vector(&hit_nodes);
    let group = api::ResultGroup::create(
        builder,
        &api::ResultGroupArgs {
            children: Some(hit_nodes),
            ..Default::default()
        },
    );
    let root = api::ResultNode::create(
        builder,
        &api::ResultNodeArgs {
            node_type: api::ResultNodeKind::ResultGroup,
            node: Some(group.as_union_value()),
        },
    );
    let total_hits = api::TotalHits::create(
        builder,
        &api::TotalHitsArgs {
            value: result.total_hits.value,
            relation: match result.total_hits.relation {
                ir::TotalHitsRelation::Exact => api::TotalHitsRelation::Exact,
                ir::TotalHitsRelation::LowerBound => api::TotalHitsRelation::LowerBound,
            },
        },
    );
    api::SearchResult::create(
        builder,
        &api::SearchResultArgs {
            root: Some(root),
            total_hits: Some(total_hits),
            coverage: None,
            timing: None,
            errors: None,
            trace: None,
            metadata: None,
        },
    )
}

pub fn analyze_result_to_api(result: &ir::AnalyzeResult) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let root = analyze_result_offset(&mut builder, result);
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

pub fn analyze_result_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    result: &ir::AnalyzeResult,
) -> flatbuffers::WIPOffset<api::AnalyzeResult<'a>> {
    let tokens = result
        .tokens
        .iter()
        .map(|token| {
            let surface = builder.create_string(&token.surface);
            let term = builder.create_string(&token.term);
            api::AnalyzedToken::create(
                builder,
                &api::AnalyzedTokenArgs {
                    surface: Some(surface),
                    term: Some(term),
                    position: token.position,
                    start_byte: token.start_byte,
                    end_byte: token.end_byte,
                },
            )
        })
        .collect::<Vec<_>>();
    let tokens = builder.create_vector(&tokens);
    api::AnalyzeResult::create(
        builder,
        &api::AnalyzeResultArgs {
            tokens: Some(tokens),
        },
    )
}

fn value_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &ir::Value,
) -> flatbuffers::WIPOffset<api::Value<'a>> {
    let (kind_type, kind) = match value {
        ir::Value::Null => (
            api::ValueKind::NullValueValue,
            api::NullValueValue::create(
                builder,
                &api::NullValueValueArgs {
                    value: api::NullValue::Null,
                },
            )
            .as_union_value(),
        ),
        ir::Value::Bool(value) => (
            api::ValueKind::BoolValue,
            api::BoolValue::create(builder, &api::BoolValueArgs { value: *value }).as_union_value(),
        ),
        ir::Value::Int64(value) => (
            api::ValueKind::Int64Value,
            api::Int64Value::create(builder, &api::Int64ValueArgs { value: *value })
                .as_union_value(),
        ),
        ir::Value::UInt64(value) => (
            api::ValueKind::UInt64Value,
            api::UInt64Value::create(builder, &api::UInt64ValueArgs { value: *value })
                .as_union_value(),
        ),
        ir::Value::Float64(value) => (
            api::ValueKind::Float64Value,
            api::Float64Value::create(builder, &api::Float64ValueArgs { value: *value })
                .as_union_value(),
        ),
        ir::Value::String(value) => {
            let value = builder.create_string(value);
            let value =
                api::StringValue::create(builder, &api::StringValueArgs { value: Some(value) });
            (api::ValueKind::StringValue, value.as_union_value())
        }
        ir::Value::Bytes(value) => {
            let value = builder.create_vector(value);
            let value =
                api::BytesValue::create(builder, &api::BytesValueArgs { value: Some(value) });
            (api::ValueKind::BytesValue, value.as_union_value())
        }
        ir::Value::List(values) => {
            let values = values
                .iter()
                .map(|value| value_offset(builder, value))
                .collect::<Vec<_>>();
            let values = builder.create_vector(&values);
            let value = api::ValueList::create(
                builder,
                &api::ValueListArgs {
                    values: Some(values),
                },
            );
            (api::ValueKind::ValueList, value.as_union_value())
        }
        ir::Value::Object(fields) => {
            let fields = fields
                .iter()
                .map(|(key, value)| {
                    let key = builder.create_string(key);
                    let value = value_offset(builder, value);
                    api::ValueEntry::create(
                        builder,
                        &api::ValueEntryArgs {
                            key: Some(key),
                            value: Some(value),
                        },
                    )
                })
                .collect::<Vec<_>>();
            let fields = builder.create_vector(&fields);
            let value = api::ValueObject::create(
                builder,
                &api::ValueObjectArgs {
                    fields: Some(fields),
                },
            );
            (api::ValueKind::ValueObject, value.as_union_value())
        }
    };
    api::Value::create(
        builder,
        &api::ValueArgs {
            kind_type,
            kind: Some(kind),
        },
    )
}

#[cfg(test)]
#[test]
fn inspect_query_encoding_preserves_tensors_empty_objects_query_type_and_nested_grouping() {
    use std::collections::BTreeMap;

    let mut inputs = BTreeMap::new();
    inputs.insert(
        "dense".to_owned(),
        ir::QueryInputIr::Literal(ir::RequestValueIr::Tensor(ir::RequestTensorIr {
            type_spec: "tensor(x[2])".to_owned(),
            payload: ir::RequestTensorPayloadIr::Dense(vec![1.0, 2.0]),
        })),
    );
    inputs.insert(
        "sparse".to_owned(),
        ir::QueryInputIr::Literal(ir::RequestValueIr::Tensor(ir::RequestTensorIr {
            type_spec: "tensor(label{})".to_owned(),
            payload: ir::RequestTensorPayloadIr::Sparse(vec![ir::RequestTensorCellIr {
                address: BTreeMap::from([("label".to_owned(), "a".to_owned())]),
                value: 3.0,
            }]),
        })),
    );
    inputs.insert(
        "mixed".to_owned(),
        ir::QueryInputIr::Literal(ir::RequestValueIr::Tensor(ir::RequestTensorIr {
            type_spec: "tensor(label{},x[2])".to_owned(),
            payload: ir::RequestTensorPayloadIr::Mixed(vec![ir::RequestTensorBlockIr {
                address: BTreeMap::from([("label".to_owned(), "a".to_owned())]),
                values: vec![4.0, 5.0],
            }]),
        })),
    );
    let request = ir::SearchRequestIr {
        query: ir::QueryIr::MatchAll,
        search_fields: Vec::new(),
        filter: None,
        sort: Vec::new(),
        grouping: vec![ir::GroupingRequestIr {
            root: ir::GroupingAllIr {
                operations: vec![ir::GroupingOperationIr::All(ir::GroupingAllIr {
                    operations: vec![ir::GroupingOperationIr::Group(
                        ir::GroupingExpressionIr::Attribute("brand".to_owned()),
                    )],
                    label: Some("nested".to_owned()),
                })],
                label: None,
            },
            continuations: Vec::new(),
        }],
        ranking: ir::RankingPolicyIr::DefaultRelevanceV1,
        ranking_request: ir::RankingRequestIr {
            profile: "default".to_owned(),
            properties: BTreeMap::new(),
        },
        inputs,
        limit: 20,
        offset: 0,
        projection: ir::ProjectionIr::new(Vec::new()),
        highlighting: ir::HighlightingIr::Disabled,
        language: None,
        query_model: ir::QueryModelIr {
            query_type: Some(ir::QueryModelTypeIr {
                composite: Some("rank".to_owned()),
                tokenization: Some("word".to_owned()),
                syntax: Some("advanced".to_owned()),
                profile: Some("default".to_owned()),
                is_yql_default: Some(false),
            }),
            ..Default::default()
        },
        presentation: ir::PresentationIr::default(),
        properties: BTreeMap::from([(
            "empty".to_owned(),
            ir::RequestValueIr::Object(BTreeMap::new()),
        )]),
        query_profile: None,
        search_chain: None,
        timeout: None,
        prefix_policy: ir::PrefixPolicy::Disabled,
        typo_policy: ir::TypoPolicy::Disabled,
    };

    let bytes = inspect_query_result_to_api(&request);
    let result =
        flatbuffers::root::<api::InspectQueryResult>(&bytes).expect("valid inspect query result");
    let resolved = result.resolved_request().expect("resolved request");
    let inputs = resolved.inputs().expect("query inputs");
    assert_eq!(inputs.len(), 3);
    assert_eq!(
        inputs
            .get(0)
            .input_as_literal_query_input()
            .unwrap()
            .value()
            .unwrap()
            .kind_type(),
        api::ValueKind::TensorValue
    );
    for (index, expected) in [
        api::TensorPayloadKind::DenseTensorValues,
        api::TensorPayloadKind::MixedTensorBlocks,
        api::TensorPayloadKind::SparseTensorCells,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            inputs
                .get(index)
                .input_as_literal_query_input()
                .unwrap()
                .value()
                .unwrap()
                .kind_as_tensor_value()
                .unwrap()
                .payload_type(),
            expected
        );
    }
    let properties = resolved.properties().expect("properties");
    let empty = properties
        .get(0)
        .value()
        .unwrap()
        .kind_as_value_object()
        .unwrap();
    assert_eq!(empty.fields().map(|fields| fields.len()), Some(0));
    let query_type = resolved.query_model().unwrap().type_().unwrap();
    assert_eq!(query_type.composite(), Some("rank"));
    let grouping = resolved.grouping().expect("grouping");
    let nested = grouping.get(0).root().unwrap().operations().unwrap().get(0);
    assert!(nested.operation_as_grouping_all_operation().is_some());
}

#[cfg(test)]
#[test]
fn rich_v2_index_definition_round_trips_through_ir() {
    use crate::{
        AttributeConfigIr, ConstantDefinitionIr, ConstantSourceIr, DataTypeIr,
        DocumentSummaryDefinitionIr, DocumentSummaryFieldDefinitionIr, FieldCapabilities, FieldIr,
        FieldName, FieldSetDefinitionIr, ImportedFieldDefinitionIr, IndexDefinitionIr,
        MatchingConfigIr, MatchingModeIr, NamedStructDefinitionIr, OnnxModelDefinitionIr,
        PrimitiveDataTypeIr, RankProfileDefinitionIr, StructFieldIr,
    };
    use std::collections::BTreeMap;

    let field = |name: &str, data_type| {
        FieldIr::new(
            FieldName::parse(name).unwrap(),
            data_type,
            None,
            FieldCapabilities::new(true, false, true, false, false),
        )
    };
    let mut text = field("text", DataTypeIr::Primitive(PrimitiveDataTypeIr::String));
    text.matching = Some(MatchingConfigIr {
        mode: MatchingModeIr::Exact,
        options: BTreeMap::from([("case".to_owned(), ir::RequestValueIr::Bool(true))]),
    });
    text.attribute = Some(AttributeConfigIr {
        enabled: Some(true),
        fast_search: Some(true),
        fast_access: None,
        distance_metric: crate::DistanceMetricIr::Unspecified,
        options: BTreeMap::new(),
    });
    let mut definition = IndexDefinitionIr::new(
        None,
        vec![
            text,
            field("byte", DataTypeIr::Primitive(PrimitiveDataTypeIr::Byte)),
            field("int32", DataTypeIr::Primitive(PrimitiveDataTypeIr::Int32)),
            field("raw", DataTypeIr::Primitive(PrimitiveDataTypeIr::Raw)),
            field(
                "tensor",
                DataTypeIr::Tensor {
                    type_spec: "tensor(x[2])".to_owned(),
                },
            ),
            field(
                "array",
                DataTypeIr::Array(Box::new(DataTypeIr::Primitive(PrimitiveDataTypeIr::Int64))),
            ),
            field(
                "map",
                DataTypeIr::Map {
                    key_type: Box::new(DataTypeIr::Primitive(PrimitiveDataTypeIr::String)),
                    value_type: Box::new(DataTypeIr::Primitive(PrimitiveDataTypeIr::Float32)),
                },
            ),
            field(
                "weighted",
                DataTypeIr::WeightedSet(Box::new(DataTypeIr::Primitive(
                    PrimitiveDataTypeIr::String,
                ))),
            ),
            field(
                "inline",
                DataTypeIr::Struct {
                    fields: vec![StructFieldIr {
                        name: "part".to_owned(),
                        data_type: DataTypeIr::Primitive(PrimitiveDataTypeIr::Bool),
                    }],
                },
            ),
            field(
                "named",
                DataTypeIr::StructReference {
                    name: "address".to_owned(),
                },
            ),
            field("predicate", DataTypeIr::Predicate),
            field("position", DataTypeIr::Position),
            field(
                "parent",
                DataTypeIr::Reference {
                    document_type: "parent_doc".to_owned(),
                },
            ),
        ],
        Vec::new(),
        crate::RankingPolicyIr::DefaultRelevanceV1,
        crate::PrefixPolicy::Disabled,
        crate::TypoPolicy::Disabled,
    )
    .unwrap();
    definition.document_type = Some("doc".to_owned());
    definition.schema_inherits = vec!["base".to_owned()];
    definition.structs = vec![NamedStructDefinitionIr {
        name: "address".to_owned(),
        fields: vec![StructFieldIr {
            name: "city".to_owned(),
            data_type: DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
        }],
        properties: BTreeMap::new(),
    }];
    definition.fieldsets = vec![FieldSetDefinitionIr {
        name: "all_text".to_owned(),
        fields: vec![FieldName::parse("text").unwrap()],
        matching: None,
        query_commands: vec!["stem".to_owned()],
        properties: BTreeMap::new(),
    }];
    definition.schema_fields = vec![field(
        "derived",
        DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
    )];
    definition.imported_fields = vec![ImportedFieldDefinitionIr {
        reference_field: FieldName::parse("parent").unwrap(),
        target_field: "name".to_owned(),
        alias: FieldName::parse("parent_name").unwrap(),
        properties: BTreeMap::new(),
    }];
    definition.document_summaries = vec![DocumentSummaryDefinitionIr {
        name: "short".to_owned(),
        inherits: Vec::new(),
        fields: vec![DocumentSummaryFieldDefinitionIr {
            name: "text".to_owned(),
            source: "text".to_owned(),
            data_type: DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
            struct_fields: Vec::new(),
            properties: BTreeMap::new(),
        }],
        from_disk: Some(false),
        omit_summary_features: Some(true),
        properties: BTreeMap::new(),
    }];
    definition.constants = vec![ConstantDefinitionIr {
        name: "bias".to_owned(),
        data_type: DataTypeIr::Primitive(PrimitiveDataTypeIr::Float64),
        source: ConstantSourceIr::Literal(ir::RequestValueIr::Float64(1.0)),
        properties: BTreeMap::new(),
    }];
    definition.onnx_models = vec![OnnxModelDefinitionIr {
        name: "model".to_owned(),
        file: "model.onnx".to_owned(),
        inputs: BTreeMap::from([("input".to_owned(), "query(q)".to_owned())]),
        outputs: BTreeMap::new(),
        gpu_device: Some(0),
        intraop_threads: Some(1),
        interop_threads: None,
        properties: BTreeMap::new(),
    }];
    definition.rank_profiles = vec![RankProfileDefinitionIr {
        name: "rank".to_owned(),
        inherits: Vec::new(),
        inputs: Vec::new(),
        functions: Vec::new(),
        first_phase: None,
        second_phase: None,
        global_phase: None,
        rank_features: Vec::new(),
        match_features: vec!["score".to_owned()],
        summary_features: Vec::new(),
        sort_features: Vec::new(),
        constants: Vec::new(),
        onnx_models: Vec::new(),
        rank_properties: BTreeMap::from([("foo".to_owned(), "bar".to_owned())]),
        strict: Some(true),
        properties: BTreeMap::new(),
    }];

    let bytes = index_definition_ir_to_api(&definition);
    let api = flatbuffers::root::<api::IndexDefinition>(&bytes).unwrap();
    let first = crate::index_definition_to_ir(api).unwrap();
    let bytes = index_definition_ir_to_api(&first);
    let api = flatbuffers::root::<api::IndexDefinition>(&bytes).unwrap();
    let second = crate::index_definition_to_ir(api).unwrap();
    assert_eq!(first, second);
}

#[cfg(test)]
#[test]
fn indexing_script_only_field_round_trips_through_v2_without_typed_capabilities() {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let name = builder.create_string("derived");
    let primitive = api::PrimitiveDataType::create(
        &mut builder,
        &api::PrimitiveDataTypeArgs {
            kind: api::PrimitiveDataTypeKind::String,
        },
    );
    let data_type = api::DataType::create(
        &mut builder,
        &api::DataTypeArgs {
            kind_type: api::DataTypeKind::PrimitiveDataType,
            kind: Some(primitive.as_union_value()),
        },
    );
    let indexing_script = builder.create_string("input title | attribute(derived)");
    let field = api::FieldDefinition::create(
        &mut builder,
        &api::FieldDefinitionArgs {
            name: Some(name),
            data_type: Some(data_type),
            indexing_script: Some(indexing_script),
            ..Default::default()
        },
    );
    let fields = builder.create_vector(&[field]);
    let definition = api::IndexDefinition::create(
        &mut builder,
        &api::IndexDefinitionArgs {
            fields: Some(fields),
            ..Default::default()
        },
    );
    builder.finish(definition, None);
    let first = crate::index_definition_to_ir(
        flatbuffers::root::<api::IndexDefinition>(builder.finished_data()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        first.fields()[0].indexing_script(),
        Some("input title | attribute(derived)")
    );

    let bytes = index_definition_ir_to_api(&first);
    let second =
        crate::index_definition_to_ir(flatbuffers::root::<api::IndexDefinition>(&bytes).unwrap())
            .unwrap();
    assert_eq!(first, second);
}

#[cfg(test)]
#[test]
fn indexing_script_with_unrelated_metadata_round_trips_losslessly() {
    fn round_trip(include_properties: bool, include_struct_fields: bool) {
        let mut builder = flatbuffers::FlatBufferBuilder::new();
        let name = builder.create_string("derived");
        let primitive = api::PrimitiveDataType::create(
            &mut builder,
            &api::PrimitiveDataTypeArgs {
                kind: api::PrimitiveDataTypeKind::String,
            },
        );
        let data_type = api::DataType::create(
            &mut builder,
            &api::DataTypeArgs {
                kind_type: api::DataTypeKind::PrimitiveDataType,
                kind: Some(primitive.as_union_value()),
            },
        );
        let indexing_script = builder.create_string("input title | attribute(derived)");
        let properties = include_properties.then(|| {
            request_value_entries_offset(
                &mut builder,
                &std::collections::BTreeMap::from([(
                    "some.unrelated.setting".to_owned(),
                    ir::RequestValueIr::String("preserve".to_owned()),
                )]),
            )
            .expect("property entry")
        });
        let struct_fields = include_struct_fields.then(|| {
            let path = builder.create_string("nested");
            let field = api::StructFieldConfig::create(
                &mut builder,
                &api::StructFieldConfigArgs {
                    path: Some(path),
                    ..Default::default()
                },
            );
            builder.create_vector(&[field])
        });
        let field = api::FieldDefinition::create(
            &mut builder,
            &api::FieldDefinitionArgs {
                name: Some(name),
                data_type: Some(data_type),
                indexing_script: Some(indexing_script),
                properties,
                struct_fields,
                ..Default::default()
            },
        );
        let fields = builder.create_vector(&[field]);
        let definition = api::IndexDefinition::create(
            &mut builder,
            &api::IndexDefinitionArgs {
                fields: Some(fields),
                ..Default::default()
            },
        );
        builder.finish(definition, None);

        let first = crate::index_definition_to_ir(
            flatbuffers::root::<api::IndexDefinition>(builder.finished_data()).unwrap(),
        )
        .unwrap();
        let bytes = index_definition_ir_to_api(&first);
        let second = crate::index_definition_to_ir(
            flatbuffers::root::<api::IndexDefinition>(&bytes).unwrap(),
        )
        .unwrap();
        assert_eq!(first, second);
    }

    round_trip(true, false);
    round_trip(false, true);
}

#[cfg(test)]
#[test]
fn rank_phase_controls_are_validated_against_their_phase() {
    use crate::{
        DataTypeIr, FieldCapabilities, FieldIr, FieldName, IndexDefinitionIr, PrimitiveDataTypeIr,
        RankPhaseDefinitionIr, RankProfileDefinitionIr,
    };
    use std::collections::BTreeMap;

    let field = FieldIr::new(
        FieldName::parse("title").unwrap(),
        DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
        None,
        FieldCapabilities::new(true, true, false, false, false),
    );
    let mut definition = IndexDefinitionIr::new(
        None,
        vec![field],
        Vec::new(),
        crate::RankingPolicyIr::DefaultRelevanceV1,
        crate::PrefixPolicy::Disabled,
        crate::TypoPolicy::Disabled,
    )
    .unwrap();
    definition.rank_profiles = vec![RankProfileDefinitionIr {
        name: "invalid".to_owned(),
        inherits: Vec::new(),
        inputs: Vec::new(),
        functions: Vec::new(),
        first_phase: Some(RankPhaseDefinitionIr {
            expression: Some("nativeRank(title)".to_owned()),
            rerank_count: Some(10),
            rank_score_drop_limit: None,
            properties: BTreeMap::new(),
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
        rank_properties: BTreeMap::new(),
        strict: None,
        properties: BTreeMap::new(),
    }];

    let bytes = index_definition_ir_to_api(&definition);
    let error =
        crate::index_definition_to_ir(flatbuffers::root::<api::IndexDefinition>(&bytes).unwrap())
            .unwrap_err();
    assert_eq!(error.details[0].code, "invalid_rank_phase");
    assert_eq!(
        error.details[0].path.as_deref(),
        Some("rank_profiles[0].first_phase")
    );
}

fn document_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    document: &ir::DocumentResult,
) -> flatbuffers::WIPOffset<api::Document<'a>> {
    let id = builder.create_string(&document.id);
    let fields = document
        .fields
        .iter()
        .map(|(key, value)| {
            let key = builder.create_string(key);
            let value = value_offset(builder, value);
            api::ValueEntry::create(
                builder,
                &api::ValueEntryArgs {
                    key: Some(key),
                    value: Some(value),
                },
            )
        })
        .collect::<Vec<_>>();
    let fields = builder.create_vector(&fields);
    api::Document::create(
        builder,
        &api::DocumentArgs {
            id: Some(id),
            fields: Some(fields),
        },
    )
}

fn hit_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    hit: &ir::Hit,
) -> flatbuffers::WIPOffset<api::Hit<'a>> {
    let id = builder.create_string(&hit.id);
    let relevance = hit.score.map(|score| {
        let value =
            api::FiniteRelevance::create(builder, &api::FiniteRelevanceArgs { value: score });
        api::Relevance::create(
            builder,
            &api::RelevanceArgs {
                value_type: api::RelevanceKind::FiniteRelevance,
                value: Some(value.as_union_value()),
            },
        )
    });
    let fields = hit
        .fields
        .iter()
        .map(|(key, value)| {
            let key = builder.create_string(key);
            let value = value_offset(builder, value);
            api::ValueEntry::create(
                builder,
                &api::ValueEntryArgs {
                    key: Some(key),
                    value: Some(value),
                },
            )
        })
        .collect::<Vec<_>>();
    let fields = builder.create_vector(&fields);
    let highlights = hit
        .highlights
        .iter()
        .map(|highlight| highlight_result_offset(builder, highlight))
        .collect::<Vec<_>>();
    let highlights = builder.create_vector(&highlights);
    api::Hit::create(
        builder,
        &api::HitArgs {
            id: Some(id),
            relevance,
            fields: Some(fields),
            highlights: Some(highlights),
            source: None,
            types: None,
            match_features: None,
            summary_features: None,
            rank_features: None,
            sort_values: None,
        },
    )
}

fn highlight_result_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    highlight: &ir::HighlightResult,
) -> flatbuffers::WIPOffset<api::HighlightResult<'a>> {
    let field = builder.create_string(&highlight.field);
    let fragments = highlight
        .fragments
        .iter()
        .map(|fragment| {
            let text = builder.create_string(&fragment.text);
            let matches = fragment
                .matches
                .iter()
                .map(|span| {
                    api::HighlightSpan::create(
                        builder,
                        &api::HighlightSpanArgs {
                            start_byte: span.start_byte,
                            end_byte: span.end_byte,
                        },
                    )
                })
                .collect::<Vec<_>>();
            let matches = builder.create_vector(&matches);
            api::HighlightFragment::create(
                builder,
                &api::HighlightFragmentArgs {
                    text: Some(text),
                    matches: Some(matches),
                },
            )
        })
        .collect::<Vec<_>>();
    let fragments = builder.create_vector(&fragments);
    api::HighlightResult::create(
        builder,
        &api::HighlightResultArgs {
            field: Some(field),
            fragments: Some(fragments),
        },
    )
}
