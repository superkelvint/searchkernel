use searchkernel::{Field, Index, IndexDefinition, Sort};
use serde_json::{json, Value};

fn matching_ids(index: &Index) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    Ok(index
        .search("quickstart")
        .filter("kind = 'guide'")
        .sort(Sort::asc("score"))
        .execute::<Value>()?
        .hits
        .into_iter()
        .map(|hit| hit.id)
        .collect())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path =
        std::env::var("SEARCHKERNEL_QUICKSTART_PATH").unwrap_or_else(|_| "./search-rust".into());
    let definition = IndexDefinition::new()
        .field(Field::text("title").stored(true).searchable(true))
        .field(Field::keyword("kind").stored(true).filterable(true))
        .field(Field::int64("score").stored(true).sortable(true));
    let documents = [
        (
            "quickstart-a",
            json!({"title":"quickstart alpha","kind":"guide","score":2}),
        ),
        (
            "quickstart-b",
            json!({"title":"quickstart beta","kind":"guide","score":1}),
        ),
        (
            "quickstart-c",
            json!({"title":"quickstart other","kind":"other","score":0}),
        ),
    ];

    let index = Index::create(&path, definition)?;
    for (id, document) in documents {
        index.insert(id, &document)?;
    }
    assert_eq!(
        matching_ids(&index)?,
        vec!["quickstart-b".to_owned(), "quickstart-a".to_owned()]
    );
    index.close()?;

    let reopened = Index::open(&path)?;
    assert_eq!(
        reopened
            .get::<Value>("quickstart-b")?
            .and_then(|document| document.get("score").cloned()),
        Some(json!(1))
    );
    assert_eq!(
        matching_ids(&reopened)?,
        vec!["quickstart-b".to_owned(), "quickstart-a".to_owned()]
    );
    reopened.close()?;

    println!("Rust release quickstart PASS");
    Ok(())
}
