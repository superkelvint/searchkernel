#[path = "support/m12.rs"]
mod m12;

use crate::m12::{canonical_response_hits, load, portable_request_json};
use searchkernel_dispatch::Dispatcher;
use searchkernel_ir::{index_definition_to_ir, Backend, BackendSession, IndexContext};
use searchkernel_ir_adapter::VespaBackend;
use searchkernel_protocol::json::request_json_to_flatbuffer;
use std::{path::PathBuf, sync::Arc};

fn main() {
    let mut a = std::env::args_os().skip(1);
    let scenario = PathBuf::from(a.next().expect("scenario"));
    let definition = PathBuf::from(a.next().expect("definition bfbs"));
    let config = PathBuf::from(a.next().expect("config"));
    let data = PathBuf::from(a.next().expect("data"));
    assert!(a.next().is_none());

    let definition_bytes = std::fs::read(definition).expect("read portable index definition");
    let root = searchkernel_ir::__flatbuffers::root::<
        searchkernel_ir::wire::v1::schema::IndexDefinition,
    >(&definition_bytes)
    .expect("parse portable index definition");
    let definition_ir = index_definition_to_ir(root).expect("definition to IR");
    let context = Arc::new(IndexContext::from_definition_ir(definition_ir));
    let backend: Arc<dyn Backend> =
        Arc::new(VespaBackend::open(&data, &config).expect("open prepared M12 fixture"));
    let dispatcher = Dispatcher::new(context, BackendSession::from_backend(backend));

    let scenario = load(&scenario);
    let mut steps = Vec::new();
    for step in scenario["steps"].as_array().expect("steps") {
        let request_json = portable_request_json(step);
        let request = request_json_to_flatbuffer(&request_json)
            .expect("standard FlatBuffers JSON -> request bytes");
        let response = dispatcher.dispatch(&request).expect("dispatch M12 request");
        steps.push(serde_json::json!({"id":step["id"],"hits":canonical_response_hits(&response)}));
    }
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({"id":scenario["id"],"steps":steps})).unwrap()
    );
}
