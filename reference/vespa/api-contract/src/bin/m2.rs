use searchkernel::{
    Document, DocumentId, DocumentUpdate, Engine, EngineConfig, Query, Result,
    SearchRequest, SearchResult, Value,
};

fn consume_result(result: &SearchResult) {
    for hit in result.hits() {
        let _: &DocumentId = hit.id();
        let _: &str = hit.id().as_str();
    }
}

fn compile_contract() -> Result<()> {
    let config = EngineConfig::from_directory("config")?;
    let mut engine = Engine::open("data", config)?;

    let id = DocumentId::parse("id:test:typea::compile-contract")?;
    let mut doc = Document::new(id.clone());
    doc.set_field("attr1", Value::I32(10));
    engine.put(doc)?;

    let mut update = DocumentUpdate::new(id.clone());
    update.assign("attr1", Value::I32(20));
    engine.update(update)?;

    let request = SearchRequest::new(Query::term("attr1", Value::I32(20))).limit(10);
    let result = engine.search(request)?;
    consume_result(&result);

    let _: bool = engine.remove(&id)?;
    Ok(())
}

fn main() {
    let _ = compile_contract as fn() -> Result<()>;
}
