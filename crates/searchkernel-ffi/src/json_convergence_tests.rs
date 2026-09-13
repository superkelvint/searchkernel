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

fn native_search_request(
    query_text: Option<&str>,
    limit_value: Option<u32>,
    projected: bool,
) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let query = query_text.map(|text| builder.create_string(text));
    let limit = limit_value.map(api::OptionalUInt32::new);
    let projection = if projected {
        let projection_field = builder.create_string("title");
        let projection_fields = builder.create_vector(&[projection_field]);
        Some(api::FieldProjection::create(
            &mut builder,
            &api::FieldProjectionArgs {
                fields: Some(projection_fields),
            },
        ))
    } else {
        None
    };
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query,
            limit: limit.as_ref(),
            projection,
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

    let native_state = handle::lookup(native_handle).unwrap();
    let json_state = handle::lookup(json_handle).unwrap();
    let cases = [
        (
            RequestKind::Search,
            native_search_request(None, None, false),
            br#"{"protocol_version":1,"operation":"search","request":{}}"#.as_slice(),
        ),
        (
            RequestKind::Search,
            native_search_request(Some("wireless headphones"), None, false),
            br#"{"protocol_version":1,"operation":"search","request":{"query":"wireless headphones"}}"#.as_slice(),
        ),
        (
            RequestKind::Search,
            native_search_request(Some("wireless headphones"), Some(20), true),
            br#"{"protocol_version":1,"operation":"search","request":{"query":"wireless headphones","limit":20,"projection":["title"]}}"#.as_slice(),
        ),
    ];
    for (kind, native_inner, json) in cases {
        native_backend.clear();
        json_backend.clear();
        let native_request = wrap_request(kind, &native_inner);
        let json_request = request_json_to_flatbuffer(json).unwrap();
        let native_result = dispatch::dispatch(&native_state, &native_request);
        assert!(native_result.is_ok(), "native {kind:?}: {native_result:?}");
        let json_result = dispatch::dispatch(&json_state, &json_request);
        assert!(json_result.is_ok(), "json {kind:?}: {json_result:?}");
        assert_eq!(native_backend.calls(), json_backend.calls(), "{kind:?}");
    }

    assert!(handle::close(native_handle));
    assert!(handle::close(json_handle));
}
