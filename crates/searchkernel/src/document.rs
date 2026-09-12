use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct DocumentId(String);

impl DocumentId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for DocumentId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for DocumentId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&String> for DocumentId {
    fn from(value: &String) -> Self {
        Self(value.clone())
    }
}

impl From<&DocumentId> for DocumentId {
    fn from(value: &DocumentId) -> Self {
        value.clone()
    }
}

impl AsRef<str> for DocumentId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for DocumentId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

pub(crate) fn serialize<T: Serialize>(value: &T) -> Result<serde_json::Value, serde_json::Error> {
    serde_json::to_value(value)
}

pub(crate) type PendingValue = std::result::Result<serde_json::Value, String>;

pub(crate) fn pending<T: Serialize>(value: &T) -> PendingValue {
    serialize(value).map_err(|error| error.to_string())
}
