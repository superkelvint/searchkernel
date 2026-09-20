use searchkernel_m9_verifier::{
    classify_error, compile_schema, grouping_request, load, populate, scalar, search_request,
};
use searchkernel_vespa_engine::{
    AttributeBound, AttributeDocument, AttributeUpdate, Engine, Query, SearchRequest, Value,
};
use serde_json::Value as Json;
use std::path::PathBuf;

fn negative_search_request(value: &Json, schema: &Json) -> SearchRequest {
    let mut request = search_request(value);
    let Some(range) = value["query"].get("attribute_range") else {
        return request;
    };
    if range["lower"] != "unbounded" || range["upper"] != "unbounded" {
        return request;
    }
    let field = range["field"].as_str().expect("range field");
    let field_type = schema["fields"]
        .as_array()
        .expect("schema fields")
        .iter()
        .find(|candidate| candidate["name"].as_str() == Some(field))
        .and_then(|candidate| candidate["type"].as_str())
        .expect("range field type");
    let unsupported_value = match field_type {
        "string" => Value::String(String::new()),
        "boolean" => Value::Boolean(false),
        _ => return request,
    };
    request.query = Query::AttributeRange {
        field: field.to_owned(),
        lower: AttributeBound::Included(unsupported_value),
        upper: AttributeBound::Unbounded,
    };
    request
}

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
        let mut d = AttributeDocument::new(p["id"].as_str().unwrap());
        for (name, v) in p["fields"].as_object().unwrap() {
            d = d.field(name, scalar(v));
        }
        engine.put_attributes(&d)
    } else if let Some(u) = op.get("update") {
        let mut update = AttributeUpdate::new(u["id"].as_str().unwrap());
        for (name, v) in u["assign"].as_object().unwrap() {
            update = update.assign(name, scalar(v));
        }
        engine.update_attributes(&update)
    } else if let Some(s) = op.get("search") {
        engine
            .search(negative_search_request(s, &case["schema"]))
            .map(|_| ())
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
