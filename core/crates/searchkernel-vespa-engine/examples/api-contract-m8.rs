use std::collections::BTreeMap;

use searchkernel_vespa_engine::{
    Engine, FieldValue, Hit, Query, Result, SearchRequest, SearchResult, SortField, SortOrder,
    SummaryRequest,
};

fn build_request() -> SearchRequest {
    SearchRequest::new(Query::Exists {
        field: "qty".into(),
    })
    .rank_profile("default")
    .limit(20)
    .offset(5)
    .sort(SortField::ascending("qty"))
    .sort(SortField::new("other_qty", SortOrder::Descending))
    .summary(SummaryRequest::Fields(vec!["qty".into()]))
}

fn build_from_m7_yql() -> Result<SearchRequest> {
    let query = Query::from_yql("select * from sources * where qty = 10")?;
    Ok(SearchRequest::new(query)
        .limit(5)
        .offset(1)
        .sort(SortField::ascending("qty"))
        .summary(SummaryRequest::Fields(vec!["qty".into()])))
}

fn inspect_result(result: SearchResult) {
    let _: u64 = result.total_hits;
    let hits: Vec<Hit> = result.hits;

    for hit in hits {
        let _: String = hit.document_id;
        let _: f64 = hit.score;
        let _: BTreeMap<String, FieldValue> = hit.fields;
    }
}

fn values() {
    let _: SummaryRequest = SummaryRequest::None;
    let _: SummaryRequest = SummaryRequest::Default;
    let _: SummaryRequest = SummaryRequest::Fields(vec!["title".into(), "qty".into()]);
}

fn execute(engine: &Engine) -> Result<()> {
    let result = engine.search(build_request())?;
    inspect_result(result);
    Ok(())
}

fn main() {
    let _ = build_request as fn() -> SearchRequest;
    let _ = values as fn();
    let _ = build_from_m7_yql as fn() -> Result<SearchRequest>;
    let _ = execute as fn(&Engine) -> Result<()>;
}
