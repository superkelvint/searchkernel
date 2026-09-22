//! The single FlatBuffers encoder retained by the Rust facade.
//!
//! This is used only to preserve the caller's schema intent in the manifest.
//! Runtime request execution and runtime schema resolution do not use it.

use crate::schema::{Analyzer, DistanceMetric, Field, FieldKind, IndexDefinition, SearchField};
use searchkernel_ir::{__flatbuffers as flatbuffers, api};
use serde_json::Value;

pub(crate) fn encode_portable_definition(definition: &IndexDefinition) -> Vec<u8> {
    // Encode the caller-visible definition. A described value must remain
    // independently serializable; recovering a hidden resolved definition
    // here would reintroduce the schema sidecar bypass.
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let name = definition
        .name
        .as_deref()
        .map(|value| builder.create_string(value));
    let fields = definition
        .fields
        .iter()
        .map(|field| encode_field(&mut builder, field))
        .collect::<Vec<_>>();
    let fields = builder.create_vector(&fields);
    let default_search_fields = definition
        .default_search_fields
        .as_ref()
        .map(|fields| encode_search_fields(&mut builder, fields));
    let root = api::IndexDefinition::create(
        &mut builder,
        &api::IndexDefinitionArgs {
            name,
            fields: Some(fields),
            default_search_fields,
            properties: None,
            ..Default::default()
        },
    );
    builder.finish(root, None);
    builder.finished_data().to_vec()
}

fn encode_field<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    field: &Field,
) -> flatbuffers::WIPOffset<api::FieldDefinition<'a>> {
    let name = builder.create_string(&field.name);
    let analyzer = field
        .analyzer
        .as_ref()
        .map(|analyzer| encode_analyzer(builder, analyzer));
    let stored = field.stored.map(api::OptionalBool::new);
    let searchable = field.searchable.map(api::OptionalBool::new);
    let filterable = field.filterable.map(api::OptionalBool::new);
    let sortable = field.sortable.map(api::OptionalBool::new);
    let facetable = field.facetable.map(api::OptionalBool::new);
    let hnsw = field.hnsw.as_ref().map(|config| {
        let max_links_per_node = config.max_links_per_node.map(api::OptionalUInt32::new);
        let neighbors_to_explore_at_insert = config
            .neighbors_to_explore_at_insert
            .map(api::OptionalUInt32::new);
        api::HnswIndexConfig::create(
            builder,
            &api::HnswIndexConfigArgs {
                max_links_per_node: max_links_per_node.as_ref(),
                neighbors_to_explore_at_insert: neighbors_to_explore_at_insert.as_ref(),
                options: None,
            },
        )
    });
    let index = hnsw.map(|hnsw| {
        let enabled = api::OptionalBool::new(true);
        api::IndexConfig::create(
            builder,
            &api::IndexConfigArgs {
                enabled: Some(&enabled),
                hnsw: Some(hnsw),
                options: None,
            },
        )
    });
    let attribute = (field.distance_metric.is_some() || field.hnsw.is_some()).then(|| {
        let enabled = api::OptionalBool::new(true);
        api::AttributeConfig::create(
            builder,
            &api::AttributeConfigArgs {
                enabled: Some(&enabled),
                fast_search: None,
                fast_access: None,
                distance_metric: match field.distance_metric {
                    None => api::DistanceMetric::Unspecified,
                    Some(DistanceMetric::Unspecified) => api::DistanceMetric::Unspecified,
                    Some(DistanceMetric::Euclidean) => api::DistanceMetric::Euclidean,
                    Some(DistanceMetric::Angular) => api::DistanceMetric::Angular,
                    Some(DistanceMetric::DotProduct) => api::DistanceMetric::DotProduct,
                    Some(DistanceMetric::PrenormalizedAngular) => {
                        api::DistanceMetric::PrenormalizedAngular
                    }
                    Some(DistanceMetric::GeoDegrees) => api::DistanceMetric::GeoDegrees,
                    Some(DistanceMetric::Hamming) => api::DistanceMetric::Hamming,
                },
                options: None,
            },
        )
    });
    let data_type = encode_data_type(builder, &field.kind, field.multi);
    let matching = matches!(&field.kind, FieldKind::Text | FieldKind::Keyword).then(|| {
        api::MatchingConfig::create(
            builder,
            &api::MatchingConfigArgs {
                mode: match &field.kind {
                    FieldKind::Text => api::MatchingMode::Text,
                    FieldKind::Keyword => api::MatchingMode::Exact,
                    _ => api::MatchingMode::Unspecified,
                },
                options: None,
            },
        )
    });
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
            summary: None,
            indexing_script: None,
            properties: None,
            ..Default::default()
        },
    )
}

fn encode_analyzer<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    analyzer: &Analyzer,
) -> flatbuffers::WIPOffset<api::Analyzer<'a>> {
    let name = builder.create_string(&analyzer.name);
    let options = (!analyzer.options.is_empty()).then(|| {
        let entries = analyzer
            .options
            .iter()
            .map(|(key, value)| {
                let key = builder.create_string(key);
                let value = encode_value(builder, value);
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
    });
    api::Analyzer::create(
        builder,
        &api::AnalyzerArgs {
            name: Some(name),
            version: analyzer.version,
            options,
        },
    )
}

fn encode_search_fields<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    fields: &[SearchField],
) -> flatbuffers::WIPOffset<api::SearchFieldSelection<'a>> {
    let fields = fields
        .iter()
        .map(|field| {
            let name = builder.create_string(&field.field);
            let weight = field.weight.map(api::OptionalDouble::new);
            api::SearchField::create(
                builder,
                &api::SearchFieldArgs {
                    field: Some(name),
                    weight: weight.as_ref(),
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

fn encode_data_type<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    kind: &FieldKind,
    multi: bool,
) -> flatbuffers::WIPOffset<api::DataType<'a>> {
    let scalar = match kind {
        FieldKind::Tensor(type_spec) => {
            let type_spec = builder.create_string(type_spec);
            let tensor = api::TensorDataType::create(
                builder,
                &api::TensorDataTypeArgs {
                    type_spec: Some(type_spec),
                },
            );
            api::DataType::create(
                builder,
                &api::DataTypeArgs {
                    kind_type: api::DataTypeKind::TensorDataType,
                    kind: Some(tensor.as_union_value()),
                },
            )
        }
        FieldKind::Position => {
            let position = api::PositionDataType::create(builder, &api::PositionDataTypeArgs {});
            api::DataType::create(
                builder,
                &api::DataTypeArgs {
                    kind_type: api::DataTypeKind::PositionDataType,
                    kind: Some(position.as_union_value()),
                },
            )
        }
        _ => {
            let primitive = api::PrimitiveDataType::create(
                builder,
                &api::PrimitiveDataTypeArgs {
                    kind: match kind {
                        FieldKind::Text | FieldKind::Keyword => api::PrimitiveDataTypeKind::String,
                        FieldKind::Int64 => api::PrimitiveDataTypeKind::Int64,
                        FieldKind::Float64 => api::PrimitiveDataTypeKind::Float64,
                        FieldKind::Bool => api::PrimitiveDataTypeKind::Bool,
                        FieldKind::Position => unreachable!(),
                        FieldKind::Tensor(_) => unreachable!(),
                    },
                },
            );
            api::DataType::create(
                builder,
                &api::DataTypeArgs {
                    kind_type: api::DataTypeKind::PrimitiveDataType,
                    kind: Some(primitive.as_union_value()),
                },
            )
        }
    };
    if !multi {
        return scalar;
    }
    let array = api::ArrayDataType::create(
        builder,
        &api::ArrayDataTypeArgs {
            element_type: Some(scalar),
        },
    );
    api::DataType::create(
        builder,
        &api::DataTypeArgs {
            kind_type: api::DataTypeKind::ArrayDataType,
            kind: Some(array.as_union_value()),
        },
    )
}

fn encode_value<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &Value,
) -> flatbuffers::WIPOffset<api::Value<'a>> {
    let (kind_type, kind) = match value {
        Value::Null => (
            api::ValueKind::NullValueValue,
            api::NullValueValue::create(
                builder,
                &api::NullValueValueArgs {
                    value: api::NullValue::Null,
                },
            )
            .as_union_value(),
        ),
        Value::Bool(value) => (
            api::ValueKind::BoolValue,
            api::BoolValue::create(builder, &api::BoolValueArgs { value: *value }).as_union_value(),
        ),
        Value::Number(value) if value.as_i64().is_some() => (
            api::ValueKind::Int64Value,
            api::Int64Value::create(
                builder,
                &api::Int64ValueArgs {
                    value: value.as_i64().unwrap_or_default(),
                },
            )
            .as_union_value(),
        ),
        Value::Number(value) if value.as_u64().is_some() => (
            api::ValueKind::UInt64Value,
            api::UInt64Value::create(
                builder,
                &api::UInt64ValueArgs {
                    value: value.as_u64().unwrap_or_default(),
                },
            )
            .as_union_value(),
        ),
        Value::Number(value) => (
            api::ValueKind::Float64Value,
            api::Float64Value::create(
                builder,
                &api::Float64ValueArgs {
                    value: value.as_f64().unwrap_or_default(),
                },
            )
            .as_union_value(),
        ),
        Value::String(value) => {
            let value = builder.create_string(value);
            (
                api::ValueKind::StringValue,
                api::StringValue::create(builder, &api::StringValueArgs { value: Some(value) })
                    .as_union_value(),
            )
        }
        Value::Array(values) => {
            let values = values
                .iter()
                .map(|value| encode_value(builder, value))
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
        Value::Object(fields) => {
            let fields = fields
                .iter()
                .map(|(key, value)| {
                    let key = builder.create_string(key);
                    let value = encode_value(builder, value);
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
            (
                api::ValueKind::ValueObject,
                api::ValueObject::create(
                    builder,
                    &api::ValueObjectArgs {
                        fields: Some(fields),
                    },
                )
                .as_union_value(),
            )
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
