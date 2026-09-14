use searchkernel_vespa::{Engine, Query, RangeBound, Result, TextSearchResult};

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
    let exists = Query::Exists { field: "qty".into() };

    let _: TextSearchResult = engine.search_query(&boolean, "default")?;
    let _: TextSearchResult = engine.search_query(&equals, "default")?;
    let _: TextSearchResult = engine.search_query(&range, "default")?;
    let _: TextSearchResult = engine.search_query(&unbounded, "default")?;
    let _: TextSearchResult = engine.search_query(&exists, "default")?;

    let parsed = Query::from_yql(
        "select * from sources * where title contains \"fox\" and qty = 10",
    )?;
    let _: TextSearchResult = engine.search_query(&parsed, "default")?;
    let _: TextSearchResult = engine.search_yql(
        "select * from sources * where title contains \"fox\"",
        "default",
    )?;
    Ok(())
}

fn main() {
    let _ = execute as fn(&Engine) -> Result<()>;
}
