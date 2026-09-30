#[path = "support/m10.rs"]
mod m10;

use crate::m10::{classify_error, compile_schema, load, populate, search_request};
use searchkernel_vespa_engine::Engine;
use serde_json::Value as Json;
use std::path::PathBuf;

fn main() {
    let mut a = std::env::args_os().skip(1);
    let cases_path = PathBuf::from(a.next().expect("errors.json"));
    let wanted = a.next().expect("case id").to_string_lossy().into_owned();
    let config = PathBuf::from(a.next().expect("config"));
    let data = PathBuf::from(a.next().expect("data"));
    assert!(a.next().is_none());
    let root = load(&cases_path);
    let case = root["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["id"] == wanted)
        .expect("unknown error case");
    let result: Result<(), searchkernel_vespa_engine::Error> =
        if case["operation"].get("compile").is_some() {
            compile_schema(case, &config)
        } else {
            match compile_schema(case, &config) {
                Err(e) => Err(e),
                Ok(()) => {
                    std::fs::create_dir_all(&data).unwrap();
                    match Engine::open(&data, &config) {
                        Err(e) => Err(e),
                        Ok(engine) => {
                            if let Err(e) = populate(&engine, case) {
                                Err(e)
                            } else {
                                let request: &Json = &case["operation"]["search"];
                                match engine.search(search_request(request)) {
                                    Ok(_) => Ok(()),
                                    Err(e) => Err(e),
                                }
                            }
                        }
                    }
                }
            }
        };
    match result {
        Ok(()) => {
            eprintln!("M10 negative case unexpectedly succeeded: {wanted}");
            std::process::exit(2);
        }
        Err(e) => println!("{}", classify_error(&e)),
    }
}
