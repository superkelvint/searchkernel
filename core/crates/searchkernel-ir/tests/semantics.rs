use searchkernel_ir::{api, *};
use std::collections::BTreeMap;

fn index_context() -> IndexContext {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let name = builder.create_string("title");
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
    let stored = api::OptionalBool::new(true);
    let searchable = api::OptionalBool::new(true);
    let analyzer_name = builder.create_string("standard");
    let analyzer = api::Analyzer::create(
        &mut builder,
        &api::AnalyzerArgs {
            name: Some(analyzer_name),
            version: 1,
            ..Default::default()
        },
    );
    let field = api::FieldDefinition::create(
        &mut builder,
        &api::FieldDefinitionArgs {
            name: Some(name),
            data_type: Some(data_type),
            stored: Some(&stored),
            searchable: Some(&searchable),
            analyzer: Some(analyzer),
            ..Default::default()
        },
    );
    let brand_name = builder.create_string("brand");
    let filterable = api::OptionalBool::new(true);
    let sortable = api::OptionalBool::new(true);
    let matching = api::MatchingConfig::create(
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
            data_type: Some(data_type),
            stored: Some(&stored),
            filterable: Some(&filterable),
            sortable: Some(&sortable),
            matching: Some(matching),
            ..Default::default()
        },
    );
    let fields = builder.create_vector(&[field, brand]);
    let title = builder.create_string("title");
    let weight = api::OptionalDouble::new(1.0);
    let search_field = api::SearchField::create(
        &mut builder,
        &api::SearchFieldArgs {
            field: Some(title),
            weight: Some(&weight),
        },
    );
    let search_fields = builder.create_vector(&[search_field]);
    let default_search_fields = api::SearchFieldSelection::create(
        &mut builder,
        &api::SearchFieldSelectionArgs {
            fields: Some(search_fields),
        },
    );
    let definition = api::IndexDefinition::create(
        &mut builder,
        &api::IndexDefinitionArgs {
            fields: Some(fields),
            default_search_fields: Some(default_search_fields),
            ..Default::default()
        },
    );
    builder.finish(definition, None);
    let definition =
        searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(builder.finished_data())
            .expect("valid V2 index definition");
    IndexContext::from_api(definition).expect("valid index context")
}

fn text_query<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    text: &str,
) -> flatbuffers::WIPOffset<api::QuerySource<'a>> {
    let text = builder.create_string(text);
    let query = api::SearchKernelTextQuery::create(
        builder,
        &api::SearchKernelTextQueryArgs { text: Some(text) },
    );
    api::QuerySource::create(
        builder,
        &api::QuerySourceArgs {
            source_type: api::QuerySourceKind::SearchKernelTextQuery,
            source: Some(query.as_union_value()),
        },
    )
}

fn standalone_value<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    value: &str,
) -> flatbuffers::WIPOffset<api::Value<'a>> {
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

#[test]
fn v2_index_definition_and_text_search_convert_to_ir() {
    let context = index_context();
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let query = text_query(&mut builder, "headphones");
    let limit = api::OptionalUInt32::new(7);
    let offset = api::OptionalUInt32::new(2);
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(query),
            limit: Some(&limit),
            offset: Some(&offset),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let request =
        searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(builder.finished_data())
            .expect("valid V2 search request");
    let converted = context.search_to_ir(request).expect("search converts");
    assert_eq!(converted.query(), &QueryIr::Text("headphones".to_owned()));
    assert_eq!(converted.limit(), 7);
    assert_eq!(converted.offset(), 2);
    assert_eq!(converted.search_fields().len(), 1);
    assert_eq!(context.describe().fields().len(), 2);
}

#[test]
fn v2_structured_filter_sort_and_ranking_convert_to_ir() {
    let context = index_context();
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let query = text_query(&mut builder, "headphones");
    let field_path = builder.create_string("brand");
    let field = api::QueryFieldRef::create(
        &mut builder,
        &api::QueryFieldRefArgs {
            path: Some(field_path),
            element_index: None,
        },
    );
    let value = standalone_value(&mut builder, "headphones");
    let comparison = api::FilterComparison::create(
        &mut builder,
        &api::FilterComparisonArgs {
            field: Some(field),
            op: api::ComparisonOp::Eq,
            value: Some(value),
        },
    );
    let expression = api::FilterExpr::create(
        &mut builder,
        &api::FilterExprArgs {
            expr_type: api::FilterExprKind::FilterComparison,
            expr: Some(comparison.as_union_value()),
        },
    );
    let filters = builder.create_vector(&[expression]);
    let filter_request = api::FilterRequest::create(
        &mut builder,
        &api::FilterRequestArgs {
            structured: Some(filters),
            ..Default::default()
        },
    );
    let field = builder.create_string("brand");
    let field_target = api::FieldSortTarget::create(
        &mut builder,
        &api::FieldSortTargetArgs { field: Some(field) },
    );
    let target = api::SortTarget::create(
        &mut builder,
        &api::SortTargetArgs {
            target_type: api::SortTargetKind::FieldSortTarget,
            target: Some(field_target.as_union_value()),
        },
    );
    let sort = api::SortExpr::create(
        &mut builder,
        &api::SortExprArgs {
            target: Some(target),
            direction: api::SortDirection::Desc,
            ..Default::default()
        },
    );
    let sorts = builder.create_vector(&[sort]);
    let profile = builder.create_string("bm25");
    let ranking = api::RankingRequest::create(
        &mut builder,
        &api::RankingRequestArgs {
            profile: Some(profile),
            ..Default::default()
        },
    );
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(query),
            filters: Some(filter_request),
            sort: Some(sorts),
            ranking: Some(ranking),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let request =
        searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(builder.finished_data())
            .expect("valid V2 search request");
    let converted = context.search_to_ir(request).expect("search converts");
    assert!(converted.filter().is_some());
    assert_eq!(converted.sort().len(), 1);
    assert_eq!(converted.ranking(), RankingPolicyIr::Bm25);
}

#[test]
fn v2_search_result_encoding_uses_result_tree_and_relevance() {
    let result = SearchResult {
        hits: vec![Hit {
            id: "doc-1".to_owned(),
            score: Some(3.5),
            fields: BTreeMap::new(),
            highlights: Vec::new(),
        }],
        total_hits: TotalHits {
            value: 1,
            relation: TotalHitsRelation::Exact,
        },
        facets: Vec::new(),
    };
    let bytes = search_result_to_api(&result);
    let encoded = searchkernel_ir::__flatbuffers::root::<api::SearchResult>(&bytes)
        .expect("valid V2 search result");
    let root = encoded.root().expect("result root");
    let group = root.node_as_result_group().expect("result group root");
    let hit = group
        .children()
        .expect("result children")
        .get(0)
        .node_as_hit()
        .expect("hit node");
    assert_eq!(hit.id(), Some("doc-1"));
    assert_eq!(
        hit.relevance()
            .and_then(|relevance| relevance.value_as_finite_relevance())
            .map(|relevance| relevance.value()),
        Some(3.5)
    );
}

#[test]
fn v2_error_encoding_remains_portable() {
    let error = PortableError::new(ErrorCode::InvalidArgument, "bad request");
    let bytes = error_to_api(&error);
    let encoded =
        searchkernel_ir::__flatbuffers::root::<api::Error>(&bytes).expect("valid V2 error");
    assert_eq!(encoded.code(), api::ErrorCode::InvalidArgument);
    assert_eq!(encoded.message(), Some("bad request"));
}
