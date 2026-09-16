use crate::document::{pending, PendingValue};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq)]
/// A collection of field updates applied to one document.
pub struct Patch {
    pub(crate) operations: Vec<PatchOperation>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PatchOperation {
    Set { field: String, value: PendingValue },
    Unset { field: String },
}

impl Patch {
    /// Start an empty patch.
    pub fn new() -> Self {
        Self {
            operations: Vec::new(),
        }
    }

    /// Set a field to a serialized value.
    pub fn set<T: Serialize>(mut self, field: impl Into<String>, value: T) -> Self {
        self.operations.push(PatchOperation::Set {
            field: field.into(),
            value: pending(&value),
        });
        self
    }

    /// Clear a field from the document.
    ///
    /// For ordinary fields this removes the value. Vespa boolean attributes
    /// have no missing sentinel, so clearing one follows Vespa semantics and
    /// reads back as `false`.
    pub fn unset(mut self, field: impl Into<String>) -> Self {
        self.operations.push(PatchOperation::Unset {
            field: field.into(),
        });
        self
    }
}

impl Default for Patch {
    fn default() -> Self {
        Self::new()
    }
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
    fn patch_builders_preserve_operation_order_and_values() {
        let patch = Patch::new()
            .set("title", "new title")
            .unset("obsolete")
            .set("count", 3_i64);
        assert_eq!(patch.operations.len(), 3);
        assert!(matches!(
            &patch.operations[0],
            PatchOperation::Set { field, value }
                if field == "title" && value == &Ok(serde_json::json!("new title"))
        ));
        assert!(matches!(
            &patch.operations[1],
            PatchOperation::Unset { field } if field == "obsolete"
        ));
        assert!(matches!(
            &patch.operations[2],
            PatchOperation::Set { field, value }
                if field == "count" && value == &Ok(serde_json::json!(3))
        ));
        assert_eq!(Patch::default(), Patch::new());
    }

    #[test]
    fn patch_set_retains_value_serialization_errors_until_encoding() {
        let patch = Patch::new().set("ratio", FailingSerialize);
        assert!(matches!(
            &patch.operations[0],
            PatchOperation::Set { field, value }
                if field == "ratio" && value.is_err()
        ));
    }
}
