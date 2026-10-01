//! Wire-major-1 request decoding, dispatch, and response encoding.

use super::super::{DispatchError, Dispatcher};
use flatbuffers::{root, WIPOffset};
use searchkernel_protocol::protocol::v1::{self as wire, PROTOCOL_VERSION};

const REQUEST_IDENTIFIER: &[u8; 4] = b"SKRQ";
const RESPONSE_IDENTIFIER: &str = "SKRS";

pub(super) fn protocol_version(request: &[u8]) -> Result<u32, DispatchError> {
    Ok(parse_request(request)?.protocol_version())
}

pub(super) fn dispatch(dispatcher: &Dispatcher, request: &[u8]) -> Result<Vec<u8>, DispatchError> {
    let envelope = parse_request(request)?;
    if envelope.protocol_version() != PROTOCOL_VERSION {
        return Err(DispatchError::UnsupportedProtocol(
            envelope.protocol_version(),
        ));
    }

    match envelope.payload_type() {
        wire::RequestPayload::SearchKernel_Wire_InsertRequest => {
            let payload = envelope
                .payload_as_search_kernel_wire_insert_request()
                .ok_or(DispatchError::InvalidRequest)?;
            let result = dispatcher.execute_backend(|context, backend| {
                context
                    .insert_to_ir(payload)
                    .and_then(|request| backend.insert(request))
            })?;
            Ok(encode_result(
                result,
                wire::ResponsePayload::SearchKernel_Wire_InsertResult,
                |builder, result| {
                    Ok(searchkernel_ir::insert_result_offset(builder, result).as_union_value())
                },
            ))
        }
        wire::RequestPayload::SearchKernel_Wire_UpsertRequest => {
            let payload = envelope
                .payload_as_search_kernel_wire_upsert_request()
                .ok_or(DispatchError::InvalidRequest)?;
            let result = dispatcher.execute_backend(|context, backend| {
                context
                    .upsert_to_ir(payload)
                    .and_then(|request| backend.upsert(request))
            })?;
            Ok(encode_result(
                result,
                wire::ResponsePayload::SearchKernel_Wire_UpsertResult,
                |builder, result| {
                    Ok(searchkernel_ir::upsert_result_offset(builder, result).as_union_value())
                },
            ))
        }
        wire::RequestPayload::SearchKernel_Wire_PatchRequest => {
            let payload = envelope
                .payload_as_search_kernel_wire_patch_request()
                .ok_or(DispatchError::InvalidRequest)?;
            let result = dispatcher.execute_backend(|context, backend| {
                context
                    .patch_to_ir(payload)
                    .and_then(|request| backend.patch(request))
            })?;
            Ok(encode_result(
                result,
                wire::ResponsePayload::SearchKernel_Wire_PatchResult,
                |builder, result| {
                    Ok(searchkernel_ir::patch_result_offset(builder, result).as_union_value())
                },
            ))
        }
        wire::RequestPayload::SearchKernel_Wire_UpdateRequest => {
            let payload = envelope
                .payload_as_search_kernel_wire_update_request()
                .ok_or(DispatchError::InvalidRequest)?;
            let result = dispatcher.execute_backend(|context, backend| {
                context
                    .update_to_ir(payload)
                    .and_then(|request| backend.update(request))
            })?;
            Ok(encode_result(
                result,
                wire::ResponsePayload::SearchKernel_Wire_UpdateResult,
                |builder, result| {
                    Ok(searchkernel_ir::update_result_offset(builder, result).as_union_value())
                },
            ))
        }
        wire::RequestPayload::SearchKernel_Wire_DeleteRequest => {
            let payload = envelope
                .payload_as_search_kernel_wire_delete_request()
                .ok_or(DispatchError::InvalidRequest)?;
            let result = dispatcher.execute_backend(|context, backend| {
                context
                    .delete_to_ir(payload)
                    .and_then(|request| backend.delete(request))
            })?;
            Ok(encode_result(
                result,
                wire::ResponsePayload::SearchKernel_Wire_DeleteResult,
                |builder, result| {
                    Ok(searchkernel_ir::delete_result_offset(builder, result).as_union_value())
                },
            ))
        }
        wire::RequestPayload::SearchKernel_Wire_GetRequest => {
            let payload = envelope
                .payload_as_search_kernel_wire_get_request()
                .ok_or(DispatchError::InvalidRequest)?;
            let result = dispatcher.execute_backend(|context, backend| {
                context
                    .get_to_ir(payload)
                    .and_then(|request| backend.get(request))
            })?;
            Ok(encode_result(
                result,
                wire::ResponsePayload::SearchKernel_Wire_GetResult,
                |builder, result| {
                    Ok(searchkernel_ir::get_result_offset(builder, result).as_union_value())
                },
            ))
        }
        wire::RequestPayload::SearchKernel_Wire_SearchRequest => {
            let payload = envelope
                .payload_as_search_kernel_wire_search_request()
                .ok_or(DispatchError::InvalidRequest)?;
            let result = dispatcher.execute_backend(|context, backend| {
                context
                    .search_to_ir(payload)
                    .and_then(|request| backend.search(request))
            })?;
            Ok(encode_result(
                result,
                wire::ResponsePayload::SearchKernel_Wire_SearchResult,
                |builder, result| {
                    searchkernel_ir::search_result_offset(builder, result)
                        .map(|result| result.as_union_value())
                },
            ))
        }
        wire::RequestPayload::SearchKernel_Wire_AnalyzeRequest => {
            let payload = envelope
                .payload_as_search_kernel_wire_analyze_request()
                .ok_or(DispatchError::InvalidRequest)?;
            let result = dispatcher.execute_backend(|context, backend| {
                context
                    .analyze_to_ir(payload)
                    .and_then(|request| backend.analyze(request))
            })?;
            Ok(encode_result(
                result,
                wire::ResponsePayload::SearchKernel_Wire_AnalyzeResult,
                |builder, result| {
                    Ok(searchkernel_ir::analyze_result_offset(builder, result).as_union_value())
                },
            ))
        }
        wire::RequestPayload::SearchKernel_Wire_DescribeRequest => {
            let payload = envelope
                .payload_as_search_kernel_wire_describe_request()
                .ok_or(DispatchError::InvalidRequest)?;
            let _ = payload;
            let result = dispatcher.execute_context(|context| Ok(context.describe()))?;
            Ok(encode_result(
                result,
                wire::ResponsePayload::SearchKernel_Wire_DescribeResult,
                |builder, result| {
                    Ok(searchkernel_ir::describe_result_offset(builder, result).as_union_value())
                },
            ))
        }
        wire::RequestPayload::SearchKernel_Wire_InspectQueryRequest => {
            let payload = envelope
                .payload_as_search_kernel_wire_inspect_query_request()
                .ok_or(DispatchError::InvalidRequest)?;
            let request = payload.request().ok_or(DispatchError::InvalidRequest)?;
            let result = dispatcher.execute_context(|context| context.inspect_query(request))?;
            Ok(encode_result(
                result,
                wire::ResponsePayload::SearchKernel_Wire_InspectQueryResult,
                |builder, result| {
                    Ok(
                        searchkernel_ir::inspect_query_result_offset(builder, result)
                            .as_union_value(),
                    )
                },
            ))
        }
        wire::RequestPayload::NONE => Err(DispatchError::InvalidRequest),
        value => Err(DispatchError::UnsupportedOperation(value.0)),
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
    ) -> searchkernel_ir::Result<WIPOffset<flatbuffers::UnionWIPOffset>>,
) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let (payload_type, payload) = match result {
        Ok(value) => match encode(&mut builder, &value) {
            Ok(payload) => (success_kind, payload),
            Err(error) => (
                wire::ResponsePayload::SearchKernel_Wire_Error,
                searchkernel_ir::error_offset(&mut builder, &error).as_union_value(),
            ),
        },
        Err(error) => (
            wire::ResponsePayload::SearchKernel_Wire_Error,
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
