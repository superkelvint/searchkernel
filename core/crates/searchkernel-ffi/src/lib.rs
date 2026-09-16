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

pub const SK_ABI_VERSION: u32 = 2;
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
pub const SK_STATUS_INDEX_CLOSED: u32 = 12;

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
/// Gracefully close an index handle and report any native durability error.
///
/// The error buffer may be null when only the status code is required. When
/// non-null, it must reference writable storage of `error_capacity` bytes.
pub extern "C" fn sk_close(handle: SkHandle, error: *mut u8, error_capacity: usize) -> SkStatus {
    abi::close(handle, error, error_capacity)
}

#[cfg(feature = "production")]
fn lifecycle_status(error: &searchkernel::Error) -> SkStatus {
    match error {
        searchkernel::Error::Search { code, .. } => match code {
            searchkernel::ErrorCode::InvalidArgument
            | searchkernel::ErrorCode::FailedPrecondition => SK_STATUS_INVALID_ARGUMENT,
            searchkernel::ErrorCode::AlreadyExists => SK_STATUS_ALREADY_EXISTS,
            searchkernel::ErrorCode::NotFound => SK_STATUS_NOT_FOUND,
            searchkernel::ErrorCode::Unsupported => SK_STATUS_UNSUPPORTED_OPERATION,
            searchkernel::ErrorCode::CorruptIndex => SK_STATUS_CORRUPT_INDEX,
            searchkernel::ErrorCode::IncompatibleIndex => SK_STATUS_INCOMPATIBLE_INDEX,
            searchkernel::ErrorCode::IndexClosed => SK_STATUS_INDEX_CLOSED,
            searchkernel::ErrorCode::ResourceExhausted
            | searchkernel::ErrorCode::Conflict
            | searchkernel::ErrorCode::Internal
            | searchkernel::ErrorCode::Unavailable => SK_STATUS_INTERNAL,
            _ => SK_STATUS_INTERNAL,
        },
        searchkernel::Error::Serialization(_) => SK_STATUS_INVALID_ARGUMENT,
        searchkernel::Error::Deserialization(_) | searchkernel::Error::Infrastructure(_) => {
            SK_STATUS_INTERNAL
        }
        _ => SK_STATUS_INTERNAL,
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
fn lifecycle_register(runtime: searchkernel::IndexRuntime) -> Result<SkHandle, (SkStatus, String)> {
    let (context, session) = runtime.into_session_parts();
    let handle = handle::register_session(context, session);
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
    searchkernel::IndexRuntime::create_from_definition_bytes(path, definition)
        .map_err(|error| (lifecycle_status(&error), error.to_string()))
        .and_then(lifecycle_register)
}

#[cfg(feature = "production")]
fn open_inner(path: *const std::os::raw::c_char) -> Result<SkHandle, (SkStatus, String)> {
    let path = lifecycle_path(path).map_err(|error| (SK_STATUS_INVALID_ARGUMENT, error))?;
    searchkernel::IndexRuntime::open(path)
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

#[cfg(all(test, feature = "production"))]
mod lifecycle_tests {
    use super::*;
    use searchkernel_ir::api;
    use std::ffi::CString;
    use std::path::PathBuf;

    fn definition_bytes() -> Vec<u8> {
        let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
        let name = builder.create_string("title");
        let stored = api::OptionalBool::new(true);
        let searchable = api::OptionalBool::new(true);
        let field = api::FieldDefinition::create(
            &mut builder,
            &api::FieldDefinitionArgs {
                name: Some(name),
                kind: api::FieldKind::Text,
                cardinality: api::FieldCardinality::Single,
                stored: Some(&stored),
                searchable: Some(&searchable),
                ..Default::default()
            },
        );
        let fields = builder.create_vector(&[field]);
        let definition = api::IndexDefinition::create(
            &mut builder,
            &api::IndexDefinitionArgs {
                fields: Some(fields),
                ..Default::default()
            },
        );
        builder.finish(definition, None);
        builder.finished_data().to_vec()
    }

    fn temporary_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "searchkernel-ffi-lifecycle-{label}-{}",
            std::process::id()
        ))
    }

    #[test]
    fn lifecycle_status_preserves_public_status_categories() {
        let cases = [
            (
                searchkernel::Error::search(searchkernel::ErrorCode::AlreadyExists, "exists"),
                SK_STATUS_ALREADY_EXISTS,
            ),
            (
                searchkernel::Error::search(searchkernel::ErrorCode::NotFound, "missing"),
                SK_STATUS_NOT_FOUND,
            ),
            (
                searchkernel::Error::search(searchkernel::ErrorCode::CorruptIndex, "corrupt"),
                SK_STATUS_CORRUPT_INDEX,
            ),
            (
                searchkernel::Error::search(
                    searchkernel::ErrorCode::IncompatibleIndex,
                    "incompatible",
                ),
                SK_STATUS_INCOMPATIBLE_INDEX,
            ),
            (
                searchkernel::Error::search(searchkernel::ErrorCode::InvalidArgument, "invalid"),
                SK_STATUS_INVALID_ARGUMENT,
            ),
            (
                searchkernel::Error::search(
                    searchkernel::ErrorCode::FailedPrecondition,
                    "precondition",
                ),
                SK_STATUS_INVALID_ARGUMENT,
            ),
            (
                searchkernel::Error::search(searchkernel::ErrorCode::AlreadyExists, "exists"),
                SK_STATUS_ALREADY_EXISTS,
            ),
            (
                searchkernel::Error::search(searchkernel::ErrorCode::NotFound, "missing"),
                SK_STATUS_NOT_FOUND,
            ),
            (
                searchkernel::Error::search(searchkernel::ErrorCode::Unsupported, "unsupported"),
                SK_STATUS_UNSUPPORTED_OPERATION,
            ),
        ];

        for (error, expected) in cases {
            assert_eq!(lifecycle_status(&error), expected);
        }

        for code in [
            searchkernel::ErrorCode::Conflict,
            searchkernel::ErrorCode::Internal,
            searchkernel::ErrorCode::Unavailable,
            searchkernel::ErrorCode::ResourceExhausted,
        ] {
            assert_eq!(
                lifecycle_status(&searchkernel::Error::search(code, "internal")),
                SK_STATUS_INTERNAL
            );
        }
        assert_eq!(
            lifecycle_status(&searchkernel::Error::Serialization("bad document".into())),
            SK_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(
            lifecycle_status(&searchkernel::Error::Deserialization("bad result".into())),
            SK_STATUS_INTERNAL
        );
        assert_eq!(
            lifecycle_status(&searchkernel::Error::Infrastructure("native".into())),
            SK_STATUS_INTERNAL
        );
    }

    #[test]
    fn lifecycle_path_validates_c_strings() {
        assert_eq!(
            lifecycle_path(std::ptr::null()).unwrap_err(),
            "path must not be null"
        );

        let empty = CString::new("").unwrap();
        assert_eq!(
            lifecycle_path(empty.as_ptr()).unwrap_err(),
            "path must not be empty"
        );

        let valid = CString::new("/tmp/searchkernel-test").unwrap();
        assert_eq!(
            lifecycle_path(valid.as_ptr()).unwrap(),
            PathBuf::from("/tmp/searchkernel-test")
        );

        let invalid_utf8 = [0xff_u8, 0];
        let error = lifecycle_path(invalid_utf8.as_ptr().cast()).unwrap_err();
        assert_eq!(error, "path must be valid UTF-8");
    }

    #[test]
    fn lifecycle_error_output_is_truncated_and_terminated() {
        let mut output = [0xa5_u8; 8];
        write_lifecycle_error("abcdefghijk", output.as_mut_ptr(), output.len());
        assert_eq!(&output, b"abcdefg\0");

        write_lifecycle_error("ignored", std::ptr::null_mut(), output.len());
        write_lifecycle_error("ignored", output.as_mut_ptr(), 0);
    }

    #[test]
    fn lifecycle_entrypoints_zero_handles_on_argument_errors() {
        let mut handle = u64::MAX;
        // SAFETY: the null output pointer is explicitly covered by the ABI.
        assert_eq!(
            unsafe {
                sk_create(
                    std::ptr::null(),
                    std::ptr::null(),
                    0,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    0,
                )
            },
            SK_STATUS_INVALID_ARGUMENT
        );

        let mut error = [0_u8; 64];
        // SAFETY: all pointers are valid for their declared roles; the null
        // path is rejected before any native operation is attempted.
        assert_eq!(
            unsafe {
                sk_create(
                    std::ptr::null(),
                    std::ptr::null(),
                    0,
                    &mut handle,
                    error.as_mut_ptr(),
                    error.len(),
                )
            },
            SK_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(handle, 0);
        assert_eq!(
            std::ffi::CStr::from_bytes_until_nul(&error)
                .unwrap()
                .to_bytes(),
            b"path must not be null"
        );

        handle = u64::MAX;
        // SAFETY: as above, with a valid writable handle and error buffer.
        assert_eq!(
            unsafe {
                sk_open(
                    std::ptr::null(),
                    &mut handle,
                    error.as_mut_ptr(),
                    error.len(),
                )
            },
            SK_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(handle, 0);
    }

    #[test]
    fn lifecycle_can_create_reopen_and_close_native_index() {
        let path = temporary_path("success");
        let path = CString::new(path.to_str().unwrap()).unwrap();
        let definition = definition_bytes();
        let mut handle = 0;
        let mut error = [0_u8; 512];

        let status = unsafe {
            sk_create(
                path.as_ptr(),
                definition.as_ptr(),
                definition.len(),
                &mut handle,
                error.as_mut_ptr(),
                error.len(),
            )
        };
        assert_eq!(status, SK_STATUS_OK);
        assert_ne!(handle, 0);
        assert_eq!(sk_close(handle, std::ptr::null_mut(), 0), SK_STATUS_OK);

        let mut reopened = 0;
        let status = unsafe {
            sk_open(
                path.as_ptr(),
                &mut reopened,
                error.as_mut_ptr(),
                error.len(),
            )
        };
        assert_eq!(status, SK_STATUS_OK);
        assert_ne!(reopened, 0);
        assert_eq!(sk_close(reopened, std::ptr::null_mut(), 0), SK_STATUS_OK);
        std::fs::remove_dir_all(path.to_str().unwrap()).unwrap();
    }

    #[test]
    fn lifecycle_reports_missing_index_and_invalid_definition() {
        let missing = temporary_path("missing");
        let missing = CString::new(missing.to_str().unwrap()).unwrap();
        let mut handle = u64::MAX;
        let mut error = [0_u8; 512];
        let status = unsafe {
            sk_open(
                missing.as_ptr(),
                &mut handle,
                error.as_mut_ptr(),
                error.len(),
            )
        };
        assert_eq!(status, SK_STATUS_NOT_FOUND);
        assert_eq!(handle, 0);
        assert!(std::ffi::CStr::from_bytes_until_nul(&error)
            .unwrap()
            .to_string_lossy()
            .starts_with("index not found:"));

        let invalid_path = temporary_path("invalid-definition");
        let invalid_path = CString::new(invalid_path.to_str().unwrap()).unwrap();
        handle = u64::MAX;
        let invalid_definition = b"not an index definition";
        let status = unsafe {
            sk_create(
                invalid_path.as_ptr(),
                invalid_definition.as_ptr(),
                invalid_definition.len(),
                &mut handle,
                error.as_mut_ptr(),
                error.len(),
            )
        };
        assert_eq!(status, SK_STATUS_INVALID_ARGUMENT);
        assert_eq!(handle, 0);
        assert!(std::ffi::CStr::from_bytes_until_nul(&error)
            .unwrap()
            .to_string_lossy()
            .contains("invalid index definition"));
    }
}
