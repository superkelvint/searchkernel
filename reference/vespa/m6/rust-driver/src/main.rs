use searchkernel_vespa_engine::{ConfigCompiler, Engine, Field, FieldType, Schema, TextDocument, TextQuery};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Deserialize)]
struct Scenario {
    schema: String,
    fields: Vec<FieldSpec>,
    smoke: Smoke,
}

#[derive(Deserialize)]
struct FieldSpec {
    name: String,
    #[serde(rename = "type")]
    kind: String,
    index: bool,
    attribute: bool,
    summary: bool,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Smoke {
    Open,
    Text { field: String, text: String, query: String },
    Integer { field: String, value: i32 },
}

fn load(path: &str) -> Scenario {
    serde_json::from_str(&fs::read_to_string(path).expect("read scenario")).expect("parse scenario")
}

fn build_schema(s: &Scenario) -> Schema {
    let mut schema = Schema::new(&s.schema);
    for f in &s.fields {
        let kind = match f.kind.as_str() {
            "text" => FieldType::Text,
            "string" => FieldType::String,
            "integer" => FieldType::Integer,
            "float" => FieldType::Float,
            "boolean" => FieldType::Boolean,
            other => panic!("unknown field type {other}"),
        };
        let mut field = Field::new(&f.name, kind);
        if f.index { field = field.indexed(); }
        if f.attribute { field = field.attribute(); }
        if f.summary { field = field.summary(); }
        schema = schema.field(field);
    }
    schema
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("usage: m6-driver compile SCENARIO CONFIG_DIR | smoke SCENARIO CONFIG_DIR DATA_DIR");
        std::process::exit(2);
    }
    let scenario = load(&args[2]);
    let config_dir = PathBuf::from(&args[3]);

    match args[1].as_str() {
        "compile" => {
            let schema = build_schema(&scenario);
            ConfigCompiler::default()
                .compile_to(&schema, &config_dir)
                .expect("compile schema");
        }
        "smoke" => {
            if args.len() != 5 { panic!("smoke needs DATA_DIR"); }
            // Deliberately do NOT reconstruct or pass Schema here. This is a
            // separate process invocation: Engine::open must learn document
            // type/fields solely from the compiled configuration directory.
            let data_dir = PathBuf::from(&args[4]);
            let mut engine = Engine::open(&data_dir, &config_dir).expect("open generated config");
            match scenario.smoke {
                Smoke::Open => {}
                Smoke::Text { field, text, query } => {
                    engine.put_text(TextDocument { id: "m6-doc", field: &field, text: &text }).expect("put text");
                    let result = engine.search_text(TextQuery::Term { field: &field, text: &query }, "default").expect("search text");
                    assert!(result.hits.iter().any(|h| h.document_id == "m6-doc"), "text smoke hit missing");
                }
                Smoke::Integer { field, value } => {
                    engine.put("m6-doc", &field, value).expect("put integer");
                    let hits = engine.search(&field, value, 10).expect("search integer");
                    assert!(hits.iter().any(|id| id == "m6-doc"), "integer smoke hit missing");
                }
            }
        }
        other => panic!("unknown mode {other}"),
    }
}
