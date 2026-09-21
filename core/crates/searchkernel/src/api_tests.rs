use super::*;
use crate::schema::ConstantSource;
use crate::{
    field, Analyzer, Filter, GroupingPlan, Query, Ranking, SearchBuilder, Sort, Tensor, WandOptions,
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

pub(super) fn public_described_canonical_fixture(
) -> (searchkernel_ir::IndexDefinitionIr, IndexDefinition) {
    let canonical = canonical_fixture_ir();
    let index = index_from_canonical_ir(canonical.clone());
    let described = index.describe().expect("describe should succeed");
    (canonical, described)
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
    assert_eq!(profile.first_phase.as_deref(), Some("bm25(title)"));
    assert_eq!(profile.second_phase.as_deref(), Some("attribute(score)"));
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
    assert_eq!(
        definition.default_ranking(),
        searchkernel_ir::RankingPolicyIr::DefaultRelevanceV1
    );
    assert_eq!(
        definition.default_prefix_policy(),
        searchkernel_ir::PrefixPolicy::Disabled
    );
    assert_eq!(
        definition.default_typo_policy(),
        searchkernel_ir::TypoPolicy::Disabled
    );
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
fn described_rich_definition_preserves_canonical_portable_provenance() {
    let (canonical, described) = public_described_canonical_fixture();
    let described_bytes = crate::portable_definition::encode_portable_definition(&described);
    let canonical_bytes = searchkernel_ir::index_definition_ir_to_api(&canonical);

    assert_eq!(described_bytes, canonical_bytes);
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
fn described_public_values_expose_the_complete_canonical_schema_state() {
    let (canonical, described) = public_described_canonical_fixture();

    assert_eq!(described.canonical_definition(), Some(&canonical));
    assert_eq!(
        described.fields()[0].canonical_definition(),
        Some(&canonical.fields()[0])
    );
    assert_eq!(
        described.rank_profiles()[0].canonical_definition(),
        Some(&canonical.rank_profiles()[0])
    );
    assert_eq!(described.rank_profiles()[2].first_phase, None);
    assert_eq!(described.rank_profiles()[2].second_phase, None);
    assert_eq!(
        described.rank_profiles()[2].canonical_definition(),
        Some(&canonical.rank_profiles()[2])
    );

    let scores = described
        .fields()
        .iter()
        .find(|field| field.name() == "scores")
        .expect("scores field should be described");
    assert!(matches!(
        scores
            .canonical_definition()
            .expect("scores should expose canonical state")
            .data_type(),
        searchkernel_ir::DataTypeIr::Map { .. }
    ));
    assert_eq!(
        scores
            .canonical_definition()
            .and_then(|field| field.index()),
        canonical.fields()[10].index()
    );
    assert_eq!(
        scores
            .canonical_definition()
            .and_then(|field| field.attribute()),
        canonical.fields()[10].attribute()
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
fn mutating_a_described_field_preserves_untouched_canonical_state() {
    let (canonical, described) = public_described_canonical_fixture();
    let modified = described.fields()[0].clone().stored(false);
    let public = modified
        .canonical_definition()
        .expect("mutated described field should retain canonical state")
        .clone();

    assert!(!public.capabilities().stored());
    assert_eq!(public.properties(), canonical.fields()[0].properties());
    assert_eq!(
        public.struct_fields(),
        canonical.fields()[0].struct_fields()
    );

    let resolved = definition_to_ir(&IndexDefinition::new().field(modified))
        .expect("mutated field should resolve");
    assert_eq!(resolved.fields()[0], public);
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
    let public = modified
        .canonical_definition()
        .expect("mutated described profile should retain canonical state")
        .clone();

    assert_eq!(public.inherits, canonical.rank_profiles()[0].inherits);
    assert_eq!(public.functions, canonical.rank_profiles()[0].functions);
    assert_eq!(public.constants, canonical.rank_profiles()[0].constants);
    assert_eq!(
        public
            .first_phase
            .as_ref()
            .and_then(|phase| phase.expression.as_deref()),
        Some("new_rank_expression")
    );
    assert!(public
        .match_features
        .iter()
        .any(|feature| feature == "new_feature"));

    let resolved = definition_to_ir(
        &IndexDefinition::new()
            .field(Field::text("title"))
            .rank_profile(modified),
    )
    .expect("mutated profile should resolve");
    assert_eq!(resolved.rank_profiles()[0], public);
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
        assert_eq!(public.data_type(), &expected.data_type);
        assert_eq!(public.properties(), &expected.properties);
        match (public.source(), &expected.source) {
            (
                ConstantSource::Literal(public_value),
                searchkernel_ir::ConstantSourceIr::Literal(expected_value),
            ) => assert_eq!(public_value, expected_value),
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
