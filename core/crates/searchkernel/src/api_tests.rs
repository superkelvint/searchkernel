use super::*;
use crate::{field, Analyzer, Filter, GroupingPlan, Query, Ranking, SearchBuilder, Sort, Tensor};
use searchkernel_ir::{IndexContext, OperationIr};
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
