use super::*;
use crate::api_to_ir::resolve::{resolve_position_value, resolve_predicate_value};
use crate::model::*;
use searchkernel_protocol::wire::v1 as api;
use std::collections::BTreeMap;

/// Convert the JSON value produced by the Rust facade into the same owned
/// request-value domain used by portable request resolution.
#[doc(hidden)]
pub fn request_value_from_json(value: &serde_json::Value, path: String) -> Result<RequestValueIr> {
    match value {
        serde_json::Value::Null => Ok(RequestValueIr::Null),
        serde_json::Value::Bool(value) => Ok(RequestValueIr::Bool(*value)),
        serde_json::Value::Number(value) if value.as_i64().is_some() => {
            Ok(RequestValueIr::Int64(value.as_i64().unwrap_or_default()))
        }
        serde_json::Value::Number(value) if value.as_u64().is_some() => {
            Ok(RequestValueIr::UInt64(value.as_u64().unwrap_or_default()))
        }
        serde_json::Value::Number(value) => value
            .as_f64()
            .filter(|value| value.is_finite())
            .map(RequestValueIr::Float64)
            .ok_or_else(|| invalid("non_finite_number", Some(path), "number must be finite")),
        serde_json::Value::String(value) => Ok(RequestValueIr::String(value.clone())),
        serde_json::Value::Array(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| request_value_from_json(value, format!("{path}.values[{index}]")))
            .collect::<Result<Vec<_>>>()
            .map(RequestValueIr::List),
        serde_json::Value::Object(values) => values
            .get("$searchkernel_tensor")
            .and_then(|payload| payload.as_object())
            .and_then(|payload| {
                let type_spec = payload.get("type_spec")?.as_str()?.to_owned();
                let dense = payload
                    .get("dense")?
                    .as_array()?
                    .iter()
                    .map(serde_json::Value::as_f64)
                    .collect::<Option<Vec<_>>>()?;
                Some(RequestValueIr::Tensor(RequestTensorIr {
                    type_spec,
                    payload: RequestTensorPayloadIr::Dense(dense),
                }))
            })
            .ok_or_else(|| {
                invalid(
                    "invalid_tensor_value",
                    Some(path.clone()),
                    "tensor value requires type_spec and dense values",
                )
            })
            .or_else(|_| {
                values
                    .iter()
                    .map(|(name, value)| {
                        request_value_from_json(value, format!("{path}.fields.{name}"))
                            .map(|value| (name.clone(), value))
                    })
                    .collect::<Result<BTreeMap<_, _>>>()
                    .map(RequestValueIr::Object)
            }),
    }
}

/// Convert a Rust-facade textual-filter binding into the value domain consumed
/// by the canonical filter binder. This is representation conversion only;
/// field lookup and filter-language validation remain in `filter_expression`.
#[doc(hidden)]
pub fn filter_binding_value_from_json(value: &serde_json::Value, path: String) -> Result<Value> {
    match value {
        serde_json::Value::Null => Ok(Value::Null),
        serde_json::Value::Bool(value) => Ok(Value::Bool(*value)),
        serde_json::Value::Number(value) if value.as_i64().is_some() => {
            Ok(Value::Int64(value.as_i64().unwrap_or_default()))
        }
        serde_json::Value::Number(value) if value.as_u64().is_some() => {
            Ok(Value::UInt64(value.as_u64().unwrap_or_default()))
        }
        serde_json::Value::Number(value) => value
            .as_f64()
            .filter(|value| value.is_finite())
            .map(Value::Float64)
            .ok_or_else(|| invalid("non_finite_number", Some(path), "number must be finite")),
        serde_json::Value::String(value) => Ok(Value::String(value.clone())),
        serde_json::Value::Array(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                filter_binding_value_from_json(value, format!("{path}.values[{index}]"))
            })
            .collect::<Result<Vec<_>>>()
            .map(Value::List),
        serde_json::Value::Object(values) => values
            .iter()
            .map(|(name, value)| {
                filter_binding_value_from_json(value, format!("{path}.fields.{name}"))
                    .map(|value| (name.clone(), value))
            })
            .collect::<Result<BTreeMap<_, _>>>()
            .map(Value::Object),
    }
}

impl IndexContext {
    pub(crate) fn value_to_ir_at(
        &self,
        field: &FieldIr,
        value: api::Value<'_>,
        path: String,
    ) -> Result<FieldValueIr> {
        if matches!(
            field.data_type(),
            DataTypeIr::Position | DataTypeIr::Predicate
        ) {
            let request = RequestValueIr::from_api(value, path.clone())?;
            return match field.data_type() {
                DataTypeIr::Position => resolve_position_value(request, path),
                DataTypeIr::Predicate => resolve_predicate_value(request, path),
                _ => unreachable!("special value type was checked above"),
            };
        }
        if matches!(field.data_type(), DataTypeIr::Tensor { .. }) {
            if value.kind_type() != api::ValueKind::TensorValue {
                return Err(invalid(
                    "type_mismatch",
                    Some(path),
                    "tensor fields require tensor values",
                ));
            }
            let tensor = value.kind_as_tensor_value().ok_or_else(|| {
                invalid(
                    "missing_value",
                    Some(path.clone()),
                    "tensor value payload is missing",
                )
            })?;
            return request_tensor_to_ir(tensor, path).map(FieldValueIr::Tensor);
        }
        let request = RequestValueIr::from_api(value, path.clone())?;
        super::resolve::resolve_field_value(field, request, path, self.definition.structs())
    }
}

impl RequestValueIr {
    pub(crate) fn from_api(value: api::Value<'_>, path: String) -> Result<Self> {
        match value.kind_type() {
            api::ValueKind::NullValueValue => Ok(Self::Null),
            api::ValueKind::BoolValue => value
                .kind_as_bool_value()
                .map(|value| Self::Bool(value.value()))
                .ok_or_else(|| invalid("missing_value", Some(path), "boolean value is missing")),
            api::ValueKind::Int64Value => value
                .kind_as_int_64_value()
                .map(|value| Self::Int64(value.value()))
                .ok_or_else(|| invalid("missing_value", Some(path), "integer value is missing")),
            api::ValueKind::UInt64Value => value
                .kind_as_uint_64_value()
                .map(|value| Self::UInt64(value.value()))
                .ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(path),
                        "unsigned integer value is missing",
                    )
                }),
            api::ValueKind::Float64Value => value
                .kind_as_float_64_value()
                .ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(path.clone()),
                        "float value is missing",
                    )
                })
                .and_then(|value| {
                    value
                        .value()
                        .is_finite()
                        .then_some(Self::Float64(value.value()))
                        .ok_or_else(|| {
                            invalid(
                                "non_finite_number",
                                Some(path),
                                "Float64Value must be finite",
                            )
                        })
                }),
            api::ValueKind::NonFiniteFloatValue => value
                .kind_as_non_finite_float_value()
                .ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(path.clone()),
                        "non-finite float value is missing",
                    )
                })
                .and_then(|value| match value.kind() {
                    api::NonFiniteFloatKind::NaN => {
                        Ok(Self::NonFiniteFloat(RequestNonFiniteFloatIr::NaN))
                    }
                    api::NonFiniteFloatKind::PositiveInfinity => Ok(Self::NonFiniteFloat(
                        RequestNonFiniteFloatIr::PositiveInfinity,
                    )),
                    api::NonFiniteFloatKind::NegativeInfinity => Ok(Self::NonFiniteFloat(
                        RequestNonFiniteFloatIr::NegativeInfinity,
                    )),
                    _ => Err(invalid(
                        "invalid_value",
                        Some(path),
                        "non-finite float kind is required",
                    )),
                }),
            api::ValueKind::StringValue => value
                .kind_as_string_value()
                .and_then(|value| value.value())
                .map(|value| Self::String(value.to_owned()))
                .ok_or_else(|| invalid("missing_value", Some(path), "string value is missing")),
            api::ValueKind::BytesValue => value
                .kind_as_bytes_value()
                .and_then(|value| value.value())
                .map(|value| Self::Bytes(value.iter().collect()))
                .ok_or_else(|| invalid("missing_value", Some(path), "bytes value is missing")),
            api::ValueKind::ValueList => {
                let values = value
                    .kind_as_value_list()
                    .ok_or_else(|| {
                        invalid("missing_value", Some(path.clone()), "value list is missing")
                    })?
                    .values()
                    .map(|values| {
                        values
                            .iter()
                            .enumerate()
                            .map(|(index, value)| {
                                Self::from_api(value, format!("{path}.values[{index}]"))
                            })
                            .collect()
                    })
                    .transpose()?
                    .unwrap_or_default();
                Ok(Self::List(values))
            }
            api::ValueKind::ValueObject => {
                let values = value
                    .kind_as_value_object()
                    .ok_or_else(|| {
                        invalid(
                            "missing_value",
                            Some(path.clone()),
                            "value object is missing",
                        )
                    })?
                    .fields()
                    .map(|values| {
                        values
                            .iter()
                            .enumerate()
                            .map(|(index, entry)| {
                                let key = entry.key().to_owned();
                                let value = entry.value().ok_or_else(|| {
                                    invalid(
                                        "missing_value",
                                        Some(format!("{path}.fields[{index}].value")),
                                        "object value is required",
                                    )
                                })?;
                                Ok((
                                    key,
                                    Self::from_api(value, format!("{path}.fields[{index}].value"))?,
                                ))
                            })
                            .collect::<Result<Vec<_>>>()
                    })
                    .transpose()?
                    .unwrap_or_default();
                let mut object = BTreeMap::new();
                for (key, value) in values {
                    if object.insert(key.clone(), value).is_some() {
                        return Err(invalid(
                            "duplicate_property",
                            Some(path),
                            format!("object field `{key}` occurs more than once"),
                        ));
                    }
                }
                Ok(Self::Object(object))
            }
            api::ValueKind::TensorValue => request_tensor_to_ir(
                value.kind_as_tensor_value().ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(path.clone()),
                        "tensor value is missing",
                    )
                })?,
                path,
            )
            .map(Self::Tensor),
            _ => Err(invalid(
                "missing_value",
                Some(path),
                "value must select a kind",
            )),
        }
    }
}

pub(crate) fn tensor_address_to_ir(
    values: Option<
        flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::TensorAddressEntry<'_>>>,
    >,
    path: &str,
) -> Result<BTreeMap<String, String>> {
    let mut address = BTreeMap::new();
    if let Some(values) = values {
        for (index, value) in values.iter().enumerate() {
            let dimension = value.dimension().to_owned();
            let label = value
                .label()
                .ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(format!("{path}[{index}].label")),
                        "tensor address label is required",
                    )
                })?
                .to_owned();
            if address.insert(dimension.clone(), label).is_some() {
                return Err(invalid(
                    "duplicate_tensor_dimension",
                    Some(format!("{path}[{index}].dimension")),
                    format!("tensor dimension `{dimension}` occurs more than once"),
                ));
            }
        }
    }
    Ok(address)
}

pub(crate) fn request_tensor_to_ir(
    value: api::TensorValue<'_>,
    path: String,
) -> Result<RequestTensorIr> {
    let type_spec = value
        .type_spec()
        .ok_or_else(|| {
            invalid(
                "missing_value",
                Some(format!("{path}.type_spec")),
                "tensor type is required",
            )
        })?
        .to_owned();
    let payload = match value.payload_type() {
        api::TensorPayloadKind::DenseTensorValues => RequestTensorPayloadIr::Dense(
            value
                .payload_as_dense_tensor_values()
                .ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(format!("{path}.payload")),
                        "dense tensor payload is missing",
                    )
                })?
                .values()
                .map(|values| values.iter().collect())
                .unwrap_or_default(),
        ),
        api::TensorPayloadKind::SparseTensorCells => RequestTensorPayloadIr::Sparse(
            value
                .payload_as_sparse_tensor_cells()
                .ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(format!("{path}.payload")),
                        "sparse tensor payload is missing",
                    )
                })?
                .cells()
                .map(|values| {
                    values
                        .iter()
                        .enumerate()
                        .map(|(index, value)| {
                            Ok(RequestTensorCellIr {
                                address: tensor_address_to_ir(
                                    value.address(),
                                    &format!("{path}.payload.cells[{index}].address"),
                                )?,
                                value: value.value(),
                            })
                        })
                        .collect()
                })
                .transpose()?
                .unwrap_or_default(),
        ),
        api::TensorPayloadKind::MixedTensorBlocks => RequestTensorPayloadIr::Mixed(
            value
                .payload_as_mixed_tensor_blocks()
                .ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(format!("{path}.payload")),
                        "mixed tensor payload is missing",
                    )
                })?
                .blocks()
                .map(|values| {
                    values
                        .iter()
                        .enumerate()
                        .map(|(index, value)| {
                            Ok(RequestTensorBlockIr {
                                address: tensor_address_to_ir(
                                    value.address(),
                                    &format!("{path}.payload.blocks[{index}].address"),
                                )?,
                                values: value
                                    .values()
                                    .map(|values| values.iter().collect())
                                    .unwrap_or_default(),
                            })
                        })
                        .collect()
                })
                .transpose()?
                .unwrap_or_default(),
        ),
        _ => {
            return Err(invalid(
                "missing_value",
                Some(format!("{path}.payload")),
                "tensor payload is required",
            ))
        }
    };
    let finite = match &payload {
        RequestTensorPayloadIr::Dense(values) => values.iter().all(|value| value.is_finite()),
        RequestTensorPayloadIr::Sparse(cells) => cells.iter().all(|cell| cell.value.is_finite()),
        RequestTensorPayloadIr::Mixed(blocks) => blocks
            .iter()
            .all(|block| block.values.iter().all(|value| value.is_finite())),
    };
    if !finite {
        return Err(invalid(
            "non_finite_tensor_cell",
            Some(format!("{path}.payload")),
            "tensor cell values must be finite",
        ));
    }
    Ok(RequestTensorIr { type_spec, payload })
}
pub(crate) fn request_values_to_map(
    values: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::ValueEntry<'_>>>>,
    path: &str,
) -> Result<BTreeMap<String, RequestValueIr>> {
    let mut output = BTreeMap::new();
    if let Some(values) = values {
        for (index, entry) in values.iter().enumerate() {
            let key = entry.key().to_owned();
            let value = entry.value().ok_or_else(|| {
                invalid(
                    "missing_value",
                    Some(format!("{path}[{index}].value")),
                    "value is required",
                )
            })?;
            if output
                .insert(
                    key.clone(),
                    RequestValueIr::from_api(value, format!("{path}[{index}].value"))?,
                )
                .is_some()
            {
                return Err(invalid(
                    "duplicate_property",
                    Some(format!("{path}[{index}].key")),
                    format!("property `{key}` occurs more than once"),
                ));
            }
        }
    }
    Ok(output)
}
pub(crate) fn value_from_api(value: api::Value<'_>, path: String) -> Result<Value> {
    match value.kind_type() {
        api::ValueKind::NullValueValue => Ok(Value::Null),
        api::ValueKind::BoolValue => value
            .kind_as_bool_value()
            .map(|value| Value::Bool(value.value()))
            .ok_or_else(|| invalid("missing_value", Some(path), "boolean value is missing")),
        api::ValueKind::Int64Value => value
            .kind_as_int_64_value()
            .map(|value| Value::Int64(value.value()))
            .ok_or_else(|| invalid("missing_value", Some(path), "integer value is missing")),
        api::ValueKind::UInt64Value => value
            .kind_as_uint_64_value()
            .map(|value| Value::UInt64(value.value()))
            .ok_or_else(|| {
                invalid(
                    "missing_value",
                    Some(path),
                    "unsigned integer value is missing",
                )
            }),
        api::ValueKind::Float64Value => {
            let value = value
                .kind_as_float_64_value()
                .ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(path.clone()),
                        "float value is missing",
                    )
                })?
                .value();
            if value.is_finite() {
                Ok(Value::Float64(value))
            } else {
                Err(invalid(
                    "non_finite_number",
                    Some(path),
                    "floating-point parameters must be finite",
                ))
            }
        }
        api::ValueKind::StringValue => value
            .kind_as_string_value()
            .and_then(|value| value.value())
            .map(|value| Value::String(value.to_owned()))
            .ok_or_else(|| invalid("missing_value", Some(path), "string value is missing")),
        api::ValueKind::BytesValue => value
            .kind_as_bytes_value()
            .and_then(|value| value.value())
            .map(|value| Value::Bytes(value.iter().collect()))
            .ok_or_else(|| invalid("missing_value", Some(path), "bytes value is missing")),
        api::ValueKind::ValueList => {
            let values = value
                .kind_as_value_list()
                .ok_or_else(|| {
                    invalid("missing_value", Some(path.clone()), "value list is missing")
                })?
                .values()
                .map(|values| {
                    values
                        .iter()
                        .enumerate()
                        .map(|(index, value)| {
                            value_from_api(value, format!("{path}.values[{index}]"))
                        })
                        .collect::<Result<Vec<_>>>()
                })
                .transpose()?
                .unwrap_or_default();
            Ok(Value::List(values))
        }
        api::ValueKind::ValueObject => {
            let mut fields = BTreeMap::new();
            if let Some(entries) = value
                .kind_as_value_object()
                .ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(path.clone()),
                        "value object is missing",
                    )
                })?
                .fields()
            {
                for (index, entry) in entries.iter().enumerate() {
                    let name = entry.key().to_owned();
                    if fields.contains_key(&name) {
                        return Err(invalid(
                            "duplicate_parameter_field",
                            Some(format!("{path}.fields[{index}]")),
                            format!("field `{name}` occurs more than once"),
                        ));
                    }
                    let value = entry.value().ok_or_else(|| {
                        invalid(
                            "missing_value",
                            Some(format!("{path}.fields[{index}].value")),
                            "object value is missing",
                        )
                    })?;
                    fields.insert(
                        name,
                        value_from_api(value, format!("{path}.fields[{index}].value"))?,
                    );
                }
            }
            Ok(Value::Object(fields))
        }
        api::ValueKind::NONE => Err(invalid(
            "missing_value",
            Some(path),
            "parameter value must select a kind",
        )),
        _ => Err(PortableError::unsupported(
            "unsupported_parameter_value",
            Some(path),
            "parameter value kind is not supported by the embedded IR",
        )),
    }
}
