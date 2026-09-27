use super::*;
use crate::schema::{ConstantSource, DataType, FieldKind, PrimitiveDataType};
use crate::{
    field, Analyzer, Filter, GroupingPlan, Query, QueryAlternative, Ranking, SearchBuilder, Sort,
    Tensor, WandOptions,
};
use searchkernel_ir::{BackendSession, IndexContext, OperationIr};
use searchkernel_protocol::v2 as api;
use searchkernel_testkit::{ExpectedCall, RecordingBackend, ScriptedBackend, ScriptedResult};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;

fn test_index() -> (IndexRuntime, Index, Arc<RecordingBackend>) {
    let backend = Arc::new(RecordingBackend::new());
    let definition = IndexDefinition::new()
        .field(Field::text("title").stored(true))
        .field(Field::float64("price"));
    let runtime =
        IndexRuntime::from_backend(definition, backend.clone()).expect("valid test index");
    let index = runtime.index();
    (runtime, index, backend)
}

fn runtime_index(definition: IndexDefinition, backend: Arc<dyn Backend>) -> (IndexRuntime, Index) {
    let runtime = IndexRuntime::from_backend(definition, backend).expect("valid test index");
    let index = runtime.index();
    (runtime, index)
}

fn index_from_canonical_ir(definition: searchkernel_ir::IndexDefinitionIr) -> Index {
    let context = IndexContext::from_definition_ir(definition);
    Index::from_owned_backend(
        context,
        BackendSession::from_backend(Arc::new(RecordingBackend::new())),
        None,
    )
    .expect("canonical schema should construct an index")
}

fn string_value<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    value: &str,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::Value<'a>> {
    let value = builder.create_string(value);
    let string = api::StringValue::create(builder, &api::StringValueArgs { value: Some(value) });
    api::Value::create(
        builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::StringValue,
            kind: Some(string.as_union_value()),
        },
    )
}

fn string_properties<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    values: &[(&str, &str)],
) -> searchkernel_ir::__flatbuffers::WIPOffset<
    searchkernel_ir::__flatbuffers::Vector<
        'a,
        searchkernel_ir::__flatbuffers::ForwardsUOffset<api::ValueEntry<'a>>,
    >,
> {
    let entries = values
        .iter()
        .map(|(key, value)| {
            let key = builder.create_string(key);
            let value = string_value(builder, value);
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
}

fn string_entries<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    values: &[(&str, &str)],
) -> searchkernel_ir::__flatbuffers::WIPOffset<
    searchkernel_ir::__flatbuffers::Vector<
        'a,
        searchkernel_ir::__flatbuffers::ForwardsUOffset<api::StringEntry<'a>>,
    >,
> {
    let entries = values
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
}

fn strings<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    values: &[&str],
) -> searchkernel_ir::__flatbuffers::WIPOffset<
    searchkernel_ir::__flatbuffers::Vector<
        'a,
        searchkernel_ir::__flatbuffers::ForwardsUOffset<&'a str>,
    >,
> {
    let values = values
        .iter()
        .map(|value| builder.create_string(value))
        .collect::<Vec<_>>();
    builder.create_vector(&values)
}

fn primitive<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    kind: api::PrimitiveDataTypeKind,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::DataType<'a>> {
    let primitive = api::PrimitiveDataType::create(builder, &api::PrimitiveDataTypeArgs { kind });
    api::DataType::create(
        builder,
        &api::DataTypeArgs {
            kind_type: api::DataTypeKind::PrimitiveDataType,
            kind: Some(primitive.as_union_value()),
        },
    )
}

fn tensor<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    type_spec: &str,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::DataType<'a>> {
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

fn array<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    element_type: searchkernel_ir::__flatbuffers::WIPOffset<api::DataType<'a>>,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::DataType<'a>> {
    let array = api::ArrayDataType::create(
        builder,
        &api::ArrayDataTypeArgs {
            element_type: Some(element_type),
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

fn map<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    key_type: searchkernel_ir::__flatbuffers::WIPOffset<api::DataType<'a>>,
    value_type: searchkernel_ir::__flatbuffers::WIPOffset<api::DataType<'a>>,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::DataType<'a>> {
    let map = api::MapDataType::create(
        builder,
        &api::MapDataTypeArgs {
            key_type: Some(key_type),
            value_type: Some(value_type),
        },
    );
    api::DataType::create(
        builder,
        &api::DataTypeArgs {
            kind_type: api::DataTypeKind::MapDataType,
            kind: Some(map.as_union_value()),
        },
    )
}

fn weighted_set<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    element_type: searchkernel_ir::__flatbuffers::WIPOffset<api::DataType<'a>>,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::DataType<'a>> {
    let weighted_set = api::WeightedSetDataType::create(
        builder,
        &api::WeightedSetDataTypeArgs {
            element_type: Some(element_type),
        },
    );
    api::DataType::create(
        builder,
        &api::DataTypeArgs {
            kind_type: api::DataTypeKind::WeightedSetDataType,
            kind: Some(weighted_set.as_union_value()),
        },
    )
}

fn struct_type<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    fields: &[(
        &str,
        searchkernel_ir::__flatbuffers::WIPOffset<api::DataType<'a>>,
    )],
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::DataType<'a>> {
    let fields = fields
        .iter()
        .map(|(name, data_type)| {
            let name = builder.create_string(name);
            api::StructFieldDefinition::create(
                builder,
                &api::StructFieldDefinitionArgs {
                    name: Some(name),
                    data_type: Some(*data_type),
                },
            )
        })
        .collect::<Vec<_>>();
    let fields = builder.create_vector(&fields);
    let value = api::StructDataType::create(
        builder,
        &api::StructDataTypeArgs {
            fields: Some(fields),
        },
    );
    api::DataType::create(
        builder,
        &api::DataTypeArgs {
            kind_type: api::DataTypeKind::StructDataType,
            kind: Some(value.as_union_value()),
        },
    )
}

fn struct_reference<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    name: &str,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::DataType<'a>> {
    let name = builder.create_string(name);
    let value = api::StructReferenceDataType::create(
        builder,
        &api::StructReferenceDataTypeArgs { name: Some(name) },
    );
    api::DataType::create(
        builder,
        &api::DataTypeArgs {
            kind_type: api::DataTypeKind::StructReferenceDataType,
            kind: Some(value.as_union_value()),
        },
    )
}

fn empty_data_type<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    kind_type: api::DataTypeKind,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::DataType<'a>> {
    let kind = match kind_type {
        api::DataTypeKind::PredicateDataType => {
            api::PredicateDataType::create(builder, &api::PredicateDataTypeArgs {}).as_union_value()
        }
        api::DataTypeKind::PositionDataType => {
            api::PositionDataType::create(builder, &api::PositionDataTypeArgs {}).as_union_value()
        }
        _ => unreachable!("only empty data types use this helper"),
    };
    api::DataType::create(
        builder,
        &api::DataTypeArgs {
            kind_type,
            kind: Some(kind),
        },
    )
}

fn reference<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    document_type: &str,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::DataType<'a>> {
    let document_type = builder.create_string(document_type);
    let value = api::ReferenceDataType::create(
        builder,
        &api::ReferenceDataTypeArgs {
            document_type: Some(document_type),
        },
    );
    api::DataType::create(
        builder,
        &api::DataTypeArgs {
            kind_type: api::DataTypeKind::ReferenceDataType,
            kind: Some(value.as_union_value()),
        },
    )
}

fn matching<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    mode: api::MatchingMode,
    properties: &[(&str, &str)],
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::MatchingConfig<'a>> {
    let options = string_properties(builder, properties);
    api::MatchingConfig::create(
        builder,
        &api::MatchingConfigArgs {
            mode,
            options: Some(options),
        },
    )
}

fn index_config<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    enabled: Option<bool>,
    hnsw: Option<searchkernel_ir::__flatbuffers::WIPOffset<api::HnswIndexConfig<'a>>>,
    properties: &[(&str, &str)],
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::IndexConfig<'a>> {
    let enabled = enabled.map(api::OptionalBool::new);
    let options = string_properties(builder, properties);
    api::IndexConfig::create(
        builder,
        &api::IndexConfigArgs {
            enabled: enabled.as_ref(),
            hnsw,
            options: Some(options),
        },
    )
}

fn hnsw_config<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    max_links_per_node: u32,
    neighbors_to_explore_at_insert: u32,
    properties: &[(&str, &str)],
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::HnswIndexConfig<'a>> {
    let max_links_per_node = api::OptionalUInt32::new(max_links_per_node);
    let neighbors_to_explore_at_insert = api::OptionalUInt32::new(neighbors_to_explore_at_insert);
    let options = string_properties(builder, properties);
    api::HnswIndexConfig::create(
        builder,
        &api::HnswIndexConfigArgs {
            max_links_per_node: Some(&max_links_per_node),
            neighbors_to_explore_at_insert: Some(&neighbors_to_explore_at_insert),
            options: Some(options),
        },
    )
}

fn attribute<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    enabled: Option<bool>,
    fast_search: Option<bool>,
    fast_access: Option<bool>,
    distance_metric: api::DistanceMetric,
    properties: &[(&str, &str)],
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::AttributeConfig<'a>> {
    let enabled = enabled.map(api::OptionalBool::new);
    let fast_search = fast_search.map(api::OptionalBool::new);
    let fast_access = fast_access.map(api::OptionalBool::new);
    let options = string_properties(builder, properties);
    api::AttributeConfig::create(
        builder,
        &api::AttributeConfigArgs {
            enabled: enabled.as_ref(),
            fast_search: fast_search.as_ref(),
            fast_access: fast_access.as_ref(),
            distance_metric,
            options: Some(options),
        },
    )
}

fn summary<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    enabled: Option<bool>,
    properties: &[(&str, &str)],
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::SummaryConfig<'a>> {
    let enabled = enabled.map(api::OptionalBool::new);
    let options = string_properties(builder, properties);
    api::SummaryConfig::create(
        builder,
        &api::SummaryConfigArgs {
            enabled: enabled.as_ref(),
            options: Some(options),
        },
    )
}

fn analyzer<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    name: &str,
    version: u32,
    properties: &[(&str, &str)],
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::Analyzer<'a>> {
    let name = builder.create_string(name);
    let options = string_properties(builder, properties);
    api::Analyzer::create(
        builder,
        &api::AnalyzerArgs {
            name: Some(name),
            version,
            options: Some(options),
        },
    )
}

fn struct_field_config<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    path: &str,
    matching: Option<searchkernel_ir::__flatbuffers::WIPOffset<api::MatchingConfig<'a>>>,
    index: Option<searchkernel_ir::__flatbuffers::WIPOffset<api::IndexConfig<'a>>>,
    attribute: Option<searchkernel_ir::__flatbuffers::WIPOffset<api::AttributeConfig<'a>>>,
    summary: Option<searchkernel_ir::__flatbuffers::WIPOffset<api::SummaryConfig<'a>>>,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::StructFieldConfig<'a>> {
    let path = builder.create_string(path);
    let indexing_script = builder.create_string("input nested | attribute(nested)");
    let rank = builder.create_string("nested_rank");
    let query_commands = strings(builder, &["nested_query"]);
    let properties = string_properties(builder, &[("nested.property", "value")]);
    api::StructFieldConfig::create(
        builder,
        &api::StructFieldConfigArgs {
            path: Some(path),
            indexing_script: Some(indexing_script),
            matching,
            index,
            attribute,
            summary,
            rank: Some(rank),
            query_commands: Some(query_commands),
            properties: Some(properties),
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn fixture_field_with_struct_fields<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    name: &str,
    data_type: searchkernel_ir::__flatbuffers::WIPOffset<api::DataType<'a>>,
    stored: Option<bool>,
    searchable: Option<bool>,
    filterable: Option<bool>,
    sortable: Option<bool>,
    facetable: Option<bool>,
    analyzer: Option<searchkernel_ir::__flatbuffers::WIPOffset<api::Analyzer<'a>>>,
    matching: Option<searchkernel_ir::__flatbuffers::WIPOffset<api::MatchingConfig<'a>>>,
    index: Option<searchkernel_ir::__flatbuffers::WIPOffset<api::IndexConfig<'a>>>,
    attribute: Option<searchkernel_ir::__flatbuffers::WIPOffset<api::AttributeConfig<'a>>>,
    summary: Option<searchkernel_ir::__flatbuffers::WIPOffset<api::SummaryConfig<'a>>>,
    indexing_script: Option<&str>,
    properties: Option<
        searchkernel_ir::__flatbuffers::WIPOffset<
            searchkernel_ir::__flatbuffers::Vector<
                'a,
                searchkernel_ir::__flatbuffers::ForwardsUOffset<api::ValueEntry<'a>>,
            >,
        >,
    >,
    struct_fields: Option<
        searchkernel_ir::__flatbuffers::WIPOffset<
            searchkernel_ir::__flatbuffers::Vector<
                'a,
                searchkernel_ir::__flatbuffers::ForwardsUOffset<api::StructFieldConfig<'a>>,
            >,
        >,
    >,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::FieldDefinition<'a>> {
    let name = builder.create_string(name);
    let stored = stored.map(api::OptionalBool::new);
    let searchable = searchable.map(api::OptionalBool::new);
    let filterable = filterable.map(api::OptionalBool::new);
    let sortable = sortable.map(api::OptionalBool::new);
    let facetable = facetable.map(api::OptionalBool::new);
    let indexing_script = indexing_script.map(|value| builder.create_string(value));
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

macro_rules! fixture_field {
    (
        $builder:expr,
        $name:expr,
        $data_type:expr,
        $stored:expr,
        $searchable:expr,
        $filterable:expr,
        $sortable:expr,
        $facetable:expr,
        $analyzer:expr,
        $matching:expr,
        $index:expr,
        $attribute:expr,
        $summary:expr,
        $indexing_script:expr,
        $properties:expr,
        $struct_fields:expr $(,)?
    ) => {{
        let data_type = $data_type;
        let analyzer = $analyzer;
        let matching = $matching;
        let index = $index;
        let attribute = $attribute;
        let summary = $summary;
        let properties = $properties;
        let struct_fields = $struct_fields;
        fixture_field_with_struct_fields(
            $builder,
            $name,
            data_type,
            $stored,
            $searchable,
            $filterable,
            $sortable,
            $facetable,
            analyzer,
            matching,
            index,
            attribute,
            summary,
            $indexing_script,
            properties,
            struct_fields,
        )
    }};
}

fn float_value<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    value: f64,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::Value<'a>> {
    let value = api::Float64Value::create(builder, &api::Float64ValueArgs { value });
    api::Value::create(
        builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::Float64Value,
            kind: Some(value.as_union_value()),
        },
    )
}

fn tensor_value<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    type_spec: &str,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::Value<'a>> {
    let type_spec = builder.create_string(type_spec);
    let values = builder.create_vector(&[1.0_f64, 2.0, 3.0]);
    let dense = api::DenseTensorValues::create(
        builder,
        &api::DenseTensorValuesArgs {
            values: Some(values),
        },
    );
    let tensor = api::TensorValue::create(
        builder,
        &api::TensorValueArgs {
            type_spec: Some(type_spec),
            payload_type: api::TensorPayloadKind::DenseTensorValues,
            payload: Some(dense.as_union_value()),
        },
    );
    api::Value::create(
        builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::TensorValue,
            kind: Some(tensor.as_union_value()),
        },
    )
}

fn rank_phase<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    expression: &str,
    rerank_count: Option<u32>,
    rank_score_drop_limit: Option<f64>,
    total_rerank_count: Option<u32>,
    keep_rank_count: Option<u32>,
    total_keep_rank_count: Option<u32>,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::RankPhaseDefinition<'a>> {
    let expression = builder.create_string(expression);
    let rerank_count = rerank_count.map(api::OptionalUInt32::new);
    let rank_score_drop_limit = rank_score_drop_limit.map(api::OptionalDouble::new);
    let total_rerank_count = total_rerank_count.map(api::OptionalUInt32::new);
    let keep_rank_count = keep_rank_count.map(api::OptionalUInt32::new);
    let total_keep_rank_count = total_keep_rank_count.map(api::OptionalUInt32::new);
    let properties = string_properties(builder, &[("phase.property", "value")]);
    api::RankPhaseDefinition::create(
        builder,
        &api::RankPhaseDefinitionArgs {
            expression: Some(expression),
            rerank_count: rerank_count.as_ref(),
            rank_score_drop_limit: rank_score_drop_limit.as_ref(),
            properties: Some(properties),
            total_rerank_count: total_rerank_count.as_ref(),
            keep_rank_count: keep_rank_count.as_ref(),
            total_keep_rank_count: total_keep_rank_count.as_ref(),
        },
    )
}

fn literal_constant<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    data_type: searchkernel_ir::__flatbuffers::WIPOffset<api::DataType<'a>>,
    value: searchkernel_ir::__flatbuffers::WIPOffset<api::Value<'a>>,
    name: &str,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::ConstantDefinition<'a>> {
    let name = builder.create_string(name);
    let source = api::LiteralConstantSource::create(
        builder,
        &api::LiteralConstantSourceArgs { value: Some(value) },
    );
    let properties = string_properties(builder, &[("constant.property", "literal")]);
    api::ConstantDefinition::create(
        builder,
        &api::ConstantDefinitionArgs {
            name: Some(name),
            data_type: Some(data_type),
            source_type: api::ConstantSourceKind::LiteralConstantSource,
            source: Some(source.as_union_value()),
            properties: Some(properties),
        },
    )
}

fn file_constant<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    data_type: searchkernel_ir::__flatbuffers::WIPOffset<api::DataType<'a>>,
    path: &str,
    name: &str,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::ConstantDefinition<'a>> {
    let name = builder.create_string(name);
    let path = builder.create_string(path);
    let source =
        api::FileConstantSource::create(builder, &api::FileConstantSourceArgs { path: Some(path) });
    let properties = string_properties(builder, &[("constant.property", "file")]);
    api::ConstantDefinition::create(
        builder,
        &api::ConstantDefinitionArgs {
            name: Some(name),
            data_type: Some(data_type),
            source_type: api::ConstantSourceKind::FileConstantSource,
            source: Some(source.as_union_value()),
            properties: Some(properties),
        },
    )
}

fn onnx_model<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    name: &str,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::OnnxModelDefinition<'a>> {
    let name = builder.create_string(name);
    let file = builder.create_string("model.onnx");
    let inputs = string_entries(builder, &[("input", "query(vector)")]);
    let outputs = string_entries(builder, &[("output", "onnx_output")]);
    let gpu_device = api::OptionalUInt32::new(2);
    let intraop_threads = api::OptionalUInt32::new(3);
    let interop_threads = api::OptionalUInt32::new(4);
    let properties = string_properties(builder, &[("onnx.property", "value")]);
    api::OnnxModelDefinition::create(
        builder,
        &api::OnnxModelDefinitionArgs {
            name: Some(name),
            file: Some(file),
            inputs: Some(inputs),
            outputs: Some(outputs),
            gpu_device: Some(&gpu_device),
            intraop_threads: Some(&intraop_threads),
            interop_threads: Some(&interop_threads),
            properties: Some(properties),
        },
    )
}

fn canonical_schema_fixture_named(name: &str, title_weight: f64, tags_weight: f64) -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();

    let title_data_type = primitive(&mut builder, api::PrimitiveDataTypeKind::String);
    let title_matching = matching(
        &mut builder,
        api::MatchingMode::Text,
        &[("matching.option", "title")],
    );
    let title_index = index_config(&mut builder, Some(true), None, &[("index.option", "title")]);
    let title_attribute = attribute(
        &mut builder,
        Some(true),
        Some(false),
        Some(true),
        api::DistanceMetric::Unspecified,
        &[("attribute.option", "title")],
    );
    let title_summary = summary(&mut builder, Some(true), &[("summary.option", "title")]);
    let title_analyzer = analyzer(&mut builder, "standard", 1, &[]);
    let nested_matching = matching(
        &mut builder,
        api::MatchingMode::Exact,
        &[("nested.matching", "exact")],
    );
    let nested_index = index_config(&mut builder, Some(true), None, &[("nested.index", "true")]);
    let nested_attribute = attribute(
        &mut builder,
        Some(true),
        Some(false),
        Some(true),
        api::DistanceMetric::Unspecified,
        &[("nested.attribute", "true")],
    );
    let nested_summary = summary(&mut builder, Some(true), &[("nested.summary", "true")]);
    let nested_config = struct_field_config(
        &mut builder,
        "value.first_name",
        Some(nested_matching),
        Some(nested_index),
        Some(nested_attribute),
        Some(nested_summary),
    );
    let title_struct_fields = builder.create_vector(&[nested_config]);
    let title_properties = string_properties(
        &mut builder,
        &[("field.property", "title"), ("field.second", "preserve")],
    );
    let title = fixture_field!(
        &mut builder,
        "title",
        title_data_type,
        Some(true),
        Some(true),
        Some(false),
        Some(false),
        Some(false),
        Some(title_analyzer),
        Some(title_matching),
        Some(title_index),
        Some(title_attribute),
        Some(title_summary),
        None,
        Some(title_properties),
        Some(title_struct_fields),
    );

    let flag = fixture_field!(
        &mut builder,
        "flag",
        primitive(&mut builder, api::PrimitiveDataTypeKind::Bool),
        Some(false),
        Some(false),
        Some(true),
        Some(true),
        Some(true),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let byte_value = fixture_field!(
        &mut builder,
        "byte_value",
        primitive(&mut builder, api::PrimitiveDataTypeKind::Byte),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let count32 = fixture_field!(
        &mut builder,
        "count32",
        primitive(&mut builder, api::PrimitiveDataTypeKind::Int32),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let count64 = fixture_field!(
        &mut builder,
        "count64",
        primitive(&mut builder, api::PrimitiveDataTypeKind::Int64),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let ratio32 = fixture_field!(
        &mut builder,
        "ratio32",
        primitive(&mut builder, api::PrimitiveDataTypeKind::Float32),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let ratio64 = fixture_field!(
        &mut builder,
        "ratio64",
        primitive(&mut builder, api::PrimitiveDataTypeKind::Float64),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let payload = fixture_field!(
        &mut builder,
        "payload",
        primitive(&mut builder, api::PrimitiveDataTypeKind::Raw),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );

    let embedding_hnsw = hnsw_config(&mut builder, 16, 100, &[("hnsw.option", "value")]);
    let embedding_index = index_config(
        &mut builder,
        Some(true),
        Some(embedding_hnsw),
        &[("embedding.index", "true")],
    );
    let embedding_attribute = attribute(
        &mut builder,
        Some(true),
        Some(false),
        Some(true),
        api::DistanceMetric::Angular,
        &[("embedding.attribute", "angular")],
    );
    let embedding = fixture_field!(
        &mut builder,
        "embedding",
        tensor(&mut builder, "tensor<float>(x[3])"),
        Some(false),
        Some(true),
        Some(false),
        Some(false),
        Some(false),
        None,
        None,
        Some(embedding_index),
        Some(embedding_attribute),
        None,
        None,
        None,
        None,
    );
    let tags_element_type = primitive(&mut builder, api::PrimitiveDataTypeKind::String);
    let tags_type = array(&mut builder, tags_element_type);
    let tags_analyzer = analyzer(&mut builder, "standard", 1, &[]);
    let tags_matching = matching(&mut builder, api::MatchingMode::Text, &[]);
    let tags = fixture_field!(
        &mut builder,
        "tags",
        tags_type,
        Some(true),
        Some(true),
        Some(false),
        Some(false),
        Some(false),
        Some(tags_analyzer),
        Some(tags_matching),
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let scores_key_type = primitive(&mut builder, api::PrimitiveDataTypeKind::String);
    let scores_value_type = primitive(&mut builder, api::PrimitiveDataTypeKind::Float32);
    let scores_type = map(&mut builder, scores_key_type, scores_value_type);
    let scores = fixture_field!(
        &mut builder,
        "scores",
        scores_type,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let labels_element_type = primitive(&mut builder, api::PrimitiveDataTypeKind::String);
    let labels_type = weighted_set(&mut builder, labels_element_type);
    let labels_matching = matching(&mut builder, api::MatchingMode::Exact, &[]);
    let labels = fixture_field!(
        &mut builder,
        "labels",
        labels_type,
        Some(true),
        Some(false),
        Some(true),
        Some(false),
        Some(true),
        None,
        Some(labels_matching),
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let inline_part_type = primitive(&mut builder, api::PrimitiveDataTypeKind::Bool);
    let inline_value_type = primitive(&mut builder, api::PrimitiveDataTypeKind::Int32);
    let inline_type = struct_type(
        &mut builder,
        &[("part", inline_part_type), ("value", inline_value_type)],
    );
    let inline = fixture_field!(
        &mut builder,
        "inline",
        inline_type,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let address = fixture_field!(
        &mut builder,
        "address",
        struct_reference(&mut builder, "address"),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let predicate = fixture_field!(
        &mut builder,
        "predicate",
        empty_data_type(&mut builder, api::DataTypeKind::PredicateDataType),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let location = fixture_field!(
        &mut builder,
        "location",
        empty_data_type(&mut builder, api::DataTypeKind::PositionDataType),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let parent = fixture_field!(
        &mut builder,
        "parent",
        reference(&mut builder, "parent_doc"),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let derived = fixture_field!(
        &mut builder,
        "derived",
        primitive(&mut builder, api::PrimitiveDataTypeKind::String),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some("input title | attribute(derived)"),
        Some(string_properties(
            &mut builder,
            &[("derived.property", "value")]
        )),
        None,
    );
    let fields = builder.create_vector(&[
        title, flag, byte_value, count32, count64, ratio32, ratio64, payload, embedding, tags,
        scores, labels, inline, address, predicate, location, parent,
    ]);
    let schema_fields = {
        let derived = api::SearchOnlyFieldDefinition::create(
            &mut builder,
            &api::SearchOnlyFieldDefinitionArgs {
                field: Some(derived),
            },
        );
        builder.create_vector(&[derived])
    };

    let address_fields = {
        let city = builder.create_string("city");
        let city_type = primitive(&mut builder, api::PrimitiveDataTypeKind::String);
        let city = api::StructFieldDefinition::create(
            &mut builder,
            &api::StructFieldDefinitionArgs {
                name: Some(city),
                data_type: Some(city_type),
            },
        );
        let zip = builder.create_string("zip");
        let zip_type = primitive(&mut builder, api::PrimitiveDataTypeKind::Int32);
        let zip = api::StructFieldDefinition::create(
            &mut builder,
            &api::StructFieldDefinitionArgs {
                name: Some(zip),
                data_type: Some(zip_type),
            },
        );
        builder.create_vector(&[city, zip])
    };
    let named_struct = {
        let name = builder.create_string("address");
        let properties = string_properties(&mut builder, &[("struct.property", "value")]);
        api::NamedStructDefinition::create(
            &mut builder,
            &api::NamedStructDefinitionArgs {
                name: Some(name),
                fields: Some(address_fields),
                properties: Some(properties),
            },
        )
    };
    let structs = builder.create_vector(&[named_struct]);

    let fieldset = {
        let name = builder.create_string("all_text");
        let field_names = strings(&mut builder, &["title", "tags"]);
        let matching = matching(
            &mut builder,
            api::MatchingMode::Word,
            &[("fieldset.matching", "word")],
        );
        let query_commands = strings(&mut builder, &["stem", "normalize"]);
        let properties = string_properties(&mut builder, &[("fieldset.property", "value")]);
        api::FieldSetDefinition::create(
            &mut builder,
            &api::FieldSetDefinitionArgs {
                name: Some(name),
                fields: Some(field_names),
                matching: Some(matching),
                query_commands: Some(query_commands),
                properties: Some(properties),
            },
        )
    };
    let fieldsets = builder.create_vector(&[fieldset]);

    let imported_field = {
        let reference_field = builder.create_string("parent");
        let target_field = builder.create_string("name");
        let alias = builder.create_string("parent_name");
        let properties = string_properties(&mut builder, &[("import.property", "value")]);
        api::ImportedFieldDefinition::create(
            &mut builder,
            &api::ImportedFieldDefinitionArgs {
                reference_field: Some(reference_field),
                target_field: Some(target_field),
                alias: Some(alias),
                properties: Some(properties),
            },
        )
    };
    let imported_fields = builder.create_vector(&[imported_field]);

    let summary_field = {
        let name = builder.create_string("title_summary");
        let source = builder.create_string("title");
        let data_type = primitive(&mut builder, api::PrimitiveDataTypeKind::String);
        let struct_fields = strings(&mut builder, &["value.first_name"]);
        let properties = string_properties(&mut builder, &[("summary.field", "value")]);
        api::DocumentSummaryFieldDefinition::create(
            &mut builder,
            &api::DocumentSummaryFieldDefinitionArgs {
                name: Some(name),
                source: Some(source),
                data_type: Some(data_type),
                struct_fields: Some(struct_fields),
                properties: Some(properties),
            },
        )
    };
    let summary_fields = builder.create_vector(&[summary_field]);
    let document_summary = {
        let name = builder.create_string("short");
        let inherits = strings(&mut builder, &["base_summary"]);
        let from_disk = api::OptionalBool::new(false);
        let omit_summary_features = api::OptionalBool::new(true);
        let properties = string_properties(&mut builder, &[("summary.property", "value")]);
        api::DocumentSummaryDefinition::create(
            &mut builder,
            &api::DocumentSummaryDefinitionArgs {
                name: Some(name),
                inherits: Some(inherits),
                fields: Some(summary_fields),
                from_disk: Some(&from_disk),
                omit_summary_features: Some(&omit_summary_features),
                properties: Some(properties),
            },
        )
    };
    let document_summaries = builder.create_vector(&[document_summary]);

    let input_scalar = {
        let name = builder.create_string("query_boost");
        let data_type = primitive(&mut builder, api::PrimitiveDataTypeKind::Float64);
        let default_value = float_value(&mut builder, 1.5);
        api::RankProfileInput::create(
            &mut builder,
            &api::RankProfileInputArgs {
                name: Some(name),
                data_type: Some(data_type),
                default_value: Some(default_value),
            },
        )
    };
    let input_tensor = {
        let name = builder.create_string("query_vector");
        let data_type = tensor(&mut builder, "tensor<float>(x[3])");
        let default_value = tensor_value(&mut builder, "tensor<float>(x[3])");
        api::RankProfileInput::create(
            &mut builder,
            &api::RankProfileInputArgs {
                name: Some(name),
                data_type: Some(data_type),
                default_value: Some(default_value),
            },
        )
    };
    let inputs = builder.create_vector(&[input_scalar, input_tensor]);
    let function_a = {
        let name = builder.create_string("boost");
        let arguments = strings(&mut builder, &["x", "y"]);
        let expression = builder.create_string("x + y");
        let inline = api::OptionalBool::new(true);
        api::RankFunctionDefinition::create(
            &mut builder,
            &api::RankFunctionDefinitionArgs {
                name: Some(name),
                arguments: Some(arguments),
                expression: Some(expression),
                inline: Some(&inline),
            },
        )
    };
    let function_b = {
        let name = builder.create_string("slow");
        let arguments = strings(&mut builder, &["x"]);
        let expression = builder.create_string("x * 2");
        let inline = api::OptionalBool::new(false);
        api::RankFunctionDefinition::create(
            &mut builder,
            &api::RankFunctionDefinitionArgs {
                name: Some(name),
                arguments: Some(arguments),
                expression: Some(expression),
                inline: Some(&inline),
            },
        )
    };
    let function_c = {
        let name = builder.create_string("default_inline");
        let arguments = strings(&mut builder, &["x"]);
        let expression = builder.create_string("x");
        api::RankFunctionDefinition::create(
            &mut builder,
            &api::RankFunctionDefinitionArgs {
                name: Some(name),
                arguments: Some(arguments),
                expression: Some(expression),
                inline: None,
            },
        )
    };
    let functions = builder.create_vector(&[function_a, function_b, function_c]);

    let complete_profile = {
        let name = builder.create_string("complete_profile");
        let inherits = strings(&mut builder, &["base_profile"]);
        let functions = Some(functions);
        let first_phase = rank_phase(
            &mut builder,
            "nativeRank(title)",
            None,
            Some(0.25),
            None,
            Some(5),
            None,
        );
        let second_phase = rank_phase(
            &mut builder,
            "rerank(embedding)",
            Some(10),
            None,
            None,
            None,
            None,
        );
        let global_phase = rank_phase(
            &mut builder,
            "globalPhaseScore",
            None,
            Some(0.5),
            None,
            None,
            None,
        );
        let rank_features = strings(&mut builder, &["nativeRank", "bm25(title)"]);
        let match_features = strings(
            &mut builder,
            &["attributeMatch(title)", "fieldMatch(title)"],
        );
        let summary_features = strings(&mut builder, &["summary(title)"]);
        let sort_features = strings(&mut builder, &["sort(title)"]);
        let profile_constant_type = primitive(&mut builder, api::PrimitiveDataTypeKind::Float64);
        let profile_constant_value = float_value(&mut builder, 2.0);
        let profile_constant = literal_constant(
            &mut builder,
            profile_constant_type,
            profile_constant_value,
            "profile_bias",
        );
        let constants = builder.create_vector(&[profile_constant]);
        let profile_model = onnx_model(&mut builder, "profile_model");
        let onnx_models = builder.create_vector(&[profile_model]);
        let rank_properties = string_entries(&mut builder, &[("rank.property", "value")]);
        let strict = api::OptionalBool::new(true);
        let properties = string_properties(&mut builder, &[("profile.property", "value")]);
        api::RankProfileDefinition::create(
            &mut builder,
            &api::RankProfileDefinitionArgs {
                name: Some(name),
                inherits: Some(inherits),
                inputs: Some(inputs),
                functions,
                first_phase: Some(first_phase),
                second_phase: Some(second_phase),
                global_phase: Some(global_phase),
                rank_features: Some(rank_features),
                match_features: Some(match_features),
                summary_features: Some(summary_features),
                sort_features: Some(sort_features),
                constants: Some(constants),
                onnx_models: Some(onnx_models),
                rank_properties: Some(rank_properties),
                strict: Some(&strict),
                properties: Some(properties),
            },
        )
    };
    let alternate_profile = {
        let name = builder.create_string("alternate_counts_profile");
        let first_phase = rank_phase(&mut builder, "first", None, None, None, None, Some(6));
        let second_phase = rank_phase(&mut builder, "second", None, None, Some(11), None, None);
        let global_phase = rank_phase(&mut builder, "global", Some(3), None, None, None, None);
        api::RankProfileDefinition::create(
            &mut builder,
            &api::RankProfileDefinitionArgs {
                name: Some(name),
                inherits: None,
                inputs: None,
                functions: None,
                first_phase: Some(first_phase),
                second_phase: Some(second_phase),
                global_phase: Some(global_phase),
                rank_features: None,
                match_features: None,
                summary_features: None,
                sort_features: None,
                constants: None,
                onnx_models: None,
                rank_properties: None,
                strict: None,
                properties: None,
            },
        )
    };
    let sparse_profile = {
        let name = builder.create_string("sparse_profile");
        api::RankProfileDefinition::create(
            &mut builder,
            &api::RankProfileDefinitionArgs {
                name: Some(name),
                ..Default::default()
            },
        )
    };
    let rank_profiles =
        builder.create_vector(&[complete_profile, alternate_profile, sparse_profile]);

    let schema_constant_type = primitive(&mut builder, api::PrimitiveDataTypeKind::Float64);
    let schema_constant_value = float_value(&mut builder, 1.0);
    let schema_constant = literal_constant(
        &mut builder,
        schema_constant_type,
        schema_constant_value,
        "schema_bias",
    );
    let file_constant_type = tensor(&mut builder, "tensor<float>(x[3])");
    let file_constant = file_constant(
        &mut builder,
        file_constant_type,
        "constants/tensor.json",
        "schema_tensor",
    );
    let constants = builder.create_vector(&[schema_constant, file_constant]);
    let schema_model = onnx_model(&mut builder, "schema_model");
    let onnx_models = builder.create_vector(&[schema_model]);

    let name = builder.create_string(name);
    let default_title = builder.create_string("title");
    let default_title_weight = api::OptionalDouble::new(title_weight);
    let default_title = api::SearchField::create(
        &mut builder,
        &api::SearchFieldArgs {
            field: Some(default_title),
            weight: Some(&default_title_weight),
        },
    );
    let default_tags = builder.create_string("tags");
    let default_tags_weight = api::OptionalDouble::new(tags_weight);
    let default_tags = api::SearchField::create(
        &mut builder,
        &api::SearchFieldArgs {
            field: Some(default_tags),
            weight: Some(&default_tags_weight),
        },
    );
    let default_search_fields = builder.create_vector(&[default_title, default_tags]);
    let default_search_fields = api::SearchFieldSelection::create(
        &mut builder,
        &api::SearchFieldSelectionArgs {
            fields: Some(default_search_fields),
        },
    );
    let properties = string_properties(
        &mut builder,
        &[("schema.property", "value"), ("schema.second", "preserve")],
    );
    let document_type = builder.create_string("complete_document");
    let schema_inherits = strings(&mut builder, &["base_schema", "common_schema"]);
    let document_inherits = strings(&mut builder, &["base_document", "audited_document"]);
    let definition = api::IndexDefinition::create(
        &mut builder,
        &api::IndexDefinitionArgs {
            name: Some(name),
            fields: Some(fields),
            default_search_fields: Some(default_search_fields),
            properties: Some(properties),
            document_type: Some(document_type),
            schema_inherits: Some(schema_inherits),
            document_inherits: Some(document_inherits),
            structs: Some(structs),
            fieldsets: Some(fieldsets),
            schema_fields: Some(schema_fields),
            imported_fields: Some(imported_fields),
            document_summaries: Some(document_summaries),
            rank_profiles: Some(rank_profiles),
            constants: Some(constants),
            onnx_models: Some(onnx_models),
        },
    );
    builder.finish(definition, None);
    builder.finished_data().to_vec()
}

fn canonical_fixture_ir() -> searchkernel_ir::IndexDefinitionIr {
    canonical_fixture_ir_named("complete_schema", 0.75, 0.25)
}

fn canonical_fixture_ir_named(
    name: &str,
    title_weight: f64,
    tags_weight: f64,
) -> searchkernel_ir::IndexDefinitionIr {
    let bytes = canonical_schema_fixture_named(name, title_weight, tags_weight);
    let root = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(&bytes)
        .expect("canonical fixture should parse");
    searchkernel_ir::index_definition_to_ir(root)
        .expect("canonical fixture should resolve through FlatBuffers")
}

fn described_canonical_fixture() -> (
    searchkernel_ir::IndexDefinitionIr,
    searchkernel_ir::IndexDefinitionIr,
) {
    let canonical = canonical_fixture_ir();
    let index = index_from_canonical_ir(canonical.clone());
    let described = index.describe().expect("describe should succeed");
    let described = definition_to_ir(&described).expect("described schema should resolve");
    (canonical, described)
}

#[test]
fn document_identity_resolution_preserves_public_intent_across_ingress_paths() {
    let cases = [
        (None, None, "searchkernel"),
        (Some("catalog"), None, "catalog"),
        (None, Some("product_document"), "product_document"),
        (
            Some("catalog"),
            Some("product_document"),
            "product_document",
        ),
    ];

    for (name, document_type, expected_identity) in cases {
        let mut definition = IndexDefinition::new().field(Field::text("title").stored(true));
        if let Some(name) = name {
            definition = definition.name(name);
        }
        definition.document_type = document_type.map(str::to_owned);

        let direct = definition_to_ir(&definition).expect("direct Rust schema resolves");
        assert_eq!(direct.document_type(), document_type);
        assert_eq!(direct.resolved_document_type(), expected_identity);

        let bytes = crate::portable_definition::encode_portable_definition(&definition);
        let portable = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(&bytes)
            .expect("portable schema parses");
        let portable =
            searchkernel_ir::index_definition_to_ir(portable).expect("portable schema resolves");
        assert_eq!(portable, direct);
        assert_eq!(portable.document_type(), document_type);
        assert_eq!(portable.resolved_document_type(), expected_identity);
    }
}

pub(super) fn public_described_canonical_fixture(
) -> (searchkernel_ir::IndexDefinitionIr, IndexDefinition) {
    let canonical = canonical_fixture_ir();
    let index = index_from_canonical_ir(canonical.clone());
    let described = index.describe().expect("describe should succeed");
    (canonical, described)
}

#[test]
fn describe_exposes_schema_fields_publicly() {
    let (canonical, described) = public_described_canonical_fixture();
    assert_eq!(
        described.schema_fields().len(),
        canonical.schema_fields().len()
    );
    assert_eq!(
        described.schema_fields()[0].name(),
        canonical.schema_fields()[0].name().as_str()
    );
}

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
fn custom_analyzer_configuration_is_shared_portable_schema_state() {
    use searchkernel_ir::{
        AnalyzerIr, DataTypeIr, FieldCapabilities, FieldIr, FieldName, PrimitiveDataTypeIr,
        SchemaPropertiesIr,
    };

    let mut options = SchemaPropertiesIr::new();
    options.insert(
        "tokenizer".to_owned(),
        searchkernel_ir::RequestValueIr::String("custom".to_owned()),
    );
    let field = FieldIr::new(
        FieldName::parse("title").expect("valid field name"),
        DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
        Some(AnalyzerIr {
            name: "custom".to_owned(),
            version: 2,
            options,
        }),
        FieldCapabilities::new(true, true, false, false, false),
    );
    let canonical = searchkernel_ir::IndexDefinitionIr::new(None, vec![field], Vec::new())
        .expect("rich analyzer is valid portable schema state");

    let index = index_from_canonical_ir(canonical.clone());
    let described = index
        .describe()
        .expect("describe should preserve rich analyzer state");
    let rust = definition_to_ir(&described).expect("Rust ingress should preserve rich analyzer");

    let bytes = searchkernel_ir::index_definition_ir_to_api(&canonical);
    let portable = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(&bytes)
        .expect("portable schema should decode");
    let portable = searchkernel_ir::index_definition_to_ir(portable)
        .expect("portable ingress should preserve rich analyzer");

    assert_eq!(
        rust.fields()[0].analyzer(),
        canonical.fields()[0].analyzer()
    );
    assert_eq!(
        portable.fields()[0].analyzer(),
        canonical.fields()[0].analyzer()
    );
    assert_eq!(rust.fields()[0].analyzer(), portable.fields()[0].analyzer());
}

#[test]
fn described_field_preserves_custom_analyzer_options() {
    let mut options = searchkernel_ir::SchemaPropertiesIr::new();
    options.insert(
        "tokenizer".to_owned(),
        searchkernel_ir::RequestValueIr::String("custom".to_owned()),
    );
    let mut field = searchkernel_ir::FieldIr::new(
        searchkernel_ir::FieldName::parse("title").expect("valid field name"),
        searchkernel_ir::DataTypeIr::Primitive(searchkernel_ir::PrimitiveDataTypeIr::String),
        Some(searchkernel_ir::AnalyzerIr {
            name: "custom".to_owned(),
            version: 2,
            options,
        }),
        searchkernel_ir::FieldCapabilities::new(true, true, false, false, false),
    );
    let mut matching_options = searchkernel_ir::SchemaPropertiesIr::new();
    matching_options.insert(
        "match-key".to_owned(),
        searchkernel_ir::RequestValueIr::String("value".to_owned()),
    );
    field.set_matching_config(Some(searchkernel_ir::MatchingConfigIr {
        mode: searchkernel_ir::MatchingModeIr::Text,
        options: matching_options,
    }));
    let definition = searchkernel_ir::IndexDefinitionIr::new(None, vec![field], Vec::new())
        .expect("valid analyzer definition");

    let index = index_from_canonical_ir(definition);
    let described = index.describe().expect("describe should succeed");
    let analyzer = described.fields()[0]
        .analyzer_config()
        .expect("described custom analyzer should remain publicly visible");
    assert_eq!(analyzer.name, "custom");
    assert_eq!(analyzer.version, 2);
    assert_eq!(
        analyzer.options.get("tokenizer"),
        Some(&crate::schema::SchemaValue::String("custom".to_owned()))
    );
    assert_eq!(
        described.fields()[0].matching(),
        Some(crate::schema::MatchingConfig {
            mode: crate::schema::MatchingMode::Text,
            options: [(
                "match-key".to_owned(),
                crate::schema::SchemaValue::String("value".to_owned()),
            )]
            .into_iter()
            .collect(),
        })
    );

    let public_field = described.fields()[0].clone();
    let reused = IndexDefinition::new().field(public_field);
    let resolved = definition_to_ir(&reused).expect("reused field should resolve");
    let analyzer_ir = searchkernel_ir::AnalyzerIr {
        name: analyzer.name.clone(),
        version: analyzer.version,
        options: analyzer
            .options
            .iter()
            .map(|(key, value)| (key.clone(), value.to_ir()))
            .collect(),
    };
    assert_eq!(resolved.fields()[0].analyzer(), Some(&analyzer_ir));
    assert_eq!(
        resolved.fields()[0].matching(),
        Some(&searchkernel_ir::MatchingConfigIr {
            mode: searchkernel_ir::MatchingModeIr::Text,
            options: [(
                "match-key".to_owned(),
                searchkernel_ir::RequestValueIr::String("value".to_owned())
            )]
            .into_iter()
            .collect(),
        })
    );
}

fn context_for(definition: &IndexDefinition) -> Arc<IndexContext> {
    let (_probe_runtime, probe) =
        runtime_index(definition.clone(), Arc::new(RecordingBackend::new()));
    Arc::clone(&probe.inner.context)
}

fn portable_match_all_request(with_search_fields: bool) -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let match_all = api::MatchAllQuery::create(&mut builder, &api::MatchAllQueryArgs {});
    let source = api::QuerySource::create(
        &mut builder,
        &api::QuerySourceArgs {
            source_type: api::QuerySourceKind::MatchAllQuery,
            source: Some(match_all.as_union_value()),
        },
    );
    let search_fields = if with_search_fields {
        let field = builder.create_string("title");
        let weight = api::OptionalDouble::new(1.0);
        let search_field = api::SearchField::create(
            &mut builder,
            &api::SearchFieldArgs {
                field: Some(field),
                weight: Some(&weight),
            },
        );
        let fields = builder.create_vector(&[search_field]);
        Some(api::SearchFieldSelection::create(
            &mut builder,
            &api::SearchFieldSelectionArgs {
                fields: Some(fields),
            },
        ))
    } else {
        None
    };
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(source),
            search_fields,
            ..Default::default()
        },
    );
    builder.finish(request, None);
    builder.finished_data().to_vec()
}

fn portable_value_string<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    value: &str,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::Value<'a>> {
    let value = builder.create_string(value);
    let string = api::StringValue::create(builder, &api::StringValueArgs { value: Some(value) });
    api::Value::create(
        builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::StringValue,
            kind: Some(string.as_union_value()),
        },
    )
}

fn portable_value_int64<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    value: i64,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::Value<'a>> {
    let integer = api::Int64Value::create(builder, &api::Int64ValueArgs { value });
    api::Value::create(
        builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::Int64Value,
            kind: Some(integer.as_union_value()),
        },
    )
}

fn portable_value_bool<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    value: bool,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::Value<'a>> {
    let boolean = api::BoolValue::create(builder, &api::BoolValueArgs { value });
    api::Value::create(
        builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::BoolValue,
            kind: Some(boolean.as_union_value()),
        },
    )
}

fn portable_field<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    path: &str,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::QueryFieldRef<'a>> {
    let path = builder.create_string(path);
    api::QueryFieldRef::create(
        builder,
        &api::QueryFieldRefArgs {
            path: Some(path),
            element_index: None,
        },
    )
}

fn finish_portable_typed_request<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    expression: searchkernel_ir::__flatbuffers::WIPOffset<api::QueryExpr<'a>>,
) -> Vec<u8> {
    let typed = api::TypedQuery::create(
        builder,
        &api::TypedQueryArgs {
            query: Some(expression),
        },
    );
    let source = api::QuerySource::create(
        builder,
        &api::QuerySourceArgs {
            source_type: api::QuerySourceKind::TypedQuery,
            source: Some(typed.as_union_value()),
        },
    );
    let request = api::SearchRequest::create(
        builder,
        &api::SearchRequestArgs {
            query: Some(source),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    builder.finished_data().to_vec()
}

fn portable_typed_contains_request(field_name: &str) -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let expression = portable_typed_contains_expression(&mut builder, field_name);
    finish_portable_typed_request(&mut builder, expression)
}

fn portable_typed_contains_expression<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    field_name: &str,
) -> searchkernel_ir::__flatbuffers::WIPOffset<api::QueryExpr<'a>> {
    let value = portable_value_string(builder, "value");
    let term = api::TermQuery::create(builder, &api::TermQueryArgs { value: Some(value) });
    let term = api::QueryExpr::create(
        builder,
        &api::QueryExprArgs {
            expr_type: api::QueryExprKind::TermQuery,
            expr: Some(term.as_union_value()),
            annotations: None,
        },
    );
    let field = portable_field(builder, field_name);
    let contains = api::QueryContains::create(
        builder,
        &api::QueryContainsArgs {
            field: Some(field),
            item: Some(term),
        },
    );
    let expression = api::QueryExpr::create(
        builder,
        &api::QueryExprArgs {
            expr_type: api::QueryExprKind::QueryContains,
            expr: Some(contains.as_union_value()),
            annotations: None,
        },
    );
    expression
}

fn portable_weak_and_request(target_hits: u32) -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let child = portable_typed_contains_expression(&mut builder, "title");
    let clauses = builder.create_vector(&[child]);
    let weak_and = api::QueryWeakAnd::create(
        &mut builder,
        &api::QueryWeakAndArgs {
            clauses: Some(clauses),
        },
    );
    let key = builder.create_string("totalTargetHits");
    let annotation_value = portable_value_int64(&mut builder, i64::from(target_hits));
    let annotation = api::ValueEntry::create(
        &mut builder,
        &api::ValueEntryArgs {
            key: Some(key),
            value: Some(annotation_value),
        },
    );
    let annotations = builder.create_vector(&[annotation]);
    let expression = api::QueryExpr::create(
        &mut builder,
        &api::QueryExprArgs {
            expr_type: api::QueryExprKind::QueryWeakAnd,
            expr: Some(weak_and.as_union_value()),
            annotations: Some(annotations),
        },
    );
    finish_portable_typed_request(&mut builder, expression)
}

fn portable_fuzzy_request(max_edit_distance: u32, prefix_length: u32, prefix: bool) -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let value = portable_value_string(&mut builder, "fone");
    let max_key = builder.create_string("maxEditDistance");
    let max_value = portable_value_int64(&mut builder, i64::from(max_edit_distance));
    let max_annotation = api::ValueEntry::create(
        &mut builder,
        &api::ValueEntryArgs {
            key: Some(max_key),
            value: Some(max_value),
        },
    );
    let prefix_length_key = builder.create_string("prefixLength");
    let prefix_length_value = portable_value_int64(&mut builder, i64::from(prefix_length));
    let prefix_length_annotation = api::ValueEntry::create(
        &mut builder,
        &api::ValueEntryArgs {
            key: Some(prefix_length_key),
            value: Some(prefix_length_value),
        },
    );
    let prefix_key = builder.create_string("prefix");
    let prefix_value = portable_value_bool(&mut builder, prefix);
    let prefix_annotation = api::ValueEntry::create(
        &mut builder,
        &api::ValueEntryArgs {
            key: Some(prefix_key),
            value: Some(prefix_value),
        },
    );
    let annotations =
        builder.create_vector(&[max_annotation, prefix_length_annotation, prefix_annotation]);
    let fuzzy = api::QueryFuzzy::create(&mut builder, &api::QueryFuzzyArgs { value: Some(value) });
    let fuzzy = api::QueryExpr::create(
        &mut builder,
        &api::QueryExprArgs {
            expr_type: api::QueryExprKind::QueryFuzzy,
            expr: Some(fuzzy.as_union_value()),
            annotations: Some(annotations),
        },
    );
    let field = portable_field(&mut builder, "title");
    let contains = api::QueryContains::create(
        &mut builder,
        &api::QueryContainsArgs {
            field: Some(field),
            item: Some(fuzzy),
        },
    );
    let expression = api::QueryExpr::create(
        &mut builder,
        &api::QueryExprArgs {
            expr_type: api::QueryExprKind::QueryContains,
            expr: Some(contains.as_union_value()),
            annotations: None,
        },
    );
    finish_portable_typed_request(&mut builder, expression)
}

fn portable_near_request(distance: u32) -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let child = portable_typed_contains_expression(&mut builder, "title");
    let children = builder.create_vector(&[child]);
    let near = api::QueryNear::create(
        &mut builder,
        &api::QueryNearArgs {
            terms: Some(children),
        },
    );
    let key = builder.create_string("distance");
    let value = portable_value_int64(&mut builder, i64::from(distance));
    let annotation = api::ValueEntry::create(
        &mut builder,
        &api::ValueEntryArgs {
            key: Some(key),
            value: Some(value),
        },
    );
    let annotations = builder.create_vector(&[annotation]);
    let near = api::QueryExpr::create(
        &mut builder,
        &api::QueryExprArgs {
            expr_type: api::QueryExprKind::QueryNear,
            expr: Some(near.as_union_value()),
            annotations: Some(annotations),
        },
    );
    let field = portable_field(&mut builder, "title");
    let contains = api::QueryContains::create(
        &mut builder,
        &api::QueryContainsArgs {
            field: Some(field),
            item: Some(near),
        },
    );
    let expression = api::QueryExpr::create(
        &mut builder,
        &api::QueryExprArgs {
            expr_type: api::QueryExprKind::QueryContains,
            expr: Some(contains.as_union_value()),
            annotations: None,
        },
    );
    finish_portable_typed_request(&mut builder, expression)
}

fn portable_text_request(text: Option<&str>) -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let text = text.map(|value| builder.create_string(value));
    let text_query =
        api::SearchKernelTextQuery::create(&mut builder, &api::SearchKernelTextQueryArgs { text });
    let source = api::QuerySource::create(
        &mut builder,
        &api::QuerySourceArgs {
            source_type: api::QuerySourceKind::SearchKernelTextQuery,
            source: Some(text_query.as_union_value()),
        },
    );
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(source),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    builder.finished_data().to_vec()
}

fn portable_text_request_with_empty_controls(
    ranking_profile: Option<&str>,
    language: Option<&str>,
    summary: Option<&str>,
) -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let text = builder.create_string("x");
    let text_query = api::SearchKernelTextQuery::create(
        &mut builder,
        &api::SearchKernelTextQueryArgs { text: Some(text) },
    );
    let query = api::QuerySource::create(
        &mut builder,
        &api::QuerySourceArgs {
            source_type: api::QuerySourceKind::SearchKernelTextQuery,
            source: Some(text_query.as_union_value()),
        },
    );
    let ranking_profile = ranking_profile.map(|value| builder.create_string(value));
    let ranking = ranking_profile.map(|profile| {
        api::RankingRequest::create(
            &mut builder,
            &api::RankingRequestArgs {
                profile: Some(profile),
                ..Default::default()
            },
        )
    });
    let language = language.map(|value| builder.create_string(value));
    let query_model = language.map(|language| {
        api::QueryModel::create(
            &mut builder,
            &api::QueryModelArgs {
                language: Some(language),
                ..Default::default()
            },
        )
    });
    let summary = summary.map(|value| builder.create_string(value));
    let presentation = summary.map(|summary| {
        api::PresentationRequest::create(
            &mut builder,
            &api::PresentationRequestArgs {
                summary: Some(summary),
                ..Default::default()
            },
        )
    });
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(query),
            ranking,
            query_model,
            presentation,
            ..Default::default()
        },
    );
    builder.finish(request, None);
    builder.finished_data().to_vec()
}

fn portable_grouping_request(attribute: &str) -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let attribute = builder.create_string(attribute);
    let attribute = api::GroupingAttributeExpression::create(
        &mut builder,
        &api::GroupingAttributeExpressionArgs {
            field: Some(attribute),
        },
    );
    let expression = api::GroupingExpression::create(
        &mut builder,
        &api::GroupingExpressionArgs {
            expr_type: api::GroupingExpressionKind::GroupingAttributeExpression,
            expr: Some(attribute.as_union_value()),
        },
    );
    let group = api::GroupingGroupOperation::create(
        &mut builder,
        &api::GroupingGroupOperationArgs {
            expression: Some(expression),
        },
    );
    let group = api::GroupingOperation::create(
        &mut builder,
        &api::GroupingOperationArgs {
            operation_type: api::GroupingOperationKind::GroupingGroupOperation,
            operation: Some(group.as_union_value()),
        },
    );
    let each = api::GroupingEachOperation::create(
        &mut builder,
        &api::GroupingEachOperationArgs {
            operations: None,
            label: None,
        },
    );
    let each = api::GroupingOperation::create(
        &mut builder,
        &api::GroupingOperationArgs {
            operation_type: api::GroupingOperationKind::GroupingEachOperation,
            operation: Some(each.as_union_value()),
        },
    );
    let operations = builder.create_vector(&[group, each]);
    let root = api::GroupingAllOperation::create(
        &mut builder,
        &api::GroupingAllOperationArgs {
            operations: Some(operations),
            label: None,
        },
    );
    let structured = api::StructuredGroupingSource::create(
        &mut builder,
        &api::StructuredGroupingSourceArgs { root: Some(root) },
    );
    let grouping = api::GroupingRequest::create(
        &mut builder,
        &api::GroupingRequestArgs {
            source_type: api::GroupingSourceKind::StructuredGroupingSource,
            source: Some(structured.as_union_value()),
            continuations: None,
        },
    );
    let grouping = builder.create_vector(&[grouping]);
    let match_all = api::MatchAllQuery::create(&mut builder, &api::MatchAllQueryArgs {});
    let source = api::QuerySource::create(
        &mut builder,
        &api::QuerySourceArgs {
            source_type: api::QuerySourceKind::MatchAllQuery,
            source: Some(match_all.as_union_value()),
        },
    );
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(source),
            grouping: Some(grouping),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    builder.finished_data().to_vec()
}

fn portable_geo_nan_request() -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let field = portable_field(&mut builder, "title");
    let radius = builder.create_string("10 km");
    let geo = api::QueryGeoLocation::create(
        &mut builder,
        &api::QueryGeoLocationArgs {
            field: Some(field),
            latitude: f64::NAN,
            longitude: 1.0,
            radius: Some(radius),
        },
    );
    let expression = api::QueryExpr::create(
        &mut builder,
        &api::QueryExprArgs {
            expr_type: api::QueryExprKind::QueryGeoLocation,
            expr: Some(geo.as_union_value()),
            annotations: None,
        },
    );
    finish_portable_typed_request(&mut builder, expression)
}

fn expected_get(
    context: &IndexContext,
    id: &str,
    projection: Option<&[String]>,
) -> searchkernel_ir::GetRequestIr {
    searchkernel_ir::resolve_get(
        context,
        searchkernel_ir::GetInput {
            id: id.to_owned(),
            projection: projection.map(ToOwned::to_owned),
        },
    )
    .expect("valid get request IR")
}

fn expected_search(
    context: &IndexContext,
    request: &SearchBuilder,
) -> searchkernel_ir::SearchRequestIr {
    searchkernel_ir::resolve_search(
        context,
        crate::search::to_resolver_input(request).expect("valid search intent"),
    )
    .expect("valid search request IR")
}

fn portable_typed_request(
    query: searchkernel_ir::QueryExprIr,
    inputs: BTreeMap<String, searchkernel_ir::QueryInputIr>,
) -> Vec<u8> {
    let request = searchkernel_ir::SearchRequestIr::new(
        searchkernel_ir::QueryIr::Typed(Box::new(query)),
        Vec::new(),
        None,
        Vec::new(),
        20,
        0,
        searchkernel_ir::ProjectionIr::new(Vec::new()),
        searchkernel_ir::HighlightingIr::Disabled,
        None,
        searchkernel_ir::PrefixPolicy::Disabled,
        searchkernel_ir::TypoPolicy::Disabled,
    )
    .with_inputs(inputs);
    searchkernel_ir::search_request_ir_to_api(&request)
}

fn typed_field(path: &str) -> searchkernel_ir::QueryFieldRefIr {
    searchkernel_ir::QueryFieldRefIr {
        path: searchkernel_ir::FieldName::parse(path.to_owned()).expect("valid test field"),
        element_index: None,
    }
}

fn typed_contains(
    field: &str,
    item: searchkernel_ir::QueryExprKindIr,
) -> searchkernel_ir::QueryExprIr {
    searchkernel_ir::QueryExprIr {
        kind: searchkernel_ir::QueryExprKindIr::Contains {
            field: typed_field(field),
            item: Box::new(searchkernel_ir::QueryExprIr {
                kind: item,
                annotations: BTreeMap::new(),
            }),
        },
        annotations: BTreeMap::new(),
    }
}

fn expected_analyze(
    context: &IndexContext,
    request: &AnalyzeBuilder,
) -> searchkernel_ir::AnalyzeRequestIr {
    searchkernel_ir::resolve_analyze(
        context,
        crate::analyze::to_resolver_input(request).expect("valid analyze intent"),
    )
    .expect("valid analyze request IR")
}

fn scripted_definition() -> IndexDefinition {
    IndexDefinition::new()
        .name("catalog")
        .field(
            Field::text("title")
                .stored(true)
                .searchable(true)
                .analyzer(Analyzer::standard_v1()),
        )
        .field(
            Field::keyword("brand")
                .stored(true)
                .filterable(true)
                .sortable(true)
                .facetable(true),
        )
        .field(
            Field::keyword("category")
                .stored(false)
                .filterable(true)
                .sortable(true)
                .facetable(true),
        )
        .field(
            Field::keyword("region")
                .stored(false)
                .filterable(true)
                .sortable(true)
                .facetable(true),
        )
        .field(
            Field::keyword("channel")
                .stored(false)
                .filterable(true)
                .sortable(true)
                .facetable(true),
        )
}

fn normalized_search(index: &Index) -> SearchBuilder {
    index
        .search("  fox  ")
        .search_fields(["title"])
        .filter(field("brand").eq("acme"))
        .filter(field("category").eq("books"))
        .sort(Sort::asc("brand").missing_last())
        .sort(Sort::desc("category").missing_first())
        .ranking(Ranking::bm25())
        .limit(7)
        .offset(2)
        .projection(["title", "brand"])
        .highlight(["title"])
        .language("EN-us")
}

#[test]
fn inspect_exposes_normalized_filter_defaults() {
    let definition = IndexDefinition::new()
        .field(Field::text("title"))
        .field(Field::float64("price"));
    let (_runtime, index) = runtime_index(definition, Arc::new(RecordingBackend::new()));

    let inspected = index
        .browse()
        .filter(field("price").range(crate::Range::new().lower(100.0)))
        .inspect()
        .unwrap();
    let Some(crate::Filter(crate::filter::FilterNode::Range { bounds, .. })) = inspected.filter
    else {
        panic!("expected normalized range filter");
    };
    assert_eq!(bounds.include_lower, Some(true));
    assert_eq!(bounds.include_upper, Some(true));
}

#[test]
fn inspect_exposes_canonical_filter_structure() {
    let definition = scripted_definition();
    let (_runtime, index) = runtime_index(definition, Arc::new(RecordingBackend::new()));

    let inspected = index
        .browse()
        .filter(Filter::all([
            field("brand").eq("acme"),
            Filter::all([field("category").eq("books"), field("region").eq("eu")]),
        ]))
        .inspect()
        .unwrap();

    assert_eq!(
        inspected.filter,
        Some(Filter::all([
            field("brand").eq("acme"),
            field("category").eq("books"),
            field("region").eq("eu"),
        ]))
    );
}

#[test]
fn scripted_backend_get_converts_documents_and_projections() {
    let definition = scripted_definition();
    let context = context_for(&definition);
    let projection = vec!["title".to_owned()];
    let document = searchkernel_ir::DocumentResult {
        id: "doc-1".to_owned(),
        fields: BTreeMap::from([
            (
                "title".to_owned(),
                searchkernel_ir::Value::String("fox".to_owned()),
            ),
            (
                "brand".to_owned(),
                searchkernel_ir::Value::String("acme".to_owned()),
            ),
        ]),
    };
    let backend = Arc::new(ScriptedBackend::new([
        ExpectedCall::success(
            OperationIr::Get(expected_get(&context, "doc-1", None)),
            ScriptedResult::Get(searchkernel_ir::GetResult {
                document: Some(document),
            }),
        ),
        ExpectedCall::success(
            OperationIr::Get(expected_get(&context, "doc-1", Some(&projection))),
            ScriptedResult::Get(searchkernel_ir::GetResult {
                document: Some(searchkernel_ir::DocumentResult {
                    id: "doc-1".to_owned(),
                    fields: BTreeMap::from([(
                        "title".to_owned(),
                        searchkernel_ir::Value::String("fox".to_owned()),
                    )]),
                }),
            }),
        ),
        ExpectedCall::success(
            OperationIr::Get(expected_get(&context, "missing", None)),
            ScriptedResult::Get(searchkernel_ir::GetResult { document: None }),
        ),
    ]));
    let (_runtime, index) = runtime_index(definition, backend.clone());

    let full: Option<serde_json::Value> = index.get("doc-1").unwrap();
    assert_eq!(full, Some(json!({"title": "fox", "brand": "acme"})));
    let projected: Option<serde_json::Value> =
        index.get_with_projection("doc-1", ["title"]).unwrap();
    assert_eq!(projected, Some(json!({"title": "fox"})));
    let missing: Option<serde_json::Value> = index.get("missing").unwrap();
    assert_eq!(missing, None);
    backend.finish().unwrap();
}

#[test]
fn scripted_backend_executes_analyze_and_converts_tokens() {
    let definition = scripted_definition();
    let context = context_for(&definition);
    let (_probe_runtime, probe) =
        runtime_index(definition.clone(), Arc::new(RecordingBackend::new()));
    let probe_request = probe.analyze("Running").field("title").language("EN-us");
    let expected = expected_analyze(&context, &probe_request);
    let backend = Arc::new(ScriptedBackend::new([ExpectedCall::success(
        OperationIr::Analyze(expected),
        ScriptedResult::Analyze(searchkernel_ir::AnalyzeResult {
            tokens: vec![searchkernel_ir::AnalyzedToken {
                surface: "Running".to_owned(),
                term: "run".to_owned(),
                position: 1,
                start_byte: 0,
                end_byte: 7,
            }],
        }),
    )]));
    let (_runtime, index) = runtime_index(definition, backend.clone());

    let result = index
        .analyze("Running")
        .field("title")
        .language("EN-us")
        .execute()
        .unwrap();
    assert_eq!(result.tokens[0].term, "run");
    assert_eq!(result.tokens[0].start_byte, 0);
    assert_eq!(result.tokens[0].end_byte, 7);
    backend.finish().unwrap();
}

#[test]
fn browse_executes_match_all_and_describe_returns_schema() {
    let definition = scripted_definition();
    let context = context_for(&definition);
    let (_probe_runtime, probe) =
        runtime_index(definition.clone(), Arc::new(RecordingBackend::new()));
    let expected = expected_search(&context, &probe.browse());
    let backend = Arc::new(ScriptedBackend::new([ExpectedCall::success(
        OperationIr::Search(expected),
        ScriptedResult::Search(Box::new(searchkernel_ir::SearchResult::empty(
            searchkernel_ir::TotalHits {
                value: 0,
                relation: searchkernel_ir::TotalHitsRelation::Exact,
            },
        ))),
    )]));
    let (_runtime, index) = runtime_index(definition, backend.clone());

    let described = index.describe().unwrap();
    assert_eq!(described.index_name(), Some("catalog"));
    assert_eq!(described.fields().len(), 5);
    assert_eq!(described.fields()[0].name(), "title");
    assert_eq!(described.fields()[1].name(), "brand");

    let result: crate::SearchResult<serde_json::Value> = index.browse().execute().unwrap();
    assert!(result.hits.is_empty());
    assert_eq!(result.total_hits.value, 0);
    backend.finish().unwrap();
}

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
fn canonical_index_definition_round_trips_through_flatbuffers_without_loss() {
    let canonical = canonical_fixture_ir();
    let bytes = searchkernel_ir::index_definition_ir_to_api(&canonical);
    let root = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(&bytes)
        .expect("encoded canonical fixture should parse");
    let round_tripped = searchkernel_ir::index_definition_to_ir(root)
        .expect("encoded canonical fixture should resolve");
    assert_eq!(canonical, round_tripped);
}

#[test]
fn rank_profile_input_default_converges_across_rust_and_flatbuffers_ingress() {
    let canonical = canonical_fixture_ir();
    let expected = canonical.rank_profiles()[0].inputs[1].default_value.clone();

    let rust_definition = definition_from_ir(&canonical);
    let rust_ir = definition_to_ir(&rust_definition)
        .expect("Rust schema ingress should preserve canonical rank-profile defaults");

    let bytes = searchkernel_ir::index_definition_ir_to_api(&canonical);
    let root = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(&bytes)
        .expect("portable schema should decode");
    let portable_ir = searchkernel_ir::index_definition_to_ir(root)
        .expect("portable schema ingress should preserve canonical rank-profile defaults");

    assert_eq!(rust_ir.rank_profiles()[0].inputs[1].default_value, expected);
    assert_eq!(
        portable_ir.rank_profiles()[0].inputs[1].default_value,
        rust_ir.rank_profiles()[0].inputs[1].default_value
    );
}

#[test]
fn canonical_fixture_covers_the_complete_schema_surface() {
    let definition = canonical_fixture_ir();
    assert_eq!(
        definition.name().map(|value| value.as_str()),
        Some("complete_schema")
    );
    assert_eq!(definition.fields().len(), 17);
    assert_eq!(definition.default_search_fields().len(), 2);
    assert_eq!(
        definition.default_search_fields()[0].field().as_str(),
        "title"
    );
    assert_eq!(definition.default_search_fields()[0].weight().get(), 0.75);
    assert_eq!(
        definition.default_search_fields()[1].field().as_str(),
        "tags"
    );
    assert_eq!(definition.default_search_fields()[1].weight().get(), 0.25);
    assert_eq!(definition.properties().len(), 2);
    assert_eq!(definition.document_type(), Some("complete_document"));
    assert_eq!(
        definition.schema_inherits(),
        ["base_schema", "common_schema"]
    );
    assert_eq!(
        definition.document_inherits(),
        ["base_document", "audited_document"]
    );
    assert_eq!(definition.structs().len(), 1);
    assert_eq!(definition.structs()[0].name, "address");
    assert_eq!(definition.structs()[0].fields.len(), 2);
    assert_eq!(definition.structs()[0].properties.len(), 1);
    assert_eq!(definition.fieldsets().len(), 1);
    assert_eq!(definition.fieldsets()[0].name, "all_text");
    assert_eq!(definition.fieldsets()[0].fields.len(), 2);
    assert_eq!(
        definition.fieldsets()[0].query_commands,
        ["stem", "normalize"]
    );
    assert_eq!(definition.schema_fields().len(), 1);
    assert_eq!(definition.schema_fields()[0].name().as_str(), "derived");
    assert_eq!(definition.imported_fields().len(), 1);
    assert_eq!(
        definition.imported_fields()[0].alias.as_str(),
        "parent_name"
    );
    assert_eq!(definition.document_summaries().len(), 1);
    assert_eq!(definition.document_summaries()[0].fields.len(), 1);
    assert_eq!(definition.document_summaries()[0].from_disk, Some(false));
    assert_eq!(
        definition.document_summaries()[0].omit_summary_features,
        Some(true)
    );
    assert_eq!(definition.rank_profiles().len(), 3);
    assert_eq!(definition.constants().len(), 2);
    assert_eq!(definition.onnx_models().len(), 1);

    let data_types = definition
        .fields()
        .iter()
        .map(|field| (field.name().as_str(), field.data_type()))
        .collect::<BTreeMap<_, _>>();
    assert!(matches!(
        data_types["title"],
        searchkernel_ir::DataTypeIr::Primitive(searchkernel_ir::PrimitiveDataTypeIr::String)
    ));
    assert!(matches!(
        data_types["flag"],
        searchkernel_ir::DataTypeIr::Primitive(searchkernel_ir::PrimitiveDataTypeIr::Bool)
    ));
    assert!(matches!(
        data_types["byte_value"],
        searchkernel_ir::DataTypeIr::Primitive(searchkernel_ir::PrimitiveDataTypeIr::Byte)
    ));
    assert!(matches!(
        data_types["count32"],
        searchkernel_ir::DataTypeIr::Primitive(searchkernel_ir::PrimitiveDataTypeIr::Int32)
    ));
    assert!(matches!(
        data_types["count64"],
        searchkernel_ir::DataTypeIr::Primitive(searchkernel_ir::PrimitiveDataTypeIr::Int64)
    ));
    assert!(matches!(
        data_types["ratio32"],
        searchkernel_ir::DataTypeIr::Primitive(searchkernel_ir::PrimitiveDataTypeIr::Float32)
    ));
    assert!(matches!(
        data_types["ratio64"],
        searchkernel_ir::DataTypeIr::Primitive(searchkernel_ir::PrimitiveDataTypeIr::Float64)
    ));
    assert!(matches!(
        data_types["payload"],
        searchkernel_ir::DataTypeIr::Primitive(searchkernel_ir::PrimitiveDataTypeIr::Raw)
    ));
    assert!(matches!(
        data_types["embedding"],
        searchkernel_ir::DataTypeIr::Tensor { .. }
    ));
    assert!(matches!(
        data_types["tags"],
        searchkernel_ir::DataTypeIr::Array(_)
    ));
    assert!(matches!(
        data_types["scores"],
        searchkernel_ir::DataTypeIr::Map { .. }
    ));
    assert!(matches!(
        data_types["labels"],
        searchkernel_ir::DataTypeIr::WeightedSet(_)
    ));
    assert!(matches!(
        data_types["inline"],
        searchkernel_ir::DataTypeIr::Struct { .. }
    ));
    assert!(matches!(
        data_types["address"],
        searchkernel_ir::DataTypeIr::StructReference { .. }
    ));
    assert!(matches!(
        data_types["predicate"],
        searchkernel_ir::DataTypeIr::Predicate
    ));
    assert!(matches!(
        data_types["location"],
        searchkernel_ir::DataTypeIr::Position
    ));
    assert!(matches!(
        data_types["parent"],
        searchkernel_ir::DataTypeIr::Reference { .. }
    ));

    let title = &definition.fields()[0];
    assert!(title.analyzer().is_some());
    assert!(title.matching().is_some());
    assert!(title.index().is_some());
    assert!(title.attribute().is_some());
    assert!(title.summary().is_some());
    assert_eq!(title.indexing_script(), None);
    assert_eq!(title.properties().len(), 2);
    assert_eq!(title.struct_fields().len(), 1);
    assert!(title.capabilities().stored());
    assert!(title.capabilities().searchable());
    assert!(!title.capabilities().filterable());
    assert!(!title.capabilities().sortable());
    assert!(!title.capabilities().groupable());
    assert_eq!(
        definition.schema_fields()[0].indexing_script(),
        Some("input title | attribute(derived)")
    );

    let complete = &definition.rank_profiles()[0];
    assert_eq!(complete.name, "complete_profile");
    assert_eq!(complete.inherits, ["base_profile"]);
    assert_eq!(complete.inputs.len(), 2);
    assert_eq!(complete.functions.len(), 3);
    assert!(complete.first_phase.is_some());
    assert!(complete.second_phase.is_some());
    assert!(complete.global_phase.is_some());
    assert_eq!(complete.rank_features.len(), 2);
    assert_eq!(complete.match_features.len(), 2);
    assert_eq!(complete.summary_features.len(), 1);
    assert_eq!(complete.sort_features.len(), 1);
    assert_eq!(complete.constants.len(), 1);
    assert_eq!(complete.onnx_models.len(), 1);
    assert_eq!(complete.rank_properties.len(), 1);
    assert_eq!(complete.strict, Some(true));
    assert_eq!(complete.properties.len(), 1);
    assert_eq!(
        complete.inputs[0].default_value,
        searchkernel_ir::RequestValueIr::Float64(1.5)
    );
    assert!(matches!(
        complete.inputs[1].default_value,
        searchkernel_ir::RequestValueIr::Tensor(_)
    ));
    assert_eq!(
        complete.inputs[1].default_value,
        searchkernel_ir::RequestValueIr::Tensor(searchkernel_ir::RequestTensorIr {
            type_spec: "tensor<float>(x[3])".to_owned(),
            payload: searchkernel_ir::RequestTensorPayloadIr::Dense(vec![1.0, 2.0, 3.0]),
        })
    );
    assert_eq!(complete.functions[0].inline, Some(true));
    assert_eq!(complete.functions[1].inline, Some(false));
    assert_eq!(complete.functions[2].inline, None);
    assert_eq!(
        complete.first_phase.as_ref().unwrap().keep_rank_count,
        Some(5)
    );
    assert_eq!(
        complete.second_phase.as_ref().unwrap().rerank_count,
        Some(10)
    );
    assert_eq!(
        complete
            .global_phase
            .as_ref()
            .unwrap()
            .rank_score_drop_limit,
        Some(0.5)
    );
    assert_eq!(
        definition.rank_profiles()[1]
            .first_phase
            .as_ref()
            .unwrap()
            .total_keep_rank_count,
        Some(6)
    );
    assert_eq!(
        definition.rank_profiles()[1]
            .second_phase
            .as_ref()
            .unwrap()
            .total_rerank_count,
        Some(11)
    );
    assert!(definition.rank_profiles()[2].first_phase.is_none());
    assert!(definition.rank_profiles()[2].second_phase.is_none());
    assert!(definition.rank_profiles()[2].global_phase.is_none());
}

#[cfg(feature = "native")]
#[test]
fn portable_definition_encoding_uses_public_state_without_a_canonical_sidecar() {
    let (_canonical, mut described) = public_described_canonical_fixture();
    described.default_search_fields = Some(vec![SearchField::new("title").weight(0.125)]);
    let described_bytes = crate::portable_definition::encode_portable_definition(&described);
    let root = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(&described_bytes)
        .expect("portable definition should parse");
    let encoded =
        searchkernel_ir::index_definition_to_ir(root).expect("portable definition should resolve");

    assert_eq!(encoded.default_search_fields()[0].weight().get(), 0.125);
}

#[test]
fn portable_definition_round_trips_all_public_schema_state() {
    let (canonical, described) = public_described_canonical_fixture();
    let bytes = crate::portable_definition::encode_portable_definition(&described);
    let root = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(&bytes)
        .expect("portable definition should parse");
    let encoded =
        searchkernel_ir::index_definition_to_ir(root).expect("portable definition should resolve");

    assert_eq!(encoded, canonical);
}

#[test]
fn portable_definition_preserves_explicit_vs_omitted_capability_presence() {
    let definition = IndexDefinition::new()
        .field(
            Field::text("explicit_defaults")
                .stored(true)
                .searchable(true),
        )
        .field(Field::text("omitted_defaults"));
    let bytes = crate::portable_definition::encode_portable_definition(&definition);
    let portable = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(&bytes)
        .expect("portable definition should parse");
    let fields = portable
        .fields()
        .expect("portable definition should contain fields");
    let explicit = fields
        .iter()
        .find(|field| field.name() == "explicit_defaults")
        .expect("explicit field should be encoded");
    let omitted = fields
        .iter()
        .find(|field| field.name() == "omitted_defaults")
        .expect("omitted field should be encoded");

    assert_eq!(explicit.stored().map(|value| value.value()), Some(true));
    assert_eq!(explicit.searchable().map(|value| value.value()), Some(true));
    assert!(omitted.stored().is_none());
    assert!(omitted.searchable().is_none());

    // Canonical resolution deliberately erases this presence distinction:
    // both fields resolve to the same default-true stored/searchable behavior.
    let resolved = searchkernel_ir::index_definition_to_ir(portable)
        .expect("portable definition should resolve");
    for name in ["explicit_defaults", "omitted_defaults"] {
        let field = resolved
            .fields()
            .iter()
            .find(|field| field.name().as_str() == name)
            .expect("resolved field should exist");
        assert!(field.capabilities().stored());
        assert!(field.capabilities().searchable());
    }
}

#[cfg(feature = "native")]
#[test]
fn create_persists_rich_schema_provenance_and_reopens_consistently() {
    let root = std::env::temp_dir().join(format!(
        "searchkernel-schema-provenance-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after unix epoch")
            .as_nanos()
    ));
    let definition = IndexDefinition::new()
        .name("rich_native_schema")
        .field(
            Field::text("title")
                .stored(true)
                .searchable(true)
                .analyzer(Analyzer::standard_v1()),
        )
        .field(
            Field::keyword("brand")
                .stored(true)
                .filterable(true)
                .sortable(true)
                .facetable(true),
        )
        .field(Field::int64("price").stored(true).filterable(true))
        .field(Field::tensor("embedding", "tensor<float>(x[3])"))
        .default_search_fields([SearchField::new("title").weight(0.75)])
        .rank_profile(
            RankProfile::new("product_search")
                .first_phase("bm25(title)")
                .match_feature("bm25(title)"),
        );
    let expected_resolved =
        definition_to_ir(&definition).expect("public definition should resolve before create");
    let expected_portable = crate::portable_definition::encode_portable_definition(&definition);

    let index = Index::create(&root, definition).expect("create native index");
    index.close().expect("close created index");

    let portable_bytes = std::fs::read(root.join("manifest/portable-index-definition.bfbs"))
        .expect("read persisted portable definition");
    let portable = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(&portable_bytes)
        .expect("persisted portable definition should parse");
    assert_eq!(portable_bytes, expected_portable);
    let portable_ir = searchkernel_ir::index_definition_to_ir(portable)
        .expect("persisted portable definition should resolve");
    assert_eq!(portable_ir, expected_resolved);
    assert_eq!(portable_ir.schema_fields().len(), 0);
    assert_eq!(portable_ir.rank_profiles().len(), 1);
    assert_eq!(portable_ir.properties().len(), 0);
    assert!(portable_ir.fields().iter().any(|field| {
        field.name().as_str() == "title"
            && field.analyzer().is_some()
            && field.capabilities().stored()
            && field.capabilities().searchable()
    }));

    let canonical_bytes = std::fs::read(root.join("manifest/index-definition.bfbs"))
        .expect("read persisted canonical definition");
    let canonical = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(&canonical_bytes)
        .expect("persisted canonical definition should parse");
    let persisted_resolved = searchkernel_ir::index_definition_to_ir(canonical)
        .expect("persisted canonical definition should resolve");
    assert_eq!(persisted_resolved, expected_resolved);

    let reopened = Index::open(&root).expect("reopen persisted index");
    let described = reopened.describe().expect("describe reopened index");
    let reopened_resolved =
        definition_to_ir(&described).expect("reopened public definition should resolve");
    assert_eq!(reopened_resolved, expected_resolved);
    reopened.close().expect("close reopened index");
    std::fs::remove_dir_all(root).expect("remove provenance test index");
}

#[cfg(feature = "native")]
#[test]
fn unnamed_index_keeps_resolved_document_identity_after_reopen() {
    let root = std::env::temp_dir().join(format!(
        "searchkernel-unnamed-document-identity-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after unix epoch")
            .as_nanos()
    ));
    let definition = IndexDefinition::new().field(Field::text("title").stored(true));
    let expected = definition_to_ir(&definition).expect("unnamed schema resolves");
    assert_eq!(expected.document_type(), None);
    assert_eq!(expected.resolved_document_type(), "searchkernel");

    let index = Index::create(&root, definition).expect("create unnamed index");
    let described = index.describe().expect("describe created index");
    let described = definition_to_ir(&described).expect("created schema resolves");
    assert_eq!(described, expected);
    index.close().expect("close unnamed index");

    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("manifest/index.json")).expect("read index manifest"),
    )
    .expect("parse index manifest");
    assert_eq!(manifest["resolved_document_type"], json!("searchkernel"));

    let reopened = Index::open(&root).expect("reopen unnamed index");
    let described = reopened.describe().expect("describe reopened index");
    let described = definition_to_ir(&described).expect("reopened schema resolves");
    assert_eq!(described, expected);
    assert_eq!(described.document_type(), None);
    assert_eq!(described.resolved_document_type(), "searchkernel");
    reopened.close().expect("close reopened unnamed index");
    std::fs::remove_dir_all(root).expect("remove unnamed index");
}

#[cfg(feature = "native")]
#[test]
fn unnamed_index_rejects_persisted_resolved_document_identity_mismatch() {
    let root = std::env::temp_dir().join(format!(
        "searchkernel-unnamed-document-identity-mismatch-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after unix epoch")
            .as_nanos()
    ));
    let index = Index::create(
        &root,
        IndexDefinition::new().field(Field::text("title").stored(true)),
    )
    .expect("create unnamed index");
    index.close().expect("close unnamed index");

    let manifest_path = root.join("manifest/index.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).expect("read index manifest"))
            .expect("parse index manifest");
    manifest["resolved_document_type"] = json!("different_document");
    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("encode index manifest"),
    )
    .expect("rewrite index manifest");

    let error = match Index::open(&root) {
        Ok(index) => {
            index.close().expect("close unexpectedly reopened index");
            panic!("reopen accepted a mismatched persisted document identity")
        }
        Err(error) => error,
    };
    assert_eq!(error.code(), crate::ErrorCode::IncompatibleIndex);
    assert!(
        error.to_string().contains("resolved document identity"),
        "identity mismatch must fail in the persistence compatibility domain: {error}"
    );
    std::fs::remove_dir_all(root).expect("remove mismatched index");
}

#[cfg(feature = "native")]
#[test]
fn pre_identity_v4_manifest_recovers_document_identity_from_compiled_config() {
    let root = std::env::temp_dir().join(format!(
        "searchkernel-v4-document-identity-compat-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after unix epoch")
            .as_nanos()
    ));
    let index = Index::create(
        &root,
        IndexDefinition::new().field(Field::text("title").stored(true)),
    )
    .expect("create unnamed index");
    index.close().expect("close unnamed index");

    let manifest_path = root.join("manifest/index.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).expect("read index manifest"))
            .expect("parse index manifest");
    manifest
        .as_object_mut()
        .expect("manifest object")
        .remove("resolved_document_type");

    let config = root.join("config");
    let schema_path = config.join("searchkernel-schema.json");
    let mut schema: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&schema_path).expect("read schema manifest"))
            .expect("parse schema manifest");
    schema["document_type"] = json!("different_document");
    std::fs::write(
        &schema_path,
        serde_json::to_vec(&schema).expect("encode schema manifest"),
    )
    .expect("rewrite schema manifest");
    manifest["resolved_engine_config_identity"] =
        config_identity(&config).expect("recompute config identity");
    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("encode index manifest"),
    )
    .expect("rewrite index manifest");

    let manifest = IndexManifest::read(&root).expect("read pre-identity v4 manifest");
    let recovered = manifest
        .resolved_context_with_config(&config)
        .expect("pre-identity v4 manifest must recover the compiled document identity");
    assert_eq!(
        recovered.definition().resolved_document_type(),
        "different_document"
    );
    assert_eq!(recovered.definition().document_type(), None);
    std::fs::remove_dir_all(root).expect("remove compatibility index");
}

#[test]
fn describe_preserves_field_properties() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(
        canonical.fields()[0].properties(),
        described.fields()[0].properties()
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
fn describe_preserves_complete_rank_phase_state() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(
        canonical.rank_profiles()[0].first_phase,
        described.rank_profiles()[0].first_phase
    );
    assert_eq!(
        canonical.rank_profiles()[0].second_phase,
        described.rank_profiles()[0].second_phase
    );
    assert_eq!(
        canonical.rank_profiles()[0].global_phase,
        described.rank_profiles()[0].global_phase
    );
}

#[test]
fn describe_preserves_absent_rank_phases() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(
        canonical.rank_profiles()[2].first_phase,
        described.rank_profiles()[2].first_phase
    );
    assert_eq!(
        canonical.rank_profiles()[2].second_phase,
        described.rank_profiles()[2].second_phase
    );
    assert_eq!(
        canonical.rank_profiles()[2].global_phase,
        described.rank_profiles()[2].global_phase
    );
}

#[test]
fn describe_preserves_rank_feature_lists() {
    let (canonical, described) = described_canonical_fixture();
    let expected = (
        &canonical.rank_profiles()[0].rank_features,
        &canonical.rank_profiles()[0].match_features,
        &canonical.rank_profiles()[0].summary_features,
        &canonical.rank_profiles()[0].sort_features,
    );
    let actual = (
        &described.rank_profiles()[0].rank_features,
        &described.rank_profiles()[0].match_features,
        &described.rank_profiles()[0].summary_features,
        &described.rank_profiles()[0].sort_features,
    );
    assert_eq!(expected, actual);
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
fn describe_preserves_rank_properties_strict_and_generic_properties() {
    let (canonical, described) = described_canonical_fixture();
    let expected = (
        &canonical.rank_profiles()[0].rank_properties,
        canonical.rank_profiles()[0].strict,
        &canonical.rank_profiles()[0].properties,
    );
    let actual = (
        &described.rank_profiles()[0].rank_properties,
        described.rank_profiles()[0].strict,
        &described.rank_profiles()[0].properties,
    );
    assert_eq!(expected, actual);
}

#[test]
fn describe_preserves_all_document_data_types() {
    let (canonical, described) = described_canonical_fixture();
    let expected = canonical
        .fields()
        .iter()
        .map(|field| field.data_type())
        .collect::<Vec<_>>();
    let actual = described
        .fields()
        .iter()
        .map(|field| field.data_type())
        .collect::<Vec<_>>();
    assert_eq!(expected, actual);
}

#[test]
fn describe_exposes_canonical_field_data_types_publicly() {
    let (canonical, described) = public_described_canonical_fixture();
    for name in [
        "byte_value",
        "count32",
        "ratio32",
        "payload",
        "scores",
        "parent",
    ] {
        let expected = canonical
            .fields()
            .iter()
            .find(|field| field.name().as_str() == name)
            .expect("fixture field should exist");
        let actual = described
            .fields()
            .iter()
            .find(|field| field.name() == name)
            .expect("described field should exist");
        assert_eq!(
            actual.data_type().to_ir(),
            *expected.data_type(),
            "field {name}"
        );
    }
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
fn described_field_exposes_exact_canonical_capabilities() {
    let (canonical, described) = public_described_canonical_fixture();
    let expected = canonical
        .fields()
        .iter()
        .find(|field| field.name().as_str() == "location")
        .expect("canonical location field should exist")
        .capabilities();
    let actual = described
        .fields()
        .iter()
        .find(|field| field.name() == "location")
        .expect("described location field should exist")
        .capabilities()
        .expect("described fields should expose resolved capabilities");

    assert_eq!(actual.stored(), expected.stored());
    assert_eq!(actual.searchable(), expected.searchable());
    assert_eq!(actual.filterable(), expected.filterable());
    assert_eq!(actual.sortable(), expected.sortable());
    assert_eq!(actual.groupable(), expected.groupable());
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
fn describe_preserves_complete_field_configuration() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(canonical.fields()[0], described.fields()[0]);
}

#[test]
fn describe_preserves_struct_field_configuration() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(
        canonical.fields()[0].struct_fields(),
        described.fields()[0].struct_fields()
    );
}

#[test]
fn describe_preserves_default_search_fields_and_weights() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(
        canonical.default_search_fields(),
        described.default_search_fields()
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
fn described_field_exposes_index_and_attribute_configuration_publicly() {
    let (canonical, described) = public_described_canonical_fixture();
    let described_field = described
        .fields()
        .iter()
        .find(|field| field.name() == "embedding")
        .expect("embedding field should be described");
    let canonical_field = canonical
        .fields()
        .iter()
        .find(|field| field.name().as_str() == "embedding")
        .expect("canonical embedding field should exist");

    assert_eq!(
        described_field.index_config.as_ref(),
        canonical_field.index()
    );
    assert_eq!(
        described_field.attribute_config.as_ref(),
        canonical_field.attribute()
    );
}

#[test]
fn tensor_builder_populates_public_index_and_attribute_configuration() {
    let field = Field::tensor("embedding", "tensor<float>(x[3])")
        .distance_metric(crate::DistanceMetric::Angular)
        .hnsw(crate::HnswIndexConfig::new().max_links_per_node(8));

    assert_eq!(
        field
            .attribute_config
            .as_ref()
            .map(|config| config.distance_metric),
        Some(searchkernel_ir::DistanceMetricIr::Angular)
    );
    let hnsw = field
        .index_config
        .as_ref()
        .and_then(|config| config.hnsw.as_ref())
        .expect("builder HNSW config should be public");
    assert_eq!(hnsw.max_links_per_node, Some(8));
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
fn described_public_index_definition_exposes_document_summaries() {
    let (canonical, described) = public_described_canonical_fixture();
    let actual = described
        .document_summaries()
        .iter()
        .map(crate::schema::DocumentSummary::to_ir)
        .collect::<Vec<_>>();
    assert_eq!(actual, canonical.document_summaries());
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
fn described_field_exposes_summary_script_and_properties() {
    let (canonical, described) = public_described_canonical_fixture();
    let expected = &canonical.fields()[0];
    let actual = &described.fields()[0];
    assert_eq!(actual.summary_config.as_ref(), expected.summary());
    assert_eq!(
        actual.indexing_script.as_deref(),
        expected.indexing_script()
    );
    assert_eq!(&actual.properties, expected.properties());
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
fn describe_preserves_schema_only_fields() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(canonical.schema_fields(), described.schema_fields());
}

#[test]
fn describe_preserves_imported_fields() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(canonical.imported_fields(), described.imported_fields());
}

#[test]
fn describe_preserves_document_summaries() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(
        canonical.document_summaries(),
        described.document_summaries()
    );
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

#[test]
fn describe_round_trips_complete_index_definition_without_semantic_loss() {
    let (canonical, described) = described_canonical_fixture();
    assert_eq!(canonical, described);
}

#[test]
fn public_rust_browse_and_portable_match_all_share_source_semantics() {
    let definition = scripted_definition();
    let context = context_for(&definition);

    let recording = Arc::new(RecordingBackend::new());
    let (_runtime, index) = runtime_index(definition.clone(), recording.clone());
    assert!(index.browse().execute::<serde_json::Value>().is_err());
    let OperationIr::Search(rust_request) = recording.last_call().unwrap() else {
        panic!("browse should reach the recording backend");
    };
    let bytes = portable_match_all_request(false);
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let portable_request = context.search_to_ir(request).unwrap();
    assert_eq!(rust_request, portable_request);

    let recording = Arc::new(RecordingBackend::new());
    let (_runtime, index) = runtime_index(definition, recording.clone());
    let rust_error = index
        .browse()
        .search_fields(["title"])
        .execute::<serde_json::Value>()
        .unwrap_err();
    let bytes = portable_match_all_request(true);
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let portable_error = context.search_to_ir(request).unwrap_err();
    assert_eq!(rust_error.code(), crate::ErrorCode::InvalidArgument);
    assert_eq!(
        portable_error.code,
        searchkernel_ir::ErrorCode::InvalidArgument
    );
    assert_eq!(rust_error.details()[0].code, portable_error.details[0].code);
    assert_eq!(rust_error.details()[0].path, portable_error.details[0].path);
}

#[test]
fn public_search_rejects_blank_text_instead_of_becoming_browse() {
    let recording = Arc::new(RecordingBackend::new());
    let (_runtime, index) = runtime_index(scripted_definition(), recording.clone());
    let error = index
        .search(" \t\n")
        .execute::<serde_json::Value>()
        .unwrap_err();
    assert_eq!(error.code(), crate::ErrorCode::InvalidArgument);
    assert!(recording.calls().is_empty());
}

#[test]
fn public_rust_typed_query_errors_match_independent_portable_requests() {
    let definition = scripted_definition();
    let context = context_for(&definition);

    let cases = vec![
        (
            "weak_and_zero",
            Query::weak_and([Query::term("title", "value")], 0),
            portable_weak_and_request(0),
        ),
        (
            "geo_nan",
            Query::GeoLocation {
                field: "title".to_owned(),
                latitude: f64::NAN,
                longitude: 1.0,
                radius: "10 km".to_owned(),
            },
            portable_geo_nan_request(),
        ),
        (
            "dotted_field",
            Query::term("people.name", "value"),
            portable_typed_contains_request("people.name"),
        ),
    ];

    for (name, query, bytes) in cases {
        let recording = Arc::new(RecordingBackend::new());
        let (_runtime, index) = runtime_index(definition.clone(), recording);
        let builder = index.search("typed").typed_query(query);
        let rust_error = builder.execute::<serde_json::Value>().unwrap_err();
        let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
        let portable_error = context.search_to_ir(request).unwrap_err();
        assert_eq!(
            rust_error.code(),
            crate::ErrorCode::InvalidArgument,
            "{name}"
        );
        assert_eq!(
            portable_error.code,
            searchkernel_ir::ErrorCode::InvalidArgument,
            "{name}"
        );
        assert_eq!(
            rust_error.details()[0].code,
            portable_error.details[0].code,
            "{name}"
        );
        assert_eq!(
            rust_error.details()[0].path,
            portable_error.details[0].path,
            "{name}"
        );
    }
}

#[test]
fn public_and_portable_advanced_query_errors_converge_in_the_ir() {
    let definition = IndexDefinition::new()
        .field(Field::text("title").searchable(true))
        .field(Field::tensor("embedding", "tensor<float>(x[2])").stored(true))
        .field(Field::int64("price").filterable(true));
    let context = context_for(&definition);
    let tensor_input = || {
        searchkernel_ir::QueryInputIr::Literal(searchkernel_ir::RequestValueIr::Tensor(
            searchkernel_ir::RequestTensorIr {
                type_spec: "tensor<float>(x[2])".to_owned(),
                payload: searchkernel_ir::RequestTensorPayloadIr::Dense(vec![1.0, 0.0]),
            },
        ))
    };
    let cases = [
        (
            "nn_non_tensor_field",
            Query::nearest_neighbor("title", "q"),
            searchkernel_ir::QueryExprIr {
                kind: searchkernel_ir::QueryExprKindIr::NearestNeighbor {
                    document_field: typed_field("title"),
                    query_tensor: "q".to_owned(),
                },
                annotations: BTreeMap::new(),
            },
            BTreeMap::from([("q".to_owned(), tensor_input())]),
        ),
        (
            "nn_scalar_query_input",
            Query::nearest_neighbor("embedding", "q"),
            searchkernel_ir::QueryExprIr {
                kind: searchkernel_ir::QueryExprKindIr::NearestNeighbor {
                    document_field: typed_field("embedding"),
                    query_tensor: "q".to_owned(),
                },
                annotations: BTreeMap::new(),
            },
            BTreeMap::from([(
                "q".to_owned(),
                searchkernel_ir::QueryInputIr::Literal(searchkernel_ir::RequestValueIr::Float64(
                    1.0,
                )),
            )]),
        ),
        (
            "term_value_type",
            Query::term("title", 42_i64),
            typed_contains(
                "title",
                searchkernel_ir::QueryExprKindIr::Term {
                    value: searchkernel_ir::RequestValueIr::Int64(42),
                },
            ),
            BTreeMap::new(),
        ),
        (
            "fuzzy_value_type",
            Query::fuzzy("title", 42_i64),
            typed_contains(
                "title",
                searchkernel_ir::QueryExprKindIr::Fuzzy {
                    value: searchkernel_ir::RequestValueIr::Int64(42),
                    max_edit_distance: 2,
                    prefix_length: 0,
                    prefix: false,
                },
            ),
            BTreeMap::new(),
        ),
        (
            "weighted_key_type",
            Query::weighted_set("price", [crate::WeightedQueryItem::new("bad", 1)]),
            searchkernel_ir::QueryExprIr {
                kind: searchkernel_ir::QueryExprKindIr::WeightedSet {
                    field: typed_field("price"),
                    items: vec![searchkernel_ir::WeightedQueryItemIr {
                        key: searchkernel_ir::RequestValueIr::String("bad".to_owned()),
                        weight: 1,
                    }],
                },
                annotations: BTreeMap::new(),
            },
            BTreeMap::new(),
        ),
        (
            "weighted_empty_key",
            Query::weighted_set("brand", [crate::WeightedQueryItem::new("", 1)]),
            searchkernel_ir::QueryExprIr {
                kind: searchkernel_ir::QueryExprKindIr::WeightedSet {
                    field: typed_field("brand"),
                    items: vec![searchkernel_ir::WeightedQueryItemIr {
                        key: searchkernel_ir::RequestValueIr::String(String::new()),
                        weight: 1,
                    }],
                },
                annotations: BTreeMap::new(),
            },
            BTreeMap::new(),
        ),
        (
            "nn_empty_query_tensor",
            Query::nearest_neighbor("embedding", ""),
            searchkernel_ir::QueryExprIr {
                kind: searchkernel_ir::QueryExprKindIr::NearestNeighbor {
                    document_field: typed_field("embedding"),
                    query_tensor: String::new(),
                },
                annotations: BTreeMap::new(),
            },
            BTreeMap::new(),
        ),
        (
            "wand_zero_threshold_boost",
            Query::Wand {
                field: "brand".to_owned(),
                items: vec![crate::WeightedQueryItem::new("acme", 1)],
                options: WandOptions {
                    threshold_boost_factor: 0.0,
                    ..WandOptions::default()
                },
            },
            searchkernel_ir::QueryExprIr {
                kind: searchkernel_ir::QueryExprKindIr::Wand {
                    field: typed_field("brand"),
                    items: vec![searchkernel_ir::WeightedQueryItemIr {
                        key: searchkernel_ir::RequestValueIr::String("acme".to_owned()),
                        weight: 1,
                    }],
                },
                annotations: BTreeMap::from([
                    (
                        "targetNumHits".to_owned(),
                        searchkernel_ir::RequestValueIr::UInt64(100),
                    ),
                    (
                        "scoreThreshold".to_owned(),
                        searchkernel_ir::RequestValueIr::Int64(0),
                    ),
                    (
                        "thresholdBoostFactor".to_owned(),
                        searchkernel_ir::RequestValueIr::Float64(0.0),
                    ),
                ]),
            },
            BTreeMap::new(),
        ),
        (
            "wand_negative_threshold_boost",
            Query::Wand {
                field: "brand".to_owned(),
                items: vec![crate::WeightedQueryItem::new("acme", 1)],
                options: WandOptions {
                    threshold_boost_factor: -1.0,
                    ..WandOptions::default()
                },
            },
            searchkernel_ir::QueryExprIr {
                kind: searchkernel_ir::QueryExprKindIr::Wand {
                    field: typed_field("brand"),
                    items: vec![searchkernel_ir::WeightedQueryItemIr {
                        key: searchkernel_ir::RequestValueIr::String("acme".to_owned()),
                        weight: 1,
                    }],
                },
                annotations: BTreeMap::from([
                    (
                        "targetNumHits".to_owned(),
                        searchkernel_ir::RequestValueIr::UInt64(100),
                    ),
                    (
                        "scoreThreshold".to_owned(),
                        searchkernel_ir::RequestValueIr::Int64(0),
                    ),
                    (
                        "thresholdBoostFactor".to_owned(),
                        searchkernel_ir::RequestValueIr::Float64(-1.0),
                    ),
                ]),
            },
            BTreeMap::new(),
        ),
    ];

    for (name, rust_query, portable_query, inputs) in cases {
        let recording = Arc::new(RecordingBackend::new());
        let (_runtime, index) = runtime_index(definition.clone(), recording);
        let mut builder = index.search("").typed_query(rust_query);
        for (input_name, input) in &inputs {
            builder = match input {
                searchkernel_ir::QueryInputIr::Literal(
                    searchkernel_ir::RequestValueIr::Float64(value),
                ) => builder.query_input(input_name.clone(), *value),
                searchkernel_ir::QueryInputIr::Literal(
                    searchkernel_ir::RequestValueIr::Tensor(tensor),
                ) => {
                    let searchkernel_ir::RequestTensorPayloadIr::Dense(values) = &tensor.payload
                    else {
                        panic!("test input must be dense")
                    };
                    builder.query_input(
                        input_name.clone(),
                        Tensor::dense(tensor.type_spec.clone(), values.clone()),
                    )
                }
                other => panic!("unexpected test query input: {other:?}"),
            };
        }
        let rust_error = builder.execute::<serde_json::Value>().unwrap_err();
        let bytes = portable_typed_request(portable_query, inputs);
        let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
        let portable_error = context.search_to_ir(request).unwrap_err();

        let portable_code = match portable_error.code {
            searchkernel_ir::ErrorCode::InvalidArgument => crate::ErrorCode::InvalidArgument,
            searchkernel_ir::ErrorCode::Unsupported => crate::ErrorCode::Unsupported,
            other => panic!("unexpected portable error code for {name}: {other:?}"),
        };
        assert_eq!(rust_error.code(), portable_code, "{name}");
        assert_eq!(
            rust_error.details()[0].code,
            portable_error.details[0].code,
            "{name}"
        );
        assert_eq!(
            rust_error.details()[0].path,
            portable_error.details[0].path,
            "{name}"
        );
    }
}

#[test]
fn public_and_portable_fuzzy_option_errors_converge_in_the_ir() {
    let definition = IndexDefinition::new().field(Field::text("title").searchable(true));
    let context = context_for(&definition);
    let recording = Arc::new(RecordingBackend::new());
    let (_runtime, index) = runtime_index(definition, recording);
    let rust_error = index
        .search("")
        .typed_query(Query::Fuzzy {
            field: "title".to_owned(),
            value: "Acme".into(),
            max_edit_distance: 0,
            prefix_length: 2,
            prefix: false,
        })
        .execute::<serde_json::Value>()
        .expect_err("invalid fuzzy options must fail before backend execution");

    let bytes = portable_fuzzy_request(0, 2, false);
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let portable_error = context
        .search_to_ir(request)
        .expect_err("portable invalid fuzzy options must fail in the resolver");
    assert_eq!(rust_error.code(), crate::ErrorCode::InvalidArgument);
    assert_eq!(
        portable_error.code,
        searchkernel_ir::ErrorCode::InvalidArgument
    );
    assert_eq!(rust_error.details()[0].code, portable_error.details[0].code);
    assert_eq!(rust_error.details()[0].path, portable_error.details[0].path);
}

#[test]
fn public_rust_typed_query_values_survive_canonical_normalization() {
    let definition = scripted_definition();
    let context = context_for(&definition);

    let cases = [
        (
            Query::weak_and([Query::term("title", "value")], 37),
            portable_weak_and_request(37),
        ),
        (
            Query::Fuzzy {
                field: "title".to_owned(),
                value: "fone".into(),
                max_edit_distance: 4,
                prefix_length: 3,
                prefix: true,
            },
            portable_fuzzy_request(4, 3, true),
        ),
        (
            Query::Near {
                field: "title".to_owned(),
                children: vec![Query::term("title", "value")],
                distance: 7,
            },
            portable_near_request(7),
        ),
    ];

    for (query, bytes) in cases {
        let recording = Arc::new(RecordingBackend::new());
        let (_runtime, index) = runtime_index(definition.clone(), recording.clone());
        assert!(index
            .search("typed")
            .typed_query(query)
            .execute::<serde_json::Value>()
            .is_err());
        let OperationIr::Search(rust_request) = recording.last_call().unwrap() else {
            panic!("typed query should reach the recording backend");
        };
        let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
        let portable_request = context.search_to_ir(request).unwrap();
        assert_eq!(rust_request, portable_request);

        let searchkernel_ir::QueryIr::Typed(query) = rust_request.query() else {
            panic!("expected a typed query");
        };
        match &query.kind {
            searchkernel_ir::QueryExprKindIr::WeakAnd { target_hits, .. } => {
                assert_eq!(*target_hits, 37)
            }
            searchkernel_ir::QueryExprKindIr::Contains { item, .. } => match &item.kind {
                searchkernel_ir::QueryExprKindIr::Fuzzy {
                    max_edit_distance,
                    prefix_length,
                    prefix,
                    ..
                } => assert_eq!((*max_edit_distance, *prefix_length, *prefix), (4, 3, true)),
                searchkernel_ir::QueryExprKindIr::Near { distance, .. } => {
                    assert_eq!(*distance, 7)
                }
                other => panic!("unexpected contained query kind: {other:?}"),
            },
            other => panic!("unexpected typed query kind: {other:?}"),
        }
    }
}

#[test]
fn public_rust_omitted_typed_query_defaults_match_portable_requests() {
    let definition = scripted_definition().field(Field::tensor("embedding", "tensor<float>(x[2])"));
    let context = context_for(&definition);
    let term = || {
        typed_contains(
            "title",
            searchkernel_ir::QueryExprKindIr::Term {
                value: searchkernel_ir::RequestValueIr::String("value".to_owned()),
            },
        )
    };
    let cases = [
        (
            Query::near("title", [Query::term("title", "value")]),
            typed_contains(
                "title",
                searchkernel_ir::QueryExprKindIr::Near {
                    children: vec![term()],
                    distance: searchkernel_ir::api_to_ir::DEFAULT_PROXIMITY_DISTANCE,
                },
            ),
        ),
        (
            Query::fuzzy("title", "value"),
            typed_contains(
                "title",
                searchkernel_ir::QueryExprKindIr::Fuzzy {
                    value: searchkernel_ir::RequestValueIr::String("value".to_owned()),
                    max_edit_distance: searchkernel_ir::api_to_ir::DEFAULT_FUZZY_MAX_EDIT_DISTANCE,
                    prefix_length: searchkernel_ir::api_to_ir::DEFAULT_FUZZY_PREFIX_LENGTH,
                    prefix: searchkernel_ir::api_to_ir::DEFAULT_FUZZY_PREFIX,
                },
            ),
        ),
        (
            Query::wand("brand", [crate::WeightedQueryItem::new("value", 1)]),
            searchkernel_ir::QueryExprIr {
                kind: searchkernel_ir::QueryExprKindIr::Wand {
                    field: typed_field("brand"),
                    items: vec![searchkernel_ir::WeightedQueryItemIr {
                        key: searchkernel_ir::RequestValueIr::String("value".to_owned()),
                        weight: 1,
                    }],
                },
                annotations: BTreeMap::new(),
            },
        ),
    ];

    for (rust_query, portable_query) in cases {
        let recording = Arc::new(RecordingBackend::new());
        let (_runtime, index) = runtime_index(definition.clone(), recording);
        let request = index.search("typed").typed_query(rust_query);
        let rust_request = searchkernel_ir::resolve_search(
            &context,
            crate::search::to_resolver_input(&request).expect("Rust query should lower"),
        )
        .expect("Rust query should resolve");
        let bytes = portable_typed_request(portable_query, BTreeMap::new());
        let portable_request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes)
            .expect("portable request should parse");
        let portable_request = context
            .search_to_ir(portable_request)
            .expect("portable query should resolve");
        assert_eq!(rust_request.query(), portable_request.query());
    }

    let tensor = searchkernel_ir::QueryInputIr::Literal(searchkernel_ir::RequestValueIr::Tensor(
        searchkernel_ir::RequestTensorIr {
            type_spec: "tensor<float>(x[2])".to_owned(),
            payload: searchkernel_ir::RequestTensorPayloadIr::Dense(vec![1.0, 0.0]),
        },
    ));
    let recording = Arc::new(RecordingBackend::new());
    let (_runtime, index) = runtime_index(definition, recording);
    let request = index
        .search("typed")
        .typed_query(Query::nearest_neighbor("embedding", "q"))
        .query_input("q", Tensor::dense("tensor<float>(x[2])", vec![1.0, 0.0]));
    let rust_request = searchkernel_ir::resolve_search(
        &context,
        crate::search::to_resolver_input(&request).expect("Rust nearest-neighbor should lower"),
    )
    .expect("Rust nearest-neighbor should resolve");
    let portable_query = searchkernel_ir::QueryExprIr {
        kind: searchkernel_ir::QueryExprKindIr::NearestNeighbor {
            document_field: typed_field("embedding"),
            query_tensor: "q".to_owned(),
        },
        annotations: BTreeMap::new(),
    };
    let bytes = portable_typed_request(portable_query, BTreeMap::from([("q".to_owned(), tensor)]));
    let portable_request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes)
        .expect("portable request should parse");
    let portable_request = context
        .search_to_ir(portable_request)
        .expect("portable nearest-neighbor should resolve");
    assert_eq!(rust_request.query(), portable_request.query());
    assert_eq!(rust_request.inputs(), portable_request.inputs());
}

#[test]
fn public_compositional_typed_queries_match_portable_requests() {
    let definition = scripted_definition();
    let context = context_for(&definition);
    let term = |value: &str| searchkernel_ir::QueryExprIr {
        kind: searchkernel_ir::QueryExprKindIr::Term {
            value: searchkernel_ir::RequestValueIr::String(value.to_owned()),
        },
        annotations: BTreeMap::new(),
    };
    let cases = [
        (
            Query::contains("title", Query::term_value("value")),
            typed_contains("title", term("value").kind),
        ),
        (
            Query::contains(
                "title",
                Query::phrase_terms([Query::term_value("quick"), Query::term_value("brown")]),
            ),
            typed_contains(
                "title",
                searchkernel_ir::QueryExprKindIr::Phrase(vec![term("quick"), term("brown")]),
            ),
        ),
        (
            Query::contains(
                "title",
                Query::equiv([Query::term_value("one"), Query::term_value("two")]),
            ),
            typed_contains(
                "title",
                searchkernel_ir::QueryExprKindIr::Equiv(vec![term("one"), term("two")]),
            ),
        ),
        (
            Query::contains(
                "title",
                Query::alternatives([
                    QueryAlternative::new(Query::term_value("one")).exactness(0.5)
                ]),
            ),
            typed_contains(
                "title",
                searchkernel_ir::QueryExprKindIr::Alternatives(vec![
                    searchkernel_ir::QueryAlternativeIr {
                        query: term("one"),
                        exactness: Some(0.5),
                    },
                ]),
            ),
        ),
        (
            Query::contains("title", Query::text("free form")),
            typed_contains(
                "title",
                searchkernel_ir::QueryExprKindIr::Text {
                    text: "free form".to_owned(),
                },
            ),
        ),
        (
            Query::matches("title", "value.*"),
            searchkernel_ir::QueryExprIr {
                kind: searchkernel_ir::QueryExprKindIr::Matches {
                    field: typed_field("title"),
                    pattern: "value.*".to_owned(),
                },
                annotations: BTreeMap::new(),
            },
        ),
        (
            Query::contains(
                "title",
                Query::user_input_with_default_index("free form", "title"),
            ),
            typed_contains(
                "title",
                searchkernel_ir::QueryExprKindIr::UserInput {
                    value: "free form".to_owned(),
                    default_index: Some("title".to_owned()),
                    target_hits: searchkernel_ir::api_to_ir::DEFAULT_WEAK_AND_TARGET_HITS,
                },
            ),
        ),
        (
            Query::user_query_with_default_index("title"),
            searchkernel_ir::QueryExprIr {
                kind: searchkernel_ir::QueryExprKindIr::UserQuery {
                    default_index: Some("title".to_owned()),
                    target_hits: searchkernel_ir::api_to_ir::DEFAULT_WEAK_AND_TARGET_HITS,
                },
                annotations: BTreeMap::new(),
            },
        ),
        (
            Query::labeled(Query::true_query(), "label", 1.0),
            searchkernel_ir::QueryExprIr {
                kind: searchkernel_ir::QueryExprKindIr::Labeled {
                    query: Box::new(searchkernel_ir::QueryExprIr {
                        kind: searchkernel_ir::QueryExprKindIr::True,
                        annotations: BTreeMap::new(),
                    }),
                    label: "label".to_owned(),
                    score: 1.0,
                },
                annotations: BTreeMap::new(),
            },
        ),
        (
            Query::non_empty(Query::false_query()),
            searchkernel_ir::QueryExprIr {
                kind: searchkernel_ir::QueryExprKindIr::NonEmpty(Box::new(
                    searchkernel_ir::QueryExprIr {
                        kind: searchkernel_ir::QueryExprKindIr::False,
                        annotations: BTreeMap::new(),
                    },
                )),
                annotations: BTreeMap::new(),
            },
        ),
        (
            Query::contains(
                "title",
                Query::near_terms([Query::term_value("quick"), Query::term_value("brown")])
                    .with_annotation("distance", 7_u32),
            ),
            typed_contains(
                "title",
                searchkernel_ir::QueryExprKindIr::Near {
                    children: vec![term("quick"), term("brown")],
                    distance: 7,
                },
            ),
        ),
    ];

    for (rust_query, portable_query) in cases {
        let recording = Arc::new(RecordingBackend::new());
        let (_runtime, index) = runtime_index(definition.clone(), recording);
        let request = index.search("typed").typed_query(rust_query);
        let rust_request = searchkernel_ir::resolve_search(
            &context,
            crate::search::to_resolver_input(&request).expect("Rust query should lower"),
        )
        .expect("Rust query should resolve");
        let bytes = portable_typed_request(portable_query, BTreeMap::new());
        let portable_request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes)
            .expect("portable request should parse");
        let portable_request = context
            .search_to_ir(portable_request)
            .expect("portable query should resolve");
        assert_eq!(rust_request.query(), portable_request.query());
    }
}

#[test]
fn blank_text_is_rejected_identically_and_match_all_remains_explicit() {
    let definition = scripted_definition();
    let context = context_for(&definition);

    for text in ["", "   "] {
        let recording = Arc::new(RecordingBackend::new());
        let (_runtime, index) = runtime_index(definition.clone(), recording);
        let rust_error = index
            .search(text)
            .execute::<serde_json::Value>()
            .unwrap_err();
        let bytes = portable_text_request(Some(text));
        let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
        let portable_error = context.search_to_ir(request).unwrap_err();
        assert_eq!(rust_error.code(), crate::ErrorCode::InvalidArgument);
        assert_eq!(
            portable_error.code,
            searchkernel_ir::ErrorCode::InvalidArgument
        );
        assert_eq!(rust_error.details()[0].code, portable_error.details[0].code);
        assert_eq!(rust_error.details()[0].path, portable_error.details[0].path);
    }

    let bytes = portable_text_request(None);
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let missing_text_error = context.search_to_ir(request).unwrap_err();
    assert_eq!(
        missing_text_error.code,
        searchkernel_ir::ErrorCode::InvalidArgument
    );

    let bytes = portable_match_all_request(false);
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let match_all = context.search_to_ir(request).unwrap();
    assert!(matches!(
        match_all.query(),
        searchkernel_ir::QueryIr::MatchAll
    ));
}

#[test]
fn structured_grouping_validation_is_shared_between_ingresses() {
    let definition = scripted_definition();
    let context = context_for(&definition);
    let grouping = || GroupingPlan::new().level(GroupingPlan::by("brand"));

    let recording = Arc::new(RecordingBackend::new());
    let (_runtime, index) = runtime_index(definition.clone(), recording.clone());
    assert!(index
        .browse()
        .grouping(grouping())
        .execute::<serde_json::Value>()
        .is_err());
    let OperationIr::Search(rust_request) = recording.last_call().unwrap() else {
        panic!("valid grouping should reach the recording backend");
    };
    let bytes = portable_grouping_request("brand");
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let portable_request = context.search_to_ir(request).unwrap();
    assert_eq!(rust_request, portable_request);

    let recording = Arc::new(RecordingBackend::new());
    let (_runtime, index) = runtime_index(definition.clone(), recording);
    let rust_error = index
        .browse()
        .grouping(GroupingPlan::new().level(GroupingPlan::by("")))
        .execute::<serde_json::Value>()
        .unwrap_err();
    let bytes = portable_grouping_request("");
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let portable_error = context.search_to_ir(request).unwrap_err();
    assert_eq!(rust_error.code(), crate::ErrorCode::InvalidArgument);
    assert_eq!(
        portable_error.code,
        searchkernel_ir::ErrorCode::InvalidArgument
    );
    assert_eq!(rust_error.details()[0].code, portable_error.details[0].code);
    assert_eq!(rust_error.details()[0].path, portable_error.details[0].path);
}

#[test]
fn public_facets_validate_schema_fields_and_duplicates_in_search_resolution() {
    let definition = IndexDefinition::new()
        .field(Field::text("title").stored(true))
        .field(Field::keyword("brand").facetable(true));
    let (_runtime, index) = runtime_index(definition, Arc::new(RecordingBackend::new()));

    for (facets, detail_code) in [
        (vec!["does_not_exist"], "unknown_field"),
        (vec!["title"], "field_not_facetable"),
        (vec!["brand", "brand"], "duplicate_facet_field"),
    ] {
        let error = index
            .search("x")
            .search_fields(["title"])
            .facets(facets)
            .inspect()
            .expect_err("invalid facet semantics must fail before backend execution");
        assert_eq!(error.details()[0].code, detail_code);
    }
}

#[test]
fn public_structured_grouping_validates_schema_fields_in_search_resolution() {
    let definition = IndexDefinition::new()
        .field(Field::text("title").stored(true))
        .field(Field::keyword("brand").facetable(true));
    let context = context_for(&definition);
    let (_runtime, index) = runtime_index(definition, Arc::new(RecordingBackend::new()));

    for (field, detail_code) in [
        ("does_not_exist", "unknown_field"),
        ("title", "field_not_facetable"),
    ] {
        let error = index
            .browse()
            .grouping(GroupingPlan::new().level(GroupingPlan::by(field)))
            .inspect()
            .expect_err("invalid grouping fields must fail before backend execution");
        assert_eq!(error.details()[0].code, detail_code);

        let bytes = portable_grouping_request(field);
        let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
        let portable_error = context
            .search_to_ir(request)
            .expect_err("portable grouping fields must use the same schema validation");
        assert_eq!(portable_error.details[0].code, detail_code);
    }
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
    let portable_error = context
        .search_to_ir(request)
        .expect_err("empty portable ranking profile must not become the default");

    assert_eq!(rust_error.details()[0].code, "invalid_ranking");
    assert_eq!(portable_error.details[0].code, "invalid_ranking");
}

#[test]
fn empty_language_has_identical_rust_and_portable_semantics() {
    let definition = IndexDefinition::new().field(Field::text("title"));
    let context = context_for(&definition);
    let (_runtime, index) = runtime_index(definition, Arc::new(RecordingBackend::new()));

    let rust_error = index
        .search("x")
        .language("")
        .inspect()
        .expect_err("empty Rust language must be invalid");
    let bytes = portable_text_request_with_empty_controls(None, Some(""), None);
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let portable_error = context
        .search_to_ir(request)
        .expect_err("empty portable language must not disappear");

    assert_eq!(rust_error.details()[0].code, "invalid_language");
    assert_eq!(portable_error.details[0].code, "invalid_language");
}

#[test]
fn search_normalization_covers_explicit_options_and_defaults() {
    let definition = scripted_definition();
    let context = context_for(&definition);
    let (_probe_runtime, probe) =
        runtime_index(definition.clone(), Arc::new(RecordingBackend::new()));
    let expected = expected_search(&context, &normalized_search(&probe));
    let backend = Arc::new(ScriptedBackend::new([ExpectedCall::success(
        OperationIr::Search(expected),
        ScriptedResult::Search(Box::new(searchkernel_ir::SearchResult::empty(
            searchkernel_ir::TotalHits {
                value: 0,
                relation: searchkernel_ir::TotalHitsRelation::Exact,
            },
        ))),
    )]));
    let (_runtime, index) = runtime_index(definition, backend.clone());

    let inspected = normalized_search(&index).inspect().unwrap();
    assert_eq!(inspected.query, "  fox  ");
    assert_eq!(inspected.search_fields.len(), 1);
    assert_eq!(inspected.search_fields[0].field(), "title");
    assert_eq!(
        inspected.filter,
        Some(Filter::all([
            field("brand").eq("acme"),
            field("category").eq("books"),
        ]))
    );
    assert_eq!(inspected.sorts.len(), 2);
    assert_eq!(inspected.sorts[0].direction, crate::SortDirection::Asc);
    assert_eq!(
        inspected.sorts[0].missing,
        Some(crate::MissingPlacement::Last)
    );
    assert_eq!(inspected.sorts[1].direction, crate::SortDirection::Desc);
    assert_eq!(
        inspected.sorts[1].missing,
        Some(crate::MissingPlacement::First)
    );
    assert!(inspected.facets.is_empty());
    assert_eq!(inspected.ranking.kind, crate::search::RankingKind::Bm25);
    assert_eq!(inspected.limit, 7);
    assert_eq!(inspected.offset, 2);
    assert_eq!(inspected.projection, ["title", "brand"]);
    assert_eq!(inspected.highlighting, Some(vec!["title".to_owned()]));
    assert_eq!(inspected.language.as_deref(), Some("en-us"));

    let result: crate::SearchResult<serde_json::Value> =
        normalized_search(&index).execute().unwrap();
    assert!(result.hits.is_empty());
    backend.finish().unwrap();

    let single_filter = index
        .search("fox")
        .filter(field("brand").eq("acme"))
        .inspect()
        .unwrap();
    assert_eq!(single_filter.query, "fox");
    assert_eq!(single_filter.filter, Some(field("brand").eq("acme")));

    let defaults = index.browse().inspect().unwrap();
    assert_eq!(defaults.query, "");
    assert!(defaults.filter.is_none());
    assert!(defaults.sorts.is_empty());
    assert!(defaults.facets.is_empty());
    assert_eq!(
        defaults.ranking.kind,
        crate::search::RankingKind::DefaultRelevanceV1
    );
    assert_eq!(defaults.limit, 20);
    assert_eq!(defaults.offset, 0);
    assert_eq!(defaults.projection, ["title", "brand"]);
    assert!(defaults.highlighting.is_none());
    assert!(defaults.language.is_none());
}

#[test]
fn scripted_backend_errors_preserve_code_message_and_details() {
    let definition = scripted_definition();
    let context = context_for(&definition);
    let (_probe_runtime, probe) =
        runtime_index(definition.clone(), Arc::new(RecordingBackend::new()));
    let mut backend_error = searchkernel_ir::PortableError::new(
        searchkernel_ir::ErrorCode::Unavailable,
        "backend unavailable",
    )
    .with_detail("backend_timeout", Some("search"), "request timed out");
    backend_error.retryable = true;
    let backend = Arc::new(ScriptedBackend::new([ExpectedCall::error(
        OperationIr::Search(expected_search(&context, &probe.search("fox"))),
        backend_error,
    )]));
    let (_runtime, index) = runtime_index(definition, backend.clone());

    let error = index
        .search("fox")
        .execute::<serde_json::Value>()
        .expect_err("scripted backend should fail the search");
    assert_eq!(error.code(), crate::ErrorCode::Unavailable);
    assert!(error.retryable());
    assert_eq!(error.details()[0].code, "backend_timeout");
    assert_eq!(error.details()[0].path.as_deref(), Some("search"));
    assert_eq!(error.details()[0].message, "request timed out");
    assert_eq!(error.to_string(), "backend unavailable");
    backend.finish().unwrap();
}

#[test]
fn invalid_values_do_not_reach_the_backend() {
    let (_runtime, index, backend) = test_index();

    let error = index
        .insert("nan", &f64::NAN)
        .expect_err("non-finite JSON values must be rejected");
    assert!(
        matches!(error, Error::Serialization(_)),
        "unexpected error: {error}"
    );
    assert!(backend.calls().is_empty());

    let error = index
        .patch("nan", Patch::new().set("price", f64::NAN))
        .expect_err("invalid patch values must be rejected");
    assert_eq!(error.code(), crate::ErrorCode::InvalidArgument);
    assert!(backend.calls().is_empty());
}

#[test]
fn backend_errors_are_returned_without_being_rewritten() {
    let (_runtime, index, backend) = test_index();
    let error = index
        .insert("doc-1", &json!({"title": "fox"}))
        .expect_err("recording backend intentionally rejects writes");
    assert_eq!(error.code(), crate::ErrorCode::Unsupported);
    assert_eq!(backend.calls().len(), 1);

    let error = index
        .search("fox")
        .limit(5)
        .execute::<serde_json::Value>()
        .expect_err("recording backend intentionally rejects searches");
    assert_eq!(error.code(), crate::ErrorCode::Unsupported);
    assert_eq!(backend.calls().len(), 2);
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
fn derived_default_search_fields_include_schema_only_fields() {
    let definition = IndexDefinition::new()
        .field(Field::text("title"))
        .schema_field(Field::text("derived"));
    let resolved = definition_to_ir(&definition).expect("schema should resolve");

    assert_eq!(
        resolved
            .default_search_fields()
            .iter()
            .map(|field| field.field().as_str())
            .collect::<Vec<_>>(),
        ["title", "derived"]
    );
}

#[test]
fn rich_public_definition_matches_portable_without_canonical_sidecar() {
    let (canonical, described) = public_described_canonical_fixture();

    let resolved = definition_to_ir(&described)
        .expect("complete public schema state should use the shared resolver");
    assert_eq!(canonical, resolved);
}

#[test]
fn fluent_search_and_portable_request_share_one_canonical_search_snapshot() {
    let definition = scripted_definition();
    let context = context_for(&definition);
    let (_runtime, probe) = runtime_index(definition, Arc::new(RecordingBackend::new()));
    let rust_request = probe
        .search("fox")
        .search_fields(["title"])
        .filter(field("brand").eq("acme"))
        .filter("category = 'books'")
        .sort(Sort::desc("brand").missing_first())
        .facet(crate::Facet::new("category").name("categories").limit(3))
        .grouping(
            GroupingPlan::new().level(
                GroupingPlan::by("brand")
                    .output(crate::GroupingAggregation::count())
                    .nested(GroupingPlan::by("category")),
            ),
        )
        .ranking(Ranking::bm25())
        .rank_property("source", "catalog")
        .projection(["title", "brand"])
        .highlight(["title"])
        .language("EN-us")
        .limit(7)
        .offset(2);

    let direct = expected_search(&context, &rust_request);
    let bytes = searchkernel_ir::search_request_ir_to_api(&direct);
    let portable = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes)
        .expect("canonical search request should decode");
    let portable = context
        .search_to_ir(portable)
        .expect("canonical portable search should resolve");

    assert_eq!(direct, portable);
    assert_eq!(
        searchkernel_ir::snapshot::search_request(&direct),
        searchkernel_ir::snapshot::search_request(&portable)
    );
}

fn assert_rust_and_portable_schema_error_match(
    definition: searchkernel_ir::IndexDefinitionIr,
    expected_detail_code: &str,
) {
    let index = index_from_canonical_ir(definition.clone());
    let described = index
        .describe()
        .expect("invalid fixture should still describe structurally");

    let rust_error = definition_to_ir(&described)
        .expect_err("Rust schema ingress should reject invalid semantic state");

    let bytes = searchkernel_ir::index_definition_ir_to_api(&definition);
    let portable = searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(&bytes)
        .expect("portable invalid fixture should still decode structurally");
    let portable_error = searchkernel_ir::index_definition_to_ir(portable)
        .expect_err("portable schema ingress should reject the same semantic state");

    assert_eq!(rust_error.details()[0].code, expected_detail_code);
    assert_eq!(portable_error.details[0].code, expected_detail_code);
    assert_eq!(rust_error.details()[0].path, portable_error.details[0].path);
}

fn invalid_schema_with_field(
    field: searchkernel_ir::FieldIr,
) -> searchkernel_ir::IndexDefinitionIr {
    searchkernel_ir::IndexDefinitionIr::new(None, vec![field], Vec::new())
        .expect("invalid field semantics are intentionally injected after construction")
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

#[cfg(feature = "native")]
#[test]
fn native_public_get_round_trips_supported_composites_projection_and_reopen() {
    use searchkernel_ir::{
        DataTypeIr, FieldCapabilities, FieldIr, FieldName, IndexDefinitionIr,
        NamedStructDefinitionIr, PrimitiveDataTypeIr, StructFieldIr,
    };

    // Vespa stores these composite values in attribute-backed fields. The
    // public stored capability controls inclusion in the default GET
    // projection, while filterable is what gives the native adapter the
    // attribute required for these types.
    let stored_attribute = || FieldCapabilities::new(true, false, true, false, false);
    let fields = vec![
        FieldIr::new(
            FieldName::parse("tags").unwrap(),
            DataTypeIr::Array(Box::new(DataTypeIr::Primitive(PrimitiveDataTypeIr::String))),
            None,
            stored_attribute(),
        ),
        FieldIr::new(
            FieldName::parse("people").unwrap(),
            DataTypeIr::Array(Box::new(DataTypeIr::Struct {
                fields: vec![
                    StructFieldIr {
                        name: "name".to_owned(),
                        data_type: DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
                    },
                    StructFieldIr {
                        name: "age".to_owned(),
                        data_type: DataTypeIr::Primitive(PrimitiveDataTypeIr::Int64),
                    },
                    StructFieldIr {
                        name: "active".to_owned(),
                        data_type: DataTypeIr::Primitive(PrimitiveDataTypeIr::Bool),
                    },
                    StructFieldIr {
                        name: "score".to_owned(),
                        data_type: DataTypeIr::Primitive(PrimitiveDataTypeIr::Float64),
                    },
                ],
            })),
            None,
            stored_attribute(),
        ),
        FieldIr::new(
            FieldName::parse("people_ref").unwrap(),
            DataTypeIr::Array(Box::new(DataTypeIr::StructReference {
                name: "person_ref".to_owned(),
            })),
            None,
            stored_attribute(),
        ),
        FieldIr::new(
            FieldName::parse("location").unwrap(),
            DataTypeIr::Position,
            None,
            stored_attribute(),
        ),
        FieldIr::new(
            FieldName::parse("feature_access").unwrap(),
            DataTypeIr::Predicate,
            None,
            stored_attribute(),
        ),
        FieldIr::new(
            FieldName::parse("expression_access").unwrap(),
            DataTypeIr::Predicate,
            None,
            stored_attribute(),
        ),
    ];
    let mut canonical = IndexDefinitionIr::new(None, fields, Vec::new())
        .expect("composite product schema should be valid canonical IR");
    canonical.set_structs(vec![NamedStructDefinitionIr {
        name: "person_ref".to_owned(),
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
        properties: std::collections::BTreeMap::new(),
    }]);
    let definition = definition_from_ir(&canonical);
    let path = std::env::temp_dir().join(format!(
        "searchkernel-public-composite-get-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&path);

    let expected = json!({
        "tags": ["alpha", "", "omega"],
        "people": [
            {"name": "Ada", "age": 37, "active": true, "score": 1.23456789},
            {"name": "Grace", "age": 85, "active": false, "score": 9.87654321}
        ],
        "people_ref": [
            {"name": "Margaret", "score": std::f64::consts::PI}
        ],
        "location": {"latitude": 37.774929, "longitude": -122.419416},
        "feature_access": {"features": {"hobby": "music"}},
        "expression_access": {
            "expression": {"type": 4, "key": "hobby", "feature_set": ["music"]}
        }
    });

    let index = Index::create(&path, definition).expect("create composite product index");
    index
        .upsert("composite-1", &expected)
        .expect("write composite product document");
    index
        .upsert(
            "explicit-empty",
            &json!({"tags": [], "people": [], "people_ref": []}),
        )
        .expect("write explicit empty collections");
    index
        .upsert(
            "absent-collections",
            &json!({"feature_access": {"features": {"hobby": "music"}}}),
        )
        .expect("write document with absent collection fields");

    let full = index
        .get::<serde_json::Value>("composite-1")
        .expect("get composite document through public facade")
        .expect("composite document exists");
    assert_eq!(full, expected);

    let projected = index
        .get_with_projection::<serde_json::Value>(
            "composite-1",
            [
                "people",
                "people_ref",
                "feature_access",
                "expression_access",
            ],
        )
        .expect("get projected composite document")
        .expect("projected document exists");
    assert_eq!(
        projected,
        json!({
            "people": expected["people"].clone(),
            "people_ref": expected["people_ref"].clone(),
            "feature_access": expected["feature_access"].clone(),
            "expression_access": expected["expression_access"].clone()
        })
    );

    let empty = index
        .get_with_projection::<serde_json::Value>("composite-1", std::iter::empty::<&str>())
        .expect("get explicit empty projection")
        .expect("document exists for empty projection");
    assert_eq!(empty, json!({}));

    let explicit_empty = index
        .get_with_projection::<serde_json::Value>(
            "explicit-empty",
            ["tags", "people", "people_ref"],
        )
        .expect("get explicit empty collections")
        .expect("explicit-empty document exists");
    assert_eq!(
        explicit_empty,
        json!({"tags": [], "people": [], "people_ref": []})
    );

    let absent_collections = index
        .get_with_projection::<serde_json::Value>(
            "absent-collections",
            ["tags", "people", "people_ref"],
        )
        .expect("get absent collections")
        .expect("absent-collections document exists");
    assert_eq!(absent_collections, json!({}));

    index.close().expect("close composite product index");
    let reopened = Index::open(&path).expect("reopen composite product index");
    let persisted = reopened
        .get::<serde_json::Value>("composite-1")
        .expect("get composite document after reopen")
        .expect("reopened composite document exists");
    assert_eq!(persisted, expected);

    let persisted_empty = reopened
        .get_with_projection::<serde_json::Value>(
            "explicit-empty",
            ["tags", "people", "people_ref"],
        )
        .expect("get persisted explicit empty collections")
        .expect("persisted explicit-empty document exists");
    assert_eq!(
        persisted_empty,
        json!({"tags": [], "people": [], "people_ref": []})
    );

    let persisted_absent = reopened
        .get_with_projection::<serde_json::Value>(
            "absent-collections",
            ["tags", "people", "people_ref"],
        )
        .expect("get persisted absent collections")
        .expect("persisted absent-collections document exists");
    assert_eq!(persisted_absent, json!({}));

    reopened
        .close()
        .expect("close reopened composite product index");
    std::fs::remove_dir_all(path).expect("remove composite product index");
}
