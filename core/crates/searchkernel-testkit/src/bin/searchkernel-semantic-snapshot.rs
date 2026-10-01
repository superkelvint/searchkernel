//! Resolve a packed portable search request and print its canonical IR snapshot.
//!
//! This is intentionally a testkit executable rather than a production API.
//! It lets first-party language tests compare their real FlatBuffers output at
//! the Rust-owned IR boundary without teaching those tests a second semantic
//! model.

use flatbuffers::root;
use searchkernel_ir::wire::v1::schema as api;
use searchkernel_ir::{snapshot, IndexContext};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

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
    let mut request_json = None;
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--index-json" => index_json = Some(argument_value(&mut args, "--index-json")),
            "--request" => request = Some(argument_value(&mut args, "--request")),
            "--request-json" => request_json = Some(argument_value(&mut args, "--request-json")),
            _ => usage(),
        }
    }
    let Some(index_json) = index_json else {
        usage();
    };
    let request = match (request, request_json) {
        (Some(path), None) => path,
        (None, Some(json)) => {
            let dir = std::env::temp_dir().join(format!(
                "searchkernel-semantic-snapshot-{}",
                std::process::id()
            ));
            fs::create_dir_all(&dir).unwrap();
            let schema = dir.join("request.fbs");
            let canonical = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../api/searchkernel.fbs")
                .canonicalize()
                .unwrap();
            let mut schema_text = fs::read_to_string(canonical).unwrap();
            schema_text.push_str("\nroot_type SearchRequest;\n");
            fs::write(&schema, schema_text).unwrap();
            let status = Command::new("flatc")
                .args(["--binary", "--strict-json", "-o"])
                .arg(&dir)
                .arg(&schema)
                .arg(&json)
                .status()
                .unwrap();
            if !status.success() {
                panic!("flatc request JSON conversion failed");
            }
            dir.join(format!(
                "{}.bin",
                json.file_stem().unwrap().to_string_lossy()
            ))
        }
        _ => usage(),
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
    println!(
        "{}",
        searchkernel_testkit::lossless_integer_tags(snapshot::search_request(&resolved))
    );
}
