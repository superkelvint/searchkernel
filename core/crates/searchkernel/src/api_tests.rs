use super::*;
use crate::{field, Analyzer, Filter, Ranking, SearchBuilder, Sort};
use searchkernel_ir::{IndexContext, OperationIr};
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
