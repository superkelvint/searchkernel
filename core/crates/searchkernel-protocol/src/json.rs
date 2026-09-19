//! Generic standard FlatBuffers JSON conversion for Search Kernel envelopes.
//!
//! This crate deliberately contains no Search Kernel field mappings. The
//! official FlatBuffers parser and printer consume the checked-in BFBS schema,
//! so adding a field changes only the `.fbs` schema and its generated BFBS.

use std::{
    ffi::{c_char, c_void},
    fmt, ptr, slice,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvelopeSchema {
    Request,
    Response,
}

#[derive(Debug)]
pub struct JsonCodecError {
    status: i32,
    message: String,
}

impl JsonCodecError {
    fn new(status: i32, message: String) -> Self {
        Self { status, message }
    }
}

impl fmt::Display for JsonCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "FlatBuffers JSON conversion failed ({}): {}",
            self.status, self.message
        )
    }
}

impl std::error::Error for JsonCodecError {}

const REQUEST_SCHEMA: &[u8] = include_bytes!("../bfbs/searchkernel_request.bfbs");
const RESPONSE_SCHEMA: &[u8] = include_bytes!("../bfbs/searchkernel_response.bfbs");

unsafe extern "C" {
    fn sk_fbjson_json_to_flatbuffer(
        bfbs: *const u8,
        bfbs_len: usize,
        json: *const u8,
        json_len: usize,
        out_ptr: *mut *mut u8,
        out_len: *mut usize,
        error_ptr: *mut *mut c_char,
        error_len: *mut usize,
    ) -> i32;

    fn sk_fbjson_flatbuffer_to_json(
        bfbs: *const u8,
        bfbs_len: usize,
        buffer: *const u8,
        buffer_len: usize,
        out_ptr: *mut *mut u8,
        out_len: *mut usize,
        error_ptr: *mut *mut c_char,
        error_len: *mut usize,
    ) -> i32;

    fn sk_fbjson_free(ptr: *mut c_void);
}

fn schema(schema: EnvelopeSchema) -> &'static [u8] {
    match schema {
        EnvelopeSchema::Request => REQUEST_SCHEMA,
        EnvelopeSchema::Response => RESPONSE_SCHEMA,
    }
}

/// Convert standard FlatBuffers JSON using the supplied BFBS envelope schema.
pub fn json_to_flatbuffer(schema: EnvelopeSchema, json: &[u8]) -> Result<Vec<u8>, JsonCodecError> {
    unsafe { convert(schema, json, true) }
}

/// Print canonical FlatBuffer bytes as standard FlatBuffers JSON.
pub fn flatbuffer_to_json(
    schema: EnvelopeSchema,
    buffer: &[u8],
) -> Result<Vec<u8>, JsonCodecError> {
    unsafe { convert(schema, buffer, false) }
}

pub fn request_json_to_flatbuffer(json: &[u8]) -> Result<Vec<u8>, JsonCodecError> {
    json_to_flatbuffer(EnvelopeSchema::Request, json)
}

pub fn response_json_to_flatbuffer(json: &[u8]) -> Result<Vec<u8>, JsonCodecError> {
    json_to_flatbuffer(EnvelopeSchema::Response, json)
}

pub fn request_flatbuffer_to_json(buffer: &[u8]) -> Result<Vec<u8>, JsonCodecError> {
    flatbuffer_to_json(EnvelopeSchema::Request, buffer)
}

pub fn response_flatbuffer_to_json(buffer: &[u8]) -> Result<Vec<u8>, JsonCodecError> {
    flatbuffer_to_json(EnvelopeSchema::Response, buffer)
}

unsafe fn convert(
    envelope: EnvelopeSchema,
    input: &[u8],
    json_to_buffer: bool,
) -> Result<Vec<u8>, JsonCodecError> {
    let schema = schema(envelope);
    let mut output = ptr::null_mut();
    let mut output_len = 0;
    let mut error = ptr::null_mut();
    let mut error_len = 0;

    let status = if json_to_buffer {
        sk_fbjson_json_to_flatbuffer(
            schema.as_ptr(),
            schema.len(),
            input.as_ptr(),
            input.len(),
            &mut output,
            &mut output_len,
            &mut error,
            &mut error_len,
        )
    } else {
        sk_fbjson_flatbuffer_to_json(
            schema.as_ptr(),
            schema.len(),
            input.as_ptr(),
            input.len(),
            &mut output,
            &mut output_len,
            &mut error,
            &mut error_len,
        )
    };

    let message = if error.is_null() {
        "unknown FlatBuffers JSON conversion error".to_owned()
    } else {
        String::from_utf8_lossy(slice::from_raw_parts(error.cast(), error_len)).into_owned()
    };
    if !error.is_null() {
        sk_fbjson_free(error.cast());
    }
    if status != 0 {
        if !output.is_null() {
            sk_fbjson_free(output.cast());
        }
        return Err(JsonCodecError::new(status, message));
    }

    if output.is_null() {
        return Err(JsonCodecError::new(
            4,
            "FlatBuffers JSON bridge returned no output".to_owned(),
        ));
    }
    let result = slice::from_raw_parts(output, output_len).to_vec();
    sk_fbjson_free(output.cast());
    Ok(result)
}
