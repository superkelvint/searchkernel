use searchkernel::{Engine, EngineConfig, Result};
use std::path::PathBuf;

fn run() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let data_dir = PathBuf::from(args.next().expect("DATA_DIR argument"));
    let config_dir = PathBuf::from(args.next().expect("CONFIG_DIR argument"));
    assert!(args.next().is_none(), "unexpected extra arguments");

    let _ = std::fs::remove_dir_all(&data_dir);

    let config = EngineConfig::from_directory(&config_dir)?;
    let engine = Engine::open(&data_dir, config)?;
    drop(engine);

    let config = EngineConfig::from_directory(&config_dir)?;
    let engine = Engine::open(&data_dir, config)?;
    drop(engine);

    println!(r#"{{"scenario":"rust-open-close","open_count":2,"status":"ok"}}"#);
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
