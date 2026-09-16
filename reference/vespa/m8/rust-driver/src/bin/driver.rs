use searchkernel_m8_verifier::{compile_schema, load, populate, request, result_json};
use searchkernel_vespa_engine::Engine;
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args_os().skip(1);
    let scenario = PathBuf::from(args.next().expect("scenario"));
    let config = PathBuf::from(args.next().expect("config dir"));
    let data = PathBuf::from(args.next().expect("data dir"));
    assert!(args.next().is_none());
    let root = load(&scenario);
    compile_schema(&root, &config);
    std::fs::create_dir_all(&data).unwrap();
    let engine = Engine::open(&data, &config).unwrap();
    populate(&engine, &root);
    let result = engine.search(request(&root["request"])).unwrap();
    println!(
        "{}",
        serde_json::to_string(&result_json(root["id"].as_str().unwrap(), result)).unwrap()
    );
    engine.close().unwrap();
}
