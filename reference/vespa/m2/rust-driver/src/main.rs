use searchkernel::{
    Document, DocumentId, DocumentUpdate, Engine, EngineConfig, Query, Result,
    SearchRequest, Value,
};
use std::path::PathBuf;

fn search_ids(engine: &Engine, value: i32) -> Result<Vec<String>> {
    let request = SearchRequest::new(Query::term("attr1", Value::I32(value))).limit(10);
    let result = engine.search(request)?;
    let mut ids: Vec<String> = result
        .hits()
        .iter()
        .map(|hit| hit.id().as_str().to_owned())
        .collect();
    ids.sort();
    Ok(ids)
}

fn json_string_array(values: &[String]) -> String {
    let body = values
        .iter()
        .map(|v| format!("\"{}\"", v.replace('\\', "\\\\").replace('\"', "\\\"")))
        .collect::<Vec<_>>()
        .join(",");
    format!("[{body}]")
}

fn run() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let data_dir = PathBuf::from(args.next().expect("DATA_DIR argument"));
    let config_dir = PathBuf::from(args.next().expect("CONFIG_DIR argument"));
    assert!(args.next().is_none(), "unexpected extra arguments");

    let _ = std::fs::remove_dir_all(&data_dir);

    let config = EngineConfig::from_directory(&config_dir)?;
    let mut engine = Engine::open(&data_dir, config)?;

    let doc1 = DocumentId::parse("id:test:typea::1")?;
    let doc2 = DocumentId::parse("id:test:typea::2")?;

    let mut d1 = Document::new(doc1.clone());
    d1.set_field("attr1", Value::I32(10));
    engine.put(d1)?;

    let mut d2 = Document::new(doc2.clone());
    d2.set_field("attr1", Value::I32(20));
    engine.put(d2)?;

    let s1 = search_ids(&engine, 10)?;

    let mut update = DocumentUpdate::new(doc2.clone());
    update.assign("attr1", Value::I32(10));
    engine.update(update)?;

    let s2 = search_ids(&engine, 10)?;
    let removed = engine.remove(&doc1)?;
    let s3 = search_ids(&engine, 10)?;
    let s4 = search_ids(&engine, 20)?;

    println!(
        concat!(
            "{{\"scenario\":\"m2-crud-attribute-search\",\"steps\":[",
            "{{\"name\":\"after_put_search_10\",\"hits\":{}}},",
            "{{\"name\":\"after_update_search_10\",\"hits\":{}}},",
            "{{\"name\":\"remove_doc1\",\"was_found\":{}}},",
            "{{\"name\":\"after_remove_search_10\",\"hits\":{}}},",
            "{{\"name\":\"after_update_search_old_20\",\"hits\":{}}}",
            "]}}"
        ),
        json_string_array(&s1),
        json_string_array(&s2),
        if removed { "true" } else { "false" },
        json_string_array(&s3),
        json_string_array(&s4),
    );

    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
