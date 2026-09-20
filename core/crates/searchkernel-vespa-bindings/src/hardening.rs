#![allow(non_camel_case_types)]

use super::sk_engine;

pub const SK_TEST_FAILPOINT_ERROR: u32 = 1;
pub const SK_TEST_FAILPOINT_STD_EXCEPTION: u32 = 2;
pub const SK_TEST_FAILPOINT_BAD_ALLOC: u32 = 3;
pub const SK_TEST_FAILPOINT_UNKNOWN_EXCEPTION: u32 = 4;
pub const SK_TEST_FAILPOINT_BLOCK: u32 = 5;

pub const SK_TEST_RESULT_FAULT_NULL_RESULT: u32 = 1;
pub const SK_TEST_RESULT_FAULT_NULL_DOCUMENT_FIELD_NAME: u32 = 2;
pub const SK_TEST_RESULT_FAULT_INVALID_DOCUMENT_FIELD_NAME: u32 = 3;
pub const SK_TEST_RESULT_FAULT_NULL_DOCUMENT_STRING: u32 = 4;
pub const SK_TEST_RESULT_FAULT_INVALID_DOCUMENT_STRING: u32 = 5;
pub const SK_TEST_RESULT_FAULT_UNKNOWN_DOCUMENT_KIND: u32 = 6;
pub const SK_TEST_RESULT_FAULT_DOCUMENT_ACCESSOR_ERROR: u32 = 7;
pub const SK_TEST_RESULT_FAULT_NULL_HIT_ID: u32 = 8;
pub const SK_TEST_RESULT_FAULT_INVALID_HIT_ID: u32 = 9;
pub const SK_TEST_RESULT_FAULT_NULL_SUMMARY_NAME: u32 = 10;
pub const SK_TEST_RESULT_FAULT_INVALID_SUMMARY_NAME: u32 = 11;
pub const SK_TEST_RESULT_FAULT_NULL_SUMMARY_STRING: u32 = 12;
pub const SK_TEST_RESULT_FAULT_INVALID_SUMMARY_STRING: u32 = 13;
pub const SK_TEST_RESULT_FAULT_UNKNOWN_SUMMARY_KIND: u32 = 14;
pub const SK_TEST_RESULT_FAULT_NULL_MATCH_FEATURE_NAME: u32 = 15;
pub const SK_TEST_RESULT_FAULT_INVALID_MATCH_FEATURE_NAME: u32 = 16;
pub const SK_TEST_RESULT_FAULT_DUPLICATE_MATCH_FEATURE_NAME: u32 = 17;
pub const SK_TEST_RESULT_FAULT_MATCH_FEATURE_VALUE_ERROR: u32 = 18;
pub const SK_TEST_RESULT_FAULT_UNSUPPORTED_GROUP_KEY: u32 = 19;
pub const SK_TEST_RESULT_FAULT_INVALID_GROUP_KEY: u32 = 20;
pub const SK_TEST_RESULT_FAULT_GROUP_ACCESSOR_ERROR: u32 = 21;
pub const SK_TEST_RESULT_FAULT_UNKNOWN_AGGREGATION_KIND: u32 = 22;
pub const SK_TEST_RESULT_FAULT_AGGREGATION_ACCESSOR_ERROR: u32 = 23;
pub const SK_TEST_RESULT_FAULT_INVALID_GROUPING_STRING: u32 = 24;
pub const SK_TEST_RESULT_FAULT_UNKNOWN_GROUPING_VALUE_KIND: u32 = 25;
pub const SK_TEST_RESULT_FAULT_HIT_LIST_GROUPING_OUTPUT: u32 = 26;
pub const SK_TEST_RESULT_FAULT_UNKNOWN_GROUPING_OUTPUT_KIND: u32 = 27;
pub const SK_TEST_RESULT_FAULT_GROUPING_TREE_GROUP_ACCESSOR_ERROR: u32 = 28;
pub const SK_TEST_RESULT_FAULT_GROUPING_TREE_ROOT_ACCESSOR_ERROR: u32 = 29;
pub const SK_TEST_RESULT_FAULT_GROUPING_TREE_KEY_ACCESSOR_ERROR: u32 = 30;
pub const SK_TEST_RESULT_FAULT_GROUPING_TREE_OUTPUT_ACCESSOR_ERROR: u32 = 31;
pub const SK_TEST_RESULT_FAULT_GROUPING_OUTPUT_VALUE_ACCESSOR_ERROR: u32 = 32;
pub const SK_TEST_RESULT_FAULT_NULL_GROUPING_CHILD: u32 = 33;
pub const SK_TEST_RESULT_FAULT_NULL_GROUPING_OUTPUT: u32 = 34;
pub const SK_TEST_RESULT_FAULT_NULL_GROUPING_VALUE: u32 = 35;

// SAFETY: these declarations mirror the private test-only C ABI. The safe
// hardening wrapper validates strings and retains Engine ownership while each
// call is in progress.
unsafe extern "C" {
    pub fn sk_test_failpoint_set(
        engine: *mut sk_engine,
        name: *const ::std::os::raw::c_char,
        hit: u32,
        action: u32,
        out_error: *mut u8,
        out_error_capacity: u32,
    ) -> i32;
    pub fn sk_test_failpoint_set_open(
        name: *const ::std::os::raw::c_char,
        hit: u32,
        action: u32,
        out_error: *mut u8,
        out_error_capacity: u32,
    ) -> i32;
    pub fn sk_test_failpoint_clear(
        engine: *mut sk_engine,
        name: *const ::std::os::raw::c_char,
        out_error: *mut u8,
        out_error_capacity: u32,
    ) -> i32;
    pub fn sk_test_failpoint_clear_open(
        name: *const ::std::os::raw::c_char,
        out_error: *mut u8,
        out_error_capacity: u32,
    ) -> i32;
    pub fn sk_test_failpoint_release(
        engine: *mut sk_engine,
        name: *const ::std::os::raw::c_char,
        out_error: *mut u8,
        out_error_capacity: u32,
    ) -> i32;
    pub fn sk_test_failpoint_is_blocked(
        engine: *const sk_engine,
        name: *const ::std::os::raw::c_char,
        out_blocked: *mut u8,
        out_error: *mut u8,
        out_error_capacity: u32,
    ) -> i32;
    pub fn sk_test_result_fault_set(
        engine: *mut sk_engine,
        fault: u32,
        out_error: *mut u8,
        out_error_capacity: u32,
    ) -> i32;
    pub fn sk_test_result_fault_clear(
        engine: *mut sk_engine,
        out_error: *mut u8,
        out_error_capacity: u32,
    ) -> i32;
    pub fn sk_test_result_destroy_count(
        engine: *const sk_engine,
        out_count: *mut u64,
        out_error: *mut u8,
        out_error_capacity: u32,
    ) -> i32;
    pub fn sk_test_search_reply_metadata(
        degradation_flags: u32,
        issue_messages: *const *const ::std::os::raw::c_char,
        issue_count: u32,
        out_result: *mut *mut super::sk_generic_search_result,
        out_error: *mut u8,
        out_error_capacity: u32,
    ) -> i32;
}
