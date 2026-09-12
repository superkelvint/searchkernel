use searchkernel_vespa::{Engine, Error};
use std::path::Path;
type Result<T> = std::result::Result<T, Error>;

fn assert_error_traits<T>()
where
    T: std::error::Error + Send + Sync + 'static,
{
}

fn compile_contract() -> Result<()> {
    assert_error_traits::<Error>();
    let engine = Engine::open(Path::new("data"), Path::new("config"))?;
    drop(engine);
    Ok(())
}

fn main() {
    let _ = compile_contract as fn() -> Result<()>;
}
