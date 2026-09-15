#![deny(unsafe_op_in_unsafe_fn)]

mod abi;
mod buffer;
mod dispatch;
#[allow(warnings)]
#[path = "../generated/mod.rs"]
mod generated;
mod handle;
mod status;
mod transport;

#[cfg(feature = "ffi-test-support")]
pub mod test_support;

pub type SkHandle = u64;
pub type SkStatus = u32;

pub const SK_ABI_VERSION: u32 = 1;
pub const SK_STATUS_OK: u32 = 0;
pub const SK_STATUS_INVALID_ARGUMENT: u32 = 1;
pub const SK_STATUS_INVALID_HANDLE: u32 = 2;
pub const SK_STATUS_INVALID_REQUEST: u32 = 3;
pub const SK_STATUS_UNSUPPORTED_PROTOCOL: u32 = 4;
pub const SK_STATUS_UNSUPPORTED_OPERATION: u32 = 5;
pub const SK_STATUS_INTERNAL: u32 = 6;
pub const SK_STATUS_PANIC: u32 = 7;

#[cfg(test)]
mod primitive_output_abi_tests {
    use super::{sk_buffer_free, sk_call, SK_STATUS_INVALID_ARGUMENT, SK_STATUS_INVALID_HANDLE};
    use std::ptr;

    #[test]
    fn output_uses_primitive_out_parameters() {
        let mut data = ptr::null_mut();
        let mut len = usize::MAX;
        let mut allocation_id = u64::MAX;
        // SAFETY: all output pointers are valid writable storage, and the
        // empty request is rejected before any request bytes are accessed.
        let status = unsafe { sk_call(0, ptr::null(), 0, &mut data, &mut len, &mut allocation_id) };
        assert_eq!(status, SK_STATUS_INVALID_HANDLE);
        assert!(data.is_null());
        assert_eq!(len, 0);
        assert_eq!(allocation_id, 0);

        let mut partial_len = usize::MAX;
        let mut partial_allocation_id = u64::MAX;
        // SAFETY: the non-null output pointers are valid writable storage.
        let status = unsafe {
            sk_call(
                0,
                ptr::null(),
                0,
                ptr::null_mut(),
                &mut partial_len,
                &mut partial_allocation_id,
            )
        };
        assert_eq!(status, SK_STATUS_INVALID_ARGUMENT);
        assert_eq!(partial_len, 0);
        assert_eq!(partial_allocation_id, 0);

        // SAFETY: null data with a zero token is the empty free operation.
        unsafe { sk_buffer_free(ptr::null_mut(), 0) };
    }
}

#[cfg(all(test, feature = "ffi-test-support"))]
#[path = "json_convergence_tests.rs"]
mod json_convergence_tests;

#[no_mangle]
pub extern "C" fn sk_abi_version() -> u32 {
    abi::abi_version()
}

#[no_mangle]
/// # Safety
///
/// `out_data`, `out_len`, and `out_allocation_id` must point to writable
/// storage. They are write-only out-parameters on entry, so their previous
/// contents are not read.
/// The caller must release any response previously held in that storage before
/// reusing it, and must not access the storage concurrently with this call.
/// `request` must point to `request_len` readable, unmodified bytes for the
/// entire duration of this call when `request_len` is non-zero.
pub unsafe extern "C" fn sk_call(
    handle: SkHandle,
    request: *const u8,
    request_len: usize,
    out_data: *mut *mut u8,
    out_len: *mut usize,
    out_allocation_id: *mut u64,
) -> SkStatus {
    // SAFETY: the function's caller is responsible for the documented
    // foreign pointer validity requirements.
    unsafe {
        abi::call(
            handle,
            request,
            request_len,
            out_data,
            out_len,
            out_allocation_id,
        )
    }
}

#[no_mangle]
pub extern "C" fn sk_close(handle: SkHandle) -> SkStatus {
    abi::close(handle)
}

#[no_mangle]
/// # Safety
///
/// `data` must be null or a pointer previously returned through `sk_call`,
/// and `allocation_id` must be the matching token. Passing a stale or already
/// released pair is harmless. The caller must not access the response bytes
/// after releasing them.
pub unsafe extern "C" fn sk_buffer_free(data: *mut u8, allocation_id: u64) {
    // SAFETY: the function's caller is responsible for the documented pointer
    // and token validity requirements.
    unsafe { abi::buffer_free(data, allocation_id) }
}
