use super::buffer::{native_predicate_expression, packed_predicate, packed_strings};
use super::document::validate_position_precision;
use super::*;

pub(crate) fn attribute_update_create(id: &str) -> Result<NativeAttributeUpdate, Error> {
    let id = cstring(id)?;
    let mut pointer = ptr::null_mut();
    let mut error = NativeError::new();
    check(
        unsafe {
            sys::sk_attribute_update_create(
                id.as_ptr(),
                &mut pointer,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        &error,
    )?;
    Ok(NativeAttributeUpdate(require_ptr(
        pointer,
        "attribute update",
    )?))
}

pub(crate) fn add_attribute_update_value(
    update: *mut sys::sk_attribute_update,
    field: &str,
    value: &Value,
) -> Result<(), Error> {
    let field = cstring(field)?;
    let mut error = NativeError::new();
    let status = match value {
        Value::String(value) => {
            let value = cstring(value)?;
            unsafe {
                sys::sk_attribute_update_add_string(
                    update,
                    field.as_ptr(),
                    value.as_ptr(),
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            }
        }
        Value::Integer(value) => unsafe {
            sys::sk_attribute_update_add_integer(
                update,
                field.as_ptr(),
                *value,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        Value::Float(value) => unsafe {
            sys::sk_attribute_update_add_float(
                update,
                field.as_ptr(),
                *value,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        Value::Boolean(value) => unsafe {
            sys::sk_attribute_update_add_boolean(
                update,
                field.as_ptr(),
                u8::from(*value),
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
    };
    check(status, &error)
}

pub(crate) fn add_update_field(
    update: *mut sys::sk_document_update,
    field: &str,
    mutation: &FieldUpdate,
) -> Result<(), Error> {
    if let FieldUpdate::Assign(FieldValue::Position(PositionValue {
        latitude,
        longitude,
    })) = mutation
    {
        validate_position_precision(*latitude, *longitude)?;
    }
    let field = cstring(field)?;
    let mut error = NativeError::new();
    let status = match mutation {
        FieldUpdate::Clear => unsafe {
            sys::sk_document_update_clear(
                update,
                field.as_ptr(),
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        FieldUpdate::Arithmetic { operator, operand } => unsafe {
            let operator = match operator {
                ArithmeticOperator::Add => sys::SK_ARITHMETIC_ADD,
                ArithmeticOperator::Divide => sys::SK_ARITHMETIC_DIV,
                ArithmeticOperator::Multiply => sys::SK_ARITHMETIC_MUL,
                ArithmeticOperator::Subtract => sys::SK_ARITHMETIC_SUB,
            };
            sys::sk_document_update_arithmetic(
                update,
                field.as_ptr(),
                operator,
                *operand,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        FieldUpdate::Assign(FieldValue::Scalar(ScalarValue::String(value))) => {
            let value = cstring(value)?;
            unsafe {
                sys::sk_document_update_assign_string(
                    update,
                    field.as_ptr(),
                    value.as_ptr(),
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            }
        }
        FieldUpdate::Assign(FieldValue::Scalar(ScalarValue::Integer(value))) => unsafe {
            sys::sk_document_update_assign_integer(
                update,
                field.as_ptr(),
                *value,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        FieldUpdate::Assign(FieldValue::Scalar(ScalarValue::Float(value))) => unsafe {
            sys::sk_document_update_assign_float(
                update,
                field.as_ptr(),
                *value,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        FieldUpdate::Assign(FieldValue::Scalar(ScalarValue::Boolean(value))) => unsafe {
            sys::sk_document_update_assign_boolean(
                update,
                field.as_ptr(),
                u8::from(*value),
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        FieldUpdate::Assign(FieldValue::Array(ArrayValue::Strings(values))) => {
            let (bytes, lengths) = packed_strings(values)?;
            unsafe {
                sys::sk_document_update_assign_string_array(
                    update,
                    field.as_ptr(),
                    bytes.as_ptr(),
                    lengths.as_ptr(),
                    u32::try_from(values.len())
                        .map_err(|_| Error::message("string array exceeds native capacity"))?,
                    u32::try_from(bytes.len())
                        .map_err(|_| Error::message("string array bytes exceed native capacity"))?,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            }
        }
        FieldUpdate::Assign(FieldValue::Array(_)) => {
            return Err(Error::unsupported(
                "only array<string> document updates are supported",
            ))
        }
        FieldUpdate::Assign(FieldValue::Position(PositionValue {
            latitude,
            longitude,
        })) => unsafe {
            sys::sk_document_update_assign_position(
                update,
                field.as_ptr(),
                *latitude,
                *longitude,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        FieldUpdate::Assign(FieldValue::Predicate(value)) => {
            if let Some(expression) = &value.expression {
                let expression = native_predicate_expression(expression)?;
                let expression = cstring(&expression)?;
                return check(
                    unsafe {
                        sys::sk_document_update_assign_predicate_json(
                            update,
                            field.as_ptr(),
                            expression.as_ptr(),
                            error.as_mut_ptr(),
                            error.capacity(),
                        )
                    },
                    &error,
                );
            }
            let (key_bytes, key_lengths, value_bytes, value_lengths) = packed_predicate(value)?;
            unsafe {
                sys::sk_document_update_assign_predicate(
                    update,
                    field.as_ptr(),
                    key_bytes.as_ptr(),
                    key_lengths.as_ptr(),
                    u32::try_from(key_bytes.len())
                        .map_err(|_| Error::message("predicate keys exceed native capacity"))?,
                    value_bytes.as_ptr(),
                    value_lengths.as_ptr(),
                    u32::try_from(value_bytes.len())
                        .map_err(|_| Error::message("predicate values exceed native capacity"))?,
                    u32::try_from(value.features.len())
                        .map_err(|_| Error::message("predicate features exceed native capacity"))?,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            }
        }
        FieldUpdate::Assign(FieldValue::Tensor(TensorValue::Dense { type_spec, values })) => {
            crate::tensor::validate_dense(type_spec, values).map_err(|error| {
                Error::invalid_argument_detail(error.detail_code, error.message)
            })?;
            let type_spec = cstring(type_spec)?;
            let value_count = u32::try_from(values.len())
                .map_err(|_| Error::message("tensor value count exceeds native capacity"))?;
            #[cfg(test)]
            super::DENSE_UPDATE_FFI_CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            unsafe {
                sys::sk_document_update_assign_tensor_dense(
                    update,
                    field.as_ptr(),
                    type_spec.as_ptr(),
                    values.as_ptr(),
                    value_count,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            }
        }
        FieldUpdate::Assign(FieldValue::Tensor(_)) => {
            return Err(Error::unsupported(
                "sparse and mixed tensor document updates are not supported by the current native path",
            ));
        }
    };
    check(status, &error)
}
