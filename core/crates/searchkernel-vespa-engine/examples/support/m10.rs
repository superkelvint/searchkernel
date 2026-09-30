// This module is included by separate driver, error, and regression examples,
// which use different subsets of the shared verifier helpers.
#![allow(dead_code)]

use searchkernel_vespa_engine::{
    ConfigCompiler, Document, DocumentAnalysis, DocumentTerm, Engine, Field, FieldType, Query,
    RankProfile, Schema, SearchRequest, TensorValue,
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
        "tensor" => FieldType::Tensor("tensor<float>(x[2])".to_owned()),
        other => panic!("unsupported verifier field type {other}"),
    }
}

pub fn schema(root: &Json) -> Schema {
    let s = &root["schema"];
    let mut schema = Schema::new(s["document_type"].as_str().unwrap());
    for f in s["fields"].as_array().unwrap() {
        let field_kind = f["type"].as_str().unwrap();
        let field_type = if field_kind == "tensor" {
            FieldType::Tensor(f["tensor_type"].as_str().unwrap().to_owned())
        } else {
            field_type(field_kind)
        };
        let mut field = Field::new(f["name"].as_str().unwrap(), field_type);
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
    for p in s
        .get("rank_profiles")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
    {
        let mut profile = RankProfile::new(p["name"].as_str().unwrap())
            .first_phase(p["first_phase"].as_str().unwrap());
        for feature in p
            .get("match_features")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
        {
            profile = profile.match_feature(feature.as_str().unwrap());
        }
        for input in p
            .get("query_inputs")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
        {
            profile = profile.query_input(
                input["name"].as_str().unwrap(),
                input["type_spec"].as_str().unwrap(),
            );
        }
        schema = schema.rank_profile(profile);
    }
    schema
}

pub fn compile_schema(root: &Json, config: &Path) -> searchkernel_vespa_engine::Result<()> {
    fs::create_dir_all(config).expect("create verifier config dir");
    ConfigCompiler::default().compile_to(&schema(root), config)
}

// The verifier never evaluates linguistics itself. It merely reconstructs the
// already-frozen M3 annotation artifact from the scenario.
fn analysis(v: &Json) -> DocumentAnalysis {
    let raw = v["text"].as_str().unwrap();
    DocumentAnalysis {
        terms: v["analysis"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| {
                let start = usize::try_from(t["byte_start"].as_u64().unwrap()).unwrap();
                let len = usize::try_from(t["byte_len"].as_u64().unwrap()).unwrap();
                DocumentTerm {
                    original: raw[start..start + len].to_owned(),
                    term: t["term"].as_str().unwrap().to_owned(),
                    byte_start: start,
                    byte_len: len,
                    position: u32::try_from(t["position"].as_u64().unwrap()).unwrap(),
                }
            })
            .collect(),
    }
}

pub fn populate(engine: &Engine, root: &Json) -> searchkernel_vespa_engine::Result<()> {
    for d in root["documents"].as_array().unwrap() {
        let mut doc = Document::new(d["id"].as_str().unwrap());
        for (name, value) in d["fields"].as_object().unwrap() {
            if let Some(v) = value.get("text").and_then(Json::as_str) {
                doc = doc.field(name, v).annotate_text(name, analysis(value));
            } else if let Some(v) = value.get("integer").and_then(Json::as_i64) {
                doc = doc.field(name, v);
            } else if let Some(v) = value.get("float").and_then(Json::as_f64) {
                doc = doc.field(name, v);
            } else if let Some(v) = value.get("boolean").and_then(Json::as_bool) {
                doc = doc.field(name, v);
            } else if let Some(v) = value.get("string").and_then(Json::as_str) {
                doc = doc.field(name, v);
            } else if let Some(type_spec) = value.get("type_spec").and_then(Json::as_str) {
                let values = value["values"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|cell| cell.as_f64().unwrap())
                    .collect::<Vec<_>>();
                doc = doc.field(name, TensorValue::dense(type_spec, values));
            } else {
                panic!("unsupported M10 verifier field encoding: {value}");
            }
        }
        engine.put(doc)?;
    }
    Ok(())
}

fn query(v: &Json) -> Query {
    let o = v.as_object().unwrap();
    assert_eq!(o.len(), 1);
    if let Some(x) = o.get("exists") {
        return Query::exists(x["field"].as_str().unwrap());
    }
    if let Some(x) = o.get("term") {
        return Query::term(x["field"].as_str().unwrap(), x["text"].as_str().unwrap());
    }
    panic!("unsupported M10 verifier query: {v}")
}

fn feature_value(v: &Json) -> f64 {
    if let Some(x) = v.get("value") {
        return x.as_f64().unwrap();
    }
    if let Some(bits) = v.get("value_bits").and_then(Json::as_str) {
        return f64::from_bits(u64::from_str_radix(bits.strip_prefix("0x").unwrap(), 16).unwrap());
    }
    panic!("query feature must contain value or value_bits")
}

pub fn search_request(v: &Json) -> SearchRequest {
    let mut r =
        SearchRequest::new(query(&v["query"])).rank_profile(v["rank_profile"].as_str().unwrap());
    if let Some(x) = v.get("limit").and_then(Json::as_u64) {
        r = r.limit(u32::try_from(x).unwrap());
    }
    if let Some(x) = v.get("offset").and_then(Json::as_u64) {
        r = r.offset(u32::try_from(x).unwrap());
    }
    for item in v
        .get("query_features")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
    {
        r = r.query_feature(item["name"].as_str().unwrap(), feature_value(item));
    }
    for item in v
        .get("query_tensors")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
    {
        let values = item["values"]
            .as_array()
            .unwrap()
            .iter()
            .map(|cell| cell.as_f64().unwrap())
            .collect::<Vec<_>>();
        r = r.query_tensor(
            item["name"].as_str().unwrap(),
            TensorValue::dense(item["type_spec"].as_str().unwrap(), values),
        );
    }
    for item in v
        .get("rank_properties")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
    {
        r = r.rank_property(
            item["name"].as_str().unwrap(),
            item["value"].as_str().unwrap(),
        );
    }
    r
}

pub fn run_positive(engine: &Engine, root: &Json) -> searchkernel_vespa_engine::Result<Json> {
    let kinded_values = root.get("feature_value_format").and_then(Json::as_str) == Some("kinded");
    let mut steps = Vec::new();
    for step in root["steps"].as_array().unwrap() {
        let result = engine.search(search_request(&step["request"]))?;
        let hits=result.hits.into_iter().map(|hit| {
            let features:Map<String,Json>=hit.match_features.into_iter()
                .map(|(name,value)| {
                    let encoded = match value {
                        searchkernel_vespa_engine::MatchFeatureValue::Double(value) => {
                            if kinded_values {
                                json!({"kind":"double","bits":format!("0x{:016x}",value.to_bits())})
                            } else {
                                Json::String(format!("0x{:016x}",value.to_bits()))
                            }
                        }
                        searchkernel_vespa_engine::MatchFeatureValue::Data(data) => {
                            if !kinded_values {
                                panic!("M10 scalar ranking verifier received data-backed match feature");
                            }
                            let mut hex = String::with_capacity(data.len() * 2);
                            for byte in data {
                                use std::fmt::Write as _;
                                write!(&mut hex, "{byte:02x}").expect("writing to String cannot fail");
                            }
                            json!({"kind":"data","hex":hex})
                        }
                    };
                    (name,encoded)
                }).collect();
            json!({"document_id":hit.document_id,"score_bits":format!("0x{:016x}",hit.score.to_bits()),"match_features":features})
        }).collect::<Vec<_>>();
        steps.push(
            json!({"id":step["id"],"op":"search","total_hit_count":result.total_hits,"hits":hits}),
        );
    }
    Ok(json!({"id":root["id"],"steps":steps}))
}

// Error strings remain an implementation detail, but the verifier freezes
// coarse semantic categories so invalid M10 inputs cannot simply reach Vespa.
pub fn classify_error(error: &searchkernel_vespa_engine::Error) -> &'static str {
    let m = error.to_string().to_ascii_lowercase();
    if m.contains("collision") {
        "rank_property_collision"
    } else if m.contains("duplicate") && m.contains("match") {
        "duplicate_match_feature"
    } else if m.contains("duplicate") && m.contains("query") && m.contains("feature") {
        "duplicate_query_feature"
    } else if m.contains("duplicate") && m.contains("profile") {
        "duplicate_rank_profile"
    } else if m.contains("duplicate") && m.contains("property") {
        "duplicate_rank_property"
    } else if m.contains("match feature") || m.contains("match-feature") {
        "invalid_match_feature"
    } else if m.contains("first phase")
        || m.contains("first-phase")
        || m.contains("rank expression")
    {
        "invalid_rank_expression"
    } else if m.contains("query feature")
        || m.contains("query-feature")
        || m.contains("finite")
        || m.contains("nan")
        || m.contains("infinite")
    {
        "invalid_query_feature"
    } else if m.contains("rank property") || m.contains("rank-property") {
        "invalid_rank_property"
    } else if m.contains("rank profile") || m.contains("rank-profile") || m.contains("profile") {
        "invalid_rank_profile"
    } else {
        "invalid_request"
    }
}
