use searchkernel::{Field, Index, IndexDefinition, Query};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn index_path() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "static-searchkernel-smoke-{}-{nonce}",
        std::process::id()
    ))
}

fn ids(result: &searchkernel::SearchResult<Value>) -> Vec<String> {
    result.hits.iter().map(|hit| hit.id.to_string()).collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = index_path();
    let definition = IndexDefinition::new()
        .field(Field::text("title").stored(true))
        .field(Field::text("body").stored(true))
        .field(Field::int64("price").stored(true).filterable(true));

    let index = Index::create(&path, definition)?;
    index.upsert(
        "one",
        &json!({
            "title": "Quick brown fox",
            "body": "A quick brown fox jumps",
            "price": 10_i64,
        }),
    )?;
    index.upsert(
        "two",
        &json!({
            "title": "Quick red fox",
            "body": "A quick red fox waits",
            "price": 20_i64,
        }),
    )?;

    let term = index.search("brown").execute::<Value>()?;
    assert_eq!(ids(&term), ["one"]);
    assert_eq!(term.hits[0].document["price"], json!(10_i64));

    let phrase = index
        .browse()
        .typed_query(Query::phrase("body", "quick brown"))
        .execute::<Value>()?;
    assert_eq!(ids(&phrase), ["one"]);
    assert_eq!(phrase.hits[0].document["title"], json!("Quick brown fox"));

    index.close()?;
    let reopened = Index::open(&path)?;
    let after_reopen = reopened.search("red").execute::<Value>()?;
    assert_eq!(ids(&after_reopen), ["two"]);
    reopened.close()?;
    std::fs::remove_dir_all(path)?;
    println!("static-searchkernel smoke PASS");
    Ok(())
}
