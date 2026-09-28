//! Validation for logical multi-index daemon targets.
//!
//! Targets are protocol identities, not filesystem paths. Keeping the grammar
//! deliberately small makes the same identifier safe to use across transports
//! and lets the daemon choose its own persistent directory mapping.

use std::fmt;

/// Maximum UTF-8 byte length of a logical index target.
///
/// The accepted grammar is ASCII-only, so this is also the maximum character
/// count.
pub const MAX_INDEX_TARGET_LEN: usize = 128;

/// Why a logical index target was rejected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IndexTargetError {
    Empty,
    TooLong { len: usize, max: usize },
    InvalidFirstByte { byte: u8 },
    InvalidByte { offset: usize, byte: u8 },
}

impl fmt::Display for IndexTargetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(formatter, "index target must not be empty"),
            Self::TooLong { len, max } => {
                write!(formatter, "index target is {len} bytes; maximum is {max}")
            }
            Self::InvalidFirstByte { byte } => write!(
                formatter,
                "index target must start with a lowercase ASCII letter or digit; got byte 0x{byte:02x}"
            ),
            Self::InvalidByte { offset, byte } => write!(
                formatter,
                "index target byte {offset} is invalid: 0x{byte:02x}"
            ),
        }
    }
}

impl std::error::Error for IndexTargetError {}

/// Validates one canonical logical index target.
///
/// Valid targets match `[a-z0-9][a-z0-9_-]{0,127}`. In particular, path
/// separators, dots, whitespace, uppercase text, and non-ASCII input are
/// rejected before a daemon is allowed to map the target into its data root.
pub fn validate_index_target(value: &str) -> Result<&str, IndexTargetError> {
    let bytes = value.as_bytes();
    if bytes.is_empty() {
        return Err(IndexTargetError::Empty);
    }
    if bytes.len() > MAX_INDEX_TARGET_LEN {
        return Err(IndexTargetError::TooLong {
            len: bytes.len(),
            max: MAX_INDEX_TARGET_LEN,
        });
    }

    if !is_initial(bytes[0]) {
        return Err(IndexTargetError::InvalidFirstByte { byte: bytes[0] });
    }
    for (offset, byte) in bytes.iter().copied().enumerate().skip(1) {
        if !is_tail(byte) {
            return Err(IndexTargetError::InvalidByte { offset, byte });
        }
    }

    Ok(value)
}

const fn is_initial(byte: u8) -> bool {
    matches!(byte, b'a'..=b'z' | b'0'..=b'9')
}

const fn is_tail(byte: u8) -> bool {
    is_initial(byte) || matches!(byte, b'-' | b'_')
}

#[cfg(test)]
mod tests {
    use super::{validate_index_target, IndexTargetError, MAX_INDEX_TARGET_LEN};

    #[test]
    fn accepts_safe_logical_targets() {
        for value in ["products", "products-v2", "tenant_42", "0", "a0_b-c"] {
            assert_eq!(validate_index_target(value), Ok(value));
        }
        let maximum = "a".repeat(MAX_INDEX_TARGET_LEN);
        assert_eq!(validate_index_target(&maximum), Ok(maximum.as_str()));
    }

    #[test]
    fn rejects_empty_and_oversized_targets() {
        assert_eq!(validate_index_target(""), Err(IndexTargetError::Empty));
        assert_eq!(
            validate_index_target(&"a".repeat(MAX_INDEX_TARGET_LEN + 1)),
            Err(IndexTargetError::TooLong {
                len: MAX_INDEX_TARGET_LEN + 1,
                max: MAX_INDEX_TARGET_LEN,
            })
        );
    }

    #[test]
    fn rejects_path_syntax_and_noncanonical_names() {
        for value in [
            "/products",
            "../products",
            "products/v2",
            "products\\v2",
            ".",
            "..",
            "_products",
            "-products",
            "Products",
            "products.v2",
            "products v2",
            "products:",
            "café",
        ] {
            assert!(
                validate_index_target(value).is_err(),
                "target {value:?} should be rejected"
            );
        }
    }

    #[test]
    fn reports_the_first_invalid_byte() {
        assert_eq!(
            validate_index_target("products.v2"),
            Err(IndexTargetError::InvalidByte {
                offset: 8,
                byte: b'.',
            })
        );
        assert_eq!(
            validate_index_target("_products"),
            Err(IndexTargetError::InvalidFirstByte { byte: b'_' })
        );
    }
}
