//! FlatBuffers-native JSON transport for the Search Kernel envelopes.

mod error;
mod sys;

pub use error::JsonCodecError;

const REQUEST_BFBS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/bfbs/searchkernel_ffi_request.bfbs"
));
const RESPONSE_BFBS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/bfbs/searchkernel_ffi_response.bfbs"
));

fn convert(
    schema: &[u8],
    input: &[u8],
    conversion: unsafe extern "C" fn(
        *const u8,
        usize,
        *const u8,
        usize,
        *mut *mut u8,
        *mut usize,
        *mut *mut std::ffi::c_char,
        *mut usize,
    ) -> i32,
) -> Result<Vec<u8>, JsonCodecError> {
    let mut output_ptr = std::ptr::null_mut();
    let mut output_len = 0;
    let mut error_ptr = std::ptr::null_mut();
    let mut error_len = 0;

    // SAFETY: all pointers refer to valid slices or writable local output
    // variables for the duration of the bridge call.
    let status = unsafe {
        conversion(
            schema.as_ptr(),
            schema.len(),
            input.as_ptr(),
            input.len(),
            &mut output_ptr,
            &mut output_len,
            &mut error_ptr,
            &mut error_len,
        )
    };

    let output = if output_ptr.is_null() {
        None
    } else {
        // SAFETY: successful native output is an initialized byte allocation
        // owned by the bridge. Copy it before returning it to the caller.
        Some(unsafe { std::slice::from_raw_parts(output_ptr, output_len) }.to_vec())
    };
    if !output_ptr.is_null() {
        // SAFETY: the pointer was returned by the matching native bridge.
        unsafe { sys::sk_fbjson_free(output_ptr.cast()) };
    }

    let message = if error_ptr.is_null() {
        String::new()
    } else {
        // SAFETY: native error output is an initialized byte allocation owned
        // by the bridge and remains valid until it is freed below.
        let bytes = unsafe { std::slice::from_raw_parts(error_ptr.cast::<u8>(), error_len) };
        let message = String::from_utf8_lossy(bytes).into_owned();
        // SAFETY: the pointer was returned by the matching native bridge.
        unsafe { sys::sk_fbjson_free(error_ptr.cast()) };
        message
    };

    match status {
        0 => output.ok_or_else(|| JsonCodecError::Native("bridge returned no output".into())),
        1 => Err(JsonCodecError::Schema(message)),
        2 => Err(JsonCodecError::InvalidJson(message)),
        3 => Err(JsonCodecError::InvalidFlatbuffer(message)),
        _ => Err(JsonCodecError::Native(if message.is_empty() {
            format!("bridge failed with status {status}")
        } else {
            message
        })),
    }
}

pub fn request_json_to_flatbuffer(json: &[u8]) -> Result<Vec<u8>, JsonCodecError> {
    convert(REQUEST_BFBS, json, sys::sk_fbjson_json_to_flatbuffer)
}

pub fn request_flatbuffer_to_json(bytes: &[u8]) -> Result<Vec<u8>, JsonCodecError> {
    convert(REQUEST_BFBS, bytes, sys::sk_fbjson_flatbuffer_to_json)
}

pub fn response_json_to_flatbuffer(json: &[u8]) -> Result<Vec<u8>, JsonCodecError> {
    convert(RESPONSE_BFBS, json, sys::sk_fbjson_json_to_flatbuffer)
}

pub fn response_flatbuffer_to_json(bytes: &[u8]) -> Result<Vec<u8>, JsonCodecError> {
    convert(RESPONSE_BFBS, bytes, sys::sk_fbjson_flatbuffer_to_json)
}
