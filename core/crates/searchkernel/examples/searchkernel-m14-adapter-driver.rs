#[path = "support/m14.rs"]
mod m14;

use crate::m14::{load, TraceReader};
use searchkernel_dispatch::Dispatcher;
use searchkernel_ir::{index_definition_to_ir, Backend, BackendSession, IndexContext};
use searchkernel_ir_adapter::VespaBackend;
use searchkernel_protocol::json::{request_json_to_flatbuffer, response_flatbuffer_to_json};
use serde_json::{json, Value as Json};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

fn definition_ir(path: &Path) -> searchkernel_ir::IndexDefinitionIr {
    let bytes = fs::read(path).expect("read M14 definition");
    let root = searchkernel_ir::__flatbuffers::root::<
        searchkernel_ir::wire::v1::schema::IndexDefinition,
    >(&bytes)
    .expect("parse M14 definition");
    index_definition_to_ir(root).expect("definition to IR")
}
fn open(definition: &Path, config: &Path, data: &Path) -> Dispatcher {
    let ir = definition_ir(definition);
    if !config.join("searchkernel-schema.json").exists() {
        searchkernel_ir_adapter::compile_index_definition(&ir, config)
            .expect("compile M14 HNSW definition");
    }
    let context = Arc::new(IndexContext::from_definition_ir(ir));
    let backend: Arc<dyn Backend> =
        Arc::new(VespaBackend::open(data, config).expect("open M14 backend"));
    Dispatcher::new(context, BackendSession::from_backend(backend))
}
fn envelope(kind: &str, payload: Json) -> Vec<u8> {
    serde_json::to_vec(&json!({"protocol_version":1,"payload_type":kind,"payload":payload}))
        .unwrap()
}
fn dispatch(d: &Dispatcher, kind: &str, payload: Json) -> Json {
    let req = request_json_to_flatbuffer(&envelope(kind, payload)).expect("request FB");
    let resp = d.dispatch(&req).expect("dispatch M14 request");
    serde_json::from_slice(&response_flatbuffer_to_json(&resp).expect("response JSON")).unwrap()
}
fn dense(root: &Json, values: &Json) -> Json {
    json!({"kind_type":"TensorValue","kind":{"type_spec":root["tensor_field"]["type_spec"],"payload_type":"DenseTensorValues","payload":{"values":values}}})
}
fn doc(root: &Json, item: &Json) -> Json {
    json!({"id":item["id"],"fields":[{"key":root["tensor_field"]["name"],"value":dense(root,&item["values"])}]})
}
fn annotations(step: &Json) -> Vec<Json> {
    let mut out = vec![
        json!({"key":"totalTargetHits","value":{"kind_type":"UInt64Value","kind":{"value":step["target_hits"].as_u64().unwrap().to_string()}}}),
        json!({"key":"approximate","value":{"kind_type":"BoolValue","kind":{"value":step["approximate"]}}}),
        json!({"key":"exploreAdditionalHits","value":{"kind_type":"UInt64Value","kind":{"value":step["explore_additional_hits"].as_u64().unwrap().to_string()}}}),
    ];
    if !step["distance_threshold"].is_null() {
        out.push(json!({"key":"distanceThreshold","value":{"kind_type":"Float64Value","kind":{"value":step["distance_threshold"]}}}));
    }
    out
}
fn hits(response: &Json) -> Vec<String> {
    fn visit(node: &Json, out: &mut Vec<String>) {
        match node.get("node_type").and_then(Json::as_str) {
            Some("Hit") | Some("ResultHit") => {
                if let Some(id) = node
                    .get("node")
                    .and_then(|n| n.get("id"))
                    .and_then(Json::as_str)
                {
                    out.push(id.to_owned())
                }
            }
            Some("ResultGroup") => {
                if let Some(children) = node
                    .get("node")
                    .and_then(|n| n.get("children"))
                    .and_then(Json::as_array)
                {
                    for child in children {
                        visit(child, out)
                    }
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    if let Some(root) = response.get("payload").and_then(|p| p.get("root")) {
        visit(root, &mut out)
    }
    out
}
fn query(
    d: &Dispatcher,
    root: &Json,
    step: &Json,
    query_values: &Json,
    traces: &mut TraceReader,
) -> (Vec<String>, Option<Json>) {
    let response = dispatch(
        d,
        "SearchKernel_Wire_SearchRequest",
        json!({
            "query":{"source_type":"TypedQuery","source":{"query":{"expr_type":"QueryNearestNeighbor","expr":{"document_field":{"path":root["tensor_field"]["name"]},"query_tensor":"query_embedding"},"annotations":annotations(step)}}},
            "ranking":{"profile":"semantic"},
            "inputs":[{"name":"query_embedding","input_type":"LiteralQueryInput","input":{"value":dense(root,query_values)}}],
            "presentation":{"fields":{"fields":[]}},
            "limit":{"value":step["limit"]}
        }),
    );
    assert_eq!(response["payload_type"], "SearchKernel_Wire_SearchResult");
    (hits(&response), traces.next())
}
fn feed(d: &Dispatcher, root: &Json) {
    for item in root["documents"].as_array().unwrap() {
        let response = dispatch(
            d,
            "SearchKernel_Wire_InsertRequest",
            json!({"document":doc(root,item)}),
        );
        assert_eq!(response["payload_type"], "SearchKernel_Wire_InsertResult");
    }
}
fn patch(d: &Dispatcher, root: &Json, id: &str, values: &Json) {
    let response = dispatch(
        d,
        "SearchKernel_Wire_PatchRequest",
        json!({"id":id,"operations":[{"operation_type":"SetField","operation":{"field":root["tensor_field"]["name"],"value":dense(root,values)}}]}),
    );
    assert_eq!(response["payload_type"], "SearchKernel_Wire_PatchResult");
}
fn run(root: &Json, definition: &Path, config: &Path, data: &Path) -> Json {
    if root["id"] == "hnsw-query-controls" {
        let d = open(definition, config, data);
        feed(&d, root);
        let mut traces = TraceReader::new();
        let mut steps = Vec::new();
        let mut evidence = Vec::new();
        for step in root["steps"].as_array().unwrap() {
            let (hits, trace) = query(&d, root, step, &root["query"], &mut traces);
            steps.push(json!({"id":step["id"],"hits":hits}));
            evidence.push(json!({"id":step["id"],"trace":trace}));
        }
        let probe = json!({"target_hits":3,"approximate":true,"distance_threshold":0.01,"explore_additional_hits":1,"limit":3});
        let (probe_hits, probe_trace) =
            query(&d, root, &probe, &json!([0.0, 0.5, 0.5]), &mut traces);
        evidence
            .push(json!({"id":"threshold-probe","trace":probe_trace,"hit_count":probe_hits.len()}));
        d.close().unwrap();
        return json!({"id":root["id"],"steps":steps,"evidence":evidence});
    }
    let opts = &root["query_options"];
    let update_id = root["mutation"]["update_id"].as_str().unwrap();
    let delete_id = root["mutation"]["delete_id"].as_str().unwrap();
    let d = open(definition, config, data);
    feed(&d, root);
    let mut traces = TraceReader::new();
    let mut evidence = Vec::new();
    let (before_update, trace) = query(&d, root, opts, &root["query"], &mut traces);
    evidence.push(json!({"id":"before_update","trace":trace}));
    let probe = json!({"target_hits":3,"approximate":true,"distance_threshold":0.01,"explore_additional_hits":1,"limit":3});
    let (probe_hits, probe_trace) = query(&d, root, &probe, &json!([0.0, 0.5, 0.5]), &mut traces);
    evidence.push(json!({"id":"threshold-probe","trace":probe_trace,"hit_count":probe_hits.len()}));
    patch(&d, root, update_id, &root["mutation"]["update_values"]);
    let (after_update, trace) = query(&d, root, opts, &root["query"], &mut traces);
    evidence.push(json!({"id":"after_update","trace":trace}));
    d.close().unwrap();
    let d = open(definition, config, data);
    let (after_reopen, trace) = query(&d, root, opts, &root["query"], &mut traces);
    evidence.push(json!({"id":"after_reopen","trace":trace}));
    let deleted = dispatch(
        &d,
        "SearchKernel_Wire_DeleteRequest",
        json!({"id":delete_id}),
    );
    assert_eq!(deleted["payload"]["deleted"], true);
    let (after_delete, trace) = query(&d, root, opts, &root["query"], &mut traces);
    evidence.push(json!({"id":"after_delete","trace":trace}));
    d.close().unwrap();
    let d = open(definition, config, data);
    let (after_delete_reopen, trace) = query(&d, root, opts, &root["query"], &mut traces);
    evidence.push(json!({"id":"after_delete_reopen","trace":trace}));
    d.close().unwrap();
    json!({"id":root["id"],"before_update":before_update,"after_update":after_update,"after_reopen":after_reopen,"after_delete":after_delete,"after_delete_reopen":after_delete_reopen,"evidence":evidence})
}
fn main() {
    let mut a = std::env::args_os().skip(1);
    let scenario = PathBuf::from(a.next().expect("scenario"));
    let definition = PathBuf::from(a.next().expect("definition"));
    let config = PathBuf::from(a.next().expect("config"));
    let data = PathBuf::from(a.next().expect("data"));
    assert!(a.next().is_none());
    let root = load(&scenario);
    println!(
        "{}",
        serde_json::to_string(&run(&root, &definition, &config, &data)).unwrap()
    );
}
