use searchkernel::{Field, Index, IndexDefinition};
use serde_json::{json, Value};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path =
        std::env::var("SEARCHKERNEL_QUICKSTART_PATH").unwrap_or_else(|_| "./search-rust".into());
    let definition = IndexDefinition::new()
        .field(Field::text("title").stored(true).searchable(true))
        .field(Field::keyword("kind").stored(true).filterable(true))
        .field(Field::int64("score").stored(true).sortable(true));

    let index = Index::create(&path, definition)?;
    index.insert(
        "quickstart-rust",
        &json!({"title":"quickstart example","kind":"quickstart","score":1}),
    )?;
    let result = index.search("quickstart").execute::<Value>()?;
    assert_eq!(
        result.hits.first().map(|hit| hit.id.as_str()),
        Some("quickstart-rust")
    );
    index.close()?;

    let reopened = Index::open(&path)?;
    assert!(reopened.get::<Value>("quickstart-rust")?.is_some());
    reopened.close()?;

    println!("Rust release quickstart PASS");
    Ok(())
}
