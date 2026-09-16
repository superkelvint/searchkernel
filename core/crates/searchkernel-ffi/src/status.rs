pub(crate) const OK: u32 = crate::SK_STATUS_OK;
pub(crate) const INVALID_ARGUMENT: u32 = crate::SK_STATUS_INVALID_ARGUMENT;
pub(crate) const INVALID_HANDLE: u32 = crate::SK_STATUS_INVALID_HANDLE;
pub(crate) const INVALID_REQUEST: u32 = crate::SK_STATUS_INVALID_REQUEST;
pub(crate) const UNSUPPORTED_PROTOCOL: u32 = crate::SK_STATUS_UNSUPPORTED_PROTOCOL;
pub(crate) const UNSUPPORTED_OPERATION: u32 = crate::SK_STATUS_UNSUPPORTED_OPERATION;
#[allow(dead_code)]
pub(crate) const INTERNAL: u32 = crate::SK_STATUS_INTERNAL;
pub(crate) const PANIC: u32 = crate::SK_STATUS_PANIC;
pub(crate) const INDEX_CLOSED: u32 = crate::SK_STATUS_INDEX_CLOSED;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn public_status_codes_are_unique_and_stable() {
        let codes = [
            OK,
            INVALID_ARGUMENT,
            INVALID_HANDLE,
            INVALID_REQUEST,
            UNSUPPORTED_PROTOCOL,
            UNSUPPORTED_OPERATION,
            INTERNAL,
            PANIC,
            INDEX_CLOSED,
        ];
        assert_eq!(codes, [0, 1, 2, 3, 4, 5, 6, 7, 12]);
        assert_eq!(
            codes.iter().copied().collect::<BTreeSet<_>>().len(),
            codes.len()
        );
    }
}
