// These helpers are shared by the M12 driver, adapter, error, and fixture
// examples, which use different subsets of the module.
#![allow(dead_code)]

use searchkernel_vespa_engine::{
    ConfigCompiler, DistanceMetric, Document, Engine, Field, FieldType, Query, RankProfile, Schema,
    SearchRequest, TensorValue,
};
use serde_json::{json, Value as Json};
use std::{collections::BTreeMap, fs, path::Path};

pub fn load(path: &Path) -> Json {
    serde_json::from_slice(&fs::read(path).expect("read scenario")).expect("parse scenario")
}

fn fixture_tensor(root: &Json, values: &Json) -> TensorValue {
    TensorValue::dense(
        root["fixture"]["tensor_field"]["type_spec"]
            .as_str()
            .expect("fixture tensor type"),
        values
            .as_array()
            .expect("fixture tensor values")
            .iter()
            .map(|value| value.as_f64().expect("finite fixture tensor cell"))
            .collect::<Vec<_>>(),
    )
}

pub fn fixture_schema(root: &Json) -> Schema {
    let fixture = &root["fixture"];
    let field = &fixture["tensor_field"];
    let field_name = field["name"].as_str().expect("fixture tensor field");
    let type_spec = field["type_spec"].as_str().expect("fixture tensor type");

    let mut query_inputs = BTreeMap::<String, String>::new();
    for step in root["steps"].as_array().expect("steps") {
        for input in step["inputs"].as_array().expect("inputs") {
            let name = input["name"].as_str().expect("query input name").to_owned();
            let input_type = input["type_spec"]
                .as_str()
                .expect("query input type")
                .to_owned();
            if let Some(previous) = query_inputs.insert(name.clone(), input_type.clone()) {
                assert_eq!(
                    previous, input_type,
                    "query input {name} changed type across M12 steps"
                );
            }
        }
    }

    let mut rank = RankProfile::new("m12_nn").first_phase(format!("closeness(field,{field_name})"));
    for (name, input_type) in query_inputs {
        rank = rank.query_input(name, input_type);
    }

    Schema::new(
        fixture["document_type"]
            .as_str()
            .expect("fixture document type"),
    )
    .field(
        Field::new(field_name, FieldType::Tensor(type_spec.to_owned()))
            .attribute()
            .distance_metric(DistanceMetric::Euclidean),
    )
    .rank_profile(rank)
}

pub fn prepare_fixture(
    root: &Json,
    config: &Path,
    data: &Path,
) -> searchkernel_vespa_engine::Result<()> {
    ConfigCompiler.compile_to(&fixture_schema(root), config)?;
    let engine = Engine::open(data, config)?;
    let field = root["fixture"]["tensor_field"]["name"]
        .as_str()
        .expect("fixture tensor field");
    for document in root["fixture"]["documents"]
        .as_array()
        .expect("fixture documents")
    {
        engine.put(
            Document::new(document["id"].as_str().expect("fixture document id"))
                .field(field, fixture_tensor(root, &document["values"])),
        )?;
    }
    engine.close()
}

fn dense_tensor(input: &Json) -> TensorValue {
    let type_spec = input["type_spec"].as_str().expect("type_spec");
    let values = input["values"]
        .as_array()
        .expect("values")
        .iter()
        .map(|v| match v.as_f64() {
            Some(value) => value,
            None => match v.as_str() {
                Some("nan") => f64::NAN,
                Some("positive_infinity") => f64::INFINITY,
                Some("negative_infinity") => f64::NEG_INFINITY,
                _ => panic!("numeric tensor cell"),
            },
        })
        .collect::<Vec<_>>();
    TensorValue::dense(type_spec, values)
}

pub fn low_level_request(step: &Json) -> SearchRequest {
    let mut request = SearchRequest::new(Query::nearest_neighbor(
        step["document_field"].as_str().expect("document_field"),
        step["query_tensor"].as_str().expect("query_tensor"),
    ))
    .limit(u32::try_from(step["limit"].as_u64().expect("limit")).expect("u32 limit"));

    for input in step["inputs"].as_array().expect("inputs") {
        assert_eq!(input["kind"], "dense");
        request = request.query_tensor(
            input["name"].as_str().expect("input name"),
            dense_tensor(input),
        );
    }
    for feature in step["scalar_query_features"]
        .as_array()
        .expect("scalar_query_features")
    {
        request = request.query_feature(
            feature["name"].as_str().expect("feature name"),
            feature["value"].as_f64().expect("feature value"),
        );
    }
    for property in step["rank_properties"].as_array().expect("rank_properties") {
        request = request.rank_property(
            property["name"].as_str().expect("property name"),
            property["value"].as_str().expect("property value"),
        );
    }
    request
}

pub fn run_low_level(engine: &Engine, root: &Json) -> searchkernel_vespa_engine::Result<Json> {
    let mut out = Vec::new();
    for step in root["steps"].as_array().expect("steps") {
        let result = engine.search(low_level_request(step))?;
        out.push(json!({
            "id":step["id"],
            "hits":result.hits.into_iter().map(|h|h.document_id).collect::<Vec<_>>()
        }));
    }
    Ok(json!({"id":root["id"],"steps":out}))
}

pub fn portable_request_json(step: &Json) -> Vec<u8> {
    let mut inputs = Vec::new();
    for input in step["inputs"].as_array().expect("inputs") {
        inputs.push(json!({
            "name": input["name"],
            "input_type":"LiteralQueryInput",
            "input":{
                "value":{
                    "kind_type":"TensorValue",
                    "kind":{
                        "type_spec":input["type_spec"],
                        "payload_type":"DenseTensorValues",
                        "payload":{"values":input["values"]}
                    }
                }
            }
        }));
    }
    for feature in step["scalar_query_features"]
        .as_array()
        .expect("scalar_query_features")
    {
        inputs.push(json!({
            "name":feature["name"],
            "input_type":"LiteralQueryInput",
            "input":{"value":{"kind_type":"Float64Value","kind":{"value":feature["value"]}}}
        }));
    }
    serde_json::to_vec(&json!({
        "protocol_version":1,
        "payload_type":"SearchKernel_Wire_SearchRequest",
        "payload":{
            "query":{
                "source_type":"TypedQuery",
                "source":{
                    "query":{
                        "expr_type":"QueryNearestNeighbor",
                        "expr":{
                            "document_field":{"path":step["document_field"]},
                            "query_tensor":step["query_tensor"]
                        }
                    }
                }
            },
            "ranking":{"properties":step["rank_properties"]},
            "inputs":inputs,
            "limit":{"value":step["limit"]}
        }
    }))
    .expect("portable request json")
}

pub fn canonical_response_hits(bytes: &[u8]) -> Json {
    let printed = searchkernel_protocol::json::response_flatbuffer_to_json(bytes)
        .expect("standard FB response JSON");
    let response: Json = serde_json::from_slice(&printed).expect("response JSON");
    assert_eq!(response["payload_type"], "SearchKernel_Wire_SearchResult");
    let mut ids = Vec::new();
    fn visit(node: &Json, ids: &mut Vec<String>) {
        match node.get("node_type").and_then(Json::as_str) {
            Some("ResultHit") => {
                if let Some(id) = node
                    .get("node")
                    .and_then(|n| n.get("id"))
                    .and_then(Json::as_str)
                {
                    ids.push(id.to_owned());
                }
            }
            Some("ResultGroup") => {
                if let Some(children) = node
                    .get("node")
                    .and_then(|n| n.get("children"))
                    .and_then(Json::as_array)
                {
                    for child in children {
                        visit(child, ids);
                    }
                }
            }
            _ => {}
        }
    }
    if let Some(root) = response.get("payload").and_then(|p| p.get("root")) {
        visit(root, &mut ids);
    }
    Json::Array(ids.into_iter().map(Json::String).collect())
}
