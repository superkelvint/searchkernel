use searchkernel_vespa_engine::{
    ConfigCompiler, Document, Engine, Field, FieldType, FieldValue, Query,
    RangeBound, ScalarValue, Schema, SearchRequest, SortField, SummaryRequest,
};
use serde_json::{json, Map, Value as Json};
use std::fs;
use std::path::Path;

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
    let s = &root["schema"];
    let mut schema = Schema::new(s["document_type"].as_str().unwrap());
    for f in s["fields"].as_array().unwrap() {
        let mut field = Field::new(
            f["name"].as_str().unwrap(),
            field_type(f["type"].as_str().unwrap()),
        );
        if f.get("index").and_then(Json::as_bool).unwrap_or(false) {
            field = field.indexed();
        }
        if f.get("attribute").and_then(Json::as_bool).unwrap_or(false) {
            field = field.attribute();
        }
        if f.get("summary").and_then(Json::as_bool).unwrap_or(false) {
            field = field.summary();
        }
        schema = schema.field(field);
    }
    fs::create_dir_all(config_dir).unwrap();
    ConfigCompiler::default()
        .compile_to(&schema, config_dir)
        .unwrap();
}

pub fn populate(engine: &Engine, root: &Json) {
    let docs = match root.get("documents").and_then(Json::as_array) {
        Some(v) => v,
        None => return,
    };
    for d in docs {
        let id = d["id"].as_str().unwrap();
        let fields = d["fields"].as_object().unwrap();
        let mut document = Document::new(id);
        let mut created = false;
        for (name, encoded) in fields {
            if let Some(value) = encoded.get("integer").and_then(Json::as_i64) {
                document = document.field(name, value);
                created = true;
            } else if let Some(text) = encoded.get("text").and_then(Json::as_str) {
                document = document.field(name, text);
                created = true;
            } else {
                panic!("M8 positive scenarios may contain only integer/text fixture values");
            }
        }
        assert!(created, "fixture document must contain a field");
        engine.put(document).unwrap();
    }
}

fn bound(v: &Json) -> RangeBound {
    if v.get("unbounded").is_some() || v == "unbounded" {
        return RangeBound::Unbounded;
    }
    if let Some(n) = v.get("included").and_then(Json::as_i64) {
        return RangeBound::Included(n);
    }
    if let Some(n) = v.get("excluded").and_then(Json::as_i64) {
        return RangeBound::Excluded(n);
    }
    panic!("invalid verifier range bound: {v}");
}

pub fn query(v: &Json) -> Query {
    let o = v.as_object().unwrap();
    assert_eq!(o.len(), 1);
    if let Some(x) = o.get("term") {
        return Query::Term {
            field: x["field"].as_str().unwrap().into(),
            text: x["text"].as_str().unwrap().into(),
        };
    }
    if let Some(x) = o.get("and") {
        return Query::And(x
            .as_array()
            .unwrap()
            .iter()
            .map(query)
            .collect());
    }
    if let Some(x) = o.get("range") {
        return Query::Range {
            field: x["field"].as_str().unwrap().into(),
            lower: bound(&x["lower"]),
            upper: bound(&x["upper"]),
        };
    }
    if let Some(x) = o.get("exists") {
        return Query::Exists {
            field: x["field"].as_str().unwrap().into(),
        };
    }
    panic!("unsupported M8 verifier query: {v}");
}

fn summary(v: Option<&Json>) -> SummaryRequest {
    match v {
        None => SummaryRequest::None,
        Some(Json::String(s)) if s == "none" => SummaryRequest::None,
        Some(Json::String(s)) if s == "default" => SummaryRequest::Default,
        Some(Json::Object(o)) if o.contains_key("fields") => SummaryRequest::Fields(
            o["fields"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_str().unwrap().to_string())
                .collect(),
        ),
        Some(v) => panic!("invalid summary request: {v}"),
    }
}

pub fn request(v: &Json) -> SearchRequest {
    let mut out = SearchRequest::new(query(&v["query"]));
    if let Some(rank) = v.get("rank_profile").and_then(Json::as_str) {
        out = out.rank_profile(rank);
    }
    if let Some(limit) = v.get("limit").and_then(Json::as_u64) {
        out = out.limit(u32::try_from(limit).unwrap());
    }
    if let Some(offset) = v.get("offset").and_then(Json::as_u64) {
        out = out.offset(u32::try_from(offset).unwrap());
    }
    for s in v.get("sort").and_then(Json::as_array).into_iter().flatten() {
        let field = s["field"].as_str().unwrap();
        out = match s["order"].as_str().unwrap() {
            "ascending" => out.sort(SortField::ascending(field)),
            "descending" => out.sort(SortField::descending(field)),
            other => panic!("invalid sort order {other}"),
        };
    }
    out.summary(summary(v.get("summary")))
}

fn value(v: &FieldValue) -> Json {
    match v {
        FieldValue::Scalar(ScalarValue::String(x)) => json!({"type":"string","value":x}),
        FieldValue::Scalar(ScalarValue::Integer(x)) => json!({"type":"integer","value":x}),
        FieldValue::Scalar(ScalarValue::Float(x)) => {
            json!({"type":"float","bits":format!("0x{:016x}",x.to_bits())})
        }
        FieldValue::Scalar(ScalarValue::Boolean(x)) => json!({"type":"boolean","value":x}),
        FieldValue::Array(_) => panic!("M8 summaries do not contain arrays"),
    }
}

pub fn result_json(id: &str, result: searchkernel_vespa_engine::SearchResult) -> Json {
    let hits = result
        .hits
        .into_iter()
        .map(|h| {
            let summary: Map<String, Json> = h
                .fields
                .iter()
                .map(|(k, v)| (k.clone(), value(v)))
                .collect();
            json!({
                "document_id":h.document_id,
                "score_bits":format!("0x{:016x}",h.score.to_bits()),
                "summary":summary,
            })
        })
        .collect::<Vec<_>>();
    json!({"id":id,"total_hit_count":result.total_hits,"hits":hits})
}

pub fn classify_error(e: &searchkernel_vespa_engine::Error) -> &'static str {
    let m = e.to_string().to_ascii_lowercase();
    if m.contains("unknown") || m.contains("does not exist") {
        "unknown_field"
    } else if m.contains("summary") && (m.contains("empty") || m.contains("duplicate")) {
        "invalid_request"
    } else if m.contains("unsupported") || m.contains("not supported") || m.contains("field type") {
        "unsupported_field_type"
    } else if m.contains("attribute") || m.contains("summary") || m.contains("capability") {
        "wrong_field_capability"
    } else {
        "invalid_request"
    }
}
