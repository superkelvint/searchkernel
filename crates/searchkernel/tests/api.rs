use searchkernel::{
    field, Error, Facet, FacetSort, Field, Index, IndexDefinition, Patch, Range, Ranking, Sort,
};
use searchkernel_core::{
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
        searchkernel_core::PrefixPolicy::Disabled,
        searchkernel_core::TypoPolicy::Disabled,
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
        .ranking(Ranking::Bm25)
        .projection(["title", "price"])
        .highlight(["title"])
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
}

#[test]
fn presence_and_explicit_zero_are_preserved() {
    let backend = Arc::new(RecordingBackend::new());
    let index = index_with(backend.clone());
    assert!(index
        .search("")
        .search_fields([])
        .projection([])
        .highlight([])
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
fn scripted_results_map_to_product_values_and_preserve_totals() {
    let expected = expected_search();
    let result = SearchResult {
        hits: vec![searchkernel_core::Hit {
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
            searchkernel_core::PortableError::new(
                searchkernel_core::ErrorCode::Unavailable,
                "down",
            ),
        ),
    ]));
    let index = index_with(backend);
    assert_eq!(index.get::<Product>("missing").unwrap(), None);
    let error = index.get::<Product>("missing").unwrap_err();
    assert!(matches!(error, Error::Portable(error) if !error.retryable && error.message == "down"));
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
            ScriptedResult::Insert(searchkernel_core::InsertResult {
                id: "inserted".into(),
            }),
        ),
        ExpectedCall::success(
            upsert,
            ScriptedResult::Upsert(searchkernel_core::UpsertResult {
                id: "upserted".into(),
            }),
        ),
        ExpectedCall::success(
            patch,
            ScriptedResult::Patch(searchkernel_core::PatchResult {
                id: "patched".into(),
            }),
        ),
        ExpectedCall::success(
            delete,
            ScriptedResult::Delete(searchkernel_core::DeleteResult {
                id: "deleted".into(),
                deleted: true,
            }),
        ),
        ExpectedCall::success(
            get,
            ScriptedResult::Get(GetResult {
                document: Some(searchkernel_core::DocumentResult {
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
