use crate::{buffer, dispatch, handle, status};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;

pub(crate) const MAX_REQUEST_BYTES: usize = 64 * 1024 * 1024;

pub(crate) unsafe fn call(
    handle_value: u64,
    request: *const u8,
    request_len: usize,
    out_data: *mut *mut u8,
    out_len: *mut usize,
    out_allocation_id: *mut u64,
) -> u32 {
    let result = catch_unwind(AssertUnwindSafe(|| unsafe {
        call_inner(
            handle_value,
            request,
            request_len,
            out_data,
            out_len,
            out_allocation_id,
        )
    }));
    match result {
        Ok(status) => status,
        Err(_) => {
            if !out_data.is_null() && !out_len.is_null() && !out_allocation_id.is_null() {
                // `call_inner` initializes all three outputs before dispatch.
                // This cleanup also handles a panic after the response
                // allocation was committed without leaking it.
                let data = unsafe { *out_data };
                let allocation_id = unsafe { *out_allocation_id };
                unsafe { buffer::free(data, allocation_id) };
                unsafe {
                    *out_data = ptr::null_mut();
                    *out_len = 0;
                    *out_allocation_id = 0;
                }
            }
            status::PANIC
        }
    }
}

unsafe fn call_inner(
    handle_value: u64,
    request: *const u8,
    request_len: usize,
    out_data: *mut *mut u8,
    out_len: *mut usize,
    out_allocation_id: *mut u64,
) -> u32 {
    if out_data.is_null() || out_len.is_null() || out_allocation_id.is_null() {
        if !out_data.is_null() {
            unsafe { *out_data = ptr::null_mut() };
        }
        if !out_len.is_null() {
            unsafe { *out_len = 0 };
        }
        if !out_allocation_id.is_null() {
            unsafe { *out_allocation_id = 0 };
        }
        return status::INVALID_ARGUMENT;
    }
    // These primitive outputs are write-only out-parameters. In particular,
    // do not read their previous contents: ordinary C callers may pass
    // pointers to uninitialized storage. The caller is responsible for
    // releasing any response it previously owned before reusing that storage.
    // SAFETY: all three pointers were checked above and the C ABI requires
    // writable storage for each primitive output.
    unsafe {
        ptr::write(out_data, ptr::null_mut());
        ptr::write(out_len, 0);
        ptr::write(out_allocation_id, 0);
    }

    if request_len > MAX_REQUEST_BYTES {
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
            // SAFETY: all output pointers were validated before dispatch.
            // The allocation is registered before any output is published.
            match unsafe { buffer::write(out_data, out_len, out_allocation_id, response) } {
                Ok(()) => status::OK,
                Err(()) => status::INTERNAL,
            }
        }
        Err(status) => status,
    }
}

pub(crate) fn close(handle_value: u64, error: *mut u8, error_capacity: usize) -> u32 {
    match catch_unwind(AssertUnwindSafe(|| handle::close(handle_value))) {
        Ok(Some(Ok(()))) => status::OK,
        Ok(Some(Err(close_error))) => {
            crate::write_lifecycle_error(close_error, error, error_capacity);
            status::INTERNAL
        }
        Ok(None) => status::INVALID_HANDLE,
        Err(_) => {
            crate::write_lifecycle_error("panic while closing index", error, error_capacity);
            status::PANIC
        }
    }
}

pub(crate) unsafe fn buffer_free(data: *mut u8, allocation_id: u64) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        buffer::free(data, allocation_id)
    }));
}

pub(crate) fn abi_version() -> u32 {
    crate::SK_ABI_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    #[test]
    fn oversized_requests_are_rejected_before_handle_lookup_or_parsing() {
        let mut data = ptr::null_mut();
        let mut len = 0;
        let mut allocation_id = 0;
        // SAFETY: `output` is a valid empty buffer. The oversized request is
        // rejected before its null pointer could be dereferenced.
        let result = unsafe {
            call(
                0,
                ptr::null(),
                MAX_REQUEST_BYTES + 1,
                &mut data,
                &mut len,
                &mut allocation_id,
            )
        };
        assert_eq!(result, status::INVALID_ARGUMENT);
        assert!(data.is_null());
        assert_eq!(len, 0);
        assert_eq!(allocation_id, 0);
    }

    #[test]
    fn output_is_initialized_before_handle_validation() {
        let mut sentinel = 0x5au8;
        let mut data = &mut sentinel as *mut u8;
        let mut len = usize::MAX;
        let mut allocation_id = u64::MAX;
        // SAFETY: all outputs are writable storage. Their values intentionally
        // hold arbitrary values to prove that `call` does not read them first.
        let result = unsafe { call(0, ptr::null(), 0, &mut data, &mut len, &mut allocation_id) };
        assert_eq!(result, status::INVALID_HANDLE);
        assert!(data.is_null());
        assert_eq!(len, 0);
        assert_eq!(allocation_id, 0);
        assert_eq!(sentinel, 0x5a);
    }

    #[test]
    fn every_missing_output_pointer_is_rejected_without_reading_outputs() {
        let mut len = usize::MAX;
        let mut allocation_id = u64::MAX;
        // SAFETY: the non-null output pointers are valid writable storage.
        let result = unsafe {
            call(
                0,
                ptr::null(),
                0,
                ptr::null_mut(),
                &mut len,
                &mut allocation_id,
            )
        };
        assert_eq!(result, status::INVALID_ARGUMENT);
        assert_eq!(len, 0);
        assert_eq!(allocation_id, 0);

        let mut data = ptr::null_mut();
        let mut allocation_id_2 = u64::MAX;
        // SAFETY: the non-null output pointers are valid writable storage.
        let result = unsafe {
            call(
                0,
                ptr::null(),
                0,
                &mut data,
                ptr::null_mut(),
                &mut allocation_id_2,
            )
        };
        assert_eq!(result, status::INVALID_ARGUMENT);
        assert!(data.is_null());
        assert_eq!(allocation_id_2, 0);

        data = ptr::null_mut();
        let mut len_3 = usize::MAX;
        // SAFETY: the non-null output pointers are valid writable storage.
        let result = unsafe { call(0, ptr::null(), 0, &mut data, &mut len_3, ptr::null_mut()) };
        assert_eq!(result, status::INVALID_ARGUMENT);
        assert!(data.is_null());
        assert_eq!(len_3, 0);
    }

    #[test]
    fn invalid_handle_and_close_statuses_are_stable() {
        assert_eq!(abi_version(), crate::SK_ABI_VERSION);
        assert_eq!(close(0, ptr::null_mut(), 0), status::INVALID_HANDLE);
        assert_eq!(close(u64::MAX, ptr::null_mut(), 0), status::INVALID_HANDLE);
    }
}
