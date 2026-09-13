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

#[repr(C)]
#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct SkBuffer {
    /// Pointer to the response bytes, or null for an empty buffer.
    pub data: *mut u8,
    /// Number of response bytes visible to the caller.
    pub len: usize,
    /// Search Kernel's non-reused process-lifetime allocation token.
    pub allocation_id: u64,
}

#[cfg(test)]
mod abi_layout_tests {
    use super::SkBuffer;
    use std::mem::{align_of, offset_of, size_of};

    #[test]
    fn buffer_layout_is_c_compatible() {
        assert_eq!(
            size_of::<SkBuffer>(),
            size_of::<usize>() * 2 + size_of::<u64>()
        );
        assert_eq!(align_of::<SkBuffer>(), align_of::<u64>());
        assert_eq!(offset_of!(SkBuffer, data), 0);
        assert_eq!(offset_of!(SkBuffer, len), size_of::<usize>());
        assert_eq!(offset_of!(SkBuffer, allocation_id), size_of::<usize>() * 2);
    }
}

#[no_mangle]
pub extern "C" fn sk_abi_version() -> u32 {
    abi::abi_version()
}

#[no_mangle]
/// # Safety
///
/// `out_response` must point to a valid writable `SkBuffer`, and `request`
/// must point to `request_len` readable bytes when `request_len` is non-zero.
pub unsafe extern "C" fn sk_call(
    handle: SkHandle,
    request: *const u8,
    request_len: usize,
    out_response: *mut SkBuffer,
) -> SkStatus {
    // SAFETY: the function's caller is responsible for the documented
    // foreign pointer validity requirements.
    unsafe { abi::call(handle, request, request_len, out_response) }
}

#[no_mangle]
pub extern "C" fn sk_close(handle: SkHandle) -> SkStatus {
    abi::close(handle)
}

#[no_mangle]
/// # Safety
///
/// `buffer` must be null or point to a valid writable `SkBuffer`. If it is
/// non-empty, it must be a buffer previously returned by `sk_call`.
pub unsafe extern "C" fn sk_buffer_free(buffer: *mut SkBuffer) {
    // SAFETY: the function's caller is responsible for the documented buffer
    // structure validity requirements.
    unsafe { abi::buffer_free(buffer) }
}
