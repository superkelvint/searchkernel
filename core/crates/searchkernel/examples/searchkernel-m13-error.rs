#[path = "support/m13.rs"]
mod m13;

use crate::m13::{compile_and_open, load};
use searchkernel_dispatch::Dispatcher;
use searchkernel_ir::{wire::v1::index_definition_to_ir, Backend, BackendSession, IndexContext};
use searchkernel_ir_adapter::VespaBackend;
use searchkernel_protocol::json::{request_json_to_flatbuffer, response_flatbuffer_to_json};
use searchkernel_vespa_engine::{
    Document, DocumentUpdate, ScalarValue, TensorAddress, TensorBlock, TensorCell, TensorValue,
};
use serde_json::{json, Value as Json};
use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::Arc,
};

fn dense_json(case: &Json) -> Json {
    json!({"kind_type":"TensorValue","kind":{"type_spec":case["type_spec"],"payload_type":"DenseTensorValues","payload":{"values":case["values"]}}})
}

fn tensor_value(case: &Json) -> TensorValue {
    let type_spec = case["type_spec"].as_str().expect("tensor type spec");
    match case["payload"].as_str().unwrap_or("dense") {
        "dense" => TensorValue::dense(
            type_spec,
            case["values"]
                .as_array()
                .expect("dense values")
                .iter()
                .map(|value| match value.as_f64() {
                    Some(value) => value,
                    None if value.as_str() == Some("nan") => f64::NAN,
                    _ => panic!("numeric dense value"),
                })
                .collect::<Vec<_>>(),
        ),
        "sparse" => TensorValue::Sparse {
            type_spec: type_spec.to_owned(),
            cells: vec![TensorCell {
                address: vec![TensorAddress::new("x", "0")],
                value: 1.0,
            }],
        },
        "mixed" => TensorValue::Mixed {
            type_spec: type_spec.to_owned(),
            blocks: vec![TensorBlock {
                address: vec![],
                values: vec![1.0, 0.0],
            }],
        },
        payload => panic!("unknown tensor payload {payload}"),
    }
}

fn engine_category(error: &searchkernel_vespa_engine::Error) -> String {
    if let Some(code) = error.detail_code() {
        return code.to_owned();
    }
    if error.kind() == searchkernel_vespa_engine::ErrorKind::Unsupported {
        return "tensor_value_not_supported".to_owned();
    }
    "backend_request".to_owned()
}

fn low_level(root: &Json, case: &Json, config: &Path, data: &Path) -> String {
    let engine = compile_and_open(root, config, data);
    let id = "id:ns:typea::negative";
    let field = case["field"].as_str().expect("field");
    if case["operation"] == "patch" {
        engine
            .put(Document::new(id).field(
                "embedding",
                TensorValue::dense("tensor<float>(x[2])", vec![1.0, 0.0]),
            ))
            .expect("baseline put");
    }
    let result = if let Some(scalar) = case["scalar"].as_f64() {
        if case["operation"] == "patch" {
            engine.update(DocumentUpdate::new(id).assign(field, ScalarValue::Float(scalar)))
        } else {
            engine.put(Document::new(id).field(field, ScalarValue::Float(scalar)))
        }
    } else if case["operation"] == "patch" {
        engine.update(DocumentUpdate::new(id).assign(field, tensor_value(case)))
    } else {
        engine.put(Document::new(id).field(field, tensor_value(case)))
    };
    let error = result.expect_err("negative low-level operation unexpectedly succeeded");
    let category = engine_category(&error);
    engine.close().expect("close negative low-level engine");
    category
}

fn context(definition: &Path) -> Arc<IndexContext> {
    let bytes = fs::read(definition).expect("read definition");
    let root = searchkernel_ir::__flatbuffers::root::<
        searchkernel_ir::wire::v1::schema::IndexDefinition,
    >(&bytes)
    .expect("parse definition");
    Arc::new(IndexContext::from_definition_ir(
        index_definition_to_ir(root).expect("definition IR"),
    ))
}

fn portable_value(case: &Json) -> Json {
    if case.get("scalar").is_some() {
        return json!({"kind_type":"Float64Value","kind":{"value":case["scalar"]}});
    }
    match case["payload"].as_str().unwrap_or("dense") {
        "dense" => dense_json(case),
        "sparse" => {
            json!({"kind_type":"TensorValue","kind":{"type_spec":case["type_spec"],"payload_type":"SparseTensorCells","payload":{"cells":[{"address":[{"dimension":"x","label":"0"}],"value":1.0}]}}})
        }
        "mixed" => {
            json!({"kind_type":"TensorValue","kind":{"type_spec":case["type_spec"],"payload_type":"MixedTensorBlocks","payload":{"blocks":[{"address":[],"values":[1.0,0.0]}]}}})
        }
        payload => panic!("unknown tensor payload {payload}"),
    }
}

fn envelope(case: &Json, operation: &str) -> Vec<u8> {
    let field = case["field"].as_str().expect("field");
    let id = "id:ns:typea::negative";
    let payload = if operation == "patch" {
        json!({"id":id,"operations":[{"operation_type":"SetField","operation":{"field":field,"value":portable_value(case)}}]})
    } else {
        json!({"document":{"id":id,"fields":[{"key":field,"value":portable_value(case)}]}})
    };
    let payload_type = if operation == "patch" {
        "SearchKernel_Wire_PatchRequest"
    } else {
        "SearchKernel_Wire_InsertRequest"
    };
    let mut bytes = serde_json::to_vec(
        &json!({"protocol_version":1,"payload_type":payload_type,"payload":payload}),
    )
    .expect("encode negative request");
    if case["portable_non_finite"] == true {
        let text = String::from_utf8(bytes)
            .expect("request utf8")
            .replace("1.0,0.0", "1e309,0.0");
        bytes = text.into_bytes();
    }
    bytes
}

fn portable(case: &Json, definition: &Path, config: &Path, data: &Path) -> String {
    if !config.join("searchkernel-schema.json").exists() {
        let bytes = fs::read(definition).expect("read definition");
        let root = searchkernel_ir::__flatbuffers::root::<
            searchkernel_ir::wire::v1::schema::IndexDefinition,
        >(&bytes)
        .expect("parse definition");
        let ir = index_definition_to_ir(root).expect("definition IR");
        searchkernel_ir_adapter::compile_index_definition(&ir, config).expect("compile definition");
    }
    let backend: Arc<dyn Backend> =
        Arc::new(VespaBackend::open(data, config).expect("open portable backend"));
    let dispatcher = Dispatcher::new(context(definition), BackendSession::from_backend(backend));
    if case["operation"] == "patch" {
        let baseline = json!({"document":{"id":"id:ns:typea::negative","fields":[{"key":"embedding","value":{"kind_type":"TensorValue","kind":{"type_spec":"tensor<float>(x[2])","payload_type":"DenseTensorValues","payload":{"values":[1.0,0.0]}}}}]}});
        let baseline_bytes = serde_json::to_vec(&json!({"protocol_version":1,"payload_type":"SearchKernel_Wire_InsertRequest","payload":baseline})).expect("serialize baseline");
        let request = request_json_to_flatbuffer(&baseline_bytes).expect("encode baseline");
        dispatcher.dispatch(&request).expect("dispatch baseline");
    }
    let request = match request_json_to_flatbuffer(&envelope(
        case,
        case["operation"].as_str().expect("operation"),
    )) {
        Ok(request) => request,
        Err(_) => return "request_conversion".to_owned(),
    };
    let response = dispatcher
        .dispatch(&request)
        .expect("dispatch negative request");
    let json: Json = serde_json::from_slice(
        &response_flatbuffer_to_json(&response).expect("decode error response"),
    )
    .expect("parse error response");
    let category = json["payload"]["details"][0]["code"]
        .as_str()
        .expect("error detail code")
        .to_owned();
    dispatcher.close().expect("close portable negative backend");
    category
}

fn main() {
    let mut args = env::args_os().skip(1);
    let errors_path = PathBuf::from(args.next().expect("errors path"));
    let case_id = args.next().expect("case id").to_string_lossy().into_owned();
    let definition = PathBuf::from(args.next().expect("definition"));
    let config = PathBuf::from(args.next().expect("config"));
    let data = PathBuf::from(args.next().expect("data"));
    assert!(args.next().is_none());
    let errors = load(&errors_path);
    let case = errors["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|case| case["id"] == case_id)
        .expect("case id");
    let root = load(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../reference/vespa/m13/scenarios/01-dense-crud.json"),
    );
    let category = if case["path"] == "low_level" {
        low_level(&root, case, &config, &data)
    } else {
        portable(case, &definition, &config, &data)
    };
    println!("{category}");
}
