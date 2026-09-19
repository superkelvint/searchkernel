use searchkernel_m9_verifier::{
    classify_error, compile_schema, grouping_request, load, populate, scalar, search_request,
};
use searchkernel_vespa_engine::{Document, DocumentUpdate, Engine};
use std::path::PathBuf;

fn main() {
    let mut a = std::env::args_os().skip(1);
    let path = PathBuf::from(a.next().expect("errors json"));
    let id = a.next().expect("case id").into_string().unwrap();
    let config = PathBuf::from(a.next().expect("config"));
    let data = PathBuf::from(a.next().expect("data"));
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
    populate(&engine, case);
    let op = &case["operation"];
    let result: searchkernel_vespa_engine::Result<()> = if let Some(p) = op.get("put") {
        let mut d = Document::new(p["id"].as_str().unwrap());
        for (name, v) in p["fields"].as_object().unwrap() {
            d = d.field(name, scalar(v));
        }
        engine.put(d)
    } else if let Some(u) = op.get("update") {
        let mut update = DocumentUpdate::new(u["id"].as_str().unwrap());
        for (name, v) in u["assign"].as_object().unwrap() {
            update = update.assign(name, scalar(v));
        }
        engine.update(update)
    } else if let Some(s) = op.get("search") {
        engine.search(search_request(s)).map(|_| ())
    } else if let Some(g) = op.get("group") {
        let (query, plan) = grouping_request(g);
        engine.group(&query, &plan).map(|_| ())
    } else {
        panic!("unsupported negative operation")
    };
    match result {
        Ok(()) => {
            eprintln!("M9 negative case unexpectedly succeeded: {id}");
            std::process::exit(3);
        }
        Err(e) => println!("{}", classify_error(&e)),
    }
    engine.close().unwrap();
}
