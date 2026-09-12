use std::collections::BTreeMap;
use std::fmt;

pub type Result<T> = std::result::Result<T, PortableError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErrorDetail {
    pub code: String,
    pub path: Option<String>,
    pub message: String,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortableError {
    pub code: ErrorCode,
    pub message: String,
    pub details: Vec<ErrorDetail>,
    pub retryable: bool,
}

impl PortableError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: Vec::new(),
            retryable: false,
        }
    }

    pub fn with_detail(
        mut self,
        code: impl Into<String>,
        path: Option<impl Into<String>>,
        message: impl Into<String>,
    ) -> Self {
        self.details.push(ErrorDetail {
            code: code.into(),
            path: path.map(Into::into),
            message: message.into(),
            metadata: BTreeMap::new(),
        });
        self
    }

    pub fn invalid(
        detail_code: impl Into<String>,
        path: Option<impl Into<String>>,
        message: impl Into<String>,
    ) -> Self {
        let message = message.into();
        Self::new(ErrorCode::InvalidArgument, message.clone()).with_detail(
            detail_code,
            path,
            message,
        )
    }

    pub fn failed_precondition(
        detail_code: impl Into<String>,
        path: Option<impl Into<String>>,
        message: impl Into<String>,
    ) -> Self {
        let message = message.into();
        Self::new(ErrorCode::FailedPrecondition, message.clone()).with_detail(
            detail_code,
            path,
            message,
        )
    }

    pub fn unsupported(
        detail_code: impl Into<String>,
        path: Option<impl Into<String>>,
        message: impl Into<String>,
    ) -> Self {
        let message = message.into();
        Self::new(ErrorCode::Unsupported, message.clone()).with_detail(detail_code, path, message)
    }
}

impl fmt::Display for PortableError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for PortableError {}
