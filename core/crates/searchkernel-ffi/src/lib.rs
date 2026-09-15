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
pub const SK_STATUS_ALREADY_EXISTS: u32 = 8;
pub const SK_STATUS_NOT_FOUND: u32 = 9;
pub const SK_STATUS_CORRUPT_INDEX: u32 = 10;
pub const SK_STATUS_INCOMPATIBLE_INDEX: u32 = 11;

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

#[cfg(feature = "production")]
fn lifecycle_status(error: &searchkernel::Error) -> SkStatus {
    match error {
        searchkernel::Error::AlreadyExists(_) => SK_STATUS_ALREADY_EXISTS,
        searchkernel::Error::NotFound(_) => SK_STATUS_NOT_FOUND,
        searchkernel::Error::CorruptIndex(_) => SK_STATUS_CORRUPT_INDEX,
        searchkernel::Error::IncompatibleIndex(_) => SK_STATUS_INCOMPATIBLE_INDEX,
        searchkernel::Error::Portable(_)
        | searchkernel::Error::Serialization(_)
        | searchkernel::Error::Deserialization(_)
        | searchkernel::Error::Infrastructure(_) => SK_STATUS_INTERNAL,
    }
}

fn write_lifecycle_error(error: impl std::fmt::Display, output: *mut u8, capacity: usize) {
    if output.is_null() || capacity == 0 {
        return;
    }
    let bytes = error.to_string();
    let bytes = bytes.as_bytes();
    let length = bytes.len().min(capacity.saturating_sub(1));
    // SAFETY: the caller contract requires `output` to refer to writable
    // storage of at least `capacity` bytes.
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), output, length);
        *output.add(length) = 0;
    }
}

#[cfg(feature = "production")]
fn lifecycle_path(path: *const std::os::raw::c_char) -> Result<std::path::PathBuf, String> {
    if path.is_null() {
        return Err("path must not be null".into());
    }
    // SAFETY: the C ABI requires a valid NUL-terminated path string.
    let path = unsafe { std::ffi::CStr::from_ptr(path) };
    let path = path
        .to_str()
        .map_err(|_| "path must be valid UTF-8".to_owned())?;
    if path.is_empty() {
        return Err("path must not be empty".into());
    }
    Ok(std::path::PathBuf::from(path))
}

#[cfg(feature = "production")]
fn lifecycle_register(index: searchkernel::Index) -> Result<SkHandle, (SkStatus, String)> {
    let (context, backend) = index.into_backend_parts();
    let handle = handle::register(context, backend);
    if handle == 0 {
        return Err((
            SK_STATUS_INTERNAL,
            "native handle registry is exhausted".into(),
        ));
    }
    Ok(handle)
}

#[cfg(feature = "production")]
fn create_inner(
    path: *const std::os::raw::c_char,
    definition: *const u8,
    definition_len: usize,
) -> Result<SkHandle, (SkStatus, String)> {
    let path = lifecycle_path(path).map_err(|error| (SK_STATUS_INVALID_ARGUMENT, error))?;
    if definition_len == 0 || definition.is_null() || definition_len > abi::MAX_REQUEST_BYTES {
        return Err((
            SK_STATUS_INVALID_ARGUMENT,
            "index definition bytes are invalid".into(),
        ));
    }
    // SAFETY: the C ABI requires `definition` to reference `definition_len`
    // readable bytes for this call.
    let definition = unsafe { std::slice::from_raw_parts(definition, definition_len) };
    searchkernel::Index::create_from_definition_bytes(path, definition)
        .map_err(|error| (lifecycle_status(&error), error.to_string()))
        .and_then(lifecycle_register)
}

#[cfg(feature = "production")]
fn open_inner(path: *const std::os::raw::c_char) -> Result<SkHandle, (SkStatus, String)> {
    let path = lifecycle_path(path).map_err(|error| (SK_STATUS_INVALID_ARGUMENT, error))?;
    searchkernel::Index::open(path)
        .map_err(|error| (lifecycle_status(&error), error.to_string()))
        .and_then(lifecycle_register)
}

#[no_mangle]
/// Create an on-disk index from its canonical FlatBuffers definition.
///
/// # Safety
///
/// `path` must be a valid NUL-terminated C string. `definition` must point to
/// `definition_len` readable bytes, and `out_handle` must point to writable
/// storage for one handle. The optional error buffer must be writable for its
/// declared capacity.
pub unsafe extern "C" fn sk_create(
    path: *const std::os::raw::c_char,
    definition: *const u8,
    definition_len: usize,
    out_handle: *mut SkHandle,
    error: *mut u8,
    error_capacity: usize,
) -> SkStatus {
    if out_handle.is_null() {
        return SK_STATUS_INVALID_ARGUMENT;
    }
    // SAFETY: `out_handle` was checked for null and is a writable primitive
    // out-parameter under the C ABI contract.
    unsafe { *out_handle = 0 };
    #[cfg(not(feature = "production"))]
    let _ = (path, definition, definition_len);
    #[cfg(feature = "production")]
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        create_inner(path, definition, definition_len)
    }));
    #[cfg(not(feature = "production"))]
    let result = Ok::<_, ()>(Err::<SkHandle, _>((
        SK_STATUS_UNSUPPORTED_OPERATION,
        String::from("production bootstrap is not enabled in this build"),
    )));
    match result {
        Ok(Ok(handle)) => {
            // SAFETY: `out_handle` remains valid for this call.
            unsafe { *out_handle = handle };
            SK_STATUS_OK
        }
        Ok(Err((status, message))) => {
            write_lifecycle_error(message, error, error_capacity);
            status
        }
        Err(_) => {
            write_lifecycle_error("panic while creating index", error, error_capacity);
            SK_STATUS_PANIC
        }
    }
}

#[no_mangle]
/// Reopen an existing index using its durable manifest.
///
/// # Safety
///
/// `path` must be a valid NUL-terminated C string and `out_handle` must point
/// to writable storage for one handle. The optional error buffer must be
/// writable for its declared capacity.
pub unsafe extern "C" fn sk_open(
    path: *const std::os::raw::c_char,
    out_handle: *mut SkHandle,
    error: *mut u8,
    error_capacity: usize,
) -> SkStatus {
    if out_handle.is_null() {
        return SK_STATUS_INVALID_ARGUMENT;
    }
    // SAFETY: `out_handle` was checked for null and is a writable primitive
    // out-parameter under the C ABI contract.
    unsafe { *out_handle = 0 };
    #[cfg(not(feature = "production"))]
    let _ = path;
    #[cfg(feature = "production")]
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| open_inner(path)));
    #[cfg(not(feature = "production"))]
    let result = Ok::<_, ()>(Err::<SkHandle, _>((
        SK_STATUS_UNSUPPORTED_OPERATION,
        String::from("production bootstrap is not enabled in this build"),
    )));
    match result {
        Ok(Ok(handle)) => {
            // SAFETY: `out_handle` remains valid for this call.
            unsafe { *out_handle = handle };
            SK_STATUS_OK
        }
        Ok(Err((status, message))) => {
            write_lifecycle_error(message, error, error_capacity);
            status
        }
        Err(_) => {
            write_lifecycle_error("panic while opening index", error, error_capacity);
            SK_STATUS_PANIC
        }
    }
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
