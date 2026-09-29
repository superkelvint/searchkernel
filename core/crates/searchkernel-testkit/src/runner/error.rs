use super::*;

#[derive(Debug)]
pub enum RunnerError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    Case {
        id: String,
        message: String,
    },
}

impl fmt::Display for RunnerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::Json { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::Case { id, message } => write!(formatter, "{id}: {message}"),
        }
    }
}
impl std::error::Error for RunnerError {}
