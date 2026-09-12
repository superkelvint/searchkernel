use searchkernel_api::v1 as api;
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

impl From<ErrorCode> for api::ErrorCode {
    fn from(code: ErrorCode) -> Self {
        match code {
            ErrorCode::InvalidArgument => Self::InvalidArgument,
            ErrorCode::NotFound => Self::NotFound,
            ErrorCode::AlreadyExists => Self::AlreadyExists,
            ErrorCode::FailedPrecondition => Self::FailedPrecondition,
            ErrorCode::Conflict => Self::Conflict,
            ErrorCode::Unsupported => Self::Unsupported,
            ErrorCode::ResourceExhausted => Self::ResourceExhausted,
            ErrorCode::Internal => Self::Internal,
            ErrorCode::Unavailable => Self::Unavailable,
        }
    }
}

impl TryFrom<api::ErrorCode> for ErrorCode {
    type Error = ();

    fn try_from(code: api::ErrorCode) -> std::result::Result<Self, Self::Error> {
        match code {
            api::ErrorCode::InvalidArgument => Ok(Self::InvalidArgument),
            api::ErrorCode::NotFound => Ok(Self::NotFound),
            api::ErrorCode::AlreadyExists => Ok(Self::AlreadyExists),
            api::ErrorCode::FailedPrecondition => Ok(Self::FailedPrecondition),
            api::ErrorCode::Conflict => Ok(Self::Conflict),
            api::ErrorCode::Unsupported => Ok(Self::Unsupported),
            api::ErrorCode::ResourceExhausted => Ok(Self::ResourceExhausted),
            api::ErrorCode::Internal => Ok(Self::Internal),
            api::ErrorCode::Unavailable => Ok(Self::Unavailable),
            api::ErrorCode::Unspecified => Err(()),
        }
    }
}

impl From<PortableError> for api::Error {
    fn from(error: PortableError) -> Self {
        Self {
            code: api::ErrorCode::from(error.code) as i32,
            message: error.message,
            details: error
                .details
                .into_iter()
                .map(|detail| api::ErrorDetail {
                    code: detail.code,
                    path: detail.path,
                    message: detail.message,
                    metadata: detail.metadata.into_iter().collect(),
                })
                .collect(),
            retryable: error.retryable,
        }
    }
}

impl TryFrom<api::Error> for PortableError {
    type Error = ();

    fn try_from(error: api::Error) -> std::result::Result<Self, Self::Error> {
        let code = ErrorCode::try_from(api::ErrorCode::try_from(error.code).map_err(|_| ())?)?;
        Ok(Self {
            code,
            message: error.message,
            details: error
                .details
                .into_iter()
                .map(|detail| ErrorDetail {
                    code: detail.code,
                    path: detail.path,
                    message: detail.message,
                    metadata: detail.metadata.into_iter().collect(),
                })
                .collect(),
            retryable: error.retryable,
        })
    }
}
