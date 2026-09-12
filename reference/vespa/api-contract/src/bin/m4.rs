use searchkernel_vespa::{Engine, TextDocument, TextQuery, TextSearchResult};

#[allow(dead_code)]
fn m4_surface(engine: &Engine) -> searchkernel_vespa::Result<()> {
    engine.put_text(TextDocument {
        id: "id:test:typea::compile-only",
        field: "title",
        text: "The quick brown fox",
    })?;

    engine.update_text(TextDocument {
        id: "id:test:typea::compile-only",
        field: "title",
        text: "The quick red fox",
    })?;

    let term: TextSearchResult = engine.search_text(
        TextQuery::Term {
            field: "title",
            text: "fox",
        },
        "default",
    )?;

    let phrase: TextSearchResult = engine.search_text(
        TextQuery::Phrase {
            field: "title",
            text: "quick red",
        },
        "bm25",
    )?;

    for hit in term.hits.iter().chain(phrase.hits.iter()) {
        let _: &str = &hit.document_id;
        let _: f64 = hit.score;
    }

    Ok(())
}

fn main() {}
