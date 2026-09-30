use searchkernel_vespa_engine::{ConfigCompiler, Field, FieldType, Schema};
use std::path::PathBuf;

fn schema() -> Schema {
    Schema::new("typea")
        .field(Field::new("title", FieldType::Text).indexed())
        .field(Field::new("qty", FieldType::Integer).attribute())
}

fn main() {
    let config = PathBuf::from(std::env::args_os().nth(1).expect("usage: CONFIG"));
    ConfigCompiler
        .compile_to(&schema(), config)
        .expect("compile SearchReply verifier schema");
}
