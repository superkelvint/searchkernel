#[path = "support/m8.rs"]
mod m8;

use crate::m8::{classify_error, compile_schema, load, request};
use searchkernel_vespa_engine::Engine;
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args_os().skip(1);
    let path = PathBuf::from(args.next().expect("errors json"));
    let id = args.next().expect("case id").into_string().unwrap();
    let config = PathBuf::from(args.next().expect("config dir"));
    let data = PathBuf::from(args.next().expect("data dir"));
    let root = load(&path);
    let case = root["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"].as_str() == Some(id.as_str()))
        .unwrap();
    compile_schema(case, &config);
    std::fs::create_dir_all(&data).unwrap();
    let engine = Engine::open(&data, &config).unwrap();
    match engine.search(request(&case["request"])) {
        Ok(_) => {
            eprintln!("M8 negative case unexpectedly succeeded: {id}");
            std::process::exit(3);
        }
        Err(e) => println!("{}", classify_error(&e)),
    }
    engine.close().unwrap();
}
