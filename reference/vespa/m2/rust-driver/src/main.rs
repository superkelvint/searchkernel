use searchkernel_vespa_engine::{Document, DocumentUpdate, Engine, Query, SearchRequest};
use std::path::PathBuf;
type Result<T> = std::result::Result<T, searchkernel_vespa_engine::Error>;

fn search_ids(engine: &Engine, value: i64) -> Result<Vec<String>> {
    let mut ids = engine.search(SearchRequest::new(Query::equals("attr1", value)).limit(10))?.hits
        .into_iter().map(|hit| hit.document_id).collect();
    ids.sort();
    Ok(ids)
}

fn json_string_array(values: &[String]) -> String {
    let body = values
        .iter()
        .map(|v| format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\"")))
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
    let mut engine = Engine::open(&data_dir, &config_dir)?;

    engine.put(Document::new("id:test:typea::1").field("attr1", 10_i64))?;
    engine.put(Document::new("id:test:typea::2").field("attr1", 20_i64))?;
    let s1 = search_ids(&engine, 10)?;
    engine.update(DocumentUpdate::new("id:test:typea::2").assign("attr1", 10_i64))?;
    let s2 = search_ids(&engine, 10)?;
    let removed = engine.delete("id:test:typea::1")?;
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
