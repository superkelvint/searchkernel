use searchkernel_vespa_engine::{Engine, Query, RangeBound, Result, SearchRequest, SearchResult};

fn execute(engine: &Engine) -> Result<()> {
    let term = Query::Term {
        field: "title".into(),
        text: "running".into(),
    };
    let phrase = Query::Phrase {
        field: "title".into(),
        text: "running foxes".into(),
    };
    let boolean = Query::And(vec![
        term.clone(),
        Query::Or(vec![phrase.clone(), Query::Not(Box::new(term.clone()))]),
    ]);
    let equals = Query::Equals {
        field: "qty".into(),
        value: 10,
    };
    let range = Query::Range {
        field: "qty".into(),
        lower: RangeBound::Included(10),
        upper: RangeBound::Excluded(20),
    };
    let unbounded = Query::Range {
        field: "qty".into(),
        lower: RangeBound::Unbounded,
        upper: RangeBound::Included(20),
    };
    let exists = Query::Exists {
        field: "qty".into(),
    };

    let _: SearchResult = engine.search(SearchRequest::new(boolean))?;
    let _: SearchResult = engine.search(SearchRequest::new(equals))?;
    let _: SearchResult = engine.search(SearchRequest::new(range))?;
    let _: SearchResult = engine.search(SearchRequest::new(unbounded))?;
    let _: SearchResult = engine.search(SearchRequest::new(exists))?;

    let parsed = SearchRequest::from_yql(
        "select * from sources * where title contains \"fox\" and qty = 10",
    )?;
    let _: SearchResult = engine.search(parsed)?;
    Ok(())
}

fn main() {
    let _ = execute as fn(&Engine) -> Result<()>;
}
