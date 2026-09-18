use searchkernel::{
    field, DocumentId, Error, ErrorCode, Facet, FacetSort, Field, Filter, Index, IndexDefinition,
    Patch, Range, Ranking, Sort,
};
use searchkernel_ir::{
    Backend, FieldName, FilterIr, GetResult, HighlightingIr, OperationIr, ProjectionIr, QueryIr,
    RankingPolicyIr, SearchFieldIr, SearchFieldWeight, SearchRequestIr, SearchResult, TotalHits,
    TotalHitsRelation, Value,
};
use searchkernel_testkit::{ExpectedCall, RecordingBackend, ScriptedBackend, ScriptedResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct Product {
    title: String,
    brand: Option<String>,
    price: f64,
    in_stock: bool,
}

fn definition() -> IndexDefinition {
    IndexDefinition::new()
        .name("products")
        .field(Field::text("title"))
        .field(Field::keyword("brand"))
        .field(Field::float64("price"))
        .field(Field::bool("in_stock"))
}

fn index_with(backend: Arc<dyn Backend>) -> Index {
    Index::from_backend(definition(), backend).unwrap()
}

fn expected_search() -> SearchRequestIr {
    SearchRequestIr::new(
        QueryIr::MatchAll,
        vec![SearchFieldIr::new(
            FieldName::parse("title").unwrap(),
            SearchFieldWeight::new(1.0).unwrap(),
        )],
        None,
        vec![],
        RankingPolicyIr::DefaultRelevanceV1,
        20,
        0,
        ProjectionIr::new(vec![
            FieldName::parse("title").unwrap(),
            FieldName::parse("brand").unwrap(),
            FieldName::parse("price").unwrap(),
            FieldName::parse("in_stock").unwrap(),
        ]),
        HighlightingIr::Disabled,
        None,
        searchkernel_ir::PrefixPolicy::Disabled,
        searchkernel_ir::TypoPolicy::Disabled,
    )
}

#[test]
fn fluent_requests_cross_the_core_boundary() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Index>();

    let backend = Arc::new(RecordingBackend::new());
    let index = index_with(backend.clone());
    let product = Product {
        title: "wireless headphones".into(),
        brand: Some("Acme".into()),
        price: 449.0,
        in_stock: true,
    };

    assert!(index.upsert("sku-42", &product).is_err());
    let patch = Patch::new()
        .set("price", 450.0)
        .set("in_stock", true)
        .unset("brand");
    assert!(index.patch("sku-42", patch).is_err());
    assert!(index
        .search("wireless headphones")
        .filter(field("in_stock").eq(true))
        .filter(field("price").lte(500.0))
        .sort(Sort::desc("price").missing_last())
        .facet(Facet::new("brand").limit(20).sort(FacetSort::CountDesc))
        .ranking(Ranking::bm25())
        .projection(vec!["title".to_owned(), "price".to_owned()])
        .highlight(vec!["title".to_owned()])
        .language("en-US")
        .limit(20)
        .offset(3)
        .execute::<Product>()
        .is_err());

    let calls = backend.calls();
    assert!(matches!(calls[0], OperationIr::Upsert(_)));
    assert!(matches!(calls[1], OperationIr::Patch(_)));
    let OperationIr::Patch(request) = &calls[1] else {
        unreachable!()
    };
    assert_eq!(request.operations().len(), 3);
    let OperationIr::Search(request) = &calls[2] else {
        unreachable!()
    };
    assert_eq!(
        request.query(),
        &QueryIr::Text("wireless headphones".into())
    );
    assert_eq!(request.limit(), 20);
    assert_eq!(request.offset(), 3);
    assert!(matches!(request.filter(), Some(FilterIr::And(_))));
    let [grouping] = request.grouping() else {
        panic!("facet convenience must compile to one grouping request");
    };
    assert!(matches!(
        grouping.root.operations.as_slice(),
        [
            searchkernel_ir::GroupingOperationIr::Group(
                searchkernel_ir::GroupingExpressionIr::Attribute(field)
            ),
            searchkernel_ir::GroupingOperationIr::Each(_)
        ] if field == "brand"
    ));
}

#[test]
fn presence_and_explicit_zero_are_preserved() {
    let backend = Arc::new(RecordingBackend::new());
    let index = index_with(backend.clone());
    assert!(index
        .search("")
        .search_fields(std::iter::empty::<searchkernel::SearchField>())
        .projection(std::iter::empty::<&str>())
        .highlight(std::iter::empty::<&str>())
        .limit(0)
        .offset(0)
        .execute::<Product>()
        .is_err());
    let OperationIr::Search(request) = backend.last_call().unwrap() else {
        unreachable!()
    };
    assert!(request.search_fields().is_empty());
    assert_eq!(request.projection().fields(), &[]);
    assert!(
        matches!(request.highlighting(), HighlightingIr::Requested { fields } if fields.is_empty())
    );
    assert_eq!(request.limit(), 0);
    assert_eq!(request.offset(), 0);
}

#[test]
fn browse_is_an_explicit_match_all_search() {
    let backend = Arc::new(RecordingBackend::new());
    let index = index_with(backend.clone());

    assert!(index.browse().execute::<Product>().is_err());
    let OperationIr::Search(request) = backend.last_call().unwrap() else {
        unreachable!()
    };
    assert_eq!(request.query(), &QueryIr::MatchAll);
}

#[test]
fn projected_get_reaches_the_portable_projection_request() {
    let backend = Arc::new(RecordingBackend::new());
    let index = index_with(backend.clone());

    assert!(index
        .get_with_projection::<serde_json::Value>("sku-42", ["title", "brand"])
        .is_err());
    let OperationIr::Get(request) = backend.last_call().unwrap() else {
        unreachable!()
    };
    assert_eq!(
        request
            .projection()
            .fields()
            .iter()
            .map(|field| field.as_str())
            .collect::<Vec<_>>(),
        ["title", "brand"]
    );
}

#[test]
fn describe_and_inspect_expose_effective_request_defaults() {
    let backend = Arc::new(RecordingBackend::new());
    let index = index_with(backend);

    let definition = index.describe().unwrap();
    assert_eq!(definition.index_name(), Some("products"));
    assert_eq!(definition.fields().len(), 4);
    assert_eq!(
        definition.configured_default_search_fields().unwrap()[0].field(),
        "title"
    );

    let inspected = index
        .search("wireless")
        .filter(field("in_stock").eq(true))
        .filter(field("price").lte(500.0))
        .limit(7)
        .inspect()
        .unwrap();
    assert_eq!(inspected.query, "wireless");
    assert_eq!(inspected.limit, 7);
    assert_eq!(inspected.search_fields.len(), 1);
    assert_eq!(inspected.search_fields[0].field(), "title");
    assert!(inspected.filter.is_some());
}

#[test]
fn analyze_uses_the_selected_field_and_returns_tokens() {
    let definition = IndexDefinition::new()
        .field(Field::text("title").analyzer(searchkernel::Analyzer::standard_v1()));
    let recording = Arc::new(RecordingBackend::new());
    let recording_index = Index::from_backend(definition.clone(), recording.clone()).unwrap();
    assert!(recording_index
        .analyze("Running with the foxes")
        .field("title")
        .execute()
        .is_err());
    let expected = recording.last_call().unwrap();

    let backend = Arc::new(ScriptedBackend::new([ExpectedCall::success(
        expected,
        ScriptedResult::Analyze(searchkernel_ir::AnalyzeResult {
            tokens: vec![searchkernel_ir::AnalyzedToken {
                surface: "Running".into(),
                term: "run".into(),
                position: 0,
                start_byte: 0,
                end_byte: 7,
            }],
        }),
    )]));
    let index = Index::from_backend(definition, backend.clone()).unwrap();
    let result = index
        .analyze("Running with the foxes")
        .field("title")
        .execute()
        .unwrap();
    assert_eq!(result.tokens[0].term, "run");
    backend.finish().unwrap();
}

#[test]
fn scripted_results_map_to_product_values_and_preserve_totals() {
    let expected = expected_search();
    let result = SearchResult {
        hits: vec![searchkernel_ir::Hit {
            id: "sku-42".into(),
            score: Some(2.5),
            fields: BTreeMap::from([(String::from("title"), Value::String("headphones".into()))]),
            highlights: vec![],
        }],
        total_hits: TotalHits {
            value: 9,
            relation: TotalHitsRelation::LowerBound,
        },
        facets: vec![],
    };
    let backend = Arc::new(ScriptedBackend::new(vec![ExpectedCall::success(
        OperationIr::Search(expected),
        ScriptedResult::Search(result),
    )]));
    let index = index_with(backend.clone());
    let result = index.search("").execute::<serde_json::Value>().unwrap();
    assert_eq!(result.hits[0].id.as_str(), "sku-42");
    assert_eq!(result.hits[0].score, Some(2.5));
    assert_eq!(result.hits[0].document["title"], "headphones");
    assert_eq!(
        result.total_hits.relation,
        searchkernel::TotalHitsRelation::LowerBound
    );
    backend.finish().unwrap();
}

#[test]
fn get_missing_and_portable_errors_are_distinct() {
    let recording = Arc::new(RecordingBackend::new());
    let recording_index = index_with(recording.clone());
    assert!(recording_index.get::<Product>("missing").is_err());
    let request = recording.last_call().unwrap();
    let backend = Arc::new(ScriptedBackend::new(vec![
        ExpectedCall::success(
            request.clone(),
            ScriptedResult::Get(GetResult { document: None }),
        ),
        ExpectedCall::error(
            request,
            searchkernel_ir::PortableError::new(searchkernel_ir::ErrorCode::Unavailable, "down"),
        ),
    ]));
    let index = index_with(backend);
    assert_eq!(index.get::<Product>("missing").unwrap(), None);
    let error = index.get::<Product>("missing").unwrap_err();
    assert!(matches!(
        error,
        Error::Search {
            code: ErrorCode::Unavailable,
            retryable: false,
            message,
            ..
        } if message == "down"
    ));
}

#[test]
fn range_and_facets_compile_without_facade_validation() {
    let backend = Arc::new(RecordingBackend::new());
    let index = index_with(backend.clone());
    assert!(index
        .search("")
        .filter(field("price").range(Range::new().lower(100.0).upper(500.0).include_lower(false)))
        .facets(["brand", "in_stock"])
        .execute::<serde_json::Value>()
        .is_err());
    assert!(backend.last_call().is_some());
}

#[test]
fn scripted_crud_successes_are_consumed_and_mapped() {
    let recording = Arc::new(RecordingBackend::new());
    let recorder = index_with(recording.clone());
    let value = serde_json::json!({"title": "headphones"});
    assert!(recorder.insert("inserted", &value).is_err());
    let insert = recording.last_call().unwrap();
    assert!(recorder.upsert("upserted", &value).is_err());
    let upsert = recording.last_call().unwrap();
    assert!(recorder
        .patch("patched", Patch::new().unset("brand"))
        .is_err());
    let patch = recording.last_call().unwrap();
    assert!(recorder.delete("deleted").is_err());
    let delete = recording.last_call().unwrap();
    assert!(recorder.get::<serde_json::Value>("fetched").is_err());
    let get = recording.last_call().unwrap();

    let backend = Arc::new(ScriptedBackend::new(vec![
        ExpectedCall::success(
            insert,
            ScriptedResult::Insert(searchkernel_ir::InsertResult {
                id: "inserted".into(),
            }),
        ),
        ExpectedCall::success(
            upsert,
            ScriptedResult::Upsert(searchkernel_ir::UpsertResult {
                id: "upserted".into(),
            }),
        ),
        ExpectedCall::success(
            patch,
            ScriptedResult::Patch(searchkernel_ir::PatchResult {
                id: "patched".into(),
            }),
        ),
        ExpectedCall::success(
            delete,
            ScriptedResult::Delete(searchkernel_ir::DeleteResult {
                id: "deleted".into(),
                deleted: true,
            }),
        ),
        ExpectedCall::success(
            get,
            ScriptedResult::Get(GetResult {
                document: Some(searchkernel_ir::DocumentResult {
                    id: "fetched".into(),
                    fields: BTreeMap::from([(
                        String::from("title"),
                        Value::String("headphones".into()),
                    )]),
                }),
            }),
        ),
    ]));
    let index = index_with(backend.clone());
    assert!(index.insert("inserted", &value).is_ok());
    assert!(index.upsert("upserted", &value).is_ok());
    assert!(index.patch("patched", Patch::new().unset("brand")).is_ok());
    assert!(index.delete("deleted").unwrap());
    assert_eq!(
        index.get::<serde_json::Value>("fetched").unwrap().unwrap()["title"],
        "headphones"
    );
    backend.finish().unwrap();
}

#[test]
fn result_variants_map_to_owned_product_values() {
    let expected = expected_search();
    let nested = BTreeMap::from([(String::from("source"), Value::String("catalog".into()))]);
    let result = SearchResult {
        hits: vec![searchkernel_ir::Hit {
            id: "sku-43".into(),
            score: None,
            fields: BTreeMap::from([
                (String::from("title"), Value::String("headphones".into())),
                (String::from("nullable"), Value::Null),
                (String::from("signed"), Value::Int64(-7)),
                (String::from("unsigned"), Value::UInt64(u64::MAX)),
                (String::from("ratio"), Value::Float64(1.25)),
                (String::from("bytes"), Value::Bytes(vec![1, 2, 255])),
                (
                    String::from("list"),
                    Value::List(vec![Value::Bool(true), Value::String("nested".into())]),
                ),
                (String::from("object"), Value::Object(nested)),
            ]),
            highlights: vec![searchkernel_ir::HighlightResult {
                field: "title".into(),
                fragments: vec![searchkernel_ir::HighlightFragment {
                    text: "<em>headphones</em>".into(),
                    matches: vec![searchkernel_ir::HighlightSpan {
                        start_byte: 0,
                        end_byte: 18,
                    }],
                }],
            }],
        }],
        total_hits: TotalHits {
            value: 1,
            relation: TotalHitsRelation::Exact,
        },
        facets: vec![searchkernel_ir::FacetResult {
            field: "brand".into(),
            values: vec![
                searchkernel_ir::FacetValue {
                    value: Value::String("Acme".into()),
                    count: 1,
                },
                searchkernel_ir::FacetValue {
                    value: Value::UInt64(2),
                    count: 2,
                },
            ],
        }],
    };
    let backend = Arc::new(ScriptedBackend::new(vec![ExpectedCall::success(
        OperationIr::Search(expected),
        ScriptedResult::Search(result),
    )]));
    let index = index_with(backend.clone());

    let result = index.search("").execute::<serde_json::Value>().unwrap();
    assert_eq!(result.hits[0].score, None);
    assert_eq!(result.hits[0].document["signed"], -7);
    assert_eq!(result.hits[0].document["unsigned"], u64::MAX);
    assert_eq!(
        result.hits[0].document["bytes"],
        serde_json::json!([1, 2, 255])
    );
    assert_eq!(result.hits[0].document["object"]["source"], "catalog");
    assert_eq!(result.hits[0].highlights[0].field, "title");
    assert_eq!(
        result.hits[0].highlights[0].fragments[0].matches[0].start_byte,
        0
    );
    assert_eq!(result.facets[0].values[1].value, serde_json::json!(2));
    assert_eq!(
        result.total_hits.relation,
        searchkernel::TotalHitsRelation::Exact
    );
    backend.finish().unwrap();
}

#[test]
fn all_filter_operators_and_search_options_cross_the_boundary() {
    let backend = Arc::new(RecordingBackend::new());
    let definition = IndexDefinition::new()
        .field(Field::text("title"))
        .field(Field::keyword("brand"))
        .field(Field::float64("price"))
        .field(Field::bool("in_stock"))
        .field(Field::keyword("category"));
    let index = Index::from_backend(definition, backend.clone()).unwrap();
    let filters = Filter::any([
        Filter::all([
            field("brand").eq("Acme"),
            field("brand").ne("Other"),
            field("price").lt(500.0),
            field("price").lte(500.0),
            field("price").gt(100.0),
            field("price").gte(100.0),
        ]),
        !field("brand").exists(),
        field("price").range(Range::new().lower(100.0).upper(500.0).include_upper(true)),
    ]);

    assert!(index
        .search("headphones")
        .search_fields([searchkernel::SearchField::new("title").weight(0.0)])
        .filter(filters)
        .sort(Sort::asc("price").missing_first())
        .sort(Sort::desc("brand").missing_last())
        .ranking(Ranking::default_relevance_v1())
        .projection(["title"])
        .highlight(["title"])
        .language("EN-us")
        .execute::<serde_json::Value>()
        .is_err());

    let OperationIr::Search(request) = backend.last_call().unwrap() else {
        unreachable!()
    };
    assert_eq!(request.search_fields()[0].weight().get(), 0.0);
    assert_eq!(request.sort().len(), 2);
    assert!(request.grouping().is_empty());
    assert_eq!(request.ranking(), RankingPolicyIr::DefaultRelevanceV1);
    assert_eq!(request.language().unwrap().as_str(), "en-us");
    let Some(FilterIr::Or(filters)) = request.filter() else {
        panic!("expected OR filter");
    };
    assert_eq!(filters.len(), 3);
    assert!(matches!(filters[0], FilterIr::And(ref clauses) if clauses.len() == 6));
    assert!(matches!(filters[1], FilterIr::Not(_)));
    assert!(
        matches!(filters[2], FilterIr::Range(ref range) if range.upper().is_some() && range.include_upper())
    );
}

#[test]
fn simple_search_field_names_cross_the_boundary() {
    let backend = Arc::new(RecordingBackend::new());
    let index = Index::from_backend(
        definition().field(Field::text("description")),
        backend.clone(),
    )
    .unwrap();

    assert!(index
        .search("headphones")
        .search_fields(["title", "description"])
        .execute::<Product>()
        .is_err());

    let OperationIr::Search(request) = backend.last_call().unwrap() else {
        unreachable!()
    };
    assert_eq!(
        request
            .search_fields()
            .iter()
            .map(|field| field.field().as_str())
            .collect::<Vec<_>>(),
        ["title", "description"]
    );
    assert!(request
        .search_fields()
        .iter()
        .all(|field| field.weight().get() == 1.0));
}

#[test]
fn portable_values_and_serialization_errors_are_preserved() {
    let definition = IndexDefinition::new()
        .field(Field::text("title"))
        .field(Field::keyword("tags").multi())
        .field(Field::int64("signed"))
        .field(Field::float64("ratio"))
        .field(Field::bool("enabled"));
    let backend = Arc::new(RecordingBackend::new());
    let index = Index::from_backend(definition, backend).unwrap();
    let document = serde_json::json!({
        "title": "headphones",
        "tags": ["audio", "sale"],
        "signed": -7,
        "ratio": 1.25,
        "enabled": true,
        "nullable": null,
        "unsigned": u64::MAX,
        "nested": {"source": "catalog"}
    });
    let error = index.insert("sku-44", &document).unwrap_err();
    assert!(matches!(error, Error::Search { .. }));

    let non_object = serde_json::json!(["not", "a", "document"]);
    let error = index.upsert("sku-45", &non_object).unwrap_err();
    assert!(matches!(error, Error::Serialization(ref message) if message.contains("object")));
    assert!(error.to_string().contains("document serialization failed"));
}

#[test]
fn result_conversion_reports_bad_documents_and_non_finite_numbers() {
    let expected = expected_search();
    let bad_document = SearchResult {
        hits: vec![searchkernel_ir::Hit {
            id: "bad".into(),
            score: Some(1.0),
            fields: BTreeMap::from([(String::from("title"), Value::String("ok".into()))]),
            highlights: vec![],
        }],
        total_hits: TotalHits {
            value: 1,
            relation: TotalHitsRelation::Exact,
        },
        facets: vec![],
    };
    let backend = Arc::new(ScriptedBackend::new(vec![
        ExpectedCall::success(
            OperationIr::Search(expected.clone()),
            ScriptedResult::Search(bad_document),
        ),
        ExpectedCall::success(
            OperationIr::Search(expected),
            ScriptedResult::Search(SearchResult {
                hits: vec![],
                total_hits: TotalHits {
                    value: 0,
                    relation: TotalHitsRelation::Exact,
                },
                facets: vec![searchkernel_ir::FacetResult {
                    field: "ratio".into(),
                    values: vec![searchkernel_ir::FacetValue {
                        value: Value::Float64(f64::NAN),
                        count: 1,
                    }],
                }],
            }),
        ),
    ]));
    let index = index_with(backend.clone());
    let error = index.search("").execute::<String>().unwrap_err();
    assert!(matches!(error, Error::Deserialization(_)));
    let error = index.search("").execute::<serde_json::Value>().unwrap_err();
    assert!(matches!(error, Error::Infrastructure(message) if message.contains("non-finite")));
    backend.finish().unwrap();
}

#[test]
fn backend_errors_and_false_deletes_are_returned_without_reinterpretation() {
    let recording = Arc::new(RecordingBackend::new());
    let recorder = index_with(recording.clone());
    let value = serde_json::json!({"title": "headphones"});
    assert!(recorder.insert("insert", &value).is_err());
    assert!(recorder.upsert("upsert", &value).is_err());
    assert!(recorder
        .patch("patch", Patch::new().unset("brand"))
        .is_err());
    assert!(recorder.delete("delete").is_err());
    assert!(recorder.search("").execute::<serde_json::Value>().is_err());
    let calls = recording.calls();
    let error = searchkernel_ir::PortableError::new(
        searchkernel_ir::ErrorCode::Unavailable,
        "backend unavailable",
    );
    let backend = Arc::new(ScriptedBackend::new(
        calls
            .into_iter()
            .map(|request| ExpectedCall::error(request, error.clone()))
            .collect::<Vec<_>>(),
    ));
    let index = index_with(backend.clone());
    assert!(matches!(
        index.insert("insert", &value),
        Err(Error::Search { .. })
    ));
    assert!(matches!(
        index.upsert("upsert", &value),
        Err(Error::Search { .. })
    ));
    assert!(matches!(
        index.patch("patch", Patch::new().unset("brand")),
        Err(Error::Search { .. })
    ));
    assert!(matches!(index.delete("delete"), Err(Error::Search { .. })));
    assert!(matches!(
        index.search("").execute::<serde_json::Value>(),
        Err(Error::Search { .. })
    ));
    backend.finish().unwrap();
}

#[test]
fn public_helper_types_and_errors_have_stable_behavior() {
    let text = String::from("sku-46");
    let from_string = DocumentId::from(text.clone());
    let from_reference = DocumentId::from(&text);
    let from_id = DocumentId::from(&from_string);
    assert_eq!(from_string.as_str(), "sku-46");
    assert_eq!(from_reference.as_ref(), "sku-46");
    assert_eq!(from_id.to_string(), "sku-46");
    assert_eq!(format!("{from_id}"), "sku-46");

    let facet: Facet = text.into();
    assert_eq!(facet, Facet::new("sku-46"));
    assert_eq!(Range::default(), Range::new());

    let index = index_with(Arc::new(RecordingBackend::new()));
    assert!(format!("{:?}", index.search("headphones")).contains("headphones"));

    let search = Error::search(ErrorCode::Unavailable, "unavailable");
    assert_eq!(search.code(), ErrorCode::Unavailable);
    assert!(!search.retryable());
    assert!(search.details().is_empty());
    assert_eq!(search.to_string(), "unavailable");
    assert!(std::error::Error::source(&search).is_none());
    let serialization = Error::Serialization("bad document".into());
    assert_eq!(serialization.code(), ErrorCode::InvalidArgument);
    assert!(serialization.details().is_empty());
    assert!(serialization.to_string().contains("bad document"));
    assert_eq!(
        Error::Deserialization("bad result".into()).to_string(),
        "document deserialization failed: bad result"
    );
    assert_eq!(Error::Infrastructure("broken".into()).to_string(), "broken");
}
