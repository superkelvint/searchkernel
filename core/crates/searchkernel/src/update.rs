use crate::document::{pending, DocumentId, PendingValue};
use crate::error::{Error, Result};
use crate::index::Index;
use searchkernel_ir::{UpdateArithmeticOperatorIr, UpdateOperationInput};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq)]
enum UpdateOperation {
    Assign {
        target: String,
        value: PendingValue,
    },
    Clear {
        target: String,
    },
    Arithmetic {
        target: String,
        operator: searchkernel_ir::UpdateArithmeticOperatorIr,
        operand: PendingValue,
    },
}

/// A fluent advanced document-update request.
///
/// The builder captures caller intent only. Field/path validation, duplicate
/// target policy, arithmetic type checks, and create-if-nonexistent semantics
/// are resolved by the shared `searchkernel-ir` update resolver at execution.
pub struct UpdateBuilder {
    index: Index,
    id: DocumentId,
    operations: Vec<UpdateOperation>,
    create: Option<bool>,
}

impl UpdateBuilder {
    pub(crate) fn new(index: Index, id: DocumentId) -> Self {
        Self {
            index,
            id,
            operations: Vec::new(),
            create: None,
        }
    }

    /// Assign a value to a field or supported update path.
    pub fn assign<T: Serialize>(mut self, target: impl Into<String>, value: T) -> Self {
        self.operations.push(UpdateOperation::Assign {
            target: target.into(),
            value: pending(&value),
        });
        self
    }

    /// Alias for [`UpdateBuilder::assign`].
    pub fn set<T: Serialize>(self, target: impl Into<String>, value: T) -> Self {
        self.assign(target, value)
    }

    /// Clear a field or supported update path.
    pub fn clear(mut self, target: impl Into<String>) -> Self {
        self.operations.push(UpdateOperation::Clear {
            target: target.into(),
        });
        self
    }

    /// Alias for [`UpdateBuilder::clear`].
    pub fn unset(self, target: impl Into<String>) -> Self {
        self.clear(target)
    }

    /// Increment a numeric field by `operand`.
    pub fn increment<T: Serialize>(self, target: impl Into<String>, operand: T) -> Self {
        self.arithmetic(
            target,
            searchkernel_ir::UpdateArithmeticOperatorIr::Increment,
            operand,
        )
    }

    /// Decrement a numeric field by `operand`.
    pub fn decrement<T: Serialize>(self, target: impl Into<String>, operand: T) -> Self {
        self.arithmetic(
            target,
            searchkernel_ir::UpdateArithmeticOperatorIr::Decrement,
            operand,
        )
    }

    /// Multiply a numeric field by `operand`.
    pub fn multiply<T: Serialize>(self, target: impl Into<String>, operand: T) -> Self {
        self.arithmetic(
            target,
            searchkernel_ir::UpdateArithmeticOperatorIr::Multiply,
            operand,
        )
    }

    /// Divide a numeric field by `operand`.
    pub fn divide<T: Serialize>(self, target: impl Into<String>, operand: T) -> Self {
        self.arithmetic(
            target,
            searchkernel_ir::UpdateArithmeticOperatorIr::Divide,
            operand,
        )
    }

    /// Control whether the update creates a missing document.
    pub fn create(mut self, enabled: bool) -> Self {
        self.create = Some(enabled);
        self
    }

    /// Resolve and execute the update through the canonical IR/backend path.
    pub fn execute(self) -> Result<()> {
        let operations = to_resolver_operations(&self.operations)?;
        self.index
            .execute_update(&self.id, &operations, self.create)
    }

    fn arithmetic<T: Serialize>(
        mut self,
        target: impl Into<String>,
        operator: searchkernel_ir::UpdateArithmeticOperatorIr,
        operand: T,
    ) -> Self {
        self.operations.push(UpdateOperation::Arithmetic {
            target: target.into(),
            operator,
            operand: pending(&operand),
        });
        self
    }
}

fn resolver_value(value: &PendingValue, path: String) -> Result<searchkernel_ir::RequestValueIr> {
    let value = crate::filter::pending_value(value).map_err(Error::Serialization)?;
    searchkernel_ir::request_value_from_json(value, path).map_err(Error::from_ir)
}

fn to_resolver_operations(
    operations: &[UpdateOperation],
) -> Result<Vec<searchkernel_ir::UpdateOperationInput>> {
    operations
        .iter()
        .enumerate()
        .map(|(index, operation)| match operation {
            UpdateOperation::Assign { target, value } => {
                Ok(searchkernel_ir::UpdateOperationInput::Assign {
                    target: target.clone(),
                    value: resolver_value(value, format!("operations[{index}].operation.value"))?,
                })
            }
            UpdateOperation::Clear { target } => Ok(searchkernel_ir::UpdateOperationInput::Clear {
                target: target.clone(),
            }),
            UpdateOperation::Arithmetic {
                target,
                operator,
                operand,
            } => Ok(searchkernel_ir::UpdateOperationInput::Arithmetic {
                target: target.clone(),
                operator: *operator,
                operand: resolver_value(operand, format!("operations[{index}].operation.operand"))?,
            }),
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq)]
enum Operation {
    Set {
        field: String,
        value: std::result::Result<serde_json::Value, String>,
    },
    Unset {
        field: String,
    },
    Arithmetic {
        field: String,
        operator: UpdateArithmeticOperatorIr,
        operand: f64,
    },
}

/// A reusable sequence of portable field updates for one document.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Update {
    operations: Vec<Operation>,
}

impl Update {
    pub fn new() -> Self {
        Self::default()
    }

    /// Assign a serialized value to a field.
    pub fn set<T: Serialize>(mut self, field: impl Into<String>, value: T) -> Self {
        self.operations.push(Operation::Set {
            field: field.into(),
            value: serde_json::to_value(value).map_err(|error| error.to_string()),
        });
        self
    }

    /// Clear a field.
    pub fn unset(mut self, field: impl Into<String>) -> Self {
        self.operations.push(Operation::Unset {
            field: field.into(),
        });
        self
    }

    /// Add a finite numeric operand to a field.
    pub fn increment(mut self, field: impl Into<String>, amount: f64) -> Self {
        self.operations.push(Operation::Arithmetic {
            field: field.into(),
            operator: UpdateArithmeticOperatorIr::Increment,
            operand: amount,
        });
        self
    }

    /// Subtract a finite numeric operand from a field.
    pub fn decrement(self, field: impl Into<String>, amount: f64) -> Self {
        self.arithmetic(field, amount, UpdateArithmeticOperatorIr::Decrement)
    }

    /// Multiply a numeric field by a finite operand.
    pub fn multiply(self, field: impl Into<String>, factor: f64) -> Self {
        self.arithmetic(field, factor, UpdateArithmeticOperatorIr::Multiply)
    }

    /// Divide a numeric field by a finite operand.
    pub fn divide(self, field: impl Into<String>, divisor: f64) -> Self {
        self.arithmetic(field, divisor, UpdateArithmeticOperatorIr::Divide)
    }

    fn arithmetic(
        mut self,
        field: impl Into<String>,
        operand: f64,
        operator: UpdateArithmeticOperatorIr,
    ) -> Self {
        self.operations.push(Operation::Arithmetic {
            field: field.into(),
            operator,
            operand,
        });
        self
    }

    pub(crate) fn into_resolver(self) -> Result<Vec<UpdateOperationInput>> {
        self.operations
            .into_iter()
            .map(|operation| match operation {
                Operation::Set { field, value } => {
                    let value = value.map_err(Error::Serialization)?;
                    let value = searchkernel_ir::request_value_from_json(
                        &value,
                        format!("operations[{field}].value"),
                    )
                    .map_err(Error::from_ir)?;
                    Ok(UpdateOperationInput::Assign {
                        target: field,
                        value,
                    })
                }
                Operation::Unset { field } => Ok(UpdateOperationInput::Clear { target: field }),
                Operation::Arithmetic {
                    field,
                    operator,
                    operand,
                } if operand.is_finite() => Ok(UpdateOperationInput::Arithmetic {
                    target: field,
                    operator,
                    operand: searchkernel_ir::RequestValueIr::Float64(operand),
                }),
                Operation::Arithmetic { .. } => Err(Error::search(
                    crate::ErrorCode::InvalidArgument,
                    "update operands must be finite",
                )),
            })
            .collect()
    }
}
