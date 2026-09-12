use searchkernel_vespa::{ConfigCompiler, Field, FieldType, Schema};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let schema = Schema::new("typea")
        .field(Field::new("title", FieldType::Text).indexed().summary())
        .field(Field::new("title_exact", FieldType::Text).indexed().attribute().summary())
        .field(Field::new("sku", FieldType::String).attribute().summary())
        .field(Field::new("qty", FieldType::Integer).attribute().summary())
        .field(Field::new("price", FieldType::Float).attribute().summary())
        .field(Field::new("active", FieldType::Boolean).attribute().summary());

    let compiler = ConfigCompiler::default();
    let dir = std::env::temp_dir().join("searchkernel-m6-api-contract");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    compiler.compile_to(&schema, &dir)?;
    Ok(())
}
