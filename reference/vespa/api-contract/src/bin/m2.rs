use searchkernel_vespa_engine::{Document, DocumentUpdate, Engine, Query, SearchRequest};
use std::path::Path;
type Result<T> = std::result::Result<T, searchkernel_vespa_engine::Error>;

fn compile_contract() -> Result<()> {
    let engine = Engine::open(Path::new("data"), Path::new("config"))?;
    engine.put(Document::new("id:test:typea::compile-contract").field("attr1", 10_i64))?;
    engine.update(DocumentUpdate::new("id:test:typea::compile-contract").assign("attr1", 20_i64))?;
    let _: Vec<_> = engine.search(SearchRequest::new(Query::equals("attr1", 20)).limit(10))?.hits;
    let _: bool = engine.delete("id:test:typea::compile-contract")?;
    Ok(())
}

fn main() {
    let _ = compile_contract as fn() -> Result<()>;
}
