use std::collections::BTreeMap;
use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorCode {
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErrorDetail {
    pub code: String,
    pub path: Option<String>,
    pub message: String,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug)]
#[non_exhaustive]
/// An error returned by the public Search Kernel API.
pub enum Error {
    Search {
        code: ErrorCode,
        message: String,
        retryable: bool,
        details: Vec<ErrorDetail>,
    },
    Serialization(String),
    Deserialization(String),
    Infrastructure(String),
}

impl Error {
    pub fn search(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::Search {
            code,
            message: message.into(),
            retryable: false,
            details: Vec::new(),
        }
    }

    pub fn with_detail(
        self,
        code: impl Into<String>,
        path: Option<impl Into<String>>,
        message: impl Into<String>,
    ) -> Self {
        match self {
            Self::Search {
                code: error_code,
                message: error_message,
                retryable,
                mut details,
            } => {
                details.push(ErrorDetail {
                    code: code.into(),
                    path: path.map(Into::into),
                    message: message.into(),
                    metadata: BTreeMap::new(),
                });
                Self::Search {
                    code: error_code,
                    message: error_message,
                    retryable,
                    details,
                }
            }
            other => other,
        }
    }

    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Search { code, .. } => *code,
            Self::Serialization(_) => ErrorCode::InvalidArgument,
            Self::Deserialization(_) => ErrorCode::Internal,
            Self::Infrastructure(_) => ErrorCode::Unavailable,
        }
    }

    pub fn retryable(&self) -> bool {
        matches!(
            self,
            Self::Search {
                retryable: true,
                ..
            }
        )
    }

    pub(crate) fn index_closed() -> Self {
        Self::search(ErrorCode::IndexClosed, "index runtime is closed")
    }

    pub fn details(&self) -> &[ErrorDetail] {
        match self {
            Self::Search { details, .. } => details,
            Self::Serialization(_) | Self::Deserialization(_) | Self::Infrastructure(_) => &[],
        }
    }

    pub(crate) fn infrastructure(message: impl Into<String>) -> Self {
        Self::Infrastructure(message.into())
    }

    #[cfg(feature = "native")]
    pub(crate) fn already_exists(message: impl Into<String>) -> Self {
        Self::search(
            ErrorCode::AlreadyExists,
            format!("index already exists: {}", message.into()),
        )
    }

    #[cfg(feature = "native")]
    pub(crate) fn not_found(message: impl Into<String>) -> Self {
        Self::search(
            ErrorCode::NotFound,
            format!("index not found: {}", message.into()),
        )
    }

    #[cfg(feature = "native")]
    pub(crate) fn corrupt_index(message: impl Into<String>) -> Self {
        Self::search(
            ErrorCode::CorruptIndex,
            format!("corrupt index: {}", message.into()),
        )
    }

    #[cfg(feature = "native")]
    pub(crate) fn incompatible_index(message: impl Into<String>) -> Self {
        Self::search(
            ErrorCode::IncompatibleIndex,
            format!("incompatible index: {}", message.into()),
        )
    }

    pub(crate) fn from_ir(error: searchkernel_ir::PortableError) -> Self {
        let code = match error.code {
            searchkernel_ir::ErrorCode::InvalidArgument => ErrorCode::InvalidArgument,
            searchkernel_ir::ErrorCode::NotFound => ErrorCode::NotFound,
            searchkernel_ir::ErrorCode::AlreadyExists => ErrorCode::AlreadyExists,
            searchkernel_ir::ErrorCode::FailedPrecondition => ErrorCode::FailedPrecondition,
            searchkernel_ir::ErrorCode::Conflict => ErrorCode::Conflict,
            searchkernel_ir::ErrorCode::Unsupported => ErrorCode::Unsupported,
            searchkernel_ir::ErrorCode::ResourceExhausted => ErrorCode::ResourceExhausted,
            searchkernel_ir::ErrorCode::Internal => ErrorCode::Internal,
            searchkernel_ir::ErrorCode::Unavailable => ErrorCode::Unavailable,
            searchkernel_ir::ErrorCode::DeadlineExceeded => ErrorCode::DeadlineExceeded,
        };
        Self::Search {
            code,
            message: error.message,
            retryable: error.retryable,
            details: error
                .details
                .into_iter()
                .map(|detail| ErrorDetail {
                    code: detail.code,
                    path: detail.path,
                    message: detail.message,
                    metadata: detail.metadata,
                })
                .collect(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Search { message, .. } => formatter.write_str(message),
            Self::Serialization(error) => {
                write!(formatter, "document serialization failed: {error}")
            }
            Self::Deserialization(error) => {
                write!(formatter, "document deserialization failed: {error}")
            }
            Self::Infrastructure(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_errors_preserve_details_retryability_and_display() {
        let error = Error::Search {
            code: ErrorCode::Unavailable,
            message: "backend unavailable".into(),
            retryable: true,
            details: vec![ErrorDetail {
                code: "field_error".into(),
                path: Some("search.query".into()),
                message: "bad query".into(),
                metadata: BTreeMap::from([(String::from("reason"), String::from("syntax"))]),
            }],
        };
        assert_eq!(error.code(), ErrorCode::Unavailable);
        assert!(error.retryable());
        assert_eq!(error.details()[0].path.as_deref(), Some("search.query"));
        assert_eq!(error.to_string(), "backend unavailable");

        let detailed = Error::search(ErrorCode::InvalidArgument, "invalid").with_detail(
            "bad_field",
            Some("document.title"),
            "wrong type",
        );
        assert_eq!(detailed.details()[0].code, "bad_field");
        assert_eq!(detailed.details()[0].message, "wrong type");
    }

    #[test]
    fn from_ir_maps_every_portable_error_code_and_metadata() {
        let codes = [
            (
                searchkernel_ir::ErrorCode::InvalidArgument,
                ErrorCode::InvalidArgument,
            ),
            (searchkernel_ir::ErrorCode::NotFound, ErrorCode::NotFound),
            (
                searchkernel_ir::ErrorCode::AlreadyExists,
                ErrorCode::AlreadyExists,
            ),
            (
                searchkernel_ir::ErrorCode::FailedPrecondition,
                ErrorCode::FailedPrecondition,
            ),
            (searchkernel_ir::ErrorCode::Conflict, ErrorCode::Conflict),
            (
                searchkernel_ir::ErrorCode::Unsupported,
                ErrorCode::Unsupported,
            ),
            (
                searchkernel_ir::ErrorCode::ResourceExhausted,
                ErrorCode::ResourceExhausted,
            ),
            (searchkernel_ir::ErrorCode::Internal, ErrorCode::Internal),
            (
                searchkernel_ir::ErrorCode::Unavailable,
                ErrorCode::Unavailable,
            ),
        ];
        for (source, expected) in codes {
            let mut source = searchkernel_ir::PortableError::new(source, "failure").with_detail(
                "detail",
                Some("path"),
                "explanation",
            );
            source.retryable = true;
            let converted = Error::from_ir(source);
            assert_eq!(converted.code(), expected);
            assert!(converted.retryable());
            assert_eq!(converted.details()[0].path.as_deref(), Some("path"));
        }

        assert_eq!(
            Error::Serialization("bad".into()).code(),
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            Error::Deserialization("bad".into()).code(),
            ErrorCode::Internal
        );
        assert_eq!(
            Error::Infrastructure("down".into()).code(),
            ErrorCode::Unavailable
        );
    }
}
