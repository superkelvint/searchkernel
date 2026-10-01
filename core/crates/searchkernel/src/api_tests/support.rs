use super::fixtures::*;
use super::*;
use searchkernel_ir::wire::v1::schema as ir_api;

pub(super) fn test_index() -> (IndexRuntime, Index, Arc<RecordingBackend>) {
    let backend = Arc::new(RecordingBackend::new());
    let definition = IndexDefinition::new()
        .field(Field::text("title").stored(true))
        .field(Field::float64("price"));
    let runtime =
        IndexRuntime::from_backend(definition, backend.clone()).expect("valid test index");
    let index = runtime.index();
    (runtime, index, backend)
}

pub(super) fn runtime_index(
    definition: IndexDefinition,
    backend: Arc<dyn Backend>,
) -> (IndexRuntime, Index) {
    let runtime = IndexRuntime::from_backend(definition, backend).expect("valid test index");
    let index = runtime.index();
    (runtime, index)
}

pub(super) fn index_from_canonical_ir(definition: searchkernel_ir::IndexDefinitionIr) -> Index {
    let context = IndexContext::from_definition_ir(definition);
    Index::from_owned_backend(
        context,
        BackendSession::from_backend(Arc::new(RecordingBackend::new())),
        None,
    )
    .expect("canonical schema should construct an index")
}

#[allow(clippy::too_many_arguments)]
pub(super) fn context_for(definition: &IndexDefinition) -> Arc<IndexContext> {
    let (_probe_runtime, probe) =
        runtime_index(definition.clone(), Arc::new(RecordingBackend::new()));
    Arc::clone(&probe.inner.context)
}

pub(super) fn portable_match_all_request(with_search_fields: bool) -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let match_all = ir_api::MatchAllQuery::create(&mut builder, &ir_api::MatchAllQueryArgs {});
    let source = ir_api::QuerySource::create(
        &mut builder,
        &ir_api::QuerySourceArgs {
            source_type: ir_api::QuerySourceKind::MatchAllQuery,
            source: Some(match_all.as_union_value()),
        },
    );
    let search_fields = if with_search_fields {
        let field = builder.create_string("title");
        let weight = ir_api::OptionalDouble::new(1.0);
        let search_field = ir_api::SearchField::create(
            &mut builder,
            &ir_api::SearchFieldArgs {
                field: Some(field),
                weight: Some(&weight),
            },
        );
        let fields = builder.create_vector(&[search_field]);
        Some(ir_api::SearchFieldSelection::create(
            &mut builder,
            &ir_api::SearchFieldSelectionArgs {
                fields: Some(fields),
            },
        ))
    } else {
        None
    };
    let request = ir_api::SearchRequest::create(
        &mut builder,
        &ir_api::SearchRequestArgs {
            query: Some(source),
            search_fields,
            ..Default::default()
        },
    );
    builder.finish(request, None);
    builder.finished_data().to_vec()
}

pub(super) fn portable_typed_contains_request(field_name: &str) -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let expression = portable_typed_contains_expression(&mut builder, field_name);
    finish_portable_typed_request(&mut builder, expression)
}

pub(super) fn portable_weak_and_request(target_hits: u32) -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let child = portable_typed_contains_expression(&mut builder, "title");
    let clauses = builder.create_vector(&[child]);
    let weak_and = ir_api::QueryWeakAnd::create(
        &mut builder,
        &ir_api::QueryWeakAndArgs {
            clauses: Some(clauses),
        },
    );
    let key = builder.create_string("totalTargetHits");
    let annotation_value = portable_value_int64(&mut builder, i64::from(target_hits));
    let annotation = ir_api::ValueEntry::create(
        &mut builder,
        &ir_api::ValueEntryArgs {
            key: Some(key),
            value: Some(annotation_value),
        },
    );
    let annotations = builder.create_vector(&[annotation]);
    let expression = ir_api::QueryExpr::create(
        &mut builder,
        &ir_api::QueryExprArgs {
            expr_type: ir_api::QueryExprKind::QueryWeakAnd,
            expr: Some(weak_and.as_union_value()),
            annotations: Some(annotations),
        },
    );
    finish_portable_typed_request(&mut builder, expression)
}

pub(super) fn portable_fuzzy_request(
    max_edit_distance: u32,
    prefix_length: u32,
    prefix: bool,
) -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let value = portable_value_string(&mut builder, "fone");
    let max_key = builder.create_string("maxEditDistance");
    let max_value = portable_value_int64(&mut builder, i64::from(max_edit_distance));
    let max_annotation = ir_api::ValueEntry::create(
        &mut builder,
        &ir_api::ValueEntryArgs {
            key: Some(max_key),
            value: Some(max_value),
        },
    );
    let prefix_length_key = builder.create_string("prefixLength");
    let prefix_length_value = portable_value_int64(&mut builder, i64::from(prefix_length));
    let prefix_length_annotation = ir_api::ValueEntry::create(
        &mut builder,
        &ir_api::ValueEntryArgs {
            key: Some(prefix_length_key),
            value: Some(prefix_length_value),
        },
    );
    let prefix_key = builder.create_string("prefix");
    let prefix_value = portable_value_bool(&mut builder, prefix);
    let prefix_annotation = ir_api::ValueEntry::create(
        &mut builder,
        &ir_api::ValueEntryArgs {
            key: Some(prefix_key),
            value: Some(prefix_value),
        },
    );
    let annotations =
        builder.create_vector(&[max_annotation, prefix_length_annotation, prefix_annotation]);
    let fuzzy =
        ir_api::QueryFuzzy::create(&mut builder, &ir_api::QueryFuzzyArgs { value: Some(value) });
    let fuzzy = ir_api::QueryExpr::create(
        &mut builder,
        &ir_api::QueryExprArgs {
            expr_type: ir_api::QueryExprKind::QueryFuzzy,
            expr: Some(fuzzy.as_union_value()),
            annotations: Some(annotations),
        },
    );
    let field = portable_field(&mut builder, "title");
    let contains = ir_api::QueryContains::create(
        &mut builder,
        &ir_api::QueryContainsArgs {
            field: Some(field),
            item: Some(fuzzy),
        },
    );
    let expression = ir_api::QueryExpr::create(
        &mut builder,
        &ir_api::QueryExprArgs {
            expr_type: ir_api::QueryExprKind::QueryContains,
            expr: Some(contains.as_union_value()),
            annotations: None,
        },
    );
    finish_portable_typed_request(&mut builder, expression)
}

pub(super) fn portable_near_request(distance: u32) -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let child = portable_typed_contains_expression(&mut builder, "title");
    let children = builder.create_vector(&[child]);
    let near = ir_api::QueryNear::create(
        &mut builder,
        &ir_api::QueryNearArgs {
            terms: Some(children),
        },
    );
    let key = builder.create_string("distance");
    let value = portable_value_int64(&mut builder, i64::from(distance));
    let annotation = ir_api::ValueEntry::create(
        &mut builder,
        &ir_api::ValueEntryArgs {
            key: Some(key),
            value: Some(value),
        },
    );
    let annotations = builder.create_vector(&[annotation]);
    let near = ir_api::QueryExpr::create(
        &mut builder,
        &ir_api::QueryExprArgs {
            expr_type: ir_api::QueryExprKind::QueryNear,
            expr: Some(near.as_union_value()),
            annotations: Some(annotations),
        },
    );
    let field = portable_field(&mut builder, "title");
    let contains = ir_api::QueryContains::create(
        &mut builder,
        &ir_api::QueryContainsArgs {
            field: Some(field),
            item: Some(near),
        },
    );
    let expression = ir_api::QueryExpr::create(
        &mut builder,
        &ir_api::QueryExprArgs {
            expr_type: ir_api::QueryExprKind::QueryContains,
            expr: Some(contains.as_union_value()),
            annotations: None,
        },
    );
    finish_portable_typed_request(&mut builder, expression)
}

pub(super) fn portable_text_request(text: Option<&str>) -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let text = text.map(|value| builder.create_string(value));
    let text_query = ir_api::SearchKernelTextQuery::create(
        &mut builder,
        &ir_api::SearchKernelTextQueryArgs { text },
    );
    let source = ir_api::QuerySource::create(
        &mut builder,
        &ir_api::QuerySourceArgs {
            source_type: ir_api::QuerySourceKind::SearchKernelTextQuery,
            source: Some(text_query.as_union_value()),
        },
    );
    let request = ir_api::SearchRequest::create(
        &mut builder,
        &ir_api::SearchRequestArgs {
            query: Some(source),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    builder.finished_data().to_vec()
}

pub(super) fn portable_text_request_with_empty_controls(
    ranking_profile: Option<&str>,
    language: Option<&str>,
    summary: Option<&str>,
) -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let text = builder.create_string("x");
    let text_query = ir_api::SearchKernelTextQuery::create(
        &mut builder,
        &ir_api::SearchKernelTextQueryArgs { text: Some(text) },
    );
    let query = ir_api::QuerySource::create(
        &mut builder,
        &ir_api::QuerySourceArgs {
            source_type: ir_api::QuerySourceKind::SearchKernelTextQuery,
            source: Some(text_query.as_union_value()),
        },
    );
    let ranking_profile = ranking_profile.map(|value| builder.create_string(value));
    let ranking = ranking_profile.map(|profile| {
        ir_api::RankingRequest::create(
            &mut builder,
            &ir_api::RankingRequestArgs {
                profile: Some(profile),
                ..Default::default()
            },
        )
    });
    let language = language.map(|value| builder.create_string(value));
    let query_model = language.map(|language| {
        ir_api::QueryModel::create(
            &mut builder,
            &ir_api::QueryModelArgs {
                language: Some(language),
                ..Default::default()
            },
        )
    });
    let summary = summary.map(|value| builder.create_string(value));
    let presentation = summary.map(|summary| {
        ir_api::PresentationRequest::create(
            &mut builder,
            &ir_api::PresentationRequestArgs {
                summary: Some(summary),
                ..Default::default()
            },
        )
    });
    let request = ir_api::SearchRequest::create(
        &mut builder,
        &ir_api::SearchRequestArgs {
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

pub(super) fn portable_grouping_request(attribute: &str) -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let attribute = builder.create_string(attribute);
    let attribute = ir_api::GroupingAttributeExpression::create(
        &mut builder,
        &ir_api::GroupingAttributeExpressionArgs {
            field: Some(attribute),
        },
    );
    let expression = ir_api::GroupingExpression::create(
        &mut builder,
        &ir_api::GroupingExpressionArgs {
            expr_type: ir_api::GroupingExpressionKind::GroupingAttributeExpression,
            expr: Some(attribute.as_union_value()),
        },
    );
    let group = ir_api::GroupingGroupOperation::create(
        &mut builder,
        &ir_api::GroupingGroupOperationArgs {
            expression: Some(expression),
        },
    );
    let group = ir_api::GroupingOperation::create(
        &mut builder,
        &ir_api::GroupingOperationArgs {
            operation_type: ir_api::GroupingOperationKind::GroupingGroupOperation,
            operation: Some(group.as_union_value()),
        },
    );
    let each = ir_api::GroupingEachOperation::create(
        &mut builder,
        &ir_api::GroupingEachOperationArgs {
            operations: None,
            label: None,
        },
    );
    let each = ir_api::GroupingOperation::create(
        &mut builder,
        &ir_api::GroupingOperationArgs {
            operation_type: ir_api::GroupingOperationKind::GroupingEachOperation,
            operation: Some(each.as_union_value()),
        },
    );
    let operations = builder.create_vector(&[group, each]);
    let root = ir_api::GroupingAllOperation::create(
        &mut builder,
        &ir_api::GroupingAllOperationArgs {
            operations: Some(operations),
            label: None,
        },
    );
    let structured = ir_api::StructuredGroupingSource::create(
        &mut builder,
        &ir_api::StructuredGroupingSourceArgs { root: Some(root) },
    );
    let grouping = ir_api::GroupingRequest::create(
        &mut builder,
        &ir_api::GroupingRequestArgs {
            source_type: ir_api::GroupingSourceKind::StructuredGroupingSource,
            source: Some(structured.as_union_value()),
            continuations: None,
        },
    );
    let grouping = builder.create_vector(&[grouping]);
    let match_all = ir_api::MatchAllQuery::create(&mut builder, &ir_api::MatchAllQueryArgs {});
    let source = ir_api::QuerySource::create(
        &mut builder,
        &ir_api::QuerySourceArgs {
            source_type: ir_api::QuerySourceKind::MatchAllQuery,
            source: Some(match_all.as_union_value()),
        },
    );
    let request = ir_api::SearchRequest::create(
        &mut builder,
        &ir_api::SearchRequestArgs {
            query: Some(source),
            grouping: Some(grouping),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    builder.finished_data().to_vec()
}

pub(super) fn portable_geo_nan_request() -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let field = portable_field(&mut builder, "title");
    let radius = builder.create_string("10 km");
    let geo = ir_api::QueryGeoLocation::create(
        &mut builder,
        &ir_api::QueryGeoLocationArgs {
            field: Some(field),
            latitude: f64::NAN,
            longitude: 1.0,
            radius: Some(radius),
        },
    );
    let expression = ir_api::QueryExpr::create(
        &mut builder,
        &ir_api::QueryExprArgs {
            expr_type: ir_api::QueryExprKind::QueryGeoLocation,
            expr: Some(geo.as_union_value()),
            annotations: None,
        },
    );
    finish_portable_typed_request(&mut builder, expression)
}

pub(super) fn expected_get(
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

pub(super) fn expected_search(
    context: &IndexContext,
    request: &SearchBuilder,
) -> searchkernel_ir::SearchRequestIr {
    searchkernel_ir::resolve_search(
        context,
        crate::search::to_resolver_input(request).expect("valid search intent"),
    )
    .expect("valid search request IR")
}

pub(super) fn portable_typed_request(
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

pub(super) fn typed_field(path: &str) -> searchkernel_ir::QueryFieldRefIr {
    searchkernel_ir::QueryFieldRefIr {
        path: searchkernel_ir::FieldName::parse(path.to_owned()).expect("valid test field"),
        element_index: None,
    }
}

pub(super) fn typed_contains(
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

pub(super) fn expected_analyze(
    context: &IndexContext,
    request: &AnalyzeBuilder,
) -> searchkernel_ir::AnalyzeRequestIr {
    searchkernel_ir::resolve_analyze(
        context,
        crate::analyze::to_resolver_input(request).expect("valid analyze intent"),
    )
    .expect("valid analyze request IR")
}

pub(super) fn scripted_definition() -> IndexDefinition {
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

pub(super) fn normalized_search(index: &Index) -> SearchBuilder {
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

pub(super) fn assert_rust_and_portable_schema_error_match(
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
    let portable = searchkernel_ir::__flatbuffers::root::<ir_api::IndexDefinition>(&bytes)
        .expect("portable invalid fixture should still decode structurally");
    let portable_error = searchkernel_ir::wire::v1::index_definition_to_ir(portable)
        .expect_err("portable schema ingress should reject the same semantic state");

    assert_eq!(rust_error.details()[0].code, expected_detail_code);
    assert_eq!(portable_error.details[0].code, expected_detail_code);
    assert_eq!(rust_error.details()[0].path, portable_error.details[0].path);
}

pub(super) fn invalid_schema_with_field(
    field: searchkernel_ir::FieldIr,
) -> searchkernel_ir::IndexDefinitionIr {
    searchkernel_ir::IndexDefinitionIr::new(None, vec![field], Vec::new())
        .expect("invalid field semantics are intentionally injected after construction")
}

use searchkernel_testkit::RecordingBackend;
#[cfg(feature = "native")]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(feature = "native")]
pub(super) fn temporary_path(label: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("searchkernel-index-test-{label}-{nonce}"))
}

#[cfg(feature = "native")]
pub(super) fn write_config(config: &std::path::Path) {
    std::fs::create_dir_all(config).unwrap();
    for name in [
        "searchkernel-schema.json",
        "indexschema.cfg",
        "attributes.cfg",
        "summary.cfg",
        "rank-profiles.cfg",
        "juniperrc.cfg",
        "proton.cfg",
    ] {
        std::fs::write(config.join(name), format!("{name}\n")).unwrap();
    }
}

#[cfg(feature = "native")]
pub(super) fn resolved_context(definition: &IndexDefinition) -> IndexContext {
    IndexContext::from_definition_ir(definition_to_ir(definition).unwrap())
}

#[cfg(feature = "native")]
pub(super) fn manifest_definition() -> IndexDefinition {
    IndexDefinition::new()
        .name("catalog")
        .field(
            Field::text("title")
                .multi()
                .stored(true)
                .searchable(true)
                .filterable(false)
                .sortable(false)
                .facetable(false)
                .analyzer(Analyzer::standard_v1()),
        )
        .field(
            Field::keyword("brand")
                .stored(false)
                .searchable(false)
                .filterable(true)
                .sortable(true)
                .facetable(true),
        )
        .field(Field::int64("quantity"))
        .field(Field::float64("price"))
        .field(Field::bool("enabled"))
        .default_search_fields([SearchField::new("title")])
}
