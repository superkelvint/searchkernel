use crate::engine::Engine;
use crate::error::{check, cstring, Result};
use crate::native::NativeError;
use searchkernel_vespa_bindings as sys;

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Error,
    StdException,
    BadAlloc,
    UnknownException,
    Block,
}

impl Action {
    fn native(self) -> u32 {
        match self {
            Self::Error => sys::hardening::SK_TEST_FAILPOINT_ERROR,
            Self::StdException => sys::hardening::SK_TEST_FAILPOINT_STD_EXCEPTION,
            Self::BadAlloc => sys::hardening::SK_TEST_FAILPOINT_BAD_ALLOC,
            Self::UnknownException => sys::hardening::SK_TEST_FAILPOINT_UNKNOWN_EXCEPTION,
            Self::Block => sys::hardening::SK_TEST_FAILPOINT_BLOCK,
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum ResultFault {
    NullResult = sys::hardening::SK_TEST_RESULT_FAULT_NULL_RESULT,
    NullDocumentFieldName = sys::hardening::SK_TEST_RESULT_FAULT_NULL_DOCUMENT_FIELD_NAME,
    InvalidDocumentFieldName = sys::hardening::SK_TEST_RESULT_FAULT_INVALID_DOCUMENT_FIELD_NAME,
    NullDocumentString = sys::hardening::SK_TEST_RESULT_FAULT_NULL_DOCUMENT_STRING,
    InvalidDocumentString = sys::hardening::SK_TEST_RESULT_FAULT_INVALID_DOCUMENT_STRING,
    UnknownDocumentKind = sys::hardening::SK_TEST_RESULT_FAULT_UNKNOWN_DOCUMENT_KIND,
    DocumentAccessorError = sys::hardening::SK_TEST_RESULT_FAULT_DOCUMENT_ACCESSOR_ERROR,
    NullHitId = sys::hardening::SK_TEST_RESULT_FAULT_NULL_HIT_ID,
    InvalidHitId = sys::hardening::SK_TEST_RESULT_FAULT_INVALID_HIT_ID,
    NullSummaryName = sys::hardening::SK_TEST_RESULT_FAULT_NULL_SUMMARY_NAME,
    InvalidSummaryName = sys::hardening::SK_TEST_RESULT_FAULT_INVALID_SUMMARY_NAME,
    NullSummaryString = sys::hardening::SK_TEST_RESULT_FAULT_NULL_SUMMARY_STRING,
    InvalidSummaryString = sys::hardening::SK_TEST_RESULT_FAULT_INVALID_SUMMARY_STRING,
    UnknownSummaryKind = sys::hardening::SK_TEST_RESULT_FAULT_UNKNOWN_SUMMARY_KIND,
    NullMatchFeatureName = sys::hardening::SK_TEST_RESULT_FAULT_NULL_MATCH_FEATURE_NAME,
    InvalidMatchFeatureName = sys::hardening::SK_TEST_RESULT_FAULT_INVALID_MATCH_FEATURE_NAME,
    DuplicateMatchFeatureName = sys::hardening::SK_TEST_RESULT_FAULT_DUPLICATE_MATCH_FEATURE_NAME,
    MatchFeatureValueError = sys::hardening::SK_TEST_RESULT_FAULT_MATCH_FEATURE_VALUE_ERROR,
    UnsupportedGroupKey = sys::hardening::SK_TEST_RESULT_FAULT_UNSUPPORTED_GROUP_KEY,
    InvalidGroupKey = sys::hardening::SK_TEST_RESULT_FAULT_INVALID_GROUP_KEY,
    GroupAccessorError = sys::hardening::SK_TEST_RESULT_FAULT_GROUP_ACCESSOR_ERROR,
    UnknownAggregationKind = sys::hardening::SK_TEST_RESULT_FAULT_UNKNOWN_AGGREGATION_KIND,
    AggregationAccessorError = sys::hardening::SK_TEST_RESULT_FAULT_AGGREGATION_ACCESSOR_ERROR,
}

pub fn set_result_fault(engine: &Engine, fault: ResultFault) -> Result<()> {
    let mut error = NativeError::new();
    let status = unsafe {
        // SAFETY: Engine owns a live opaque handle and the temporary
        // diagnostic buffer remains valid for this call.
        sys::hardening::sk_test_result_fault_set(
            engine.handle.as_ptr(),
            fault as u32,
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

pub fn clear_result_fault(engine: &Engine) -> Result<()> {
    let mut error = NativeError::new();
    let status = unsafe {
        // SAFETY: Engine owns a live opaque handle and the temporary
        // diagnostic buffer remains valid for this call.
        sys::hardening::sk_test_result_fault_clear(
            engine.handle.as_ptr(),
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

pub fn result_destroy_count(engine: &Engine) -> Result<u64> {
    let mut count = 0;
    let mut error = NativeError::new();
    let status = unsafe {
        // SAFETY: Engine owns a live opaque handle and the output and
        // diagnostic buffers remain valid for this call.
        sys::hardening::sk_test_result_destroy_count(
            engine.handle.as_ptr(),
            &mut count,
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)?;
    Ok(count)
}

pub fn set(engine: &Engine, name: &str, hit: u32, action: Action) -> Result<()> {
    let name = cstring(name)?;
    let mut error = NativeError::new();
    let status = unsafe {
        // SAFETY: Engine owns a live opaque handle and the temporary C
        // string/error buffer remain valid for this call only.
        sys::hardening::sk_test_failpoint_set(
            engine.handle.as_ptr(),
            name.as_ptr(),
            hit,
            action.native(),
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

pub fn set_open(name: &str, hit: u32, action: Action) -> Result<()> {
    let name = cstring(name)?;
    let mut error = NativeError::new();
    let status = unsafe {
        // SAFETY: the temporary C string/error buffer remain valid for
        // the duration of this test-control call.
        sys::hardening::sk_test_failpoint_set_open(
            name.as_ptr(),
            hit,
            action.native(),
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

pub fn clear(engine: &Engine, name: &str) -> Result<()> {
    let name = cstring(name)?;
    let mut error = NativeError::new();
    let status = unsafe {
        // SAFETY: Engine owns a live opaque handle and the temporary C
        // string/error buffer remain valid for this call only.
        sys::hardening::sk_test_failpoint_clear(
            engine.handle.as_ptr(),
            name.as_ptr(),
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

pub fn clear_open(name: &str) -> Result<()> {
    let name = cstring(name)?;
    let mut error = NativeError::new();
    let status = unsafe {
        // SAFETY: the temporary C string/error buffer remain valid for
        // the duration of this test-control call.
        sys::hardening::sk_test_failpoint_clear_open(
            name.as_ptr(),
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

pub fn release(engine: &Engine, name: &str) -> Result<()> {
    let name = cstring(name)?;
    let mut error = NativeError::new();
    let status = unsafe {
        // SAFETY: Engine owns a live opaque handle and the temporary C
        // string/error buffer remain valid for this call only.
        sys::hardening::sk_test_failpoint_release(
            engine.handle.as_ptr(),
            name.as_ptr(),
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

pub fn is_blocked(engine: &Engine, name: &str) -> Result<bool> {
    let name = cstring(name)?;
    let mut blocked = 0;
    let mut error = NativeError::new();
    let status = unsafe {
        // SAFETY: Engine owns a live opaque handle and the temporary C
        // string/error buffer remain valid for this call only.
        sys::hardening::sk_test_failpoint_is_blocked(
            engine.handle.as_ptr(),
            name.as_ptr(),
            &mut blocked,
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)?;
    Ok(blocked != 0)
}

#[cfg(test)]
mod tests {
    use super::{sys, Action};

    #[test]
    fn actions_use_the_stable_native_failpoint_codes() {
        let actions = [
            (Action::Error, sys::hardening::SK_TEST_FAILPOINT_ERROR),
            (
                Action::StdException,
                sys::hardening::SK_TEST_FAILPOINT_STD_EXCEPTION,
            ),
            (
                Action::BadAlloc,
                sys::hardening::SK_TEST_FAILPOINT_BAD_ALLOC,
            ),
            (
                Action::UnknownException,
                sys::hardening::SK_TEST_FAILPOINT_UNKNOWN_EXCEPTION,
            ),
            (Action::Block, sys::hardening::SK_TEST_FAILPOINT_BLOCK),
        ];

        for (action, expected) in actions {
            assert_eq!(action.native(), expected);
        }
    }
}
