use super::buffer::{
    native_predicate_expression, packed_predicate, packed_strings, NativeArrayOccurrences,
};
use super::*;

pub(crate) fn attribute_document_create(id: &str) -> Result<NativeAttributeDocument, Error> {
    let id = cstring(id)?;
    let mut pointer = ptr::null_mut();
    let mut error = NativeError::new();
    check(
        unsafe {
            sys::sk_attribute_document_create(
                id.as_ptr(),
                &mut pointer,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        &error,
    )?;
    Ok(NativeAttributeDocument(require_ptr(
        pointer,
        "attribute document",
    )?))
}

pub(crate) fn add_attribute_document_value(
    document: *mut sys::sk_attribute_document,
    field: &str,
    value: &Value,
) -> Result<(), Error> {
    let field = cstring(field)?;
    let mut error = NativeError::new();
    let status = match value {
        Value::String(value) => {
            let value = cstring(value)?;
            unsafe {
                sys::sk_attribute_document_add_string(
                    document,
                    field.as_ptr(),
                    value.as_ptr(),
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            }
        }
        Value::Integer(value) => unsafe {
            sys::sk_attribute_document_add_integer(
                document,
                field.as_ptr(),
                *value,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        Value::Float(value) => unsafe {
            sys::sk_attribute_document_add_float(
                document,
                field.as_ptr(),
                *value,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        Value::Boolean(value) => unsafe {
            sys::sk_attribute_document_add_boolean(
                document,
                field.as_ptr(),
                u8::from(*value),
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
    };
    check(status, &error)
}

struct NativeStructArray(NonNull<sys::sk_struct_array>);

impl Drop for NativeStructArray {
    fn drop(&mut self) {
        unsafe { sys::sk_struct_array_destroy(self.0.as_ptr()) };
    }
}

fn add_struct_array_field(
    document: *mut sys::sk_document,
    field: *const std::os::raw::c_char,
    values: &[BTreeMap<String, ScalarValue>],
    error: &mut NativeError,
) -> Result<(), Error> {
    let mut pointer = ptr::null_mut();
    check(
        unsafe { sys::sk_struct_array_create(&mut pointer, error.as_mut_ptr(), error.capacity()) },
        error,
    )?;
    let array = NativeStructArray(require_ptr(pointer, "native struct array")?);
    for (element_index, element) in values.iter().enumerate() {
        let mut native_index = 0;
        check(
            unsafe {
                sys::sk_struct_array_add_element(
                    array.0.as_ptr(),
                    &mut native_index,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            },
            error,
        )?;
        debug_assert_eq!(native_index as usize, element_index);
        for (name, value) in element {
            let name = cstring(name)?;
            let status = match value {
                ScalarValue::String(value) => {
                    let value = cstring(value)?;
                    unsafe {
                        sys::sk_struct_array_set_string(
                            array.0.as_ptr(),
                            native_index,
                            name.as_ptr(),
                            value.as_ptr(),
                            error.as_mut_ptr(),
                            error.capacity(),
                        )
                    }
                }
                ScalarValue::Integer(value) => unsafe {
                    sys::sk_struct_array_set_integer(
                        array.0.as_ptr(),
                        native_index,
                        name.as_ptr(),
                        *value,
                        error.as_mut_ptr(),
                        error.capacity(),
                    )
                },
                ScalarValue::Float(value) => unsafe {
                    sys::sk_struct_array_set_float(
                        array.0.as_ptr(),
                        native_index,
                        name.as_ptr(),
                        *value,
                        error.as_mut_ptr(),
                        error.capacity(),
                    )
                },
                ScalarValue::Boolean(value) => unsafe {
                    sys::sk_struct_array_set_boolean(
                        array.0.as_ptr(),
                        native_index,
                        name.as_ptr(),
                        u8::from(*value),
                        error.as_mut_ptr(),
                        error.capacity(),
                    )
                },
            };
            check(status, error)?;
        }
    }
    check(
        unsafe {
            sys::sk_document_add_struct_array(
                document,
                field,
                array.0.as_ptr(),
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        error,
    )
}

pub(super) fn validate_position_precision(latitude: f64, longitude: f64) -> Result<(), Error> {
    for (name, value) in [("latitude", latitude), ("longitude", longitude)] {
        let scaled = value * 1_000_000.0;
        if !scaled.is_finite() || scaled.round() != scaled {
            return Err(Error::unsupported(format!(
                "position {name} exceeds Vespa microdegree precision"
            )));
        }
    }
    Ok(())
}

pub(crate) fn add_document_field(
    document: *mut sys::sk_document,
    field: &str,
    value: &FieldValue,
) -> Result<(), Error> {
    if let FieldValue::Position(PositionValue {
        latitude,
        longitude,
    }) = value
    {
        validate_position_precision(*latitude, *longitude)?;
    }
    let field = cstring(field)?;
    let mut error = NativeError::new();
    let status = match value {
        FieldValue::Scalar(ScalarValue::String(value)) => {
            let value = cstring(value)?;
            unsafe {
                sys::sk_document_add_string(
                    document,
                    field.as_ptr(),
                    value.as_ptr(),
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            }
        }
        FieldValue::Scalar(ScalarValue::Integer(value)) => unsafe {
            sys::sk_document_add_integer(
                document,
                field.as_ptr(),
                *value,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        FieldValue::Scalar(ScalarValue::Float(value)) => unsafe {
            sys::sk_document_add_float(
                document,
                field.as_ptr(),
                *value,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        FieldValue::Scalar(ScalarValue::Boolean(value)) => unsafe {
            sys::sk_document_add_boolean(
                document,
                field.as_ptr(),
                u8::from(*value),
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        FieldValue::Array(ArrayValue::Strings(values)) => {
            let (bytes, lengths) = packed_strings(values)?;
            unsafe {
                sys::sk_document_add_string_array(
                    document,
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
        FieldValue::Array(ArrayValue::Structs(values)) => {
            add_struct_array_field(document, field.as_ptr(), values, &mut error)?;
            return Ok(());
        }
        FieldValue::Array(_) => {
            return Err(Error::unsupported(
                "only array<string> document values are supported",
            ))
        }
        FieldValue::Position(PositionValue {
            latitude,
            longitude,
        }) => unsafe {
            sys::sk_document_add_position(
                document,
                field.as_ptr(),
                *latitude,
                *longitude,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        FieldValue::Predicate(value) => {
            if let Some(expression) = &value.expression {
                let expression = native_predicate_expression(expression)?;
                let expression = cstring(&expression)?;
                return check(
                    unsafe {
                        sys::sk_document_add_predicate_json(
                            document,
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
                sys::sk_document_add_predicate(
                    document,
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
        FieldValue::Tensor(TensorValue::Dense { type_spec, values }) => {
            let type_spec = cstring(type_spec)?;
            let value_count = u32::try_from(values.len())
                .map_err(|_| Error::message("tensor value count exceeds native capacity"))?;
            unsafe {
                sys::sk_document_add_tensor_dense(
                    document,
                    field.as_ptr(),
                    type_spec.as_ptr(),
                    values.as_ptr(),
                    value_count,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            }
        }
        FieldValue::Tensor(_) => {
            return Err(Error::unsupported(
                "sparse and mixed tensor document values are not supported by the current native path",
            ));
        }
    };
    check(status, &error)
}

pub(crate) fn add_text_annotation<T>(
    handle: *mut T,
    field: &str,
    analysis: &DocumentAnalysis,
    update: bool,
) -> Result<(), Error> {
    let occurrences = text_occurrences(analysis)?;
    let field = cstring(field)?;
    let mut error = NativeError::new();
    let status = unsafe {
        if update {
            sys::sk_document_update_annotate_text(
                handle.cast(),
                field.as_ptr(),
                occurrences.term_bytes.as_ptr(),
                length_u32(occurrences.term_bytes.len(), "text term bytes")?,
                occurrences.term_lengths.as_ptr(),
                occurrences.byte_starts.as_ptr(),
                occurrences.byte_lengths.as_ptr(),
                length_u32(occurrences.term_lengths.len(), "text occurrence")?,
                error.as_mut_ptr(),
                error.capacity(),
            )
        } else {
            sys::sk_document_annotate_text(
                handle.cast(),
                field.as_ptr(),
                occurrences.term_bytes.as_ptr(),
                length_u32(occurrences.term_bytes.len(), "text term bytes")?,
                occurrences.term_lengths.as_ptr(),
                occurrences.byte_starts.as_ptr(),
                occurrences.byte_lengths.as_ptr(),
                length_u32(occurrences.term_lengths.len(), "text occurrence")?,
                error.as_mut_ptr(),
                error.capacity(),
            )
        }
    };
    check(status, &error)
}

pub(crate) fn add_text_array_annotation<T>(
    handle: *mut T,
    field: &str,
    analyses: &[crate::linguistics::DocumentAnalysis],
    update: bool,
) -> Result<(), Error> {
    let mut occurrences = NativeArrayOccurrences::new();
    for analysis in analyses {
        occurrences.push(analysis)?;
    }
    let field = cstring(field)?;
    let mut error = NativeError::new();
    let status = unsafe {
        if update {
            sys::sk_document_update_annotate_string_array(
                handle.cast(),
                field.as_ptr(),
                occurrences.term_bytes.as_ptr(),
                length_u32(occurrences.term_bytes.len(), "array term bytes")?,
                occurrences.term_lengths.as_ptr(),
                occurrences.byte_starts.as_ptr(),
                occurrences.byte_lengths.as_ptr(),
                occurrences.element_offsets.as_ptr(),
                length_u32(occurrences.element_offsets.len() - 1, "array element")?,
                length_u32(occurrences.term_lengths.len(), "array occurrence")?,
                error.as_mut_ptr(),
                error.capacity(),
            )
        } else {
            sys::sk_document_annotate_string_array(
                handle.cast(),
                field.as_ptr(),
                occurrences.term_bytes.as_ptr(),
                length_u32(occurrences.term_bytes.len(), "array term bytes")?,
                occurrences.term_lengths.as_ptr(),
                occurrences.byte_starts.as_ptr(),
                occurrences.byte_lengths.as_ptr(),
                occurrences.element_offsets.as_ptr(),
                length_u32(occurrences.element_offsets.len() - 1, "array element")?,
                length_u32(occurrences.term_lengths.len(), "array occurrence")?,
                error.as_mut_ptr(),
                error.capacity(),
            )
        }
    };
    check(status, &error)
}
