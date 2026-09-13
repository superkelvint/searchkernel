#![allow(non_camel_case_types)]

mod abi;
pub use abi::EXPECTED_ABI;

include!("bindings.rs");

#[cfg(test)]
unsafe extern "C" {
    fn searchkernel_schema_identity_fd_reuse_regression() -> i32;
}

#[cfg(test)]
mod abi_layout_tests {
    use super::*;
    use std::mem::{align_of, offset_of, size_of};

    #[test]
    fn generated_layout_matches_the_frozen_c_header_layout() {
        assert_eq!(size_of::<sk_text_occurrence>(), 24);
        assert_eq!(align_of::<sk_text_occurrence>(), 8);
        assert_eq!(offset_of!(sk_text_occurrence, term), 0);
        assert_eq!(offset_of!(sk_text_occurrence, term_length), 8);
        assert_eq!(offset_of!(sk_text_occurrence, byte_start), 12);
        assert_eq!(offset_of!(sk_text_occurrence, byte_length), 16);
        assert_eq!(offset_of!(sk_text_occurrence, position), 20);

        assert_eq!(size_of::<sk_text_query_term>(), 16);
        assert_eq!(align_of::<sk_text_query_term>(), 8);
        assert_eq!(offset_of!(sk_text_query_term, term), 0);
        assert_eq!(offset_of!(sk_text_query_term, term_length), 8);
        assert_eq!(offset_of!(sk_text_query_term, position), 12);

        assert_eq!(size_of::<sk_error>(), 516);
        assert_eq!(align_of::<sk_error>(), 4);
        assert_eq!(offset_of!(sk_error, code), 0);
        assert_eq!(offset_of!(sk_error, message), 4);
    }
}

#[cfg(test)]
mod native_regression_tests {
    use super::searchkernel_schema_identity_fd_reuse_regression;

    #[test]
    fn schema_identity_fd_cleanup_survives_descriptor_reuse() {
        let result = unsafe { searchkernel_schema_identity_fd_reuse_regression() };
        assert_eq!(
            result, 0,
            "descriptor reuse regression failed with code {result}"
        );
    }
}
