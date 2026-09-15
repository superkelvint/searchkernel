use searchkernel_ir::PortableError;
use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
/// An error returned by the public Search Kernel API.
pub enum Error {
    AlreadyExists(String),
    NotFound(String),
    CorruptIndex(String),
    IncompatibleIndex(String),
    Portable(PortableError),
    Serialization(String),
    Deserialization(String),
    Infrastructure(String),
}

impl Error {
    pub(crate) fn infrastructure(message: impl Into<String>) -> Self {
        Self::Infrastructure(message.into())
    }

    pub fn portable(&self) -> Option<&PortableError> {
        match self {
            Self::Portable(error) => Some(error),
            _ => None,
        }
    }
}

impl From<PortableError> for Error {
    fn from(error: PortableError) -> Self {
        Self::Portable(error)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyExists(message) => write!(formatter, "index already exists: {message}"),
            Self::NotFound(message) => write!(formatter, "index not found: {message}"),
            Self::CorruptIndex(message) => write!(formatter, "corrupt index: {message}"),
            Self::IncompatibleIndex(message) => write!(formatter, "incompatible index: {message}"),
            Self::Portable(error) => error.fmt(formatter),
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

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Portable(error) => Some(error),
            Self::AlreadyExists(_)
            | Self::NotFound(_)
            | Self::CorruptIndex(_)
            | Self::IncompatibleIndex(_)
            | Self::Serialization(_)
            | Self::Deserialization(_)
            | Self::Infrastructure(_) => None,
        }
    }
}
