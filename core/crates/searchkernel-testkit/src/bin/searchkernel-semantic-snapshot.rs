//! Resolve a packed portable search request and print its canonical IR snapshot.
//!
//! This is intentionally a testkit executable rather than a production API.
//! It lets first-party language tests compare their real FlatBuffers output at
//! the Rust-owned IR boundary without teaching those tests a second semantic
//! model.

use flatbuffers::root;
use searchkernel_ir::{api, snapshot, IndexContext};
use std::fs;
use std::path::PathBuf;

fn usage() -> ! {
    eprintln!("usage: searchkernel-semantic-snapshot --index-json PATH --request PATH");
    std::process::exit(2);
}

fn argument_value(args: &mut impl Iterator<Item = String>, name: &str) -> PathBuf {
    match args.next() {
        Some(path) => PathBuf::from(path),
        None => {
            eprintln!("missing value for {name}");
            usage();
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut index_json = None;
    let mut request = None;
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--index-json" => index_json = Some(argument_value(&mut args, "--index-json")),
            "--request" => request = Some(argument_value(&mut args, "--request")),
            _ => usage(),
        }
    }
    let (Some(index_json), Some(request)) = (index_json, request) else {
        usage();
    };

    let index_input: serde_json::Value = serde_json::from_slice(
        &fs::read(&index_json)
            .unwrap_or_else(|error| panic!("read {}: {error}", index_json.display())),
    )
    .unwrap_or_else(|error| panic!("parse {}: {error}", index_json.display()));
    let index_bytes = searchkernel_testkit::build_index_definition_from_json(&index_input)
        .unwrap_or_else(|error| panic!("encode index fixture: {error}"));
    let index = root::<api::IndexDefinition<'_>>(&index_bytes)
        .unwrap_or_else(|error| panic!("decode index fixture: {error}"));
    let context = IndexContext::from_api(index)
        .unwrap_or_else(|error| panic!("resolve index fixture: {error}"));

    let request_bytes =
        fs::read(&request).unwrap_or_else(|error| panic!("read {}: {error}", request.display()));
    let request = root::<api::SearchRequest<'_>>(&request_bytes)
        .unwrap_or_else(|error| panic!("decode request: {error}"));
    let resolved = context
        .search_to_ir(request)
        .unwrap_or_else(|error| panic!("resolve request: {error}"));
    println!("{}", snapshot::search_request(&resolved));
}
