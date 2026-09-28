//! Validation and stable error vocabulary for logical multi-index daemon targets.
//!
//! Targets are protocol identities, not filesystem paths. Keeping the grammar
//! deliberately small makes the same identifier safe to use across transports
//! and lets the daemon choose its own persistent directory mapping.

use crate::protocol::v2 as wire;
use std::fmt;

/// Maximum UTF-8 byte length of a logical index target.
///
/// The accepted grammar is ASCII-only, so this is also the maximum character
/// count.
pub const MAX_INDEX_TARGET_LEN: usize = 128;

/// Stable daemon-routing/lifecycle error detail codes and paths.
///
/// #848 establishes these strings as part of the forward-compatible portable
/// error contract consumed by language clients. Transport layers must not
/// replace them with transport-specific status text.
pub const MISSING_INDEX_TARGET_DETAIL_CODE: &str = "missing_index_target";
pub const INVALID_INDEX_TARGET_DETAIL_CODE: &str = "invalid_index_target";
pub const MISSING_INDEX_DEFINITION_DETAIL_CODE: &str = "missing_index_definition";
pub const INDEX_ALREADY_EXISTS_DETAIL_CODE: &str = "index_already_exists";
pub const INDEX_NOT_FOUND_DETAIL_CODE: &str = "index_not_found";
pub const TARGET_PATH: &str = "target";
pub const TARGET_NAME_PATH: &str = "target.name";
pub const CREATE_INDEX_DEFINITION_PATH: &str = "payload.definition";

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

/// Canonical daemon routing/lifecycle failure vocabulary.
///
/// These variants bind the portable error code, stable detail code, and stable
/// path in one executable contract so daemon transports cannot invent
/// transport-local semantics for control-plane failures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IndexControlError {
    MissingTarget,
    InvalidTarget(IndexTargetError),
    MissingIndexDefinition,
    IndexAlreadyExists,
    IndexNotFound,
}

impl IndexControlError {
    /// Portable Search Kernel error code for this control-plane failure.
    pub const fn error_code(&self) -> crate::v2::ErrorCode {
        match self {
            Self::MissingTarget | Self::InvalidTarget(_) | Self::MissingIndexDefinition => {
                crate::v2::ErrorCode::InvalidArgument
            }
            Self::IndexAlreadyExists => crate::v2::ErrorCode::AlreadyExists,
            Self::IndexNotFound => crate::v2::ErrorCode::NotFound,
        }
    }

    /// Stable detail code for language clients.
    pub const fn detail_code(&self) -> &'static str {
        match self {
            Self::MissingTarget => MISSING_INDEX_TARGET_DETAIL_CODE,
            Self::InvalidTarget(_) => INVALID_INDEX_TARGET_DETAIL_CODE,
            Self::MissingIndexDefinition => MISSING_INDEX_DEFINITION_DETAIL_CODE,
            Self::IndexAlreadyExists => INDEX_ALREADY_EXISTS_DETAIL_CODE,
            Self::IndexNotFound => INDEX_NOT_FOUND_DETAIL_CODE,
        }
    }

    /// Canonical semantic path when the failure refers to a request field.
    pub const fn path(&self) -> Option<&'static str> {
        match self {
            Self::MissingTarget => Some(TARGET_PATH),
            Self::InvalidTarget(_) => Some(TARGET_NAME_PATH),
            Self::MissingIndexDefinition => Some(CREATE_INDEX_DEFINITION_PATH),
            Self::IndexAlreadyExists | Self::IndexNotFound => None,
        }
    }
}

impl fmt::Display for IndexControlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingTarget => formatter.write_str("Search Kernel request is missing an index target"),
            Self::InvalidTarget(error) => {
                write!(formatter, "invalid Search Kernel index target: {error}")
            }
            Self::MissingIndexDefinition => {
                formatter.write_str("CreateIndex request is missing an index definition")
            }
            Self::IndexAlreadyExists => formatter.write_str("Search Kernel index already exists"),
            Self::IndexNotFound => formatter.write_str("Search Kernel index was not found"),
        }
    }
}

impl std::error::Error for IndexControlError {}

/// Envelope-level target failure used by multi-index routing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RequestTargetError {
    /// The envelope did not carry an index target.
    Missing,
    /// A target table was present but its logical name was invalid.
    Invalid(IndexTargetError),
}

impl fmt::Display for RequestTargetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => write!(
                formatter,
                "Search Kernel request is missing an index target"
            ),
            Self::Invalid(error) => {
                write!(formatter, "invalid Search Kernel index target: {error}")
            }
        }
    }
}

impl std::error::Error for RequestTargetError {}

impl From<RequestTargetError> for IndexControlError {
    fn from(error: RequestTargetError) -> Self {
        match error {
            RequestTargetError::Missing => Self::MissingTarget,
            RequestTargetError::Invalid(error) => Self::InvalidTarget(error),
        }
    }
}

/// Extracts and validates the canonical target from one request envelope.
///
/// Embedded/per-index dispatchers may intentionally ignore this helper because
/// their target is already bound by the local handle. Multi-index daemon
/// ingress must call it before selecting a dispatcher and must never substitute
/// a default target when this returns `RequestTargetError::Missing`.
pub fn request_index_target<'a>(
    envelope: wire::RequestEnvelope<'a>,
) -> Result<&'a str, RequestTargetError> {
    let target = envelope.target().ok_or(RequestTargetError::Missing)?;
    let name = target
        .name()
        .ok_or(RequestTargetError::Invalid(IndexTargetError::Empty))?;
    validate_index_target(name).map_err(RequestTargetError::Invalid)
}

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
    use super::{
        validate_index_target, IndexControlError, IndexTargetError, CREATE_INDEX_DEFINITION_PATH,
        INDEX_ALREADY_EXISTS_DETAIL_CODE, INDEX_NOT_FOUND_DETAIL_CODE,
        INVALID_INDEX_TARGET_DETAIL_CODE, MAX_INDEX_TARGET_LEN,
        MISSING_INDEX_DEFINITION_DETAIL_CODE, MISSING_INDEX_TARGET_DETAIL_CODE, TARGET_NAME_PATH,
        TARGET_PATH,
    };
    use crate::v2 as api;

    #[test]
    fn stable_daemon_error_contract_is_frozen_from_multi_index_baseline() {
        assert_eq!(MISSING_INDEX_TARGET_DETAIL_CODE, "missing_index_target");
        assert_eq!(INVALID_INDEX_TARGET_DETAIL_CODE, "invalid_index_target");
        assert_eq!(
            MISSING_INDEX_DEFINITION_DETAIL_CODE,
            "missing_index_definition"
        );
        assert_eq!(INDEX_ALREADY_EXISTS_DETAIL_CODE, "index_already_exists");
        assert_eq!(INDEX_NOT_FOUND_DETAIL_CODE, "index_not_found");
        assert_eq!(TARGET_PATH, "target");
        assert_eq!(TARGET_NAME_PATH, "target.name");
        assert_eq!(CREATE_INDEX_DEFINITION_PATH, "payload.definition");
    }

    #[test]
    fn canonical_daemon_control_errors_bind_code_detail_and_path() {
        let cases = [
            (
                IndexControlError::MissingTarget,
                api::ErrorCode::InvalidArgument,
                MISSING_INDEX_TARGET_DETAIL_CODE,
                Some(TARGET_PATH),
            ),
            (
                IndexControlError::InvalidTarget(IndexTargetError::Empty),
                api::ErrorCode::InvalidArgument,
                INVALID_INDEX_TARGET_DETAIL_CODE,
                Some(TARGET_NAME_PATH),
            ),
            (
                IndexControlError::MissingIndexDefinition,
                api::ErrorCode::InvalidArgument,
                MISSING_INDEX_DEFINITION_DETAIL_CODE,
                Some(CREATE_INDEX_DEFINITION_PATH),
            ),
            (
                IndexControlError::IndexAlreadyExists,
                api::ErrorCode::AlreadyExists,
                INDEX_ALREADY_EXISTS_DETAIL_CODE,
                None,
            ),
            (
                IndexControlError::IndexNotFound,
                api::ErrorCode::NotFound,
                INDEX_NOT_FOUND_DETAIL_CODE,
                None,
            ),
        ];

        for (error, code, detail_code, path) in cases {
            assert_eq!(error.error_code(), code);
            assert_eq!(error.detail_code(), detail_code);
            assert_eq!(error.path(), path);
        }
    }

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
