use crate::document::{pending, PendingValue};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq)]
pub struct Patch {
    pub(crate) operations: Vec<PatchOperation>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PatchOperation {
    Set { field: String, value: PendingValue },
    Unset { field: String },
}

impl Patch {
    pub fn new() -> Self {
        Self {
            operations: Vec::new(),
        }
    }

    pub fn set<T: Serialize>(mut self, field: impl Into<String>, value: T) -> Self {
        self.operations.push(PatchOperation::Set {
            field: field.into(),
            value: pending(&value),
        });
        self
    }

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
