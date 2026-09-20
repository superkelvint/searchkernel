//! Documents and document operations.

use super::*;
use crate::error::{PortableError, Result};

#[derive(Clone, Debug, PartialEq)]
pub struct DocumentIr {
    pub(crate) id: DocumentId,
    pub(crate) fields: std::collections::BTreeMap<FieldName, FieldValueIr>,
}

impl DocumentIr {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(
        id: DocumentId,
        fields: std::collections::BTreeMap<FieldName, FieldValueIr>,
    ) -> Self {
        Self { id, fields }
    }

    pub fn id(&self) -> &DocumentId {
        &self.id
    }

    pub fn fields(&self) -> &std::collections::BTreeMap<FieldName, FieldValueIr> {
        &self.fields
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct InsertRequestIr {
    pub(crate) document: DocumentIr,
}

impl InsertRequestIr {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(document: DocumentIr) -> Self {
        Self { document }
    }

    pub fn document(&self) -> &DocumentIr {
        &self.document
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UpsertRequestIr {
    pub(crate) document: DocumentIr,
    /// Document-selection test-and-set condition. `None` is an
    /// unconditional upsert; an empty portable string is normalized to None.
    pub(crate) condition: Option<String>,
}

impl UpsertRequestIr {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(document: DocumentIr) -> Self {
        Self {
            document,
            condition: None,
        }
    }

    pub fn document(&self) -> &DocumentIr {
        &self.document
    }

    pub fn condition(&self) -> Option<&str> {
        self.condition.as_deref()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn with_condition(mut self, condition: impl Into<String>) -> Self {
        self.condition = Some(condition.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PatchOperationIr {
    Set {
        field: FieldName,
        value: FieldValueIr,
    },
    Unset {
        field: FieldName,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct PatchRequestIr {
    pub(crate) id: DocumentId,
    pub(crate) operations: Vec<PatchOperationIr>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UpdateTargetIr(String);

impl UpdateTargetIr {
    pub fn parse(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(PortableError::invalid(
                "invalid_update_target",
                None::<String>,
                "update target must not be empty",
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_simple_field(&self) -> bool {
        FieldName::parse(self.0.clone()).is_ok()
    }
}

impl AsRef<str> for UpdateTargetIr {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateArithmeticOperatorIr {
    Add,
    Increment,
    Subtract,
    Decrement,
    Multiply,
    Divide,
}

#[derive(Clone, Debug, PartialEq)]
pub enum UpdateOperationIr {
    Assign {
        target: UpdateTargetIr,
        value: FieldValueIr,
    },
    Clear {
        target: UpdateTargetIr,
    },
    Arithmetic {
        target: UpdateTargetIr,
        operator: UpdateArithmeticOperatorIr,
        operand: f64,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct UpdateRequestIr {
    pub(crate) id: DocumentId,
    pub(crate) operations: Vec<UpdateOperationIr>,
    pub(crate) condition: Option<String>,
    pub(crate) create: Option<bool>,
}

impl UpdateRequestIr {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(id: DocumentId, operations: Vec<UpdateOperationIr>) -> Self {
        Self {
            id,
            operations,
            condition: None,
            create: None,
        }
    }

    pub fn id(&self) -> &DocumentId {
        &self.id
    }

    pub fn operations(&self) -> &[UpdateOperationIr] {
        &self.operations
    }

    pub fn condition(&self) -> Option<&str> {
        self.condition.as_deref()
    }

    pub fn create(&self) -> Option<bool> {
        self.create
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn with_condition(mut self, condition: impl Into<String>) -> Self {
        self.condition = Some(condition.into());
        self
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn with_create(mut self, create: bool) -> Self {
        self.create = Some(create);
        self
    }
}

impl PatchRequestIr {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(id: DocumentId, operations: Vec<PatchOperationIr>) -> Self {
        Self { id, operations }
    }

    pub fn id(&self) -> &DocumentId {
        &self.id
    }

    pub fn operations(&self) -> &[PatchOperationIr] {
        &self.operations
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeleteRequestIr {
    pub(crate) id: DocumentId,
    /// Document-selection test-and-set condition. `None` is an
    /// unconditional delete; an empty portable string is normalized to None.
    pub(crate) condition: Option<String>,
}

impl DeleteRequestIr {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(id: DocumentId) -> Self {
        Self {
            id,
            condition: None,
        }
    }

    pub fn id(&self) -> &DocumentId {
        &self.id
    }

    pub fn condition(&self) -> Option<&str> {
        self.condition.as_deref()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn with_condition(mut self, condition: impl Into<String>) -> Self {
        self.condition = Some(condition.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectionIr {
    pub(crate) fields: Vec<FieldName>,
}

impl ProjectionIr {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(fields: Vec<FieldName>) -> Self {
        Self { fields }
    }

    pub fn fields(&self) -> &[FieldName] {
        &self.fields
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GetRequestIr {
    pub(crate) id: DocumentId,
    pub(crate) projection: ProjectionIr,
}

impl GetRequestIr {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(id: DocumentId, projection: ProjectionIr) -> Self {
        Self { id, projection }
    }

    pub fn id(&self) -> &DocumentId {
        &self.id
    }

    pub fn projection(&self) -> &ProjectionIr {
        &self.projection
    }
}
