use searchkernel::{Engine, EngineConfig, Error, Result};

fn assert_error_traits<T>()
where
    T: std::error::Error + Send + Sync + 'static,
{
}

fn compile_contract() -> Result<()> {
    assert_error_traits::<Error>();
    let config = EngineConfig::from_directory("config")?;
    let engine = Engine::open("data", config)?;
    drop(engine);
    Ok(())
}

fn main() {
    let _ = compile_contract as fn() -> Result<()>;
}
