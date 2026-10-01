//! FlatBuffers-to-IR resolution for multi-index targets and lifecycle requests.
//!
//! Keep generated envelope and target views inside this API-to-IR boundary.

use crate::{ErrorCode, ErrorDetail, PortableError};
use searchkernel_protocol::{protocol::v2 as wire, v2 as api};
use std::collections::BTreeMap;

pub const MAX_INDEX_TARGET_LEN: usize = 128;

pub const MISSING_INDEX_TARGET_DETAIL_CODE: &str = "missing_index_target";
pub const INVALID_INDEX_TARGET_DETAIL_CODE: &str = "invalid_index_target";
pub const MISSING_INDEX_DEFINITION_DETAIL_CODE: &str = "missing_index_definition";
pub const INDEX_ALREADY_EXISTS_DETAIL_CODE: &str = "index_already_exists";
pub const INDEX_NOT_FOUND_DETAIL_CODE: &str = "index_not_found";
pub const INDEX_IN_USE_DETAIL_CODE: &str = "index_in_use";
pub const STALE_INDEX_HANDLE_DETAIL_CODE: &str = "stale_index_handle";
pub const TARGET_PATH: &str = "target";
pub const TARGET_NAME_PATH: &str = "target.name";
pub const CREATE_INDEX_DEFINITION_PATH: &str = "payload.definition";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleOperation {
    Create,
    Open,
    Delete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleFailureCode {
    InvalidArgument,
    NotFound,
    AlreadyExists,
    FailedPrecondition,
    Conflict,
    Unsupported,
    ResourceExhausted,
    Internal,
    Unavailable,
    DeadlineExceeded,
    CorruptIndex,
    IncompatibleIndex,
    IndexClosed,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LifecycleFailureDetail {
    pub code: String,
    pub path: Option<String>,
    pub message: String,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LifecycleFailure {
    pub code: LifecycleFailureCode,
    pub message: String,
    pub details: Vec<LifecycleFailureDetail>,
    pub retryable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathFailure {
    SymbolicLink,
    NotDirectory,
    InspectFailed,
}

#[derive(Debug)]
pub enum ControlPlaneRequest<'a> {
    Create {
        target: &'a str,
        definition: api::IndexDefinition<'a>,
    },
    Open {
        target: &'a str,
    },
    Delete {
        target: &'a str,
    },
    Route {
        target: &'a str,
    },
}

#[derive(Debug)]
pub enum ControlPlaneResolutionError {
    InvalidEnvelope,
    Semantic(PortableError),
}

/// Resolve target and lifecycle meaning from a mechanically verified envelope.
/// Malformed payload unions remain envelope errors; valid wire objects with
/// missing or invalid domain values produce canonical portable errors.
pub fn resolve_control_plane_request<'a>(
    envelope: wire::RequestEnvelope<'a>,
) -> Result<ControlPlaneRequest<'a>, ControlPlaneResolutionError> {
    let target =
        resolve_index_target(envelope.target()).map_err(ControlPlaneResolutionError::Semantic)?;
    match envelope.payload_type() {
        wire::RequestPayload::SearchKernel_V2_CreateIndexRequest => {
            let payload = envelope
                .payload_as_search_kernel_v2_create_index_request()
                .ok_or(ControlPlaneResolutionError::InvalidEnvelope)?;
            let definition = payload
                .definition()
                .ok_or_else(|| ControlPlaneResolutionError::Semantic(missing_definition_error()))?;
            Ok(ControlPlaneRequest::Create { target, definition })
        }
        wire::RequestPayload::SearchKernel_V2_OpenIndexRequest => {
            envelope
                .payload_as_search_kernel_v2_open_index_request()
                .ok_or(ControlPlaneResolutionError::InvalidEnvelope)?;
            Ok(ControlPlaneRequest::Open { target })
        }
        wire::RequestPayload::SearchKernel_V2_DeleteIndexRequest => {
            envelope
                .payload_as_search_kernel_v2_delete_index_request()
                .ok_or(ControlPlaneResolutionError::InvalidEnvelope)?;
            Ok(ControlPlaneRequest::Delete { target })
        }
        wire::RequestPayload::NONE => Err(ControlPlaneResolutionError::InvalidEnvelope),
        _ => Ok(ControlPlaneRequest::Route { target }),
    }
}

pub fn resolve_index_target<'a>(
    target: Option<api::IndexTarget<'a>>,
) -> Result<&'a str, PortableError> {
    let Some(target) = target else {
        return Err(control_error(
            ErrorCode::InvalidArgument,
            MISSING_INDEX_TARGET_DETAIL_CODE,
            Some(TARGET_PATH),
            "Search Kernel request is missing an index target".to_owned(),
        ));
    };
    let Some(name) = target.name() else {
        return Err(invalid_target_error(TargetValidationError::Empty));
    };
    validate_index_target(name).map_err(invalid_target_error)
}

pub fn validate_index_target(value: &str) -> Result<&str, TargetValidationError> {
    let bytes = value.as_bytes();
    if bytes.is_empty() {
        return Err(TargetValidationError::Empty);
    }
    if bytes.len() > MAX_INDEX_TARGET_LEN {
        return Err(TargetValidationError::TooLong { len: bytes.len() });
    }
    if !is_initial(bytes[0]) {
        return Err(TargetValidationError::InvalidFirstByte { byte: bytes[0] });
    }
    for (offset, byte) in bytes.iter().copied().enumerate().skip(1) {
        if !is_tail(byte) {
            return Err(TargetValidationError::InvalidByte { offset, byte });
        }
    }
    Ok(value)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TargetValidationError {
    Empty,
    TooLong { len: usize },
    InvalidFirstByte { byte: u8 },
    InvalidByte { offset: usize, byte: u8 },
}

impl std::fmt::Display for TargetValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => formatter.write_str("index target must not be empty"),
            Self::TooLong { len } => write!(formatter, "index target is {len} bytes; maximum is {MAX_INDEX_TARGET_LEN}"),
            Self::InvalidFirstByte { byte } => write!(formatter, "index target must start with a lowercase ASCII letter or digit; got byte 0x{byte:02x}"),
            Self::InvalidByte { offset, byte } => write!(formatter, "index target byte {offset} is invalid: 0x{byte:02x}"),
        }
    }
}

impl std::error::Error for TargetValidationError {}

pub fn lifecycle_failure(
    operation: LifecycleOperation,
    failure: LifecycleFailure,
) -> PortableError {
    match failure.code {
        LifecycleFailureCode::AlreadyExists if operation == LifecycleOperation::Create => {
            already_exists_error()
        }
        LifecycleFailureCode::NotFound => not_found_error(),
        LifecycleFailureCode::AlreadyExists | LifecycleFailureCode::Conflict => {
            index_in_use_error()
        }
        _ => {
            let code = match failure.code {
                LifecycleFailureCode::InvalidArgument => ErrorCode::InvalidArgument,
                LifecycleFailureCode::FailedPrecondition
                | LifecycleFailureCode::CorruptIndex
                | LifecycleFailureCode::IncompatibleIndex
                | LifecycleFailureCode::IndexClosed => ErrorCode::FailedPrecondition,
                LifecycleFailureCode::Unsupported => ErrorCode::Unsupported,
                LifecycleFailureCode::ResourceExhausted => ErrorCode::ResourceExhausted,
                LifecycleFailureCode::Unavailable => ErrorCode::Unavailable,
                LifecycleFailureCode::DeadlineExceeded => ErrorCode::DeadlineExceeded,
                LifecycleFailureCode::Internal | LifecycleFailureCode::Other => ErrorCode::Internal,
                LifecycleFailureCode::NotFound
                | LifecycleFailureCode::AlreadyExists
                | LifecycleFailureCode::Conflict => unreachable!("handled above"),
            };
            PortableError {
                code,
                message: failure.message,
                details: failure
                    .details
                    .into_iter()
                    .map(|detail| ErrorDetail {
                        code: detail.code,
                        path: detail.path,
                        message: detail.message,
                        metadata: detail.metadata,
                    })
                    .collect(),
                retryable: failure.retryable,
            }
        }
    }
}

pub fn path_failure(target: &str, failure: PathFailure, reason: Option<&str>) -> PortableError {
    let message = match failure {
        PathFailure::SymbolicLink => {
            format!("index target {target:?} resolves through a symbolic link")
        }
        PathFailure::NotDirectory => format!("index target {target:?} is not a directory"),
        PathFailure::InspectFailed => format!(
            "inspect index target {target:?}: {}",
            reason.unwrap_or("unknown error")
        ),
    };
    if failure == PathFailure::InspectFailed {
        PortableError::new(ErrorCode::Unavailable, message)
    } else {
        control_error(
            ErrorCode::FailedPrecondition,
            "unsafe_index_path",
            Some("target"),
            message,
        )
    }
}

pub fn already_exists_error() -> PortableError {
    control_error(
        ErrorCode::AlreadyExists,
        INDEX_ALREADY_EXISTS_DETAIL_CODE,
        None,
        "Search Kernel index already exists".to_owned(),
    )
}

pub fn not_found_error() -> PortableError {
    control_error(
        ErrorCode::NotFound,
        INDEX_NOT_FOUND_DETAIL_CODE,
        None,
        "Search Kernel index was not found".to_owned(),
    )
}

pub fn index_in_use_error() -> PortableError {
    control_error(
        ErrorCode::Conflict,
        INDEX_IN_USE_DETAIL_CODE,
        None,
        "Search Kernel index is in use by another runtime".to_owned(),
    )
}

pub fn stale_index_handle_error() -> PortableError {
    control_error(
        ErrorCode::FailedPrecondition,
        STALE_INDEX_HANDLE_DETAIL_CODE,
        Some(TARGET_PATH),
        "Search Kernel index handle refers to an obsolete target incarnation".to_owned(),
    )
}

fn missing_definition_error() -> PortableError {
    control_error(
        ErrorCode::InvalidArgument,
        MISSING_INDEX_DEFINITION_DETAIL_CODE,
        Some(CREATE_INDEX_DEFINITION_PATH),
        "CreateIndex request is missing an index definition".to_owned(),
    )
}

fn invalid_target_error(error: TargetValidationError) -> PortableError {
    control_error(
        ErrorCode::InvalidArgument,
        INVALID_INDEX_TARGET_DETAIL_CODE,
        Some(TARGET_NAME_PATH),
        format!("invalid Search Kernel index target: {error}"),
    )
}

fn control_error(
    code: ErrorCode,
    detail: &str,
    path: Option<&str>,
    message: String,
) -> PortableError {
    PortableError::new(code, message.clone()).with_detail(detail, path, message)
}

const fn is_initial(byte: u8) -> bool {
    matches!(byte, b'a'..=b'z' | b'0'..=b'9')
}

const fn is_tail(byte: u8) -> bool {
    is_initial(byte) || matches!(byte, b'-' | b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_target_matrix_and_portable_contract() {
        for value in ["a", "0", "products", "products-v2", "tenant_42"] {
            assert_eq!(validate_index_target(value), Ok(value));
        }
        assert_eq!(
            validate_index_target(&"a".repeat(128)),
            Ok("a".repeat(128).as_str())
        );
        let invalid = [
            ("", "index target must not be empty".to_owned()),
            (
                &"a".repeat(129),
                "index target is 129 bytes; maximum is 128".to_owned(),
            ),
            (
                "_products",
                "index target must start with a lowercase ASCII letter or digit; got byte 0x5f"
                    .to_owned(),
            ),
            (
                "-products",
                "index target must start with a lowercase ASCII letter or digit; got byte 0x2d"
                    .to_owned(),
            ),
            (
                "Products",
                "index target must start with a lowercase ASCII letter or digit; got byte 0x50"
                    .to_owned(),
            ),
            (
                "products.v2",
                "index target byte 8 is invalid: 0x2e".to_owned(),
            ),
            (
                "products/v2",
                "index target byte 8 is invalid: 0x2f".to_owned(),
            ),
            (
                "products\\v2",
                "index target byte 8 is invalid: 0x5c".to_owned(),
            ),
            (
                "../products",
                "index target must start with a lowercase ASCII letter or digit; got byte 0x2e"
                    .to_owned(),
            ),
            (
                "products v2",
                "index target byte 8 is invalid: 0x20".to_owned(),
            ),
            ("café", "index target byte 3 is invalid: 0xc3".to_owned()),
        ];
        for (value, validation_message) in invalid {
            let error = invalid_target_error(validate_index_target(value).unwrap_err());
            assert_eq!(error.code, ErrorCode::InvalidArgument, "{value:?}");
            assert_eq!(
                error.message,
                format!("invalid Search Kernel index target: {validation_message}"),
                "{value:?}"
            );
            assert_eq!(
                error.details[0].code, INVALID_INDEX_TARGET_DETAIL_CODE,
                "{value:?}"
            );
            assert_eq!(
                error.details[0].path.as_deref(),
                Some(TARGET_NAME_PATH),
                "{value:?}"
            );
            assert_eq!(error.details[0].message, error.message, "{value:?}");
        }
    }

    #[test]
    fn stale_remote_handle_has_canonical_failed_precondition_error() {
        let error = stale_index_handle_error();
        assert_eq!(error.code, ErrorCode::FailedPrecondition);
        assert_eq!(error.details[0].code, STALE_INDEX_HANDLE_DETAIL_CODE);
        assert_eq!(error.details[0].path.as_deref(), Some(TARGET_PATH));
        assert!(!error.retryable);
    }

    #[test]
    fn missing_target_and_definition_have_canonical_portable_errors() {
        let missing_target = resolve_index_target(None).unwrap_err();
        assert_eq!(missing_target.code, ErrorCode::InvalidArgument);
        assert_eq!(
            missing_target.message,
            "Search Kernel request is missing an index target"
        );
        assert_eq!(
            missing_target.details[0].code,
            MISSING_INDEX_TARGET_DETAIL_CODE
        );
        assert_eq!(missing_target.details[0].path.as_deref(), Some(TARGET_PATH));
        assert_eq!(missing_target.details[0].message, missing_target.message);

        let missing_definition = missing_definition_error();
        assert_eq!(missing_definition.code, ErrorCode::InvalidArgument);
        assert_eq!(
            missing_definition.message,
            "CreateIndex request is missing an index definition"
        );
        assert_eq!(
            missing_definition.details[0].code,
            MISSING_INDEX_DEFINITION_DETAIL_CODE
        );
        assert_eq!(
            missing_definition.details[0].path.as_deref(),
            Some(CREATE_INDEX_DEFINITION_PATH)
        );
        assert_eq!(
            missing_definition.details[0].message,
            missing_definition.message
        );
    }

    #[test]
    fn direct_target_resolution_returns_the_canonical_error() {
        let mut builder = flatbuffers::FlatBufferBuilder::new();
        let name = builder.create_string("Products");
        let target =
            api::IndexTarget::create(&mut builder, &api::IndexTargetArgs { name: Some(name) });
        builder.finish(target, None);
        let target = flatbuffers::root::<api::IndexTarget<'_>>(builder.finished_data())
            .expect("valid target view");
        let error = resolve_index_target(Some(target)).expect_err("uppercase target is invalid");
        assert_eq!(error.code, ErrorCode::InvalidArgument);
        assert_eq!(error.message, "invalid Search Kernel index target: index target must start with a lowercase ASCII letter or digit; got byte 0x50");
        assert_eq!(error.details[0].code, INVALID_INDEX_TARGET_DETAIL_CODE);
        assert_eq!(error.details[0].path.as_deref(), Some(TARGET_NAME_PATH));
        assert_eq!(error.details[0].message, error.message);
    }

    #[test]
    fn request_resolution_owns_presence_and_lifecycle_meaning() {
        use searchkernel_protocol::protocol::v2 as wire;

        let mut builder = flatbuffers::FlatBufferBuilder::new();
        let open =
            api::OpenIndexRequest::create(&mut builder, &api::OpenIndexRequestArgs::default());
        let open_envelope = wire::RequestEnvelope::create(
            &mut builder,
            &wire::RequestEnvelopeArgs {
                protocol_version: searchkernel_protocol::PROTOCOL_VERSION,
                payload_type: wire::RequestPayload::SearchKernel_V2_OpenIndexRequest,
                payload: Some(open.as_union_value()),
                ..Default::default()
            },
        );
        wire::finish_request_envelope_buffer(&mut builder, open_envelope);
        let missing_target_envelope =
            flatbuffers::root::<wire::RequestEnvelope<'_>>(builder.finished_data())
                .expect("valid target-less open envelope");
        let missing_target = match resolve_control_plane_request(missing_target_envelope) {
            Err(ControlPlaneResolutionError::Semantic(error)) => error,
            result => panic!("expected canonical missing-target error, got {result:?}"),
        };
        assert_eq!(missing_target.code, ErrorCode::InvalidArgument);
        assert_eq!(
            missing_target.details[0].code,
            MISSING_INDEX_TARGET_DETAIL_CODE
        );
        assert_eq!(missing_target.details[0].path.as_deref(), Some(TARGET_PATH));

        let mut builder = flatbuffers::FlatBufferBuilder::new();
        let name = builder.create_string("products");
        let target =
            api::IndexTarget::create(&mut builder, &api::IndexTargetArgs { name: Some(name) });
        let create = api::CreateIndexRequest::create(
            &mut builder,
            &api::CreateIndexRequestArgs { definition: None },
        );
        let create_envelope = wire::RequestEnvelope::create(
            &mut builder,
            &wire::RequestEnvelopeArgs {
                protocol_version: searchkernel_protocol::PROTOCOL_VERSION,
                target: Some(target),
                payload_type: wire::RequestPayload::SearchKernel_V2_CreateIndexRequest,
                payload: Some(create.as_union_value()),
            },
        );
        wire::finish_request_envelope_buffer(&mut builder, create_envelope);
        let missing_definition_envelope =
            flatbuffers::root::<wire::RequestEnvelope<'_>>(builder.finished_data())
                .expect("valid create envelope with missing definition");
        let missing_definition = match resolve_control_plane_request(missing_definition_envelope) {
            Err(ControlPlaneResolutionError::Semantic(error)) => error,
            result => panic!("expected canonical missing-definition error, got {result:?}"),
        };
        assert_eq!(missing_definition.code, ErrorCode::InvalidArgument);
        assert_eq!(
            missing_definition.details[0].code,
            MISSING_INDEX_DEFINITION_DETAIL_CODE
        );
        assert_eq!(
            missing_definition.details[0].path.as_deref(),
            Some(CREATE_INDEX_DEFINITION_PATH)
        );
    }

    #[test]
    fn lifecycle_failure_domains_have_canonical_portable_errors() {
        let create_collision = lifecycle_failure(
            LifecycleOperation::Create,
            failure(LifecycleFailureCode::AlreadyExists),
        );
        assert_eq!(create_collision.code, ErrorCode::AlreadyExists);
        assert_eq!(
            create_collision.details[0].code,
            INDEX_ALREADY_EXISTS_DETAIL_CODE
        );
        assert_eq!(
            create_collision.message,
            "Search Kernel index already exists"
        );
        assert_eq!(create_collision.details[0].path, None);
        assert_eq!(
            create_collision.details[0].message,
            create_collision.message
        );
        let open_owner = lifecycle_failure(
            LifecycleOperation::Open,
            failure(LifecycleFailureCode::AlreadyExists),
        );
        assert_eq!(open_owner.code, ErrorCode::Conflict);
        assert_eq!(open_owner.details[0].code, INDEX_IN_USE_DETAIL_CODE);
        assert_eq!(
            open_owner.message,
            "Search Kernel index is in use by another runtime"
        );
        assert_eq!(open_owner.details[0].path, None);
        assert_eq!(open_owner.details[0].message, open_owner.message);
        for operation in [LifecycleOperation::Open, LifecycleOperation::Delete] {
            let missing = lifecycle_failure(operation, failure(LifecycleFailureCode::NotFound));
            assert_eq!(missing.code, ErrorCode::NotFound);
            assert_eq!(missing.details[0].code, INDEX_NOT_FOUND_DETAIL_CODE);
            assert_eq!(missing.message, "Search Kernel index was not found");
            assert_eq!(missing.details[0].path, None);
            assert_eq!(missing.details[0].message, missing.message);
            let conflict = lifecycle_failure(operation, failure(LifecycleFailureCode::Conflict));
            assert_eq!(conflict.code, ErrorCode::Conflict);
            assert_eq!(conflict.details[0].code, INDEX_IN_USE_DETAIL_CODE);
            assert_eq!(
                conflict.message,
                "Search Kernel index is in use by another runtime"
            );
            assert_eq!(conflict.details[0].path, None);
            assert_eq!(conflict.details[0].message, conflict.message);
        }
    }

    fn failure(code: LifecycleFailureCode) -> LifecycleFailure {
        LifecycleFailure {
            code,
            message: "backend message".into(),
            details: vec![],
            retryable: false,
        }
    }
}
