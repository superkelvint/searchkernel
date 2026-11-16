use crate::native::NativeError;
use std::ffi::{CString, NulError};
use std::path::Path;

#[derive(Debug)]
/// An error returned while calling the native adapter.
pub struct Error {
    kind: ErrorKind,
    code: i32,
    detail_code: Option<String>,
    message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    InvalidArgument,
    ResourceExhausted,
    Unsupported,
    Type,
    DeadlineExceeded,
    Internal,
}

impl Error {
    /// Caller-side validation. Native output invariants use `corrupt`.
    pub(crate) fn message(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::InvalidArgument,
            code: 0,
            detail_code: None,
            message: message.into(),
        }
    }

    pub(crate) fn corrupt(message: impl Into<String>) -> Self {
        Self::native(2, Some("native_result_invalid".to_owned()), message)
    }

    pub(crate) fn native(
        code: i32,
        detail_code: Option<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            kind: match code {
                1 => ErrorKind::InvalidArgument,
                3 => ErrorKind::ResourceExhausted,
                4 => ErrorKind::Unsupported,
                6 => ErrorKind::Type,
                5 => ErrorKind::DeadlineExceeded,
                _ => ErrorKind::Internal,
            },
            code,
            detail_code,
            message: message.into(),
        }
    }

    pub(crate) fn invalid_argument_detail(
        detail_code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::native(1, Some(detail_code.into()), message)
    }

    pub(crate) fn unsupported(message: impl Into<String>) -> Self {
        Self::native(4, None, message)
    }

    pub(crate) fn from_nul(error: NulError) -> Self {
        Self::message(format!("string contains an interior NUL: {error}"))
    }

    /// Returns the native Search Kernel error code, or zero for an adapter-side error.
    pub fn code(&self) -> i32 {
        self.code
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Returns the stable machine-readable native detail code, when supplied
    /// by the adapter. Human-readable diagnostics are intentionally separate.
    pub fn detail_code(&self) -> Option<&str> {
        self.detail_code.as_deref()
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub(crate) fn native_error(error: &NativeError, status: i32) -> Error {
    let message_bytes: Vec<u8> = error
        .0
        .iter()
        .take_while(|&&byte| byte != 0)
        .copied()
        .collect();
    let diagnostic = String::from_utf8_lossy(&message_bytes);
    let (detail_code, message) = match diagnostic
        .strip_prefix("SK_DETAIL_V1:")
        .and_then(|value| value.split_once(':'))
    {
        Some((code, message)) => (Some(code.to_owned()), message.to_owned()),
        None => (None, diagnostic.into_owned()),
    };
    let message = if message.is_empty() {
        format!("native operation failed with status {status}")
    } else {
        message
    };
    Error::native(status, detail_code, message)
}

pub(crate) fn check(status: i32, error: &NativeError) -> Result<(), Error> {
    if status == 0 {
        Ok(())
    } else {
        Err(native_error(error, status))
    }
}

pub(crate) fn cstring(value: &str) -> Result<CString, Error> {
    CString::new(value).map_err(Error::from_nul)
}

pub(crate) fn path_cstring(path: &Path) -> Result<CString, Error> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        CString::new(path.as_os_str().as_bytes()).map_err(Error::from_nul)
    }
    #[cfg(not(unix))]
    {
        cstring(&path.to_string_lossy())
    }
}
