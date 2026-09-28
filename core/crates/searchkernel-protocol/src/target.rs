//! Validation for logical multi-index daemon targets.
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

/// Stable daemon-routing error detail codes and paths.
///
/// These strings are part of the portable error contract consumed by language
/// clients and must not be replaced by transport-specific status text.
pub const TARGETED_REQUEST_REQUIRED_DETAIL_CODE: &str = "targeted_request_required";
pub const MISSING_INDEX_TARGET_DETAIL_CODE: &str = "missing_index_target";
pub const INVALID_INDEX_TARGET_DETAIL_CODE: &str = "invalid_index_target";
pub const INVALID_TARGETED_REQUEST_DETAIL_CODE: &str = "invalid_targeted_request";
pub const MISSING_INDEX_DEFINITION_DETAIL_CODE: &str = "missing_index_definition";
pub const INDEX_ALREADY_EXISTS_DETAIL_CODE: &str = "index_already_exists";
pub const INDEX_NOT_FOUND_DETAIL_CODE: &str = "index_not_found";
pub const PAYLOAD_TYPE_PATH: &str = "payload_type";
pub const TARGET_PATH: &str = "target";
pub const TARGET_NAME_PATH: &str = "target.name";
pub const TARGETED_PAYLOAD_PATH: &str = "payload";
pub const CREATE_INDEX_DEFINITION_PATH: &str = "payload.payload.definition";

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

/// Envelope-level target failure used by multi-index routing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RequestTargetError {
    /// The legacy/embedded envelope did not carry an index target.
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

/// Failure to admit a request at the multi-index daemon routing boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DaemonRequestError {
    /// A direct per-index V2 operation was presented to daemon ingress.
    TargetedRequestRequired { payload_type: u8 },
    /// The targeted wrapper did not carry an index target.
    MissingTarget,
    /// The targeted wrapper carried an invalid logical target.
    InvalidTarget(IndexTargetError),
    /// The wrapper was present but did not contain a concrete inner operation.
    InvalidTargetedRequest,
    /// CreateIndex was selected but its schema definition was absent.
    MissingIndexDefinition,
}

impl fmt::Display for DaemonRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetedRequestRequired { payload_type } => write!(
                formatter,
                "multi-index daemon ingress requires TargetedRequest; got payload type {payload_type}"
            ),
            Self::MissingTarget => write!(formatter, "targeted request is missing an index target"),
            Self::InvalidTarget(error) => {
                write!(formatter, "targeted request has an invalid index target: {error}")
            }
            Self::InvalidTargetedRequest => {
                write!(formatter, "TargetedRequest is missing its inner operation")
            }
            Self::MissingIndexDefinition => {
                write!(formatter, "CreateIndexRequest is missing its index definition")
            }
        }
    }
}

impl std::error::Error for DaemonRequestError {}

impl DaemonRequestError {
    /// Canonical portable error category for daemon-ingress validation.
    pub const fn error_code(&self) -> wire::ErrorCode {
        wire::ErrorCode::InvalidArgument
    }

    /// Stable portable error-detail code for this ingress failure.
    pub const fn detail_code(&self) -> &'static str {
        match self {
            Self::TargetedRequestRequired { .. } => TARGETED_REQUEST_REQUIRED_DETAIL_CODE,
            Self::MissingTarget => MISSING_INDEX_TARGET_DETAIL_CODE,
            Self::InvalidTarget(_) => INVALID_INDEX_TARGET_DETAIL_CODE,
            Self::InvalidTargetedRequest => INVALID_TARGETED_REQUEST_DETAIL_CODE,
            Self::MissingIndexDefinition => MISSING_INDEX_DEFINITION_DETAIL_CODE,
        }
    }

    /// Stable portable error-detail path for this ingress failure.
    pub const fn path(&self) -> &'static str {
        match self {
            Self::TargetedRequestRequired { .. } => PAYLOAD_TYPE_PATH,
            Self::MissingTarget => TARGET_PATH,
            Self::InvalidTarget(_) => TARGET_NAME_PATH,
            Self::InvalidTargetedRequest => TARGETED_PAYLOAD_PATH,
            Self::MissingIndexDefinition => CREATE_INDEX_DEFINITION_PATH,
        }
    }
}

/// Validates the structural routing contract required at multi-index daemon ingress.
///
/// Direct top-level operation variants are intentionally the per-index/embedded
/// ABI. A daemon must call this helper before selecting a dispatcher so a legacy
/// target-less frame or a direct operation with a forged target cannot bypass
/// the TargetedRequest downgrade barrier.
pub fn daemon_request<'a>(
    envelope: wire::RequestEnvelope<'a>,
) -> Result<(&'a str, wire::TargetedRequest<'a>), DaemonRequestError> {
    if envelope.payload_type() != wire::RequestPayload::TargetedRequest {
        return Err(DaemonRequestError::TargetedRequestRequired {
            payload_type: envelope.payload_type().0,
        });
    }

    let request = envelope
        .payload_as_targeted_request()
        .ok_or(DaemonRequestError::InvalidTargetedRequest)?;

    let target = request_index_target(envelope).map_err(|error| match error {
        RequestTargetError::Missing => DaemonRequestError::MissingTarget,
        RequestTargetError::Invalid(error) => DaemonRequestError::InvalidTarget(error),
    })?;

    if request.payload_type() == wire::TargetedRequestPayload::NONE || request.payload().is_none() {
        return Err(DaemonRequestError::InvalidTargetedRequest);
    }

    if request.payload_type() == wire::TargetedRequestPayload::SearchKernel_V2_CreateIndexRequest {
        let create = request
            .payload_as_search_kernel_v2_create_index_request()
            .ok_or(DaemonRequestError::InvalidTargetedRequest)?;
        if create.definition().is_none() {
            return Err(DaemonRequestError::MissingIndexDefinition);
        }
    }

    Ok((target, request))
}

/// Extracts and validates the canonical target from one request envelope.
///
/// Per-index embedded dispatchers may intentionally ignore this helper. A
/// multi-index daemon/router must call it before selecting a dispatcher; it
/// must never substitute a default target when this returns `RequestTargetError::Missing`.
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
        validate_index_target, DaemonRequestError, IndexTargetError,
        INDEX_ALREADY_EXISTS_DETAIL_CODE, INDEX_NOT_FOUND_DETAIL_CODE,
        INVALID_INDEX_TARGET_DETAIL_CODE, INVALID_TARGETED_REQUEST_DETAIL_CODE,
        MAX_INDEX_TARGET_LEN, MISSING_INDEX_TARGET_DETAIL_CODE, PAYLOAD_TYPE_PATH,
        TARGETED_PAYLOAD_PATH, TARGETED_REQUEST_REQUIRED_DETAIL_CODE, TARGET_NAME_PATH,
        TARGET_PATH,
    };

    #[test]
    fn stable_daemon_error_contract_is_frozen() {
        assert_eq!(
            TARGETED_REQUEST_REQUIRED_DETAIL_CODE,
            "targeted_request_required"
        );
        assert_eq!(MISSING_INDEX_TARGET_DETAIL_CODE, "missing_index_target");
        assert_eq!(INVALID_INDEX_TARGET_DETAIL_CODE, "invalid_index_target");
        assert_eq!(
            INVALID_TARGETED_REQUEST_DETAIL_CODE,
            "invalid_targeted_request"
        );
        assert_eq!(INDEX_ALREADY_EXISTS_DETAIL_CODE, "index_already_exists");
        assert_eq!(INDEX_NOT_FOUND_DETAIL_CODE, "index_not_found");
        assert_eq!(PAYLOAD_TYPE_PATH, "payload_type");
        assert_eq!(TARGET_PATH, "target");
        assert_eq!(TARGET_NAME_PATH, "target.name");
        assert_eq!(TARGETED_PAYLOAD_PATH, "payload");
        assert_eq!(CREATE_INDEX_DEFINITION_PATH, "payload.payload.definition");
    }

    #[test]
    fn daemon_ingress_errors_map_to_one_portable_domain() {
        let cases = [
            (
                DaemonRequestError::TargetedRequestRequired { payload_type: 6 },
                TARGETED_REQUEST_REQUIRED_DETAIL_CODE,
                PAYLOAD_TYPE_PATH,
            ),
            (
                DaemonRequestError::MissingTarget,
                MISSING_INDEX_TARGET_DETAIL_CODE,
                TARGET_PATH,
            ),
            (
                DaemonRequestError::InvalidTarget(IndexTargetError::Empty),
                INVALID_INDEX_TARGET_DETAIL_CODE,
                TARGET_NAME_PATH,
            ),
            (
                DaemonRequestError::InvalidTargetedRequest,
                INVALID_TARGETED_REQUEST_DETAIL_CODE,
                TARGETED_PAYLOAD_PATH,
            ),
            (
                DaemonRequestError::MissingIndexDefinition,
                MISSING_INDEX_DEFINITION_DETAIL_CODE,
                CREATE_INDEX_DEFINITION_PATH,
            ),
        ];
        for (error, code, path) in cases {
            assert_eq!(error.error_code(), crate::v2::ErrorCode::InvalidArgument);
            assert_eq!(error.detail_code(), code);
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
