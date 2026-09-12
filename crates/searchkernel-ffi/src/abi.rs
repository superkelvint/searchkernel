use crate::SkBuffer;
use crate::{buffer, dispatch, handle, status};
use std::panic::{catch_unwind, AssertUnwindSafe};

pub(crate) fn call(
    handle_value: u64,
    request: *const u8,
    request_len: usize,
    out_response: *mut SkBuffer,
) -> u32 {
    let result = catch_unwind(AssertUnwindSafe(|| unsafe {
        call_inner(handle_value, request, request_len, out_response)
    }));
    match result {
        Ok(status) => status,
        Err(_) => {
            if !out_response.is_null() {
                // `call_inner` only writes a validated empty output buffer at
                // the final commit point. This cleanup also handles a panic
                // after such a commit without leaking the allocation.
                unsafe { buffer::free(out_response) };
            }
            status::PANIC
        }
    }
}

unsafe fn call_inner(
    handle_value: u64,
    request: *const u8,
    request_len: usize,
    out_response: *mut SkBuffer,
) -> u32 {
    if out_response.is_null() {
        return status::INVALID_ARGUMENT;
    }
    // SAFETY: null was checked above; the caller must provide a valid output
    // structure as required by the C ABI.
    let output = unsafe { &mut *out_response };
    if !output.data.is_null() || output.len != 0 {
        return status::INVALID_ARGUMENT;
    }

    let Some(state) = handle::lookup(handle_value) else {
        return status::INVALID_HANDLE;
    };

    if request_len > isize::MAX as usize {
        return status::INVALID_ARGUMENT;
    }
    if request_len > 0 && request.is_null() {
        return status::INVALID_ARGUMENT;
    }
    if request_len == 0 {
        return status::INVALID_REQUEST;
    }

    // SAFETY: the pointer/length pair was validated above and remains borrowed
    // only for the duration of this call.
    let request = unsafe { std::slice::from_raw_parts(request, request_len) };
    match dispatch::dispatch(&state, request) {
        Ok(response) => {
            buffer::write(out_response, response);
            status::OK
        }
        Err(status) => status,
    }
}

pub(crate) fn close(handle_value: u64) -> u32 {
    match catch_unwind(AssertUnwindSafe(|| handle::close(handle_value))) {
        Ok(true) => status::OK,
        Ok(false) => status::INVALID_HANDLE,
        Err(_) => status::PANIC,
    }
}

pub(crate) fn buffer_free(buffer: *mut SkBuffer) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe { buffer::free(buffer) }));
}

pub(crate) fn abi_version() -> u32 {
    1
}
