#[path = "support/m13.rs"]
mod m13;

use crate::m13::load;
use searchkernel_dispatch::Dispatcher;
use searchkernel_ir::wire::v1::index_definition_to_ir;
use searchkernel_ir::{Backend, BackendSession, IndexContext};
use searchkernel_ir_adapter::VespaBackend;
use searchkernel_protocol::json::{request_json_to_flatbuffer, response_flatbuffer_to_json};
use serde_json::{json, Value as Json};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

fn context(path: &Path) -> Arc<IndexContext> {
    let bytes = fs::read(path).expect("read M13 portable definition");
    let root = searchkernel_ir::__flatbuffers::root::<
        searchkernel_ir::wire::v1::schema::IndexDefinition,
    >(&bytes)
    .expect("parse M13 portable definition");
    Arc::new(IndexContext::from_definition_ir(
        index_definition_to_ir(root).expect("convert M13 portable definition"),
    ))
}

fn create_dispatcher(definition: &Path, config: &Path, data: &Path) -> Dispatcher {
    if !config.join("searchkernel-schema.json").exists() {
        let definition_bytes = fs::read(definition).expect("read M13 portable definition");
        let root = searchkernel_ir::__flatbuffers::root::<
            searchkernel_ir::wire::v1::schema::IndexDefinition,
        >(&definition_bytes)
        .expect("parse M13 portable definition");
        let definition_ir = index_definition_to_ir(root).expect("convert M13 portable definition");
        searchkernel_ir_adapter::compile_index_definition(&definition_ir, config)
            .expect("compile M13 portable definition");
    }
    let backend: Arc<dyn Backend> =
        Arc::new(VespaBackend::open(data, config).expect("open M13 portable backend"));
    Dispatcher::new(context(definition), BackendSession::from_backend(backend))
}

fn dense_value(root: &Json, name: &str) -> Json {
    json!({
        "kind_type":"TensorValue",
        "kind":{
            "type_spec":root["tensor_field"]["type_spec"],
            "payload_type":"DenseTensorValues",
            "payload":{"values":root["tensors"][name]}
        }
    })
}

fn document(root: &Json, id: &str, tensor_name: &str) -> Json {
    json!({
        "id":id,
        "fields":[{"key":root["tensor_field"]["name"],"value":dense_value(root, tensor_name)}]
    })
}

fn envelope(payload_type: &str, payload: Json) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "protocol_version":1,
        "payload_type":payload_type,
        "payload":payload
    }))
    .expect("encode M13 request JSON")
}

fn dispatch(dispatcher: &Dispatcher, payload_type: &str, payload: Json) -> Json {
    let request = request_json_to_flatbuffer(&envelope(payload_type, payload))
        .expect("encode M13 request FlatBuffer");
    let response = dispatcher.dispatch(&request).expect("dispatch M13 request");
    serde_json::from_slice(
        &response_flatbuffer_to_json(&response).expect("decode M13 response FlatBuffer"),
    )
    .expect("parse M13 response JSON")
}

fn get(dispatcher: &Dispatcher, id: &str, field: &str) -> Json {
    let response = dispatch(
        dispatcher,
        "SearchKernel_Wire_GetRequest",
        json!({"id":id,"projection":{"fields":[field]}}),
    );
    let fields = response["payload"]["document"]["fields"].as_array();
    let Some(fields) = fields else {
        return Json::Null;
    };
    let value = fields
        .iter()
        .find(|entry| entry["key"] == field)
        .map(|entry| &entry["value"])
        .expect("tensor field in portable GET");
    json!({
        "type_spec":value["kind"]["type_spec"],
        "values":value["kind"]["payload"]["values"]
    })
}

fn field_names(dispatcher: &Dispatcher, id: &str, field: &str) -> Json {
    let response = dispatch(
        dispatcher,
        "SearchKernel_Wire_GetRequest",
        json!({"id":id,"projection":{"fields":[field]}}),
    );
    response["payload"]["document"]["fields"]
        .as_array()
        .map(|fields| {
            Json::Array(
                fields
                    .iter()
                    .filter_map(|field| field["key"].as_str().map(String::from).map(Json::String))
                    .collect(),
            )
        })
        .unwrap_or(Json::Null)
}

fn patch(dispatcher: &Dispatcher, root: &Json, id: &str, tensor_name: Option<&str>) {
    let operation = match tensor_name {
        Some(name) => json!({
            "operation_type":"SetField",
            "operation":{"field":root["tensor_field"]["name"],"value":dense_value(root,name)}
        }),
        None => json!({
            "operation_type":"UnsetField",
            "operation":{"field":root["tensor_field"]["name"]}
        }),
    };
    let response = dispatch(
        dispatcher,
        "SearchKernel_Wire_PatchRequest",
        json!({"id":id,"operations":[operation]}),
    );
    assert_eq!(response["payload_type"], "SearchKernel_Wire_PatchResult");
}

fn search(dispatcher: &Dispatcher, root: &Json) -> Json {
    let response = dispatch(
        dispatcher,
        "SearchKernel_Wire_SearchRequest",
        json!({
            "query":{"source_type":"TypedQuery","source":{"query":{
                "expr_type":"QueryNearestNeighbor","expr":{
                    "document_field":{"path":root["tensor_field"]["name"]},
                    "query_tensor":"query_embedding"
                }
            }}},
            "ranking":{"profile":"semantic"},
            "inputs":[{"name":"query_embedding","input_type":"LiteralQueryInput","input":{"value":dense_value(root,"query")}}],
            "presentation":{"fields":{"fields":[]}},
            "limit":{"value":2}
        }),
    );
    let mut ids = Vec::new();
    fn visit(value: &Json, ids: &mut Vec<Json>) {
        match value["node_type"].as_str() {
            Some("Hit") | Some("ResultHit") => {
                if let Some(id) = value["node"]["id"].as_str() {
                    ids.push(Json::String(id.to_owned()));
                }
            }
            Some("ResultGroup") => {
                if let Some(children) = value["node"]["children"].as_array() {
                    for child in children {
                        visit(child, ids);
                    }
                }
            }
            _ => {}
        }
    }
    if let Some(root) = response["payload"]["root"].as_object() {
        visit(&Json::Object(root.clone()), &mut ids);
    }
    Json::Array(ids)
}

fn run(root: &Json, definition: &Path, config: &Path, data: &Path) -> Json {
    let field = root["tensor_field"]["name"].as_str().expect("tensor field");
    let id_a = "id:ns:typea::a";
    let id_b = "id:ns:typea::b";
    let dispatcher = create_dispatcher(definition, config, data);
    assert_eq!(
        dispatch(
            &dispatcher,
            "SearchKernel_Wire_InsertRequest",
            json!({"document":document(root,id_a,"a")})
        )["payload_type"],
        "SearchKernel_Wire_InsertResult"
    );
    dispatch(
        &dispatcher,
        "SearchKernel_Wire_InsertRequest",
        json!({"document":document(root,id_b,"b")}),
    );
    let inserted = get(&dispatcher, id_a, field);
    let nn_before_patch = search(&dispatcher, root);
    dispatch(
        &dispatcher,
        "SearchKernel_Wire_UpsertRequest",
        json!({"document":document(root,id_a,"b")}),
    );
    let upserted = get(&dispatcher, id_a, field);
    patch(&dispatcher, root, id_a, Some("replacement"));
    let patched = get(&dispatcher, id_a, field);
    dispatcher.close().expect("close portable M13 backend");

    let dispatcher = create_dispatcher(definition, config, data);
    let persisted_patch = get(&dispatcher, id_a, field);
    let nn_after_patch = search(&dispatcher, root);
    patch(&dispatcher, root, id_a, None);
    let unset_fields = field_names(&dispatcher, id_a, field);
    patch(&dispatcher, root, id_a, Some("reassign"));
    let reassigned = get(&dispatcher, id_a, field);
    dispatcher
        .close()
        .expect("close portable M13 reassign backend");

    let dispatcher = create_dispatcher(definition, config, data);
    let persisted_reassign = get(&dispatcher, id_a, field);
    let deleted = dispatch(
        &dispatcher,
        "SearchKernel_Wire_DeleteRequest",
        json!({"id":id_a}),
    );
    let deleted_get = get(&dispatcher, id_a, field).is_null();
    dispatcher
        .close()
        .expect("final close portable M13 backend");

    json!({
        "id":root["id"], "inserted":inserted, "upserted":upserted,
        "patched":patched, "persisted_patch":persisted_patch,
        "nn_before_patch":nn_before_patch, "nn_after_patch":nn_after_patch,
        "unset_fields":unset_fields,
        "reassigned":reassigned, "persisted_reassign":persisted_reassign,
        "deleted":deleted["payload"]["deleted"], "deleted_get":deleted_get
    })
}

fn main() {
    let mut args = std::env::args_os().skip(1);
    let scenario = PathBuf::from(args.next().expect("scenario"));
    let definition = PathBuf::from(args.next().expect("definition"));
    let config = PathBuf::from(args.next().expect("config"));
    let data = PathBuf::from(args.next().expect("data"));
    assert!(args.next().is_none());
    let root = load(&scenario);
    println!(
        "{}",
        serde_json::to_string(&run(&root, &definition, &config, &data)).unwrap()
    );
}
