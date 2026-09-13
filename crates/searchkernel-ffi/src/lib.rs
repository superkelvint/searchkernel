#![deny(unsafe_op_in_unsafe_fn)]

mod abi;
mod buffer;
mod dispatch;
mod abi_contract {
    include!(concat!(env!("OUT_DIR"), "/abi_contract.rs"));
}
#[allow(warnings)]
#[path = "../generated/mod.rs"]
mod generated;
mod handle;
mod status;
mod transport;

#[cfg(feature = "ffi-test-support")]
pub mod test_support;

#[repr(C)]
#[doc(hidden)]
pub struct SkBuffer {
    pub data: *mut u8,
    pub len: usize,
}

#[cfg(test)]
mod abi_layout_tests {
    use super::SkBuffer;
    use std::mem::{align_of, size_of};

    #[test]
    fn buffer_layout_is_two_native_pointer_sized_words() {
        assert_eq!(size_of::<SkBuffer>(), size_of::<usize>() * 2);
        assert_eq!(align_of::<SkBuffer>(), align_of::<usize>());
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
    handle: u64,
    request: *const u8,
    request_len: usize,
    out_response: *mut SkBuffer,
) -> u32 {
    // SAFETY: the function's caller is responsible for the documented
    // foreign pointer validity requirements.
    unsafe { abi::call(handle, request, request_len, out_response) }
}

#[no_mangle]
pub extern "C" fn sk_close(handle: u64) -> u32 {
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
