use std::collections::BTreeMap;

use searchkernel_vespa_engine::{
    Engine, LocalHit, LocalSearchRequest, LocalSearchResult, Query, Result, SortField, SortOrder,
    SummaryRequest, Value,
};

fn build_request() -> LocalSearchRequest {
    LocalSearchRequest::new(Query::Exists {
        field: "qty".into(),
    })
    .rank_profile("default")
    .limit(20)
    .offset(5)
    .sort_by(SortField::ascending("qty"))
    .sort_by(SortField::new("other_qty", SortOrder::Descending))
    .summary(SummaryRequest::Fields(vec!["qty".into()]))
}

fn build_from_m7_yql() -> Result<LocalSearchRequest> {
    let query = Query::from_yql("select * from sources * where qty = 10")?;
    Ok(LocalSearchRequest::new(query)
        .limit(5)
        .offset(1)
        .sort_by(SortField::ascending("qty"))
        .summary(SummaryRequest::Fields(vec!["qty".into()])))
}

fn inspect_result(result: LocalSearchResult) {
    let _: u64 = result.total_hit_count;
    let hits: Vec<LocalHit> = result.hits;

    for hit in hits {
        let _: String = hit.document_id;
        let _: f64 = hit.score;
        let _: BTreeMap<String, Value> = hit.summary;
    }
}

fn values() {
    let _: Value = Value::String("sku-1".into());
    let _: Value = Value::Integer(42);
    let _: Value = Value::Float(12.5);
    let _: Value = Value::Boolean(true);

    let _: SummaryRequest = SummaryRequest::None;
    let _: SummaryRequest = SummaryRequest::Default;
    let _: SummaryRequest = SummaryRequest::Fields(vec!["title".into(), "qty".into()]);
}

fn execute(engine: &Engine) -> Result<()> {
    let result = engine.search_local(&build_request())?;
    inspect_result(result);
    Ok(())
}

fn main() {
    let _ = build_request as fn() -> LocalSearchRequest;
    let _ = values as fn();
    let _ = build_from_m7_yql as fn() -> Result<LocalSearchRequest>;
    let _ = execute as fn(&Engine) -> Result<()>;
}
