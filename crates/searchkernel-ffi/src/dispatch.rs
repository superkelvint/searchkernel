use crate::handle::HandleState;
use crate::status;
use crate::transport;

pub(crate) fn dispatch(state: &HandleState, request: &[u8]) -> Result<Vec<u8>, u32> {
    let envelope = transport::parse_request(request).map_err(|_| status::INVALID_REQUEST)?;
    if envelope.protocol_version() != transport::PROTOCOL_VERSION {
        return Err(status::UNSUPPORTED_PROTOCOL);
    }

    match envelope.payload_type().0 {
        transport::REQUEST_INSERT => {
            let payload = envelope
                .payload_as_search_kernel_v1_insert_request()
                .ok_or(status::INVALID_REQUEST)?;
            let result = state
                .context
                .insert_to_ir(payload)
                .and_then(|request| state.backend.insert(request));
            let response = encode_result(result, transport::RESPONSE_INSERT, |builder, result| {
                searchkernel_core::insert_result_offset(builder, result).as_union_value()
            });
            Ok(response)
        }
        transport::REQUEST_UPSERT => {
            let payload = envelope
                .payload_as_search_kernel_v1_upsert_request()
                .ok_or(status::INVALID_REQUEST)?;
            let result = state
                .context
                .upsert_to_ir(payload)
                .and_then(|request| state.backend.upsert(request));
            let response = encode_result(result, transport::RESPONSE_UPSERT, |builder, result| {
                searchkernel_core::upsert_result_offset(builder, result).as_union_value()
            });
            Ok(response)
        }
        transport::REQUEST_PATCH => {
            let payload = envelope
                .payload_as_search_kernel_v1_patch_request()
                .ok_or(status::INVALID_REQUEST)?;
            let result = state
                .context
                .patch_to_ir(payload)
                .and_then(|request| state.backend.patch(request));
            let response = encode_result(result, transport::RESPONSE_PATCH, |builder, result| {
                searchkernel_core::patch_result_offset(builder, result).as_union_value()
            });
            Ok(response)
        }
        transport::REQUEST_DELETE => {
            let payload = envelope
                .payload_as_search_kernel_v1_delete_request()
                .ok_or(status::INVALID_REQUEST)?;
            let result = state
                .context
                .delete_to_ir(payload)
                .and_then(|request| state.backend.delete(request));
            let response = encode_result(result, transport::RESPONSE_DELETE, |builder, result| {
                searchkernel_core::delete_result_offset(builder, result).as_union_value()
            });
            Ok(response)
        }
        transport::REQUEST_GET => {
            let payload = envelope
                .payload_as_search_kernel_v1_get_request()
                .ok_or(status::INVALID_REQUEST)?;
            let result = state
                .context
                .get_to_ir(payload)
                .and_then(|request| state.backend.get(request));
            let response = encode_result(result, transport::RESPONSE_GET, |builder, result| {
                searchkernel_core::get_result_offset(builder, result).as_union_value()
            });
            Ok(response)
        }
        transport::REQUEST_SEARCH => {
            let payload = envelope
                .payload_as_search_kernel_v1_search_request()
                .ok_or(status::INVALID_REQUEST)?;
            let result = state
                .context
                .search_to_ir(payload)
                .and_then(|request| state.backend.search(request));
            let response = encode_result(result, transport::RESPONSE_SEARCH, |builder, result| {
                searchkernel_core::search_result_offset(builder, result).as_union_value()
            });
            Ok(response)
        }
        transport::REQUEST_ANALYZE => {
            let payload = envelope
                .payload_as_search_kernel_v1_analyze_request()
                .ok_or(status::INVALID_REQUEST)?;
            let result = state
                .context
                .analyze_to_ir(payload)
                .and_then(|request| state.backend.analyze(request));
            let response = encode_result(result, transport::RESPONSE_ANALYZE, |builder, result| {
                searchkernel_core::analyze_result_offset(builder, result).as_union_value()
            });
            Ok(response)
        }
        0 => Err(status::INVALID_REQUEST),
        _ => Err(status::UNSUPPORTED_OPERATION),
    }
}

fn encode_result<T>(
    result: searchkernel_core::Result<T>,
    success_kind: u8,
    encode: impl for<'a> FnOnce(
        &mut flatbuffers::FlatBufferBuilder<'a>,
        &T,
    ) -> flatbuffers::WIPOffset<flatbuffers::UnionWIPOffset>,
) -> Vec<u8> {
    transport::frame_response(
        match result {
            Ok(_) => success_kind,
            Err(_) => transport::RESPONSE_ERROR,
        },
        |builder| match result {
            Ok(result) => encode(builder, &result),
            Err(error) => searchkernel_core::error_offset(builder, &error).as_union_value(),
        },
    )
}
