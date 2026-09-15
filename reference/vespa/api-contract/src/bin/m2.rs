use searchkernel_vespa_engine::Engine;
use std::path::Path;
type Result<T> = std::result::Result<T, searchkernel_vespa_engine::Error>;

fn compile_contract() -> Result<()> {
    let mut engine = Engine::open(Path::new("data"), Path::new("config"))?;
    engine.put("id:test:typea::compile-contract", "attr1", 10)?;
    engine.update("id:test:typea::compile-contract", "attr1", 20)?;
    let _: Vec<String> = engine.search("attr1", 20, 10)?;
    let _: bool = engine.remove("id:test:typea::compile-contract")?;
    Ok(())
}

fn main() {
    let _ = compile_contract as fn() -> Result<()>;
}
