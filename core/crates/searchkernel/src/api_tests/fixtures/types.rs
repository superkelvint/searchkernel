use searchkernel_protocol::wire::v1 as api;

pub(in crate::index::api_tests) fn analyzer<'a>(
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

pub(in crate::index::api_tests) fn array<'a>(
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

pub(in crate::index::api_tests) fn attribute<'a>(
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

pub(in crate::index::api_tests) fn empty_data_type<'a>(
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

pub(in crate::index::api_tests) fn file_constant<'a>(
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

pub(in crate::index::api_tests) fn finish_portable_typed_request<'a>(
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

#[allow(clippy::too_many_arguments)]
pub(in crate::index::api_tests) fn fixture_field_with_struct_fields<'a>(
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

pub(in crate::index::api_tests) fn float_value<'a>(
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

pub(in crate::index::api_tests) fn hnsw_config<'a>(
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

pub(in crate::index::api_tests) fn index_config<'a>(
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

pub(in crate::index::api_tests) fn literal_constant<'a>(
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

pub(in crate::index::api_tests) fn map<'a>(
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

pub(in crate::index::api_tests) fn matching<'a>(
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

pub(in crate::index::api_tests) fn onnx_model<'a>(
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

pub(in crate::index::api_tests) fn portable_field<'a>(
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

pub(in crate::index::api_tests) fn portable_typed_contains_expression<'a>(
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

pub(in crate::index::api_tests) fn portable_value_bool<'a>(
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

pub(in crate::index::api_tests) fn portable_value_int64<'a>(
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

pub(in crate::index::api_tests) fn portable_value_string<'a>(
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

pub(in crate::index::api_tests) fn primitive<'a>(
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

pub(in crate::index::api_tests) fn rank_phase<'a>(
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

pub(in crate::index::api_tests) fn reference<'a>(
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

pub(in crate::index::api_tests) fn string_entries<'a>(
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

pub(in crate::index::api_tests) fn string_properties<'a>(
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

pub(in crate::index::api_tests) fn string_value<'a>(
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

pub(in crate::index::api_tests) fn strings<'a>(
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

pub(in crate::index::api_tests) fn struct_field_config<'a>(
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

pub(in crate::index::api_tests) fn struct_reference<'a>(
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

pub(in crate::index::api_tests) fn struct_type<'a>(
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

pub(in crate::index::api_tests) fn summary<'a>(
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

pub(in crate::index::api_tests) fn tensor<'a>(
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

pub(in crate::index::api_tests) fn tensor_value<'a>(
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

pub(in crate::index::api_tests) fn weighted_set<'a>(
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
