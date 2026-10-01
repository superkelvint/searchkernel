#![deny(unsafe_op_in_unsafe_fn)]
#![allow(non_camel_case_types)]

mod abi;
pub use abi::EXPECTED_ABI;

include!("bindings.rs");

#[cfg(test)]
#[path = "consumer_runtime.rs"]
mod consumer_runtime_tests;

#[cfg(feature = "hardening")]
pub mod hardening;

#[cfg(all(test, feature = "native-contract-tests"))]
unsafe extern "C" {
    fn searchkernel_schema_identity_fd_reuse_regression() -> i32;
    fn searchkernel_transport_destructor_regression() -> i32;
}

#[cfg(all(test, feature = "native-contract-tests"))]
mod native_regression_tests {
    use super::{
        searchkernel_schema_identity_fd_reuse_regression,
        searchkernel_transport_destructor_regression,
    };

    #[test]
    fn schema_identity_fd_cleanup_survives_descriptor_reuse() {
        let result = unsafe { searchkernel_schema_identity_fd_reuse_regression() };
        assert_eq!(
            result, 0,
            "descriptor reuse regression failed with code {result}"
        );
    }

    #[test]
    fn started_transport_is_shutdown_before_destruction() {
        let result = unsafe { searchkernel_transport_destructor_regression() };
        assert_eq!(
            result, 0,
            "transport destructor regression failed with code {result}"
        );
    }
}

#[cfg(all(test, feature = "native"))]
mod text_soa_abi_regression_tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn text_transport_uses_opaque_soa_builders() {
        let _document_add: unsafe extern "C" fn(
            *mut sk_text_document,
            *const u8,
            u32,
            *const u32,
            *const u32,
            *const u32,
            u32,
            *mut u8,
            u32,
        ) -> i32 = sk_text_document_add_occurrences;
        let _request_add: unsafe extern "C" fn(
            *mut sk_text_request,
            *const u8,
            u32,
            *const u32,
            u32,
            *mut u8,
            u32,
        ) -> i32 = sk_text_request_add_terms;
    }

    #[test]
    fn malformed_text_memory_preconditions_are_controlled_errors() {
        let id = CString::new("id").unwrap();
        let field = CString::new("title").unwrap();
        let mut document = std::ptr::null_mut();
        let mut error = [0_u8; SK_ERROR_MESSAGE_CAPACITY as usize];
        let status = unsafe {
            // SAFETY: the null pointer is an intentional ABI precondition
            // probe; the native function must reject length-one null input
            // without dereferencing it.
            sk_text_document_create(
                id.as_ptr(),
                field.as_ptr(),
                std::ptr::null(),
                1,
                &mut document,
                error.as_mut_ptr(),
                error.len() as u32,
            )
        };
        assert_eq!(status, SK_NATIVE_STATUS_INVALID_ARGUMENT as i32);
        assert!(document.is_null());

        let invalid_utf8 = [0xff_u8];
        let status = unsafe {
            // SAFETY: the byte array is live for this call and intentionally
            // contains invalid UTF-8 to exercise native validation.
            sk_text_document_create(
                id.as_ptr(),
                field.as_ptr(),
                invalid_utf8.as_ptr(),
                invalid_utf8.len() as u32,
                &mut document,
                error.as_mut_ptr(),
                error.len() as u32,
            )
        };
        assert_eq!(status, SK_NATIVE_STATUS_INVALID_ARGUMENT as i32);
        assert!(document.is_null());
    }

    #[test]
    fn text_soa_builder_validates_parallel_arrays_before_mutation() {
        let id = CString::new("id").unwrap();
        let field = CString::new("title").unwrap();
        let text = b"quick fox";
        let mut document = std::ptr::null_mut();
        let mut error = [0_u8; SK_ERROR_MESSAGE_CAPACITY as usize];
        let status = unsafe {
            sk_text_document_create(
                id.as_ptr(),
                field.as_ptr(),
                text.as_ptr(),
                text.len() as u32,
                &mut document,
                error.as_mut_ptr(),
                error.len() as u32,
            )
        };
        assert_eq!(status, SK_NATIVE_STATUS_OK as i32);
        assert!(!document.is_null());

        let term_bytes = b"quick";
        let term_lengths = [4_u32];
        let byte_starts = [0_u32];
        let byte_lengths = [5_u32];
        let status = unsafe {
            sk_text_document_add_occurrences(
                document,
                term_bytes.as_ptr(),
                term_bytes.len() as u32,
                term_lengths.as_ptr(),
                byte_starts.as_ptr(),
                byte_lengths.as_ptr(),
                1,
                error.as_mut_ptr(),
                error.len() as u32,
            )
        };
        assert_eq!(status, SK_NATIVE_STATUS_INVALID_ARGUMENT as i32);

        unsafe { sk_text_document_destroy(document) };
    }

    #[test]
    fn text_soa_builders_accept_only_one_population_transfer() {
        let id = CString::new("id").unwrap();
        let field = CString::new("title").unwrap();
        let rank_profile = CString::new("default").unwrap();
        let text = b"quick fox";
        let term_bytes = b"quick";
        let term_lengths = [5_u32];
        let byte_starts = [0_u32];
        let byte_lengths = [5_u32];
        let mut error = [0_u8; SK_ERROR_MESSAGE_CAPACITY as usize];

        let mut document = std::ptr::null_mut();
        let status = unsafe {
            sk_text_document_create(
                id.as_ptr(),
                field.as_ptr(),
                text.as_ptr(),
                text.len() as u32,
                &mut document,
                error.as_mut_ptr(),
                error.len() as u32,
            )
        };
        assert_eq!(status, SK_NATIVE_STATUS_OK as i32);
        let status = unsafe {
            sk_text_document_add_occurrences(
                document,
                term_bytes.as_ptr(),
                term_bytes.len() as u32,
                term_lengths.as_ptr(),
                byte_starts.as_ptr(),
                byte_lengths.as_ptr(),
                1,
                error.as_mut_ptr(),
                error.len() as u32,
            )
        };
        assert_eq!(status, SK_NATIVE_STATUS_OK as i32);
        let status = unsafe {
            sk_text_document_add_occurrences(
                document,
                term_bytes.as_ptr(),
                term_bytes.len() as u32,
                term_lengths.as_ptr(),
                byte_starts.as_ptr(),
                byte_lengths.as_ptr(),
                1,
                error.as_mut_ptr(),
                error.len() as u32,
            )
        };
        assert_eq!(status, SK_NATIVE_STATUS_INVALID_ARGUMENT as i32);
        unsafe { sk_text_document_destroy(document) };

        let mut update = std::ptr::null_mut();
        let status = unsafe {
            sk_text_update_create(
                id.as_ptr(),
                field.as_ptr(),
                text.as_ptr(),
                text.len() as u32,
                &mut update,
                error.as_mut_ptr(),
                error.len() as u32,
            )
        };
        assert_eq!(status, SK_NATIVE_STATUS_OK as i32);
        let status = unsafe {
            sk_text_update_add_occurrences(
                update,
                term_bytes.as_ptr(),
                term_bytes.len() as u32,
                term_lengths.as_ptr(),
                byte_starts.as_ptr(),
                byte_lengths.as_ptr(),
                1,
                error.as_mut_ptr(),
                error.len() as u32,
            )
        };
        assert_eq!(status, SK_NATIVE_STATUS_OK as i32);
        let status = unsafe {
            sk_text_update_add_occurrences(
                update,
                term_bytes.as_ptr(),
                term_bytes.len() as u32,
                term_lengths.as_ptr(),
                byte_starts.as_ptr(),
                byte_lengths.as_ptr(),
                1,
                error.as_mut_ptr(),
                error.len() as u32,
            )
        };
        assert_eq!(status, SK_NATIVE_STATUS_INVALID_ARGUMENT as i32);
        unsafe { sk_text_update_destroy(update) };

        let mut request = std::ptr::null_mut();
        let status = unsafe {
            sk_text_request_create(
                SK_TEXT_QUERY_PHRASE,
                field.as_ptr(),
                rank_profile.as_ptr(),
                &mut request,
                error.as_mut_ptr(),
                error.len() as u32,
            )
        };
        assert_eq!(status, SK_NATIVE_STATUS_OK as i32);
        let status = unsafe {
            sk_text_request_add_terms(
                request,
                term_bytes.as_ptr(),
                term_bytes.len() as u32,
                term_lengths.as_ptr(),
                1,
                error.as_mut_ptr(),
                error.len() as u32,
            )
        };
        assert_eq!(status, SK_NATIVE_STATUS_OK as i32);
        let status = unsafe {
            sk_text_request_add_terms(
                request,
                term_bytes.as_ptr(),
                term_bytes.len() as u32,
                term_lengths.as_ptr(),
                1,
                error.as_mut_ptr(),
                error.len() as u32,
            )
        };
        assert_eq!(status, SK_NATIVE_STATUS_INVALID_ARGUMENT as i32);
        unsafe { sk_text_request_destroy(request) };
    }
}

#[cfg(all(test, feature = "native"))]
mod packed_document_buffer_tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn zero_total_null_packed_buffers_are_rejected_for_nonempty_items() {
        let id = CString::new("packed-buffer-test").expect("literal has no NUL");
        let field = CString::new("values").expect("literal has no NUL");
        let mut error = [0_u8; SK_ERROR_MESSAGE_CAPACITY as usize];
        let mut document = std::ptr::null_mut();
        let status = unsafe {
            sk_document_create_empty(
                id.as_ptr(),
                &mut document,
                error.as_mut_ptr(),
                error.len() as u32,
            )
        };
        assert_eq!(status, SK_NATIVE_STATUS_OK as i32);

        let empty_length = [0_u32];
        let status = unsafe {
            sk_document_add_string_array(
                document,
                field.as_ptr(),
                std::ptr::null(),
                empty_length.as_ptr(),
                1,
                0,
                error.as_mut_ptr(),
                error.len() as u32,
            )
        };
        assert_eq!(status, SK_NATIVE_STATUS_INVALID_ARGUMENT as i32);

        let status = unsafe {
            sk_document_add_predicate(
                document,
                field.as_ptr(),
                std::ptr::null(),
                empty_length.as_ptr(),
                0,
                std::ptr::null(),
                empty_length.as_ptr(),
                0,
                1,
                error.as_mut_ptr(),
                error.len() as u32,
            )
        };
        assert_eq!(status, SK_NATIVE_STATUS_INVALID_ARGUMENT as i32);

        unsafe { sk_document_destroy(document) };
    }
}
