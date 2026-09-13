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

#[repr(C)]
#[doc(hidden)]
pub struct SkBuffer {
    pub data: *mut u8,
    pub len: usize,
}

#[no_mangle]
pub extern "C" fn sk_abi_version() -> u32 {
    abi::abi_version()
}

#[no_mangle]
pub extern "C" fn sk_call(
    handle: u64,
    request: *const u8,
    request_len: usize,
    out_response: *mut SkBuffer,
) -> u32 {
    abi::call(handle, request, request_len, out_response)
}

#[no_mangle]
pub extern "C" fn sk_close(handle: u64) -> u32 {
    abi::close(handle)
}

#[no_mangle]
pub extern "C" fn sk_buffer_free(buffer: *mut SkBuffer) {
    abi::buffer_free(buffer)
}
