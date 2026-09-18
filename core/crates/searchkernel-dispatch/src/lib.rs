//! Transport-independent dispatch for canonical Search Kernel V2 envelopes.

use flatbuffers::{root, WIPOffset};
use searchkernel_ir::{Backend, BackendSession, IndexContext, SessionError};
use searchkernel_protocol::protocol::v2 as wire;
use std::sync::Arc;

/// The canonical portable protocol version.
pub use searchkernel_protocol::PROTOCOL_VERSION;
const REQUEST_IDENTIFIER: &[u8; 4] = b"SKRQ";
const RESPONSE_IDENTIFIER: &str = "SKRS";

/// Failures at the dispatcher/framing boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DispatchError {
    InvalidRequest,
    UnsupportedProtocol(u32),
    UnsupportedOperation(u8),
    Closed,
}

/// An execution target bound to its schema context and lifecycle session.
pub struct Dispatcher {
    context: Arc<IndexContext>,
    session: Arc<BackendSession>,
}

impl Dispatcher {
    /// Creates a dispatcher for one already-bound execution target.
    pub fn new(context: Arc<IndexContext>, session: Arc<BackendSession>) -> Self {
        Self { context, session }
    }

    /// Creates an execution target from a backend for tests and embedded callers.
    pub fn from_backend(context: Arc<IndexContext>, backend: Arc<dyn Backend>) -> Self {
        Self::new(context, BackendSession::from_backend(backend))
    }

    /// Runs one canonical request envelope.
    pub fn dispatch(&self, request: &[u8]) -> Result<Vec<u8>, DispatchError> {
        let envelope = parse_request(request)?;
        if envelope.protocol_version() != PROTOCOL_VERSION {
            return Err(DispatchError::UnsupportedProtocol(
                envelope.protocol_version(),
            ));
        }

        match envelope.payload_type() {
            wire::RequestPayload::SearchKernel_V2_InsertRequest => {
                let payload = envelope
                    .payload_as_search_kernel_v2_insert_request()
                    .ok_or(DispatchError::InvalidRequest)?;
                let result = self.execute_backend(|context, backend| {
                    context
                        .insert_to_ir(payload)
                        .and_then(|request| backend.insert(request))
                })?;
                Ok(encode_result(
                    result,
                    wire::ResponsePayload::SearchKernel_V2_InsertResult,
                    |builder, result| {
                        searchkernel_ir::insert_result_offset(builder, result).as_union_value()
                    },
                ))
            }
            wire::RequestPayload::SearchKernel_V2_UpsertRequest => {
                let payload = envelope
                    .payload_as_search_kernel_v2_upsert_request()
                    .ok_or(DispatchError::InvalidRequest)?;
                let result = self.execute_backend(|context, backend| {
                    context
                        .upsert_to_ir(payload)
                        .and_then(|request| backend.upsert(request))
                })?;
                Ok(encode_result(
                    result,
                    wire::ResponsePayload::SearchKernel_V2_UpsertResult,
                    |builder, result| {
                        searchkernel_ir::upsert_result_offset(builder, result).as_union_value()
                    },
                ))
            }
            wire::RequestPayload::SearchKernel_V2_PatchRequest => {
                let payload = envelope
                    .payload_as_search_kernel_v2_patch_request()
                    .ok_or(DispatchError::InvalidRequest)?;
                let result = self.execute_backend(|context, backend| {
                    context
                        .patch_to_ir(payload)
                        .and_then(|request| backend.patch(request))
                })?;
                Ok(encode_result(
                    result,
                    wire::ResponsePayload::SearchKernel_V2_PatchResult,
                    |builder, result| {
                        searchkernel_ir::patch_result_offset(builder, result).as_union_value()
                    },
                ))
            }
            wire::RequestPayload::SearchKernel_V2_DeleteRequest => {
                let payload = envelope
                    .payload_as_search_kernel_v2_delete_request()
                    .ok_or(DispatchError::InvalidRequest)?;
                let result = self.execute_backend(|context, backend| {
                    context
                        .delete_to_ir(payload)
                        .and_then(|request| backend.delete(request))
                })?;
                Ok(encode_result(
                    result,
                    wire::ResponsePayload::SearchKernel_V2_DeleteResult,
                    |builder, result| {
                        searchkernel_ir::delete_result_offset(builder, result).as_union_value()
                    },
                ))
            }
            wire::RequestPayload::SearchKernel_V2_GetRequest => {
                let payload = envelope
                    .payload_as_search_kernel_v2_get_request()
                    .ok_or(DispatchError::InvalidRequest)?;
                let result = self.execute_backend(|context, backend| {
                    context
                        .get_to_ir(payload)
                        .and_then(|request| backend.get(request))
                })?;
                Ok(encode_result(
                    result,
                    wire::ResponsePayload::SearchKernel_V2_GetResult,
                    |builder, result| {
                        searchkernel_ir::get_result_offset(builder, result).as_union_value()
                    },
                ))
            }
            wire::RequestPayload::SearchKernel_V2_SearchRequest => {
                let payload = envelope
                    .payload_as_search_kernel_v2_search_request()
                    .ok_or(DispatchError::InvalidRequest)?;
                let result = self.execute_backend(|context, backend| {
                    context
                        .search_to_ir(payload)
                        .and_then(|request| backend.search(request))
                })?;
                Ok(encode_result(
                    result,
                    wire::ResponsePayload::SearchKernel_V2_SearchResult,
                    |builder, result| {
                        searchkernel_ir::search_result_offset(builder, result).as_union_value()
                    },
                ))
            }
            wire::RequestPayload::SearchKernel_V2_AnalyzeRequest => {
                let payload = envelope
                    .payload_as_search_kernel_v2_analyze_request()
                    .ok_or(DispatchError::InvalidRequest)?;
                let result = self.execute_backend(|context, backend| {
                    context
                        .analyze_to_ir(payload)
                        .and_then(|request| backend.analyze(request))
                })?;
                Ok(encode_result(
                    result,
                    wire::ResponsePayload::SearchKernel_V2_AnalyzeResult,
                    |builder, result| {
                        searchkernel_ir::analyze_result_offset(builder, result).as_union_value()
                    },
                ))
            }
            wire::RequestPayload::SearchKernel_V2_DescribeRequest => {
                let payload = envelope
                    .payload_as_search_kernel_v2_describe_request()
                    .ok_or(DispatchError::InvalidRequest)?;
                let _ = payload;
                let result = self.execute_context(|context| Ok(context.describe()))?;
                Ok(encode_result(
                    result,
                    wire::ResponsePayload::SearchKernel_V2_DescribeResult,
                    |builder, result| {
                        searchkernel_ir::describe_result_offset(builder, result).as_union_value()
                    },
                ))
            }
            wire::RequestPayload::SearchKernel_V2_InspectQueryRequest => {
                let payload = envelope
                    .payload_as_search_kernel_v2_inspect_query_request()
                    .ok_or(DispatchError::InvalidRequest)?;
                let request = payload.request().ok_or(DispatchError::InvalidRequest)?;
                let result = self.execute_context(|context| context.inspect_query(request))?;
                Ok(encode_result(
                    result,
                    wire::ResponsePayload::SearchKernel_V2_InspectQueryResult,
                    |builder, result| {
                        searchkernel_ir::inspect_query_result_offset(builder, result)
                            .as_union_value()
                    },
                ))
            }
            wire::RequestPayload::NONE => Err(DispatchError::InvalidRequest),
            value => Err(DispatchError::UnsupportedOperation(value.0)),
        }
    }

    /// Closes the underlying execution target after active operations drain.
    pub fn close(&self) -> searchkernel_ir::Result<()> {
        self.session.close()
    }

    fn execute_backend<T>(
        &self,
        callback: impl FnOnce(&IndexContext, &dyn Backend) -> searchkernel_ir::Result<T>,
    ) -> Result<searchkernel_ir::Result<T>, DispatchError> {
        self.session
            .execute(|backend| callback(&self.context, backend))
            .map_err(|SessionError::Closed| DispatchError::Closed)
    }

    fn execute_context<T>(
        &self,
        callback: impl FnOnce(&IndexContext) -> searchkernel_ir::Result<T>,
    ) -> Result<searchkernel_ir::Result<T>, DispatchError> {
        self.session
            .execute(|_| callback(&self.context))
            .map_err(|SessionError::Closed| DispatchError::Closed)
    }
}

fn parse_request(bytes: &[u8]) -> Result<wire::RequestEnvelope<'_>, DispatchError> {
    if bytes.len() < 8 || bytes[4..8] != *REQUEST_IDENTIFIER {
        return Err(DispatchError::InvalidRequest);
    }
    root::<wire::RequestEnvelope<'_>>(bytes).map_err(|_| DispatchError::InvalidRequest)
}

fn encode_result<T>(
    result: searchkernel_ir::Result<T>,
    success_kind: wire::ResponsePayload,
    encode: impl for<'a> FnOnce(
        &mut flatbuffers::FlatBufferBuilder<'a>,
        &T,
    ) -> WIPOffset<flatbuffers::UnionWIPOffset>,
) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let (payload_type, payload) = match result {
        Ok(value) => (success_kind, encode(&mut builder, &value)),
        Err(error) => (
            wire::ResponsePayload::SearchKernel_V2_Error,
            searchkernel_ir::error_offset(&mut builder, &error).as_union_value(),
        ),
    };
    let envelope = wire::ResponseEnvelope::create(
        &mut builder,
        &wire::ResponseEnvelopeArgs {
            protocol_version: PROTOCOL_VERSION,
            payload_type,
            payload: Some(payload),
        },
    );
    builder.finish(envelope, Some(RESPONSE_IDENTIFIER));
    builder.finished_data().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use searchkernel_ir::{api, OperationIr};
    use searchkernel_testkit::{ExpectedCall, RecordingBackend, ScriptedBackend, ScriptedResult};
    use std::fs;

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
            IndexContext::from_api(
                root::<api::IndexDefinition<'_>>(builder.finished_data()).unwrap(),
            )
            .unwrap(),
        )
    }

    fn describe_request(version: u32, kind: wire::RequestPayload) -> Vec<u8> {
        let mut builder = flatbuffers::FlatBufferBuilder::new();
        let request =
            api::DescribeRequest::create(&mut builder, &api::DescribeRequestArgs::default());
        let envelope = wire::RequestEnvelope::create(
            &mut builder,
            &wire::RequestEnvelopeArgs {
                protocol_version: version,
                payload_type: kind,
                payload: Some(request.as_union_value()),
            },
        );
        builder.finish(envelope, Some("SKRQ"));
        builder.finished_data().to_vec()
    }

    fn empty_search_request() -> Vec<u8> {
        let mut builder = flatbuffers::FlatBufferBuilder::new();
        let request = api::SearchRequest::create(&mut builder, &api::SearchRequestArgs::default());
        let envelope = wire::RequestEnvelope::create(
            &mut builder,
            &wire::RequestEnvelopeArgs {
                protocol_version: PROTOCOL_VERSION,
                payload_type: wire::RequestPayload::SearchKernel_V2_SearchRequest,
                payload: Some(request.as_union_value()),
            },
        );
        builder.finish(envelope, Some("SKRQ"));
        builder.finished_data().to_vec()
    }

    #[test]
    fn accepts_v2_and_rejects_stale_or_future_protocols() {
        let dispatcher = Dispatcher::from_backend(context(), Arc::new(RecordingBackend::new()));
        assert!(dispatcher
            .dispatch(&describe_request(
                PROTOCOL_VERSION,
                wire::RequestPayload::SearchKernel_V2_DescribeRequest
            ))
            .is_ok());
        assert_eq!(
            dispatcher.dispatch(&describe_request(
                1,
                wire::RequestPayload::SearchKernel_V2_DescribeRequest
            )),
            Err(DispatchError::UnsupportedProtocol(1))
        );
        assert_eq!(
            dispatcher.dispatch(&describe_request(
                PROTOCOL_VERSION + 1,
                wire::RequestPayload::SearchKernel_V2_DescribeRequest
            )),
            Err(DispatchError::UnsupportedProtocol(PROTOCOL_VERSION + 1))
        );
    }

    #[test]
    fn v2_producers_encode_protocol_version_explicitly() {
        let request = describe_request(
            PROTOCOL_VERSION,
            wire::RequestPayload::SearchKernel_V2_DescribeRequest,
        );
        let request_envelope = root::<wire::RequestEnvelope<'_>>(&request).unwrap();
        assert_ne!(
            request_envelope
                ._tab
                .vtable()
                .get(wire::RequestEnvelope::VT_PROTOCOL_VERSION),
            0,
            "canonical V2 requests must physically encode protocol_version"
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
            "canonical V2 responses must physically encode protocol_version"
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
                .join("../searchkernel-json/tests/fixtures/requests/write.insert.skrq"),
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
                wire::RequestPayload::SearchKernel_V2_DescribeRequest,
            ))
            .unwrap();
        assert_eq!(
            dispatcher.dispatch(&response),
            Err(DispatchError::InvalidRequest),
            "a response envelope cannot be reused as a request"
        );
        dispatcher.close().unwrap();
        assert_eq!(
            dispatcher.dispatch(&describe_request(
                PROTOCOL_VERSION,
                wire::RequestPayload::SearchKernel_V2_DescribeRequest
            )),
            Err(DispatchError::Closed)
        );
    }

    #[test]
    fn semantic_errors_are_portable_error_responses() {
        let dispatcher = Dispatcher::from_backend(context(), Arc::new(RecordingBackend::new()));
        let response = dispatcher.dispatch(&empty_search_request()).unwrap();
        let envelope = root::<wire::ResponseEnvelope<'_>>(&response).unwrap();
        assert_eq!(
            envelope.payload_type(),
            wire::ResponsePayload::SearchKernel_V2_Error
        );
        assert!(envelope.payload_as_search_kernel_v2_error().is_some());
    }

    #[test]
    fn routes_every_operation_and_encodes_its_canonical_response() {
        let fixture_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../searchkernel-json/tests/fixtures/requests");
        let backend_cases = [
            (
                "write.insert.skrq",
                wire::ResponsePayload::SearchKernel_V2_InsertResult,
            ),
            (
                "write.upsert.skrq",
                wire::ResponsePayload::SearchKernel_V2_UpsertResult,
            ),
            (
                "write.patch-set.skrq",
                wire::ResponsePayload::SearchKernel_V2_PatchResult,
            ),
            (
                "write.delete.skrq",
                wire::ResponsePayload::SearchKernel_V2_DeleteResult,
            ),
            (
                "write.get.skrq",
                wire::ResponsePayload::SearchKernel_V2_GetResult,
            ),
            (
                "search.defaults.skrq",
                wire::ResponsePayload::SearchKernel_V2_SearchResult,
            ),
            (
                "analyze.text-request.skrq",
                wire::ResponsePayload::SearchKernel_V2_AnalyzeResult,
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
                wire::ResponsePayload::SearchKernel_V2_Error,
                "{fixture} must preserve backend PortableError as a response"
            );
            let calls = recording.take_calls();
            let error_message = root::<wire::ResponseEnvelope<'_>>(&response)
                .unwrap()
                .payload_as_search_kernel_v2_error()
                .and_then(|error| error.message())
                .unwrap_or("response was not a portable error");
            assert_eq!(
                calls.len(),
                1,
                "{fixture} must route one validated operation to the backend; {error_message}"
            );
            let operation = calls.into_iter().next().unwrap();

            let result = match &operation {
                OperationIr::Insert(request) => {
                    ScriptedResult::Insert(searchkernel_ir::InsertResult {
                        id: request.document().id().as_str().to_owned(),
                    })
                }
                OperationIr::Upsert(request) => {
                    ScriptedResult::Upsert(searchkernel_ir::UpsertResult {
                        id: request.document().id().as_str().to_owned(),
                    })
                }
                OperationIr::Patch(request) => {
                    ScriptedResult::Patch(searchkernel_ir::PatchResult {
                        id: request.id().as_str().to_owned(),
                    })
                }
                OperationIr::Delete(request) => {
                    ScriptedResult::Delete(searchkernel_ir::DeleteResult {
                        id: request.id().as_str().to_owned(),
                        deleted: true,
                    })
                }
                OperationIr::Get(_) => {
                    ScriptedResult::Get(searchkernel_ir::GetResult { document: None })
                }
                OperationIr::Search(_) => ScriptedResult::Search(searchkernel_ir::SearchResult {
                    hits: Vec::new(),
                    total_hits: searchkernel_ir::TotalHits {
                        value: 0,
                        relation: searchkernel_ir::TotalHitsRelation::Exact,
                    },
                    facets: Vec::new(),
                }),
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
            backend.assert_exhausted().unwrap();
        }

        for (fixture, response_kind) in [
            (
                "introspection.describe-index.skrq",
                wire::ResponsePayload::SearchKernel_V2_DescribeResult,
            ),
            (
                "introspection.inspect-query.skrq",
                wire::ResponsePayload::SearchKernel_V2_InspectQueryResult,
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
}
