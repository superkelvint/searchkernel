use crate::error::{Error, Result};
use searchkernel_ir::{UpdateArithmeticOperatorIr, UpdateOperationInput};
use serde::Serialize;

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

/// A sequence of portable field updates for one existing document.
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
