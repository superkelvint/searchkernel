use searchkernel_ir::__flatbuffers::{root, FlatBufferBuilder};
use searchkernel_ir::wire::v1::schema as api;
use searchkernel_ir::{IndexContext, QueryIr};

fn context() -> IndexContext {
    let mut builder = FlatBufferBuilder::new();
    let analyzer_name = builder.create_string("standard");
    let analyzer = api::Analyzer::create(
        &mut builder,
        &api::AnalyzerArgs {
            name: Some(analyzer_name),
            version: 1,
            ..Default::default()
        },
    );
    let title_name = builder.create_string("title");
    let title_type = string_type(&mut builder);
    let title = api::FieldDefinition::create(
        &mut builder,
        &api::FieldDefinitionArgs {
            name: Some(title_name),
            data_type: Some(title_type),
            stored: Some(&api::OptionalBool::new(true)),
            searchable: Some(&api::OptionalBool::new(true)),
            analyzer: Some(analyzer),
            ..Default::default()
        },
    );
    let brand_name = builder.create_string("brand");
    let brand_type = string_type(&mut builder);
    let brand_matching = api::MatchingConfig::create(
        &mut builder,
        &api::MatchingConfigArgs {
            mode: api::MatchingMode::Exact,
            ..Default::default()
        },
    );
    let brand = api::FieldDefinition::create(
        &mut builder,
        &api::FieldDefinitionArgs {
            name: Some(brand_name),
            data_type: Some(brand_type),
            matching: Some(brand_matching),
            filterable: Some(&api::OptionalBool::new(true)),
            sortable: Some(&api::OptionalBool::new(true)),
            ..Default::default()
        },
    );
    let fields = builder.create_vector(&[title, brand]);
    let definition = api::IndexDefinition::create(
        &mut builder,
        &api::IndexDefinitionArgs {
            fields: Some(fields),
            ..Default::default()
        },
    );
    builder.finish(definition, None);
    IndexContext::from_api(root::<api::IndexDefinition>(builder.finished_data()).unwrap()).unwrap()
}

fn string_type<'a>(
    builder: &mut FlatBufferBuilder<'a>,
) -> flatbuffers::WIPOffset<api::DataType<'a>> {
    let primitive = api::PrimitiveDataType::create(
        builder,
        &api::PrimitiveDataTypeArgs {
            kind: api::PrimitiveDataTypeKind::String,
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

#[test]
fn representative_v2_converters_use_direct_core_calls() {
    let context = context();
    let mut builder = FlatBufferBuilder::new();
    let query_text = builder.create_string("headphones");
    let query_node = api::SearchKernelTextQuery::create(
        &mut builder,
        &api::SearchKernelTextQueryArgs {
            text: Some(query_text),
        },
    );
    let query = api::QuerySource::create(
        &mut builder,
        &api::QuerySourceArgs {
            source_type: api::QuerySourceKind::SearchKernelTextQuery,
            source: Some(query_node.as_union_value()),
        },
    );
    let value_text = builder.create_string("Acme");
    let value_node = api::StringValue::create(
        &mut builder,
        &api::StringValueArgs {
            value: Some(value_text),
        },
    );
    let value = api::Value::create(
        &mut builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::StringValue,
            kind: Some(value_node.as_union_value()),
        },
    );
    let brand_path = builder.create_string("brand");
    let brand = api::QueryFieldRef::create(
        &mut builder,
        &api::QueryFieldRefArgs {
            path: Some(brand_path),
            element_index: None,
        },
    );
    let comparison = api::FilterComparison::create(
        &mut builder,
        &api::FilterComparisonArgs {
            field: Some(brand),
            op: api::ComparisonOp::Eq,
            value: Some(value),
        },
    );
    let filter = api::FilterExpr::create(
        &mut builder,
        &api::FilterExprArgs {
            expr_type: api::FilterExprKind::FilterComparison,
            expr: Some(comparison.as_union_value()),
        },
    );
    let structured = builder.create_vector(&[filter]);
    let filters = api::FilterRequest::create(
        &mut builder,
        &api::FilterRequestArgs {
            structured: Some(structured),
            ..Default::default()
        },
    );
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(query),
            filters: Some(filters),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let request = root::<api::SearchRequest>(builder.finished_data()).unwrap();
    let title = context
        .definition()
        .fields()
        .iter()
        .find(|field| field.name().as_str() == "title")
        .unwrap();
    assert_eq!(title.name().as_str(), "title");
    let converted = context.search_to_ir(request).unwrap();
    assert!(matches!(converted.query(), QueryIr::Text(text) if text == "headphones"));
    assert!(converted.filter().is_some());
    assert_eq!(converted.ranking_request().profile, "default");
}
