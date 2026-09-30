#[path = "support/m10.rs"]
mod m10;

use crate::m10::{compile_schema, load};
use searchkernel_vespa_engine::Engine;
use std::fs;
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args_os().skip(1);
    let cases_path = PathBuf::from(args.next().expect("config-regressions.json"));
    let config_root = PathBuf::from(args.next().expect("config root"));
    let data_root = PathBuf::from(args.next().expect("data root"));
    assert!(args.next().is_none());

    let root = load(&cases_path);
    for case in root["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let config = config_root.join(id);
        let data = data_root.join(id);
        compile_schema(case, &config).unwrap();
        fs::create_dir_all(&data).unwrap();
        let engine = Engine::open(&data, &config).unwrap();
        engine.close().unwrap();
    }
}
