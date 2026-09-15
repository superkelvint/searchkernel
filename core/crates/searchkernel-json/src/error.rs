use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub enum JsonCodecError {
    InvalidJson(String),
    InvalidFlatbuffer(String),
    Schema(String),
    Native(String),
}

impl fmt::Display for JsonCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(message) => write!(formatter, "invalid JSON: {message}"),
            Self::InvalidFlatbuffer(message) => {
                write!(formatter, "invalid FlatBuffer: {message}")
            }
            Self::Schema(message) => write!(formatter, "schema error: {message}"),
            Self::Native(message) => write!(formatter, "native JSON bridge error: {message}"),
        }
    }
}

impl std::error::Error for JsonCodecError {}
