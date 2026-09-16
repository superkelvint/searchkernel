use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
/// The stable identifier of a document in an index.
pub struct DocumentId(String);

impl DocumentId {
    /// Borrow the identifier as a string.
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

#[cfg(test)]
mod tests {
    use super::*;

    struct FailingSerialize;

    impl serde::Serialize for FailingSerialize {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            Err(serde::ser::Error::custom(
                "intentional serialization failure",
            ))
        }
    }

    #[test]
    fn document_ids_are_owned_and_borrowable() {
        let source = String::from("id:ns:type::1");
        let id = DocumentId::from(&source);
        assert_eq!(id.as_str(), source);
        assert_eq!(id.as_ref(), "id:ns:type::1");
        assert_eq!(id.to_string(), "id:ns:type::1");
        assert_eq!(DocumentId::from(&id), id);
    }

    #[test]
    fn pending_values_preserve_serialization_failures() {
        assert_eq!(pending(&"value"), Ok(serde_json::json!("value")));
        assert_eq!(pending(&f64::NAN), Ok(serde_json::Value::Null));
        let error = pending(&FailingSerialize).expect_err("serializer failure must be retained");
        assert!(error.contains("intentional serialization failure"));
    }
}
