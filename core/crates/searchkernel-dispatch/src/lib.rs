//! Transport-independent dispatch for canonical Search Kernel envelopes.

use flatbuffers::{root, WIPOffset};
use searchkernel_ir::{Backend, BackendSession, IndexContext, SessionError};
use searchkernel_protocol::protocol::v1::{self as wire, PROTOCOL_VERSION};
use std::sync::Arc;

#[cfg(test)]
mod tests;

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
            wire::RequestPayload::SearchKernel_Wire_InsertRequest => {
                let payload = envelope
                    .payload_as_search_kernel_wire_insert_request()
                    .ok_or(DispatchError::InvalidRequest)?;
                let result = self.execute_backend(|context, backend| {
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
                let result = self.execute_backend(|context, backend| {
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
                let result = self.execute_backend(|context, backend| {
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
                let result = self.execute_backend(|context, backend| {
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
                let result = self.execute_backend(|context, backend| {
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
                let result = self.execute_backend(|context, backend| {
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
                let result = self.execute_backend(|context, backend| {
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
                let result = self.execute_backend(|context, backend| {
                    context
                        .analyze_to_ir(payload)
                        .and_then(|request| backend.analyze(request))
                })?;
                Ok(encode_result(
                    result,
                    wire::ResponsePayload::SearchKernel_Wire_AnalyzeResult,
                    |builder, result| {
                        Ok(
                            searchkernel_ir::analyze_result_offset(builder, result)
                                .as_union_value(),
                        )
                    },
                ))
            }
            wire::RequestPayload::SearchKernel_Wire_DescribeRequest => {
                let payload = envelope
                    .payload_as_search_kernel_wire_describe_request()
                    .ok_or(DispatchError::InvalidRequest)?;
                let _ = payload;
                let result = self.execute_context(|context| Ok(context.describe()))?;
                Ok(encode_result(
                    result,
                    wire::ResponsePayload::SearchKernel_Wire_DescribeResult,
                    |builder, result| {
                        Ok(searchkernel_ir::describe_result_offset(builder, result)
                            .as_union_value())
                    },
                ))
            }
            wire::RequestPayload::SearchKernel_Wire_InspectQueryRequest => {
                let payload = envelope
                    .payload_as_search_kernel_wire_inspect_query_request()
                    .ok_or(DispatchError::InvalidRequest)?;
                let request = payload.request().ok_or(DispatchError::InvalidRequest)?;
                let result = self.execute_context(|context| context.inspect_query(request))?;
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

    /// Closes the underlying execution target after active operations drain.
    pub fn close(&self) -> searchkernel_ir::Result<()> {
        self.session.close()
    }

    fn execute_backend<T>(
        &self,
        callback: impl FnOnce(&IndexContext, &dyn Backend) -> searchkernel_ir::Result<T>,
    ) -> Result<searchkernel_ir::Result<T>, DispatchError> {
        Ok(
            match self
                .session
                .execute(|backend| callback(&self.context, backend))
            {
                Ok(result) => result,
                Err(SessionError::Closed) => Err(closed_target_error()),
            },
        )
    }

    fn execute_context<T>(
        &self,
        callback: impl FnOnce(&IndexContext) -> searchkernel_ir::Result<T>,
    ) -> Result<searchkernel_ir::Result<T>, DispatchError> {
        Ok(match self.session.execute(|_| callback(&self.context)) {
            Ok(result) => result,
            Err(SessionError::Closed) => Err(closed_target_error()),
        })
    }
}

fn closed_target_error() -> searchkernel_ir::PortableError {
    searchkernel_ir::PortableError::failed_precondition(
        "target_closed",
        None::<String>,
        "Search Kernel target is closed",
    )
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
