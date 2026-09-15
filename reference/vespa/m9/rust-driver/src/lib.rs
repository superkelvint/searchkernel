use searchkernel_vespa_engine::{
    AggregateValue, Aggregation, AttributeBound, AttributeDocument, AttributeUpdate, ConfigCompiler,
    Engine, Field, FieldType, LocalGroupingRequest, LocalSearchRequest, Query, Schema, SortField,
    SummaryRequest, Value,
};
use serde_json::{json, Map, Value as Json};
use std::fs;
use std::path::{Path, PathBuf};

pub fn load(path: &Path) -> Json {
    serde_json::from_slice(&fs::read(path).expect("read scenario")).expect("parse scenario")
}

fn field_type(value: &str) -> FieldType {
    match value {
        "text" => FieldType::Text,
        "string" => FieldType::String,
        "integer" => FieldType::Integer,
        "float" => FieldType::Float,
        "boolean" => FieldType::Boolean,
        other => panic!("unsupported verifier field type {other}"),
    }
}

pub fn compile_schema(root: &Json, config_dir: &Path) {
    let s=&root["schema"];
    let mut schema=Schema::new(s["document_type"].as_str().unwrap());
    for f in s["fields"].as_array().unwrap() {
        let mut field=Field::new(f["name"].as_str().unwrap(),field_type(f["type"].as_str().unwrap()));
        if f.get("index").and_then(Json::as_bool).unwrap_or(false) { field=field.indexed(); }
        if f.get("attribute").and_then(Json::as_bool).unwrap_or(false) { field=field.attribute(); }
        if f.get("summary").and_then(Json::as_bool).unwrap_or(false) { field=field.summary(); }
        schema=schema.field(field);
    }
    fs::create_dir_all(config_dir).unwrap();
    ConfigCompiler::default().compile_to(&schema,config_dir).unwrap();
    if let Some(profile)=root.get("fixture_rank_profile") {
        assert_eq!(profile["name"],"attribute_price");
        assert_eq!(profile["expression"],"attribute(price)");
        let fixture=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fixture/rank-profiles.attribute-price.cfg");
        fs::copy(fixture,config_dir.join("rank-profiles.cfg")).unwrap();
    }
}

pub fn scalar(v:&Json)->Value {
    let o=v.as_object().expect("encoded scalar object");
    assert_eq!(o.len(),1);
    if let Some(v)=o.get("string").and_then(Json::as_str) { return Value::String(v.into()); }
    if let Some(v)=o.get("integer").and_then(Json::as_i64) { return Value::Integer(i32::try_from(v).unwrap()); }
    if let Some(v)=o.get("float").and_then(Json::as_f64) { return Value::Float(v); }
    if let Some(v)=o.get("boolean").and_then(Json::as_bool) { return Value::Boolean(v); }
    if let Some(bits)=o.get("float_bits").and_then(Json::as_str) {
        let raw=u64::from_str_radix(bits.strip_prefix("0x").unwrap(),16).unwrap();
        return Value::Float(f64::from_bits(raw));
    }
    panic!("unsupported encoded scalar: {v}");
}

pub fn populate(engine:&Engine,root:&Json) {
    for d in root.get("documents").and_then(Json::as_array).into_iter().flatten() {
        let mut doc=AttributeDocument::new(d["id"].as_str().unwrap());
        for (name,v) in d["fields"].as_object().unwrap() { doc=doc.field(name,scalar(v)); }
        engine.put_attributes(&doc).unwrap();
    }
}

fn attr_bound(v:&Json)->AttributeBound {
    if v=="unbounded" || v.get("unbounded").is_some() { return AttributeBound::Unbounded; }
    if let Some(x)=v.get("included") { return AttributeBound::Included(scalar(x)); }
    if let Some(x)=v.get("excluded") { return AttributeBound::Excluded(scalar(x)); }
    panic!("invalid attribute bound: {v}");
}

pub fn query(v:&Json)->Query {
    let o=v.as_object().unwrap(); assert_eq!(o.len(),1);
    if let Some(x)=o.get("exists") { return Query::Exists{field:x["field"].as_str().unwrap().into()}; }
    if let Some(x)=o.get("term") { return Query::Term{field:x["field"].as_str().unwrap().into(),text:x["text"].as_str().unwrap().into()}; }
    if let Some(x)=o.get("attribute_equals") { return Query::AttributeEquals{field:x["field"].as_str().unwrap().into(),value:scalar(&x["value"])}; }
    if let Some(x)=o.get("attribute_range") { return Query::AttributeRange{field:x["field"].as_str().unwrap().into(),lower:attr_bound(&x["lower"]),upper:attr_bound(&x["upper"])}; }
    panic!("unsupported M9 verifier query: {v}");
}

fn summary(v:Option<&Json>)->SummaryRequest {
    match v {
        None=>SummaryRequest::None,
        Some(Json::String(s)) if s=="none"=>SummaryRequest::None,
        Some(Json::String(s)) if s=="default"=>SummaryRequest::Default,
        Some(Json::Object(o)) if o.contains_key("fields")=>SummaryRequest::Fields(o["fields"].as_array().unwrap().iter().map(|x|x.as_str().unwrap().into()).collect()),
        Some(v)=>panic!("bad summary: {v}"),
    }
}

pub fn search_request(v:&Json)->LocalSearchRequest {
    let mut r=LocalSearchRequest::new(query(&v["query"]));
    if let Some(x)=v.get("rank_profile").and_then(Json::as_str) { r=r.rank_profile(x); }
    if let Some(x)=v.get("limit").and_then(Json::as_u64) { r=r.limit(u32::try_from(x).unwrap()); }
    if let Some(x)=v.get("offset").and_then(Json::as_u64) { r=r.offset(u32::try_from(x).unwrap()); }
    for s in v.get("sort").and_then(Json::as_array).into_iter().flatten() {
        r=match s["order"].as_str().unwrap() {
            "ascending"=>r.sort_by(SortField::ascending(s["field"].as_str().unwrap())),
            "descending"=>r.sort_by(SortField::descending(s["field"].as_str().unwrap())),
            x=>panic!("bad sort order {x}"),
        };
    }
    r.summary(summary(v.get("summary")))
}

fn aggregation(v:&Json)->Aggregation {
    let o=v.as_object().unwrap(); assert_eq!(o.len(),1);
    if let Some(x)=o.get("sum").and_then(Json::as_str) { return Aggregation::Sum(x.into()); }
    if let Some(x)=o.get("min").and_then(Json::as_str) { return Aggregation::Min(x.into()); }
    if let Some(x)=o.get("max").and_then(Json::as_str) { return Aggregation::Max(x.into()); }
    if let Some(x)=o.get("average").and_then(Json::as_str) { return Aggregation::Average(x.into()); }
    panic!("bad aggregation: {v}");
}

pub fn grouping_request(v:&Json)->LocalGroupingRequest {
    let mut r=LocalGroupingRequest::new(query(&v["query"]),v["group_by"].as_str().unwrap());
    for a in v["aggregations"].as_array().unwrap() { r=r.aggregation(aggregation(a)); }
    r
}

fn output_value(v:&Value)->Json {
    match v {
        Value::String(x)=>json!({"type":"string","value":x}),
        Value::Integer(x)=>json!({"type":"integer","value":x}),
        Value::Float(x)=>json!({"type":"float","bits":format!("0x{:016x}",x.to_bits())}),
        Value::Boolean(x)=>json!({"type":"boolean","value":x}),
    }
}
fn aggregate_value(v:&AggregateValue)->Json {
    match v {
        AggregateValue::Integer(x)=>json!({"type":"integer","value":x}),
        AggregateValue::Float(x)=>json!({"type":"float","bits":format!("0x{:016x}",x.to_bits())}),
    }
}
fn hit(h:searchkernel_vespa_engine::LocalHit)->Json {
    let summary:Map<String,Json>=h.summary.iter().map(|(k,v)|(k.clone(),output_value(v))).collect();
    json!({"document_id":h.document_id,"score_bits":format!("0x{:016x}",h.score.to_bits()),"summary":summary})
}

pub fn run_positive(engine:&Engine,root:&Json)->Json {
    let mut outputs=Vec::new();
    for step in root["steps"].as_array().unwrap() {
        let id=step["id"].as_str().unwrap();
        match step["op"].as_str().unwrap() {
            "update"=>{
                let mut u=AttributeUpdate::new(step["document_id"].as_str().unwrap());
                for (name,v) in step["assign"].as_object().unwrap() { u=u.assign(name,scalar(v)); }
                engine.update_attributes(&u).unwrap();
                outputs.push(json!({"id":id,"op":"update","ok":true}));
            }
            "search"=>{
                let r=engine.search_local(&search_request(&step["request"])).unwrap();
                outputs.push(json!({"id":id,"op":"search","total_hit_count":r.total_hit_count,"hits":r.hits.into_iter().map(hit).collect::<Vec<_>>() }));
            }
            "group"=>{
                let r=engine.group_local(&grouping_request(&step["request"])).unwrap();
                let groups=r.groups.into_iter().map(|g|json!({
                    "key":output_value(&g.key),"count":g.count,
                    "aggregations":g.aggregations.iter().map(aggregate_value).collect::<Vec<_>>()
                })).collect::<Vec<_>>();
                outputs.push(json!({"id":id,"op":"group","groups":groups}));
            }
            other=>panic!("unsupported step {other}"),
        }
    }
    json!({"id":root["id"],"steps":outputs})
}

pub fn classify_error(e:&searchkernel_vespa_engine::Error)->&'static str {
    let m=e.to_string().to_ascii_lowercase();
    if m.contains("nan") || m.contains("infinite") || m.contains("finite") { "invalid_float" }
    else if m.contains("unknown") || m.contains("does not exist") { "unknown_field" }
    else if m.contains("unsupported") || m.contains("not supported") || m.contains("non-numeric") { "unsupported_field_type" }
    else if m.contains("type mismatch") || m.contains("wrong type") || m.contains("value type") || m.contains("expected ") { "wrong_value_type" }
    else if m.contains("attribute") || m.contains("capability") { "wrong_field_capability" }
    else if m.contains("numeric") || m.contains("group") || m.contains("sort") { "unsupported_field_type" }
    else { "invalid_request" }
}
