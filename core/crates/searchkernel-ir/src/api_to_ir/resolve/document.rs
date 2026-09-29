use super::*;

/// Resolve a get request independently of its transport representation.
#[doc(hidden)]
pub fn resolve_get(context: &IndexContext, input: GetInput) -> Result<GetRequestIr> {
    let id = DocumentId::parse(input.id).map_err(|_| {
        invalid(
            "invalid_document_id",
            Some("id"),
            "document ID must not be empty",
        )
    })?;
    let projection = resolve_get_projection(context, input.projection.as_deref())?;
    Ok(GetRequestIr { id, projection })
}

/// Resolve a document against the index schema.
#[doc(hidden)]
pub fn resolve_document(context: &IndexContext, input: DocumentInput) -> Result<DocumentIr> {
    let id = DocumentId::parse(input.id).map_err(|_| {
        invalid(
            "invalid_document_id",
            Some("document.id"),
            "document ID must not be empty",
        )
    })?;
    let mut fields = BTreeMap::new();
    for (name, value) in input.fields {
        let field = context.field_for_resolver(&name, format!("document.fields.{name}"))?;
        let value = resolve_field_value(
            field,
            value,
            format!("document.fields.{name}"),
            context.definition.structs(),
        )?;
        fields.insert(field.name().clone(), value);
    }
    Ok(DocumentIr { id, fields })
}

/// Resolve an insert request against the index schema.
#[doc(hidden)]
pub fn resolve_insert(context: &IndexContext, input: DocumentInput) -> Result<InsertRequestIr> {
    resolve_document(context, input).map(|document| InsertRequestIr { document })
}

/// Resolve an upsert request against the index schema.
#[doc(hidden)]
pub fn resolve_upsert(
    context: &IndexContext,
    input: DocumentInput,
    condition: Option<&str>,
) -> Result<UpsertRequestIr> {
    resolve_document(context, input).map(|document| UpsertRequestIr {
        document,
        condition: condition_to_ir(condition),
    })
}

/// Resolve a patch against the index schema.
#[doc(hidden)]
pub fn resolve_patch(
    context: &IndexContext,
    id: &str,
    operations: &[PatchOperationInput],
) -> Result<PatchRequestIr> {
    if operations.is_empty() {
        return Err(invalid(
            "empty_patch",
            Some("operations"),
            "patch must contain at least one operation",
        ));
    }
    let id = DocumentId::parse(id.to_owned()).map_err(|_| {
        invalid(
            "invalid_document_id",
            Some("id"),
            "document ID must not be empty",
        )
    })?;
    let mut seen = BTreeSet::new();
    let mut resolved = Vec::with_capacity(operations.len());
    for (index, operation) in operations.iter().enumerate() {
        let path = format!("operations[{index}]");
        match operation {
            PatchOperationInput::Set { field, value } => {
                let schema_field =
                    context.field_for_resolver(field, format!("{path}.set.field"))?;
                ensure_field_is_not_repeated(&mut seen, schema_field.name(), &path)?;
                resolved.push(PatchOperationIr::Set {
                    field: schema_field.name().clone(),
                    value: resolve_field_value(
                        schema_field,
                        value.clone(),
                        format!("{path}.set.value"),
                        context.definition.structs(),
                    )?,
                });
            }
            PatchOperationInput::Unset { field } => {
                let schema_field =
                    context.field_for_resolver(field, format!("{path}.unset.field"))?;
                ensure_field_is_not_repeated(&mut seen, schema_field.name(), &path)?;
                resolved.push(PatchOperationIr::Unset {
                    field: schema_field.name().clone(),
                });
            }
        }
    }
    Ok(PatchRequestIr {
        id,
        operations: resolved,
    })
}

/// Resolve a full document update against the index schema.
#[doc(hidden)]
pub fn resolve_update(
    context: &IndexContext,
    id: &str,
    operations: &[UpdateOperationInput],
    condition: Option<&str>,
    create: Option<bool>,
) -> Result<UpdateRequestIr> {
    if operations.is_empty() {
        return Err(invalid(
            "empty_update",
            Some("operations"),
            "update must contain at least one operation",
        ));
    }
    let id = DocumentId::parse(id.to_owned()).map_err(|_| {
        invalid(
            "invalid_document_id",
            Some("id"),
            "document ID must not be empty",
        )
    })?;
    let mut seen = BTreeSet::new();
    let mut resolved = Vec::with_capacity(operations.len());
    for (index, operation) in operations.iter().enumerate() {
        let path = format!("operations[{index}]");
        let target = match operation {
            UpdateOperationInput::Assign { target, .. }
            | UpdateOperationInput::Clear { target }
            | UpdateOperationInput::Arithmetic { target, .. } => target,
        };
        let target = UpdateTargetIr::parse(target.clone()).map_err(|_| {
            invalid(
                "invalid_update_target",
                Some(format!("{path}.target.path")),
                "update target must not be empty",
            )
        })?;
        if !target.is_simple_field() {
            return Err(PortableError::unsupported(
                "update_field_path_not_supported",
                Some(format!("{path}.target.path")),
                "the current native DocumentUpdate adapter supports simple fields only",
            ));
        }
        if !seen.insert(target.as_str().to_owned()) {
            return Err(PortableError::unsupported(
                "multiple_updates_per_target_not_supported",
                Some(path),
                "ordered multiple updates for one target are not supported",
            ));
        }
        let field = context.field_for_resolver(target.as_str(), format!("{path}.target.path"))?;
        match operation {
            UpdateOperationInput::Assign { value, .. } => {
                resolved.push(UpdateOperationIr::Assign {
                    target,
                    value: resolve_field_value(
                        field,
                        value.clone(),
                        format!("{path}.operation.value"),
                        context.definition.structs(),
                    )?,
                })
            }
            UpdateOperationInput::Clear { .. } => {
                resolved.push(UpdateOperationIr::Clear { target })
            }
            UpdateOperationInput::Arithmetic {
                operator, operand, ..
            } => {
                if !matches!(field.kind(), FieldKind::Int64 | FieldKind::Float64) {
                    return Err(invalid(
                        "arithmetic_requires_numeric_field",
                        Some(format!("{path}.target.path")),
                        "arithmetic updates require an integer or float field",
                    ));
                }
                let operand = match operand {
                    RequestValueIr::Int64(value) => *value as f64,
                    RequestValueIr::UInt64(value) => *value as f64,
                    RequestValueIr::Float64(value) if value.is_finite() => *value,
                    _ => {
                        return Err(invalid(
                            "arithmetic_operand_type",
                            Some(format!("{path}.operation.operand")),
                            "arithmetic operands must be finite numbers",
                        ));
                    }
                };
                resolved.push(UpdateOperationIr::Arithmetic {
                    target,
                    operator: *operator,
                    operand,
                });
            }
        }
    }
    Ok(UpdateRequestIr {
        id,
        operations: resolved,
        condition: condition_to_ir(condition),
        create,
    })
}

fn ensure_field_is_not_repeated(
    seen_fields: &mut BTreeSet<FieldName>,
    field: &FieldName,
    path: &str,
) -> Result<()> {
    if !seen_fields.insert(field.clone()) {
        return Err(invalid(
            "duplicate_patch_field",
            Some(path),
            "a patch may mention each field at most once",
        ));
    }
    Ok(())
}

pub(crate) fn resolve_field_value(
    field: &crate::model::FieldIr,
    value: RequestValueIr,
    path: String,
    structs: &[crate::model::NamedStructDefinitionIr],
) -> Result<FieldValueIr> {
    match field.data_type() {
        crate::model::DataTypeIr::Position => return resolve_position_value(value, path),
        crate::model::DataTypeIr::Predicate => return resolve_predicate_value(value, path),
        _ => {}
    }
    if matches!(field.data_type(), crate::model::DataTypeIr::Tensor { .. }) {
        return match value {
            RequestValueIr::Tensor(value) => Ok(FieldValueIr::Tensor(value)),
            _ => Err(invalid(
                "type_mismatch",
                Some(path),
                "tensor fields require tensor values",
            )),
        };
    }
    if let crate::model::DataTypeIr::Array(element) = field.data_type() {
        let fields = match element.as_ref() {
            crate::model::DataTypeIr::Struct { fields } => fields.as_slice(),
            crate::model::DataTypeIr::StructReference { name } => structs
                .iter()
                .find(|definition| definition.name == *name)
                .map(|definition| definition.fields.as_slice())
                .ok_or_else(|| {
                    invalid(
                        "unknown_struct",
                        Some(path.clone()),
                        format!("struct `{name}` is not declared"),
                    )
                })?,
            _ => return resolve_scalar_or_multi_value(field, value, path),
        };
        let RequestValueIr::List(values) = value else {
            return Err(invalid(
                "cardinality_mismatch",
                Some(path),
                "array-of-struct fields require a list value",
            ));
        };
        return values
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                let RequestValueIr::Object(values) = value else {
                    return Err(invalid(
                        "type_mismatch",
                        Some(format!("{path}[{index}]")),
                        "array-of-struct elements require an object value",
                    ));
                };
                let mut members = std::collections::BTreeMap::new();
                for (name, value) in values {
                    let Some(field_definition) = fields.iter().find(|field| field.name == name)
                    else {
                        return Err(invalid(
                            "unknown_field",
                            Some(format!("{path}[{index}].{name}")),
                            "struct member is not defined",
                        ));
                    };
                    let member_path = format!("{path}[{index}].{name}");
                    members.insert(
                        name,
                        resolve_struct_scalar(value, &field_definition.data_type, member_path)?,
                    );
                }
                Ok(members)
            })
            .collect::<Result<Vec<_>>>()
            .map(FieldValueIr::Structured);
    }
    resolve_scalar_or_multi_value(field, value, path)
}

fn resolve_scalar_or_multi_value(
    field: &crate::model::FieldIr,
    value: RequestValueIr,
    path: String,
) -> Result<FieldValueIr> {
    let list = match (&field.cardinality(), value) {
        (FieldCardinality::Single, RequestValueIr::List(_)) => {
            return Err(invalid(
                "cardinality_mismatch",
                Some(path),
                "single-valued fields require a scalar value",
            ));
        }
        (FieldCardinality::Multi, RequestValueIr::List(values)) => {
            let values = values
                .into_iter()
                .enumerate()
                .map(|(index, value)| {
                    resolve_scalar_value(value, field.kind(), format!("{path}.values[{index}]"))
                })
                .collect::<Result<Vec<_>>>()?;
            return Ok(FieldValueIr::Multi(values));
        }
        (FieldCardinality::Multi, _) => {
            return Err(invalid(
                "cardinality_mismatch",
                Some(path),
                "multi-valued fields require a list value",
            ));
        }
        (FieldCardinality::Single, value) => value,
    };
    Ok(FieldValueIr::Single(resolve_scalar_value(
        list,
        field.kind(),
        path,
    )?))
}

fn resolve_struct_scalar(
    value: RequestValueIr,
    data_type: &crate::model::DataTypeIr,
    path: String,
) -> Result<ScalarValueIr> {
    let scalar = match value {
        RequestValueIr::Bool(value) => ScalarValueIr::Bool(value),
        RequestValueIr::Int64(value) => ScalarValueIr::Int64(value),
        RequestValueIr::Float64(value) if value.is_finite() => ScalarValueIr::Float64(value),
        RequestValueIr::String(value) => ScalarValueIr::String(value),
        _ => {
            return Err(invalid(
                "type_mismatch",
                Some(path),
                "struct members require scalar values",
            ));
        }
    };
    let matches = matches!(
        (&scalar, data_type),
        (
            ScalarValueIr::String(_),
            crate::model::DataTypeIr::Primitive(crate::model::PrimitiveDataTypeIr::String)
        ) | (
            ScalarValueIr::Bool(_),
            crate::model::DataTypeIr::Primitive(crate::model::PrimitiveDataTypeIr::Bool)
        ) | (
            ScalarValueIr::Int64(_),
            crate::model::DataTypeIr::Primitive(
                crate::model::PrimitiveDataTypeIr::Int32 | crate::model::PrimitiveDataTypeIr::Int64,
            )
        ) | (
            ScalarValueIr::Float64(_),
            crate::model::DataTypeIr::Primitive(
                crate::model::PrimitiveDataTypeIr::Float32
                    | crate::model::PrimitiveDataTypeIr::Float64,
            )
        )
    );
    if !matches {
        return Err(invalid(
            "type_mismatch",
            Some(path),
            "struct member value does not match its field type",
        ));
    }
    Ok(scalar)
}

pub(crate) fn resolve_position_value(value: RequestValueIr, path: String) -> Result<FieldValueIr> {
    let RequestValueIr::Object(mut fields) = value else {
        return Err(invalid(
            "type_mismatch",
            Some(path),
            "position fields require an object with latitude and longitude",
        ));
    };
    let latitude = resolve_position_coordinate(fields.remove("latitude"), "latitude", &path)?;
    let longitude = resolve_position_coordinate(fields.remove("longitude"), "longitude", &path)?;
    if !fields.is_empty() {
        return Err(invalid(
            "invalid_value",
            Some(path),
            "position objects may contain only latitude and longitude",
        ));
    }
    Ok(FieldValueIr::Position {
        latitude,
        longitude,
    })
}

fn resolve_position_coordinate(
    value: Option<RequestValueIr>,
    name: &str,
    path: &str,
) -> Result<f64> {
    let Some(value) = value else {
        return Err(invalid(
            "missing_value",
            Some(format!("{path}.{name}")),
            format!("position {name} is required"),
        ));
    };
    let value = match value {
        RequestValueIr::Int64(value) => value as f64,
        RequestValueIr::Float64(value) if value.is_finite() => value,
        _ => {
            return Err(invalid(
                "type_mismatch",
                Some(format!("{path}.{name}")),
                "position coordinates must be finite numbers",
            ));
        }
    };
    Ok(value)
}

pub(crate) fn resolve_predicate_value(value: RequestValueIr, path: String) -> Result<FieldValueIr> {
    let RequestValueIr::Object(mut fields) = value else {
        return Err(invalid(
            "type_mismatch",
            Some(path),
            "predicate fields require a Vespa predicate expression object or a features object",
        ));
    };
    if let Some(expression) = fields.remove("expression") {
        if !fields.is_empty() {
            return Err(invalid(
                "invalid_value",
                Some(path.clone()),
                "predicate expression objects may contain only expression",
            ));
        }
        let RequestValueIr::Object(expression) = expression else {
            return Err(invalid(
                "type_mismatch",
                Some(format!("{path}.expression")),
                "predicate expression must be an object",
            ));
        };
        return Ok(FieldValueIr::PredicateExpression {
            expression: serde_json::to_string(&request_value_to_json(
                RequestValueIr::Object(expression),
                &format!("{path}.expression"),
            )?)
            .map_err(|error| invalid("invalid_value", Some(path), error.to_string()))?,
        });
    }
    if fields.contains_key("type") {
        return Ok(FieldValueIr::PredicateExpression {
            expression: serde_json::to_string(&request_value_to_json(
                RequestValueIr::Object(fields),
                &path,
            )?)
            .map_err(|error| invalid("invalid_value", Some(path), error.to_string()))?,
        });
    }
    let Some(RequestValueIr::Object(features)) = fields.remove("features") else {
        return Err(invalid(
            "missing_value",
            Some(format!("{path}.features")),
            "predicate features are required",
        ));
    };
    if !fields.is_empty() {
        return Err(invalid(
            "invalid_value",
            Some(path.clone()),
            "predicate objects may contain only features",
        ));
    }
    let features = features
        .into_iter()
        .map(|(key, value)| match value {
            RequestValueIr::String(value) => Ok((key, value)),
            _ => Err(invalid(
                "type_mismatch",
                Some(format!("{path}.features.{key}")),
                "predicate feature values must be strings",
            )),
        })
        .collect::<Result<std::collections::BTreeMap<_, _>>>()?;
    if features.is_empty() {
        return Err(invalid(
            "invalid_value",
            Some(format!("{path}.features")),
            "predicate fields require at least one feature",
        ));
    }
    Ok(FieldValueIr::Predicate { features })
}

fn request_value_to_json(value: RequestValueIr, path: &str) -> Result<serde_json::Value> {
    match value {
        RequestValueIr::Null => Ok(serde_json::Value::Null),
        RequestValueIr::Bool(value) => Ok(serde_json::Value::Bool(value)),
        RequestValueIr::Int64(value) => Ok(serde_json::Value::Number(value.into())),
        RequestValueIr::UInt64(value) => Ok(serde_json::Value::Number(value.into())),
        RequestValueIr::Float64(value) if value.is_finite() => {
            Ok(serde_json::Number::from_f64(value)
                .map_or(serde_json::Value::Null, serde_json::Value::Number))
        }
        RequestValueIr::String(value) => Ok(serde_json::Value::String(value)),
        RequestValueIr::List(values) => values
            .into_iter()
            .enumerate()
            .map(|(index, value)| request_value_to_json(value, &format!("{path}[{index}]")))
            .collect::<Result<Vec<_>>>()
            .map(serde_json::Value::Array),
        RequestValueIr::Object(values) => values
            .into_iter()
            .map(|(name, value)| {
                request_value_to_json(value, &format!("{path}.{name}")).map(|value| (name, value))
            })
            .collect::<Result<serde_json::Map<_, _>>>()
            .map(serde_json::Value::Object),
        RequestValueIr::Float64(_)
        | RequestValueIr::NonFiniteFloat(_)
        | RequestValueIr::Bytes(_)
        | RequestValueIr::Tensor(_) => Err(invalid(
            "unsupported_value",
            Some(path.to_owned()),
            "predicate expressions support JSON scalar, list, and object values only",
        )),
    }
}

fn resolve_scalar_value(
    value: RequestValueIr,
    kind: FieldKind,
    path: String,
) -> Result<ScalarValueIr> {
    let scalar = match value {
        RequestValueIr::Bool(value) => ScalarValueIr::Bool(value),
        RequestValueIr::Int64(value) => ScalarValueIr::Int64(value),
        RequestValueIr::Float64(value) if value.is_finite() => ScalarValueIr::Float64(value),
        RequestValueIr::String(value) => ScalarValueIr::String(value),
        _ => {
            return Err(invalid(
                "type_mismatch",
                Some(path),
                "value form is not supported for this field",
            ));
        }
    };
    let matches = matches!(
        (&scalar, kind),
        (
            ScalarValueIr::String(_),
            FieldKind::Text | FieldKind::Keyword
        ) | (ScalarValueIr::Int64(_), FieldKind::Int64)
            | (ScalarValueIr::Float64(_), FieldKind::Float64)
            | (ScalarValueIr::Bool(_), FieldKind::Bool)
    );
    if !matches {
        return Err(invalid(
            "type_mismatch",
            Some(path),
            "value type does not match field kind",
        ));
    }
    Ok(scalar)
}
