#![allow(non_camel_case_types)]

use super::sk_engine;

pub const SK_TEST_FAILPOINT_ERROR: u32 = 1;
pub const SK_TEST_FAILPOINT_STD_EXCEPTION: u32 = 2;
pub const SK_TEST_FAILPOINT_BAD_ALLOC: u32 = 3;
pub const SK_TEST_FAILPOINT_UNKNOWN_EXCEPTION: u32 = 4;
pub const SK_TEST_FAILPOINT_BLOCK: u32 = 5;

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
}
