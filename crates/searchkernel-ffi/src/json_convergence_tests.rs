use crate::generated::search_kernel::ffi::v1 as transport;
use crate::test_support::RecordingBackend;
use crate::test_support::RequestKind;
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
    finish_request(&mut builder, RequestKind::Search, request.as_union_value());
    builder.finished_data().to_vec()
}

fn finish_request(
    builder: &mut flatbuffers::FlatBufferBuilder<'_>,
    kind: RequestKind,
    payload: flatbuffers::WIPOffset<flatbuffers::UnionWIPOffset>,
) {
    let envelope = transport::RequestEnvelope::create(
        builder,
        &transport::RequestEnvelopeArgs {
            protocol_version: 1,
            payload_type: request_payload(kind),
            payload: Some(payload),
        },
    );
    builder.finish(envelope, Some(transport::REQUEST_ENVELOPE_IDENTIFIER));
}

fn request_payload(kind: RequestKind) -> transport::RequestPayload {
    match kind {
        RequestKind::Insert => transport::RequestPayload::SearchKernel_V1_InsertRequest,
        RequestKind::Upsert => transport::RequestPayload::SearchKernel_V1_UpsertRequest,
        RequestKind::Patch => transport::RequestPayload::SearchKernel_V1_PatchRequest,
        RequestKind::Delete => transport::RequestPayload::SearchKernel_V1_DeleteRequest,
        RequestKind::Get => transport::RequestPayload::SearchKernel_V1_GetRequest,
        RequestKind::Search => transport::RequestPayload::SearchKernel_V1_SearchRequest,
        RequestKind::Analyze => transport::RequestPayload::SearchKernel_V1_AnalyzeRequest,
        RequestKind::Describe => transport::RequestPayload::SearchKernel_V1_DescribeRequest,
        RequestKind::InspectQuery => transport::RequestPayload::SearchKernel_V1_InspectQueryRequest,
    }
}

fn native_insert_request() -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string("doc-1");
    let document = api::Document::create(
        &mut builder,
        &api::DocumentArgs {
            id: Some(id),
            fields: None,
        },
    );
    let request = api::InsertRequest::create(
        &mut builder,
        &api::InsertRequestArgs {
            document: Some(document),
        },
    );
    finish_request(&mut builder, RequestKind::Insert, request.as_union_value());
    builder.finished_data().to_vec()
}

fn native_upsert_request() -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string("doc-1");
    let document = api::Document::create(
        &mut builder,
        &api::DocumentArgs {
            id: Some(id),
            fields: None,
        },
    );
    let request = api::UpsertRequest::create(
        &mut builder,
        &api::UpsertRequestArgs {
            document: Some(document),
        },
    );
    finish_request(&mut builder, RequestKind::Upsert, request.as_union_value());
    builder.finished_data().to_vec()
}

fn native_delete_request() -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string("doc-1");
    let request =
        api::DeleteRequest::create(&mut builder, &api::DeleteRequestArgs { id: Some(id) });
    finish_request(&mut builder, RequestKind::Delete, request.as_union_value());
    builder.finished_data().to_vec()
}

fn native_get_request() -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string("doc-1");
    let request = api::GetRequest::create(
        &mut builder,
        &api::GetRequestArgs {
            id: Some(id),
            ..Default::default()
        },
    );
    finish_request(&mut builder, RequestKind::Get, request.as_union_value());
    builder.finished_data().to_vec()
}

fn native_patch_request() -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string("doc-1");
    let field = builder.create_string("title");
    let text = builder.create_string("updated");
    let string =
        api::StringValue::create(&mut builder, &api::StringValueArgs { value: Some(text) });
    let value = api::Value::create(
        &mut builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::StringValue,
            kind: Some(string.as_union_value()),
        },
    );
    let set = api::SetField::create(
        &mut builder,
        &api::SetFieldArgs {
            field: Some(field),
            value: Some(value),
        },
    );
    let operation = api::PatchOperation::create(
        &mut builder,
        &api::PatchOperationArgs {
            operation_type: api::PatchOperationKind::SetField,
            operation: Some(set.as_union_value()),
        },
    );
    let operations = builder.create_vector(&[operation]);
    let request = api::PatchRequest::create(
        &mut builder,
        &api::PatchRequestArgs {
            id: Some(id),
            operations: Some(operations),
        },
    );
    finish_request(&mut builder, RequestKind::Patch, request.as_union_value());
    builder.finished_data().to_vec()
}

fn native_analyze_request() -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let name = builder.create_string("standard");
    let analyzer = api::Analyzer::create(
        &mut builder,
        &api::AnalyzerArgs {
            name: Some(name),
            version: 1,
            options: None,
        },
    );
    let text = builder.create_string("wireless");
    let request = api::AnalyzeRequest::create(
        &mut builder,
        &api::AnalyzeRequestArgs {
            target_type: api::AnalyzeTarget::Analyzer,
            target: Some(analyzer.as_union_value()),
            text: Some(text),
            ..Default::default()
        },
    );
    finish_request(&mut builder, RequestKind::Analyze, request.as_union_value());
    builder.finished_data().to_vec()
}

fn native_describe_request() -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let request = api::DescribeRequest::create(&mut builder, &api::DescribeRequestArgs {});
    finish_request(
        &mut builder,
        RequestKind::Describe,
        request.as_union_value(),
    );
    builder.finished_data().to_vec()
}

fn native_inspect_query_request() -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let request = api::SearchRequest::create(&mut builder, &api::SearchRequestArgs::default());
    let inspect = api::InspectQueryRequest::create(
        &mut builder,
        &api::InspectQueryRequestArgs {
            request: Some(request),
        },
    );
    finish_request(
        &mut builder,
        RequestKind::InspectQuery,
        inspect.as_union_value(),
    );
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
            RequestKind::Insert,
            native_insert_request(),
            br#"{"protocol_version":1,"operation":"insert","request":{"document":{"id":"doc-1"}}}"#.as_slice(),
        ),
        (
            RequestKind::Upsert,
            native_upsert_request(),
            br#"{"protocol_version":1,"operation":"upsert","request":{"document":{"id":"doc-1"}}}"#.as_slice(),
        ),
        (
            RequestKind::Patch,
            native_patch_request(),
            br#"{"protocol_version":1,"operation":"patch","request":{"id":"doc-1","operations":[{"set_field":{"field":"title","value":{"string":"updated"}}}]}}"#.as_slice(),
        ),
        (
            RequestKind::Delete,
            native_delete_request(),
            br#"{"protocol_version":1,"operation":"delete","request":{"id":"doc-1"}}"#.as_slice(),
        ),
        (
            RequestKind::Get,
            native_get_request(),
            br#"{"protocol_version":1,"operation":"get","request":{"id":"doc-1"}}"#.as_slice(),
        ),
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
        (
            RequestKind::Analyze,
            native_analyze_request(),
            br#"{"protocol_version":1,"operation":"analyze","request":{"target":{"analyzer":{"name":"standard","version":1}},"text":"wireless"}}"#.as_slice(),
        ),
        (
            RequestKind::Describe,
            native_describe_request(),
            br#"{"protocol_version":1,"operation":"describe","request":{}}"#.as_slice(),
        ),
        (
            RequestKind::InspectQuery,
            native_inspect_query_request(),
            br#"{"protocol_version":1,"operation":"inspect_query","request":{"request":{}}}"#.as_slice(),
        ),
    ];
    for (kind, native_inner, json) in cases {
        native_backend.clear();
        json_backend.clear();
        let native_request = native_inner;
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
