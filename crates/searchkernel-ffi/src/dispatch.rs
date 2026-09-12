use crate::handle::HandleState;
use crate::status;
use crate::transport;

pub(crate) fn dispatch(state: &HandleState, request: &[u8]) -> Result<Vec<u8>, u32> {
    let envelope = transport::parse_request(request).map_err(|_| status::INVALID_REQUEST)?;
    if envelope.protocol_version() != transport::PROTOCOL_VERSION {
        return Err(status::UNSUPPORTED_PROTOCOL);
    }

    match envelope.payload_type() {
        transport::REQUEST_INSERT => {
            let payload = envelope.insert().ok_or(status::INVALID_REQUEST)?;
            let result = state
                .context
                .insert_to_ir(payload)
                .and_then(|request| state.backend.insert(request));
            let (kind, response) = encode_result(
                result,
                transport::RESPONSE_INSERT,
                searchkernel_core::insert_result_to_api,
            );
            Ok(transport::frame_response(kind, &response))
        }
        transport::REQUEST_UPSERT => {
            let payload = envelope.upsert().ok_or(status::INVALID_REQUEST)?;
            let result = state
                .context
                .upsert_to_ir(payload)
                .and_then(|request| state.backend.upsert(request));
            let (kind, response) = encode_result(
                result,
                transport::RESPONSE_UPSERT,
                searchkernel_core::upsert_result_to_api,
            );
            Ok(transport::frame_response(kind, &response))
        }
        transport::REQUEST_PATCH => {
            let payload = envelope.patch().ok_or(status::INVALID_REQUEST)?;
            let result = state
                .context
                .patch_to_ir(payload)
                .and_then(|request| state.backend.patch(request));
            let (kind, response) = encode_result(
                result,
                transport::RESPONSE_PATCH,
                searchkernel_core::patch_result_to_api,
            );
            Ok(transport::frame_response(kind, &response))
        }
        transport::REQUEST_DELETE => {
            let payload = envelope.delete().ok_or(status::INVALID_REQUEST)?;
            let result = state
                .context
                .delete_to_ir(payload)
                .and_then(|request| state.backend.delete(request));
            let (kind, response) = encode_result(
                result,
                transport::RESPONSE_DELETE,
                searchkernel_core::delete_result_to_api,
            );
            Ok(transport::frame_response(kind, &response))
        }
        transport::REQUEST_GET => {
            let payload = envelope.get().ok_or(status::INVALID_REQUEST)?;
            let result = state
                .context
                .get_to_ir(payload)
                .and_then(|request| state.backend.get(request));
            let (kind, response) = encode_result(
                result,
                transport::RESPONSE_GET,
                searchkernel_core::get_result_to_api,
            );
            Ok(transport::frame_response(kind, &response))
        }
        transport::REQUEST_SEARCH => {
            let payload = envelope.search().ok_or(status::INVALID_REQUEST)?;
            let result = state
                .context
                .search_to_ir(payload)
                .and_then(|request| state.backend.search(request));
            let (kind, response) = encode_result(
                result,
                transport::RESPONSE_SEARCH,
                searchkernel_core::search_result_to_api,
            );
            Ok(transport::frame_response(kind, &response))
        }
        transport::REQUEST_ANALYZE => {
            let payload = envelope.analyze().ok_or(status::INVALID_REQUEST)?;
            let result = state
                .context
                .analyze_to_ir(payload)
                .and_then(|request| state.backend.analyze(request));
            let (kind, response) = encode_result(
                result,
                transport::RESPONSE_ANALYZE,
                searchkernel_core::analyze_result_to_api,
            );
            Ok(transport::frame_response(kind, &response))
        }
        0 => Err(status::INVALID_REQUEST),
        _ => Err(status::UNSUPPORTED_OPERATION),
    }
}

fn encode_result<T>(
    result: searchkernel_core::Result<T>,
    success_kind: u8,
    encode: impl Fn(&T) -> Vec<u8>,
) -> (u8, Vec<u8>) {
    match result {
        Ok(result) => (success_kind, encode(&result)),
        Err(error) => (
            transport::RESPONSE_ERROR,
            searchkernel_core::error_to_api(&error),
        ),
    }
}
