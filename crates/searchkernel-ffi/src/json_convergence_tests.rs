use crate::test_support::RecordingBackend;
use crate::test_support::{wrap_request, RequestKind};
use crate::{dispatch, handle};
use searchkernel_core::{api, IndexContext};
use searchkernel_json::request_json_to_flatbuffer;
use std::sync::Arc;

fn context() -> Arc<IndexContext> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let name = builder.create_string("title");
    let stored = api::OptionalBool::new(true);
    let searchable = api::OptionalBool::new(true);
    let analyzer_name = builder.create_string("standard");
    let analyzer = api::Analyzer::create(
        &mut builder,
        &api::AnalyzerArgs {
            name: Some(analyzer_name),
            version: 1,
            options: None,
        },
    );
    let field = api::FieldDefinition::create(
        &mut builder,
        &api::FieldDefinitionArgs {
            name: Some(name),
            kind: api::FieldKind::Text,
            cardinality: api::FieldCardinality::Single,
            stored: Some(&stored),
            searchable: Some(&searchable),
            analyzer: Some(analyzer),
            ..Default::default()
        },
    );
    let fields = builder.create_vector(&[field]);
    let definition = api::IndexDefinition::create(
        &mut builder,
        &api::IndexDefinitionArgs {
            fields: Some(fields),
            ..Default::default()
        },
    );
    builder.finish(definition, None);
    let bytes = builder.finished_data().to_vec();
    Arc::new(
        IndexContext::from_api(flatbuffers::root::<api::IndexDefinition>(&bytes).unwrap()).unwrap(),
    )
}

fn native_search_request() -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let query = builder.create_string("wireless headphones");
    let limit = api::OptionalUInt32::new(20);
    let projection_field = builder.create_string("title");
    let projection_fields = builder.create_vector(&[projection_field]);
    let projection = api::FieldProjection::create(
        &mut builder,
        &api::FieldProjectionArgs {
            fields: Some(projection_fields),
        },
    );
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(query),
            limit: Some(&limit),
            projection: Some(projection),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    builder.finished_data().to_vec()
}

#[test]
fn native_and_json_requests_converge_before_backend_semantics() {
    let context = context();
    let native_backend = Arc::new(RecordingBackend::new());
    let json_backend = Arc::new(RecordingBackend::new());
    let native_handle =
        crate::test_support::register_index(context.clone(), native_backend.clone());
    let json_handle = crate::test_support::register_index(context, json_backend.clone());

    let native_request = wrap_request(RequestKind::Search, &native_search_request());
    let json_request = request_json_to_flatbuffer(
        br#"{"protocol_version":1,"operation":"search","request":{"query":"wireless headphones","limit":20,"projection":["title"]}}"#,
    )
    .unwrap();

    let native_state = handle::lookup(native_handle).unwrap();
    let json_state = handle::lookup(json_handle).unwrap();
    assert!(dispatch::dispatch(&native_state, &native_request).is_ok());
    assert!(dispatch::dispatch(&json_state, &json_request).is_ok());
    assert_eq!(native_backend.calls(), json_backend.calls());

    assert!(handle::close(native_handle));
    assert!(handle::close(json_handle));
}
