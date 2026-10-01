use super::*;
use crate::wire::v1::encode_result;
use flatbuffers::root;
use searchkernel_ir::{wire::v1::schema as api, OperationIr};
use searchkernel_protocol::protocol::v1::{self as wire, PROTOCOL_VERSION};
use searchkernel_testkit::{
    assert_api_search_result_matches, canonical_result_fixtures, representative_search_result,
    ExpectedCall, RecordingBackend, ScriptedBackend, ScriptedResult,
};
use std::fs;

#[test]
fn first_released_protocol_major_is_one() {
    assert_eq!(PROTOCOL_VERSION, 1);
}

#[test]
fn invalid_search_result_encoding_becomes_error_response_without_panicking() {
    let mut invalid = searchkernel_ir::SearchResult::empty(searchkernel_ir::TotalHits {
        value: 0,
        relation: searchkernel_ir::TotalHitsRelation::Exact,
    });
    invalid.root = searchkernel_ir::ResultNode::Hit(searchkernel_ir::Hit {
        score: Some(f64::NAN),
        ..Default::default()
    });

    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        encode_result(
            Ok(invalid),
            wire::ResponsePayload::SearchKernel_Wire_SearchResult,
            |builder, result| {
                searchkernel_ir::wire::v1::search_result_offset(builder, result)
                    .map(|result| result.as_union_value())
            },
        )
    }));
    let response = outcome.expect("invalid result encoding must not panic");
    let envelope = root::<wire::ResponseEnvelope<'_>>(&response).unwrap();
    assert_eq!(
        envelope.payload_type(),
        wire::ResponsePayload::SearchKernel_Wire_Error
    );
    let error = envelope.payload_as_search_kernel_wire_error().unwrap();
    assert_eq!(error.code(), api::ErrorCode::InvalidArgument);
    assert_eq!(
        error.details().unwrap().get(0).code(),
        Some("non_finite_relevance")
    );
    assert_eq!(
        error.details().unwrap().get(0).path(),
        Some("root.relevance")
    );
}

fn context() -> Arc<IndexContext> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let fields = [
        ("title", api::PrimitiveDataTypeKind::String, true, true),
        ("brand", api::PrimitiveDataTypeKind::String, false, false),
        ("sku", api::PrimitiveDataTypeKind::String, false, false),
        ("price", api::PrimitiveDataTypeKind::Float64, false, false),
        (
            "popularity",
            api::PrimitiveDataTypeKind::Int64,
            false,
            false,
        ),
        ("in_stock", api::PrimitiveDataTypeKind::Bool, false, false),
        (
            "image_url",
            api::PrimitiveDataTypeKind::String,
            false,
            false,
        ),
        (
            "description",
            api::PrimitiveDataTypeKind::String,
            false,
            true,
        ),
        ("category", api::PrimitiveDataTypeKind::String, false, false),
        ("tags", api::PrimitiveDataTypeKind::String, false, false),
    ]
    .into_iter()
    .map(|(field_name, kind, stored, searchable)| {
        let name = builder.create_string(field_name);
        let primitive =
            api::PrimitiveDataType::create(&mut builder, &api::PrimitiveDataTypeArgs { kind });
        let data_type = api::DataType::create(
            &mut builder,
            &api::DataTypeArgs {
                kind_type: api::DataTypeKind::PrimitiveDataType,
                kind: Some(primitive.as_union_value()),
            },
        );
        let data_type = if field_name == "tags" {
            let array = api::ArrayDataType::create(
                &mut builder,
                &api::ArrayDataTypeArgs {
                    element_type: Some(data_type),
                },
            );
            api::DataType::create(
                &mut builder,
                &api::DataTypeArgs {
                    kind_type: api::DataTypeKind::ArrayDataType,
                    kind: Some(array.as_union_value()),
                },
            )
        } else {
            data_type
        };
        let stored = api::OptionalBool::new(stored);
        let searchable = api::OptionalBool::new(searchable);
        api::FieldDefinition::create(
            &mut builder,
            &api::FieldDefinitionArgs {
                name: Some(name),
                data_type: Some(data_type),
                stored: Some(&stored),
                searchable: Some(&searchable),
                ..Default::default()
            },
        )
    })
    .collect::<Vec<_>>();
    let fields = builder.create_vector(&fields);
    let definition = api::IndexDefinition::create(
        &mut builder,
        &api::IndexDefinitionArgs {
            fields: Some(fields),
            ..Default::default()
        },
    );
    builder.finish(definition, None);
    Arc::new(
        IndexContext::from_api(root::<api::IndexDefinition<'_>>(builder.finished_data()).unwrap())
            .unwrap(),
    )
}

fn describe_request(version: u32, kind: wire::RequestPayload) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let request = api::DescribeRequest::create(&mut builder, &api::DescribeRequestArgs::default());
    let envelope = wire::RequestEnvelope::create(
        &mut builder,
        &wire::RequestEnvelopeArgs {
            protocol_version: version,
            target: None,
            payload_type: kind,
            payload: Some(request.as_union_value()),
        },
    );
    builder.finish(envelope, Some("SKRQ"));
    builder.finished_data().to_vec()
}

fn lifecycle_request(kind: wire::RequestPayload) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let target_name = builder.create_string("products");
    let target = api::IndexTarget::create(
        &mut builder,
        &api::IndexTargetArgs {
            name: Some(target_name),
            generation: None,
        },
    );
    let payload = match kind {
        wire::RequestPayload::SearchKernel_Wire_CreateIndexRequest => {
            let definition =
                api::IndexDefinition::create(&mut builder, &api::IndexDefinitionArgs::default());
            api::CreateIndexRequest::create(
                &mut builder,
                &api::CreateIndexRequestArgs {
                    definition: Some(definition),
                },
            )
            .as_union_value()
        }
        wire::RequestPayload::SearchKernel_Wire_OpenIndexRequest => {
            api::OpenIndexRequest::create(&mut builder, &api::OpenIndexRequestArgs::default())
                .as_union_value()
        }
        other => panic!("not a lifecycle request: {other:?}"),
    };
    let envelope = wire::RequestEnvelope::create(
        &mut builder,
        &wire::RequestEnvelopeArgs {
            protocol_version: PROTOCOL_VERSION,
            target: Some(target),
            payload_type: kind,
            payload: Some(payload),
        },
    );
    wire::finish_request_envelope_buffer(&mut builder, envelope);
    builder.finished_data().to_vec()
}

fn empty_search_request() -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let request = api::SearchRequest::create(&mut builder, &api::SearchRequestArgs::default());
    let envelope = wire::RequestEnvelope::create(
        &mut builder,
        &wire::RequestEnvelopeArgs {
            protocol_version: PROTOCOL_VERSION,
            target: None,
            payload_type: wire::RequestPayload::SearchKernel_Wire_SearchRequest,
            payload: Some(request.as_union_value()),
        },
    );
    builder.finish(envelope, Some("SKRQ"));
    builder.finished_data().to_vec()
}

fn canonical_search_request() -> Vec<u8> {
    fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/portable/requests/search.defaults.skrq"),
    )
    .unwrap()
}

fn expected_search_operation() -> OperationIr {
    let recording = Arc::new(RecordingBackend::new());
    let dispatcher = Dispatcher::from_backend(context(), recording.clone());
    dispatcher.dispatch(&canonical_search_request()).unwrap();
    recording
        .take_calls()
        .into_iter()
        .next()
        .expect("canonical search request must reach the backend")
}

#[test]
fn targetless_embedded_request_remains_valid_without_daemon_routing() {
    let request = canonical_search_request();
    let envelope = root::<wire::RequestEnvelope<'_>>(&request).unwrap();

    assert_eq!(envelope.protocol_version(), PROTOCOL_VERSION);
    assert_eq!(
        envelope.payload_type(),
        wire::RequestPayload::SearchKernel_Wire_SearchRequest
    );
    assert!(envelope.target().is_none());
    assert!(envelope.target().is_none());
}

#[test]
fn scripted_dispatch_preserves_every_canonical_result_fixture() {
    for (name, expected) in canonical_result_fixtures() {
        let backend = Arc::new(ScriptedBackend::new([ExpectedCall::success(
            expected_search_operation(),
            ScriptedResult::Search(Box::new(expected.clone())),
        )]));
        let dispatcher = Dispatcher::from_backend(context(), backend);
        let response = dispatcher
            .dispatch(&canonical_search_request())
            .unwrap_or_else(|error| panic!("fixture {name} dispatch failed: {error:?}"));
        let envelope = root::<wire::ResponseEnvelope<'_>>(&response).unwrap();
        let actual = envelope
            .payload_as_search_kernel_wire_search_result()
            .unwrap_or_else(|| panic!("fixture {name} did not produce SearchResult"));
        assert_api_search_result_matches(actual, &expected);
    }
}

#[test]
fn accepts_v2_and_rejects_stale_or_future_protocols() {
    let dispatcher = Dispatcher::from_backend(context(), Arc::new(RecordingBackend::new()));
    assert!(dispatcher
        .dispatch(&describe_request(
            PROTOCOL_VERSION,
            wire::RequestPayload::SearchKernel_Wire_DescribeRequest
        ))
        .is_ok());
    assert_eq!(
        dispatcher.dispatch(&describe_request(
            1,
            wire::RequestPayload::SearchKernel_Wire_DescribeRequest
        )),
        Err(DispatchError::UnsupportedProtocol(1))
    );
    assert_eq!(
        dispatcher.dispatch(&describe_request(
            PROTOCOL_VERSION + 1,
            wire::RequestPayload::SearchKernel_Wire_DescribeRequest
        )),
        Err(DispatchError::UnsupportedProtocol(PROTOCOL_VERSION + 1))
    );
}

#[test]
fn per_index_dispatcher_does_not_own_daemon_lifecycle() {
    let dispatcher = Dispatcher::from_backend(context(), Arc::new(RecordingBackend::new()));
    for kind in [
        wire::RequestPayload::SearchKernel_Wire_CreateIndexRequest,
        wire::RequestPayload::SearchKernel_Wire_OpenIndexRequest,
    ] {
        assert_eq!(
            dispatcher.dispatch(&lifecycle_request(kind)),
            Err(DispatchError::UnsupportedOperation(kind.0))
        );
    }
}

#[test]
fn v2_producers_encode_protocol_version_explicitly() {
    let request = describe_request(
        PROTOCOL_VERSION,
        wire::RequestPayload::SearchKernel_Wire_DescribeRequest,
    );
    let request_envelope = root::<wire::RequestEnvelope<'_>>(&request).unwrap();
    assert_ne!(
        request_envelope
            ._tab
            .vtable()
            .get(wire::RequestEnvelope::VT_PROTOCOL_VERSION),
        0,
        "canonical requests must physically encode protocol_version"
    );

    let dispatcher = Dispatcher::from_backend(context(), Arc::new(RecordingBackend::new()));
    let response = dispatcher.dispatch(&request).unwrap();
    let response_envelope = root::<wire::ResponseEnvelope<'_>>(&response).unwrap();
    assert_ne!(
        response_envelope
            ._tab
            .vtable()
            .get(wire::ResponseEnvelope::VT_PROTOCOL_VERSION),
        0,
        "canonical responses must physically encode protocol_version"
    );
}

#[test]
fn frozen_v1_envelope_with_omitted_version_is_rejected() {
    // Generated with the pre-V2 schema at commit bf38531. Its V1 default
    // version is physically absent, exactly as a real legacy writer emits.
    let request = include_bytes!("../tests/fixtures/stale_v1_describe.skrq");
    let envelope = root::<wire::RequestEnvelope<'_>>(request).unwrap();
    assert_eq!(envelope.protocol_version(), 0);

    let dispatcher = Dispatcher::from_backend(context(), Arc::new(RecordingBackend::new()));
    assert_eq!(
        dispatcher.dispatch(request),
        Err(DispatchError::UnsupportedProtocol(0))
    );
}

#[test]
fn rejects_malformed_unknown_and_closed_requests() {
    let recording = Arc::new(RecordingBackend::new());
    let dispatcher = Dispatcher::from_backend(context(), recording.clone());
    assert_eq!(
        dispatcher.dispatch(b"not a flatbuffer"),
        Err(DispatchError::InvalidRequest)
    );
    assert_eq!(
        dispatcher.dispatch(&describe_request(
            PROTOCOL_VERSION,
            wire::RequestPayload(250)
        )),
        Err(DispatchError::UnsupportedOperation(250))
    );
    let valid_insert = fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/portable/requests/write.insert.skrq"),
    )
    .unwrap();
    // FlatBuffers permits trailing padding, so exclude the final padding
    // byte and exercise every prefix that removes referenced structure.
    for end in 0..valid_insert.len() - 1 {
        assert_eq!(
            dispatcher.dispatch(&valid_insert[..end]),
            Err(DispatchError::InvalidRequest),
            "truncated request prefix {end}"
        );
    }
    assert!(
        recording.take_calls().is_empty(),
        "malformed and truncated envelopes must not reach the backend"
    );

    let response = dispatcher
        .dispatch(&describe_request(
            PROTOCOL_VERSION,
            wire::RequestPayload::SearchKernel_Wire_DescribeRequest,
        ))
        .unwrap();
    assert_eq!(
        dispatcher.dispatch(&response),
        Err(DispatchError::InvalidRequest),
        "a response envelope cannot be reused as a request"
    );
    dispatcher.close().unwrap();
    let response = dispatcher
        .dispatch(&describe_request(
            PROTOCOL_VERSION,
            wire::RequestPayload::SearchKernel_Wire_DescribeRequest,
        ))
        .expect("closed target is a canonical response");
    let envelope = root::<wire::ResponseEnvelope<'_>>(&response).unwrap();
    assert_eq!(
        envelope.payload_type(),
        wire::ResponsePayload::SearchKernel_Wire_Error
    );
    let error = envelope.payload_as_search_kernel_wire_error().unwrap();
    assert_eq!(error.code(), api::ErrorCode::FailedPrecondition);
    assert_eq!(error.message(), Some("Search Kernel target is closed"));
    let details = error.details().expect("closed target detail");
    assert_eq!(details.len(), 1);
    assert_eq!(details.get(0).code(), Some("target_closed"));
}

#[test]
fn semantic_errors_are_portable_error_responses() {
    let dispatcher = Dispatcher::from_backend(context(), Arc::new(RecordingBackend::new()));
    let response = dispatcher.dispatch(&empty_search_request()).unwrap();
    let envelope = root::<wire::ResponseEnvelope<'_>>(&response).unwrap();
    assert_eq!(
        envelope.payload_type(),
        wire::ResponsePayload::SearchKernel_Wire_Error
    );
    assert!(envelope.payload_as_search_kernel_wire_error().is_some());
}

#[test]
fn routes_every_operation_and_encodes_its_canonical_response() {
    let fixture_root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/portable/requests");
    let backend_cases = [
        (
            "write.insert.skrq",
            wire::ResponsePayload::SearchKernel_Wire_InsertResult,
        ),
        (
            "write.upsert.skrq",
            wire::ResponsePayload::SearchKernel_Wire_UpsertResult,
        ),
        (
            "write.patch-set.skrq",
            wire::ResponsePayload::SearchKernel_Wire_PatchResult,
        ),
        (
            "write.update-assign.skrq",
            wire::ResponsePayload::SearchKernel_Wire_UpdateResult,
        ),
        (
            "write.delete.skrq",
            wire::ResponsePayload::SearchKernel_Wire_DeleteResult,
        ),
        (
            "write.get.skrq",
            wire::ResponsePayload::SearchKernel_Wire_GetResult,
        ),
        (
            "search.defaults.skrq",
            wire::ResponsePayload::SearchKernel_Wire_SearchResult,
        ),
        (
            "analyze.text-request.skrq",
            wire::ResponsePayload::SearchKernel_Wire_AnalyzeResult,
        ),
    ];

    for (fixture, response_kind) in backend_cases {
        let request = fs::read(fixture_root.join(fixture)).unwrap();
        let recording = Arc::new(RecordingBackend::new());
        let recording_dispatcher = Dispatcher::from_backend(context(), recording.clone());
        let response = recording_dispatcher.dispatch(&request).unwrap();
        assert_eq!(
            root::<wire::ResponseEnvelope<'_>>(&response)
                .unwrap()
                .payload_type(),
            wire::ResponsePayload::SearchKernel_Wire_Error,
            "{fixture} must preserve backend PortableError as a response"
        );
        let calls = recording.take_calls();
        let error_message = root::<wire::ResponseEnvelope<'_>>(&response)
            .unwrap()
            .payload_as_search_kernel_wire_error()
            .and_then(|error| error.message())
            .unwrap_or("response was not a portable error");
        assert_eq!(
            calls.len(),
            1,
            "{fixture} must route one validated operation to the backend; {error_message}"
        );
        let operation = calls.into_iter().next().unwrap();

        let result = match &operation {
            OperationIr::Insert(request) => ScriptedResult::Insert(searchkernel_ir::InsertResult {
                id: request.document().id().as_str().to_owned(),
            }),
            OperationIr::Upsert(request) => ScriptedResult::Upsert(searchkernel_ir::UpsertResult {
                id: request.document().id().as_str().to_owned(),
            }),
            OperationIr::Patch(request) => ScriptedResult::Patch(searchkernel_ir::PatchResult {
                id: request.id().as_str().to_owned(),
            }),
            OperationIr::Update(request) => ScriptedResult::Update(searchkernel_ir::UpdateResult {
                id: request.id().as_str().to_owned(),
            }),
            OperationIr::Delete(request) => ScriptedResult::Delete(searchkernel_ir::DeleteResult {
                id: request.id().as_str().to_owned(),
                deleted: true,
            }),
            OperationIr::Get(_) => {
                ScriptedResult::Get(searchkernel_ir::GetResult { document: None })
            }
            OperationIr::Search(_) => {
                ScriptedResult::Search(Box::new(representative_search_result()))
            }
            OperationIr::Analyze(_) => {
                ScriptedResult::Analyze(searchkernel_ir::AnalyzeResult { tokens: Vec::new() })
            }
        };
        let backend = Arc::new(ScriptedBackend::new([ExpectedCall::success(
            operation, result,
        )]));
        let dispatcher = Dispatcher::from_backend(context(), backend.clone());
        let response = dispatcher.dispatch(&request).unwrap();
        assert_eq!(
            root::<wire::ResponseEnvelope<'_>>(&response)
                .unwrap()
                .payload_type(),
            response_kind,
            "{fixture} response kind"
        );
        if response_kind == wire::ResponsePayload::SearchKernel_Wire_SearchResult {
            let envelope = root::<wire::ResponseEnvelope<'_>>(&response).unwrap();
            let result = envelope
                .payload_as_search_kernel_wire_search_result()
                .expect("search response payload");
            assert_eq!(result.total_hits().unwrap().value(), 12);
            assert_eq!(
                result.total_hits().unwrap().relation(),
                api::TotalHitsRelation::LowerBound
            );
            let root = result.root().unwrap().node_as_result_group().unwrap();
            assert_eq!(root.id(), Some("root"));
            assert_eq!(root.label(), Some("Root"));
            assert_eq!(
                root.value()
                    .unwrap()
                    .kind_as_string_value()
                    .unwrap()
                    .value(),
                Some("all")
            );
            assert_eq!(root.fields().unwrap().get(0).key(), "count");
            assert_eq!(
                root.fields()
                    .unwrap()
                    .get(0)
                    .value()
                    .unwrap()
                    .kind_as_uint_64_value()
                    .unwrap()
                    .value(),
                2
            );
            assert_eq!(root.continuations().unwrap().get(0).key(), "page");
            assert_eq!(
                root.continuations().unwrap().get(0).value(),
                Some("token-1")
            );
            assert_eq!(root.source(), Some("search"));
            assert_eq!(root.types().unwrap().iter().collect::<Vec<_>>(), ["root"]);
            assert_eq!(
                root.relevance()
                    .unwrap()
                    .value_as_finite_relevance()
                    .unwrap()
                    .value(),
                3.5
            );
            assert_eq!(
                root.limits()
                    .unwrap()
                    .from()
                    .unwrap()
                    .kind_as_uint_64_value()
                    .unwrap()
                    .value(),
                0
            );
            assert_eq!(root.metadata().unwrap().get(0).key(), "trace_id");
            assert_eq!(
                root.metadata()
                    .unwrap()
                    .get(0)
                    .value()
                    .unwrap()
                    .kind_as_string_value()
                    .unwrap()
                    .value(),
                Some("r-1")
            );
            let children = root.children().unwrap();
            assert_eq!(children.len(), 2);
            let hit = children.get(0).node_as_hit().unwrap();
            assert_eq!(hit.id(), Some("doc-1"));
            assert_eq!(
                hit.relevance()
                    .unwrap()
                    .value_as_finite_relevance()
                    .unwrap()
                    .value(),
                4.5
            );
            assert_eq!(hit.fields().unwrap().get(0).key(), "title");
            assert_eq!(
                hit.fields()
                    .unwrap()
                    .get(0)
                    .value()
                    .unwrap()
                    .kind_as_string_value()
                    .unwrap()
                    .value(),
                Some("First")
            );
            assert_eq!(hit.highlights().unwrap().get(0).field(), Some("title"));
            assert_eq!(
                hit.highlights()
                    .unwrap()
                    .get(0)
                    .fragments()
                    .unwrap()
                    .get(0)
                    .text(),
                Some("<em>First</em>")
            );
            assert_eq!(
                hit.highlights()
                    .unwrap()
                    .get(0)
                    .fragments()
                    .unwrap()
                    .get(0)
                    .matches()
                    .unwrap()
                    .get(0)
                    .start_byte(),
                0
            );
            assert_eq!(
                hit.highlights()
                    .unwrap()
                    .get(0)
                    .fragments()
                    .unwrap()
                    .get(0)
                    .matches()
                    .unwrap()
                    .get(0)
                    .end_byte(),
                5
            );
            assert_eq!(hit.source(), Some("catalog"));
            assert_eq!(hit.types().unwrap().iter().collect::<Vec<_>>(), ["product"]);
            assert_eq!(hit.match_features().unwrap().get(0).key(), "bm25");
            assert_eq!(
                hit.match_features()
                    .unwrap()
                    .get(0)
                    .value()
                    .unwrap()
                    .kind_as_float_64_value()
                    .unwrap()
                    .value(),
                2.0
            );
            assert_eq!(hit.summary_features().unwrap().get(0).key(), "summary");
            assert_eq!(
                hit.summary_features()
                    .unwrap()
                    .get(0)
                    .value()
                    .unwrap()
                    .kind_as_string_value()
                    .unwrap()
                    .value(),
                Some("ready")
            );
            assert_eq!(hit.rank_features().unwrap().get(0).key(), "rank");
            assert_eq!(
                hit.rank_features()
                    .unwrap()
                    .get(0)
                    .value()
                    .unwrap()
                    .kind_as_float_64_value()
                    .unwrap()
                    .value(),
                8.0
            );
            assert_eq!(
                hit.sort_values().unwrap().get(0).kind_type(),
                api::ValueKind::UInt64Value
            );
            assert_eq!(
                hit.sort_values()
                    .unwrap()
                    .get(0)
                    .kind_as_uint_64_value()
                    .unwrap()
                    .value(),
                1
            );
            let nested = children.get(1).node_as_result_group().unwrap();
            assert_eq!(nested.id(), Some("nested"));
            assert_eq!(nested.label(), Some("Nested"));
            assert_eq!(
                nested
                    .value()
                    .unwrap()
                    .kind_as_int_64_value()
                    .unwrap()
                    .value(),
                7
            );
            assert_eq!(nested.fields().unwrap().get(0).key(), "count");
            assert_eq!(
                nested
                    .fields()
                    .unwrap()
                    .get(0)
                    .value()
                    .unwrap()
                    .kind_as_uint_64_value()
                    .unwrap()
                    .value(),
                1
            );
            assert_eq!(
                nested.continuations().unwrap().get(0).value(),
                Some("token")
            );
            assert_eq!(nested.source(), Some("group-source"));
            assert_eq!(
                nested.types().unwrap().iter().collect::<Vec<_>>(),
                ["bucket"]
            );
            assert_eq!(
                nested
                    .relevance()
                    .unwrap()
                    .value_as_finite_relevance()
                    .unwrap()
                    .value(),
                2.5
            );
            assert_eq!(
                nested
                    .limits()
                    .unwrap()
                    .to()
                    .unwrap()
                    .kind_as_uint_64_value()
                    .unwrap()
                    .value(),
                10
            );
            assert_eq!(
                nested
                    .limits()
                    .unwrap()
                    .from()
                    .unwrap()
                    .kind_as_uint_64_value()
                    .unwrap()
                    .value(),
                0
            );
            assert!(nested
                .metadata()
                .unwrap()
                .get(0)
                .value()
                .unwrap()
                .kind_as_bool_value()
                .unwrap()
                .value());
            assert_eq!(
                nested
                    .children()
                    .unwrap()
                    .get(0)
                    .node_as_hit()
                    .unwrap()
                    .id(),
                Some("doc-2")
            );
            assert_eq!(
                nested
                    .children()
                    .unwrap()
                    .get(0)
                    .node_as_hit()
                    .unwrap()
                    .fields()
                    .unwrap()
                    .get(0)
                    .key(),
                "price"
            );
            assert_eq!(
                nested
                    .children()
                    .unwrap()
                    .get(0)
                    .node_as_hit()
                    .unwrap()
                    .fields()
                    .unwrap()
                    .get(0)
                    .value()
                    .unwrap()
                    .kind_as_float_64_value()
                    .unwrap()
                    .value(),
                12.5
            );
            assert_eq!(
                result
                    .coverage()
                    .unwrap()
                    .coverage_percent()
                    .unwrap()
                    .value(),
                99.0
            );
            let coverage = result.coverage().unwrap();
            assert_eq!(coverage.documents().unwrap().value(), 12);
            assert!(coverage.full().unwrap().value());
            assert_eq!(coverage.nodes().unwrap().value(), 2);
            assert_eq!(coverage.results().unwrap().value(), 12);
            assert_eq!(coverage.results_full().unwrap().value(), 12);
            let degraded = coverage.degraded().unwrap();
            assert!(degraded.match_phase().unwrap().value());
            assert!(!degraded.timeout().unwrap().value());
            assert!(degraded.adaptive_timeout().unwrap().value());
            assert!(!degraded.ann_timeout().unwrap().value());
            assert!(degraded.non_ideal_state().unwrap().value());
            assert_eq!(degraded.metadata().unwrap().get(0).key(), "reason");
            assert_eq!(
                degraded
                    .metadata()
                    .unwrap()
                    .get(0)
                    .value()
                    .unwrap()
                    .kind_as_string_value()
                    .unwrap()
                    .value(),
                Some("load")
            );
            assert_eq!(coverage.metadata().unwrap().get(0).key(), "region");
            assert_eq!(
                coverage
                    .metadata()
                    .unwrap()
                    .get(0)
                    .value()
                    .unwrap()
                    .kind_as_string_value()
                    .unwrap()
                    .value(),
                Some("west")
            );
            assert_eq!(
                result
                    .timing()
                    .unwrap()
                    .search_time_seconds()
                    .unwrap()
                    .value(),
                0.002
            );
            let timing = result.timing().unwrap();
            assert_eq!(timing.query_time_seconds().unwrap().value(), 0.001);
            assert_eq!(timing.summary_fetch_time_seconds().unwrap().value(), 0.003);
            assert_eq!(timing.metadata().unwrap().get(0).key(), "phase");
            assert_eq!(
                timing
                    .metadata()
                    .unwrap()
                    .get(0)
                    .value()
                    .unwrap()
                    .kind_as_string_value()
                    .unwrap()
                    .value(),
                Some("done")
            );
            let error = result.errors().unwrap().get(0);
            assert_eq!(error.code(), 503);
            assert_eq!(error.summary(), Some("partial"));
            assert_eq!(error.message(), Some("one node timed out"));
            assert_eq!(error.source(), Some("node-2"));
            assert_eq!(error.stack_trace(), Some("trace"));
            assert!(error.transient().unwrap().value());
            assert_eq!(error.metadata().unwrap().get(0).key(), "node");
            assert_eq!(
                error
                    .metadata()
                    .unwrap()
                    .get(0)
                    .value()
                    .unwrap()
                    .kind_as_string_value()
                    .unwrap()
                    .value(),
                Some("2")
            );
            assert_eq!(
                result
                    .trace()
                    .unwrap()
                    .kind_as_value_object()
                    .unwrap()
                    .fields()
                    .unwrap()
                    .get(0)
                    .key(),
                "level"
            );
            assert_eq!(
                result
                    .trace()
                    .unwrap()
                    .kind_as_value_object()
                    .unwrap()
                    .fields()
                    .unwrap()
                    .get(0)
                    .value()
                    .unwrap()
                    .kind_as_uint_64_value()
                    .unwrap()
                    .value(),
                3
            );
            assert_eq!(result.metadata().unwrap().get(0).key(), "request_id");
            assert_eq!(
                result
                    .metadata()
                    .unwrap()
                    .get(0)
                    .value()
                    .unwrap()
                    .kind_as_string_value()
                    .unwrap()
                    .value(),
                Some("r1")
            );
        }
        backend.assert_exhausted().unwrap();
    }

    for (fixture, response_kind) in [
        (
            "introspection.describe-index.skrq",
            wire::ResponsePayload::SearchKernel_Wire_DescribeResult,
        ),
        (
            "introspection.inspect-query.skrq",
            wire::ResponsePayload::SearchKernel_Wire_InspectQueryResult,
        ),
    ] {
        let request = fs::read(fixture_root.join(fixture)).unwrap();
        let dispatcher = Dispatcher::from_backend(context(), Arc::new(RecordingBackend::new()));
        let response = dispatcher.dispatch(&request).unwrap();
        assert_eq!(
            root::<wire::ResponseEnvelope<'_>>(&response)
                .unwrap()
                .payload_type(),
            response_kind,
            "{fixture} response kind"
        );
    }
}

#[test]
fn rejects_update_type_errors_before_backend_execution() {
    let fixture_root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/portable/requests");
    let request = fs::read(fixture_root.join("write.update-type-error.skrq")).unwrap();
    let recording = Arc::new(RecordingBackend::new());
    let dispatcher = Dispatcher::from_backend(context(), recording.clone());
    let response = dispatcher.dispatch(&request).unwrap();
    let envelope = root::<wire::ResponseEnvelope<'_>>(&response).unwrap();
    assert_eq!(
        envelope.payload_type(),
        wire::ResponsePayload::SearchKernel_Wire_Error
    );
    assert_eq!(
        envelope
            .payload_as_search_kernel_wire_error()
            .unwrap()
            .code(),
        api::ErrorCode::InvalidArgument
    );
    assert!(
        recording.take_calls().is_empty(),
        "invalid update must not reach the backend"
    );
}

#[test]
fn standard_flatbuffers_json_enters_the_canonical_dispatch_path() {
    let request = searchkernel_protocol::json::request_json_to_flatbuffer(
        br#"{"protocol_version":1,"payload_type":"SearchKernel_Wire_DescribeRequest","payload":{}}"#,
    )
    .expect("standard JSON request");
    let dispatcher = Dispatcher::from_backend(context(), Arc::new(RecordingBackend::new()));
    let response = dispatcher
        .dispatch(&request)
        .expect("dispatch standard JSON bytes");
    assert_eq!(
        root::<wire::ResponseEnvelope<'_>>(&response)
            .unwrap()
            .payload_type(),
        wire::ResponsePayload::SearchKernel_Wire_DescribeResult
    );
}

#[test]
fn standard_flatbuffers_json_duplicate_document_fields_are_rejected_before_backend() {
    let request = searchkernel_protocol::json::request_json_to_flatbuffer(
        br#"{
          "protocol_version": 1,
          "payload_type": "SearchKernel_Wire_InsertRequest",
          "payload": {
            "document": {
              "id": "doc-duplicate",
              "fields": [
                {"key":"brand","value":{"kind_type":"StringValue","kind":{"value":"first"}}},
                {"key":"brand","value":{"kind_type":"StringValue","kind":{"value":"second"}}}
              ]
            }
          }
        }"#,
    )
    .expect("standard JSON duplicate-field request");
    let recording = Arc::new(RecordingBackend::new());
    let dispatcher = Dispatcher::from_backend(context(), recording.clone());
    let response = dispatcher
        .dispatch(&request)
        .expect("semantic validation returns a portable error response");
    let envelope = root::<wire::ResponseEnvelope<'_>>(&response).unwrap();
    let error = envelope
        .payload_as_search_kernel_wire_error()
        .expect("duplicate document fields must produce an error response");
    assert_eq!(error.code(), api::ErrorCode::InvalidArgument);
    assert_eq!(
        error.details().unwrap().get(0).code(),
        Some("duplicate_document_field")
    );
    assert_eq!(
        error.details().unwrap().get(0).path(),
        Some("document.fields[1].key")
    );
    assert!(
        recording.take_calls().is_empty(),
        "invalid duplicate fields must not reach the backend"
    );
}
