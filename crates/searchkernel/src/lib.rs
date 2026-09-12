use std::fmt;
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    message: String,
}

impl Error {
    fn message(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

pub struct EngineConfig {
    directory: PathBuf,
}

impl EngineConfig {
    pub fn from_directory(path: impl AsRef<Path>) -> Result<Self> {
        let directory = path.as_ref().to_path_buf();
        if !directory.is_dir() {
            return Err(Error::message(format!(
                "configuration directory does not exist: {}",
                directory.display()
            )));
        }
        Ok(Self { directory })
    }
}

pub struct Engine {
    native: searchkernel_vespa::Engine,
}

impl Engine {
    pub fn open(data_dir: impl AsRef<Path>, config: EngineConfig) -> Result<Self> {
        let native = searchkernel_vespa::Engine::open(data_dir.as_ref(), &config.directory)
            .map_err(|error| Error::message(error.to_string()))?;
        Ok(Self { native })
    }

    pub fn put(&mut self, document: Document) -> Result<()> {
        let (field, value) = document
            .field
            .ok_or_else(|| Error::message("document has no fields"))?;
        self.native
            .put(document.id.as_str(), &field, value.as_i32())
            .map_err(|error| Error::message(error.to_string()))
    }

    pub fn update(&mut self, update: DocumentUpdate) -> Result<()> {
        let (field, value) = update
            .field
            .ok_or_else(|| Error::message("update has no fields"))?;
        self.native
            .update(update.id.as_str(), &field, value.as_i32())
            .map_err(|error| Error::message(error.to_string()))
    }

    pub fn remove(&mut self, id: &DocumentId) -> Result<bool> {
        self.native
            .remove(id.as_str())
            .map_err(|error| Error::message(error.to_string()))
    }

    pub fn search(&self, request: SearchRequest) -> Result<SearchResult> {
        let limit = u32::try_from(request.limit)
            .map_err(|_| Error::message("search limit exceeds native maximum"))?;
        let ids = self
            .native
            .search(&request.query.field, request.query.value.as_i32(), limit)
            .map_err(|error| Error::message(error.to_string()))?;
        let hits = ids
            .into_iter()
            .map(|id| DocumentId::parse(id).map(|id| Hit { id }))
            .collect::<Result<Vec<_>>>()?;
        Ok(SearchResult { hits })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct DocumentId {
    value: String,
}

impl DocumentId {
    pub fn parse(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if value.is_empty() || value.as_bytes().contains(&0) {
            return Err(Error::message(
                "document ID must not be empty or contain NUL",
            ));
        }
        Ok(Self { value })
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    I32(i32),
}

impl Value {
    fn as_i32(&self) -> i32 {
        match self {
            Self::I32(value) => *value,
        }
    }
}

pub struct Document {
    id: DocumentId,
    field: Option<(String, Value)>,
}

impl Document {
    pub fn new(id: DocumentId) -> Self {
        Self { id, field: None }
    }

    pub fn set_field(&mut self, name: impl Into<String>, value: Value) {
        self.field = Some((name.into(), value));
    }
}

pub struct DocumentUpdate {
    id: DocumentId,
    field: Option<(String, Value)>,
}

impl DocumentUpdate {
    pub fn new(id: DocumentId) -> Self {
        Self { id, field: None }
    }

    pub fn assign(&mut self, name: impl Into<String>, value: Value) {
        self.field = Some((name.into(), value));
    }
}

pub struct Query {
    field: String,
    value: Value,
}

impl Query {
    pub fn term(field: impl Into<String>, value: Value) -> Self {
        Self {
            field: field.into(),
            value,
        }
    }
}

pub struct SearchRequest {
    query: Query,
    limit: usize,
}

impl SearchRequest {
    pub fn new(query: Query) -> Self {
        Self { query, limit: 10 }
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }
}

pub struct Hit {
    id: DocumentId,
}

impl Hit {
    pub fn id(&self) -> &DocumentId {
        &self.id
    }
}

pub struct SearchResult {
    hits: Vec<Hit>,
}

impl SearchResult {
    pub fn hits(&self) -> &[Hit] {
        &self.hits
    }
}
