use searchkernel_vespa_engine::{Document, Engine, Query, SearchRequest, SearchResult};

#[allow(dead_code)]
fn m4_surface(engine: &Engine) -> searchkernel_vespa_engine::Result<()> {
    engine
        .put(Document::new("id:test:typea::compile-only").field("title", "The quick brown fox"))?;

    engine.update(
        searchkernel_vespa_engine::DocumentUpdate::new("id:test:typea::compile-only")
            .assign("title", "The quick red fox"),
    )?;

    let term: SearchResult = engine.search(SearchRequest::new(Query::term("title", "fox")))?;

    let phrase: SearchResult = engine
        .search(SearchRequest::new(Query::phrase("title", "quick red")).rank_profile("bm25"))?;

    for hit in term.hits.iter().chain(phrase.hits.iter()) {
        let _: &str = &hit.document_id;
        let _: f64 = hit.score;
    }

    Ok(())
}

fn main() {}
