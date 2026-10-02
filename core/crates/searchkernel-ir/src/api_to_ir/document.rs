use super::*;
use crate::model::*;
use searchkernel_protocol::wire::v1 as api;
use std::collections::BTreeMap;

impl IndexContext {
    pub(crate) fn insert_to_ir(&self, request: api::InsertRequest<'_>) -> Result<InsertRequestIr> {
        Ok(InsertRequestIr {
            document: resolve_document(
                self,
                document_input_from_optional(request.document(), "document")?,
            )?,
        })
    }
    pub(crate) fn upsert_to_ir(&self, request: api::UpsertRequest<'_>) -> Result<UpsertRequestIr> {
        Ok(UpsertRequestIr {
            document: resolve_document(
                self,
                document_input_from_optional(request.document(), "document")?,
            )?,
            condition: condition_to_ir(request.condition()),
        })
    }
    pub(crate) fn patch_to_ir(&self, request: api::PatchRequest<'_>) -> Result<PatchRequestIr> {
        let operations = request
            .operations()
            .map(|operations| {
                operations
                    .iter()
                    .map(|operation| match operation.operation_type() {
                        api::PatchOperationKind::SetField => {
                            let set = operation.operation_as_set_field().ok_or_else(|| {
                                invalid(
                                    "invalid_patch_operation",
                                    Some("operations"),
                                    "patch operation must select set or unset",
                                )
                            })?;
                            let value = set.value().ok_or_else(|| {
                                invalid(
                                    "missing_value",
                                    Some("operations"),
                                    "set operation requires a value",
                                )
                            })?;
                            Ok(PatchOperationInput::Set {
                                field: set.field().unwrap_or_default().to_owned(),
                                value: RequestValueIr::from_api(
                                    value,
                                    "operations.value".to_owned(),
                                )?,
                            })
                        }
                        api::PatchOperationKind::UnsetField => {
                            let unset = operation.operation_as_unset_field().ok_or_else(|| {
                                invalid(
                                    "invalid_patch_operation",
                                    Some("operations"),
                                    "patch operation must select set or unset",
                                )
                            })?;
                            Ok(PatchOperationInput::Unset {
                                field: unset.field().unwrap_or_default().to_owned(),
                            })
                        }
                        _ => Err(invalid(
                            "invalid_patch_operation",
                            Some("operations"),
                            "patch operation must select set or unset",
                        )),
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_default();
        resolve_patch(self, request.id().unwrap_or_default(), &operations)
    }
    pub(crate) fn update_to_ir(&self, request: api::UpdateRequest<'_>) -> Result<UpdateRequestIr> {
        let operations = request
            .operations()
            .map(|operations| {
                operations
                    .iter()
                    .map(|operation| {
                        let target = operation
                            .target()
                            .and_then(|target| target.path())
                            .unwrap_or_default()
                            .to_owned();
                        match operation.operation_type() {
                            api::UpdateOperationKind::AssignUpdate => {
                                let assign = operation.operation_as_assign_update().ok_or_else(|| {
                                    invalid(
                                        "invalid_update_operation",
                                        Some("operations"),
                                        "update operation must contain an assign payload",
                                    )
                                })?;
                                let value = assign.value().ok_or_else(|| {
                                    invalid(
                                        "missing_value",
                                        Some("operations.value"),
                                        "assign operation requires a value",
                                    )
                                })?;
                                Ok(UpdateOperationInput::Assign {
                                    target,
                                    value: RequestValueIr::from_api(
                                        value,
                                        "operations.value".to_owned(),
                                    )?,
                                })
                            }
                            api::UpdateOperationKind::ClearUpdate => {
                                if operation.operation_as_clear_update().is_none() {
                                    return Err(invalid(
                                        "invalid_update_operation",
                                        Some("operations"),
                                        "update operation must contain a clear payload",
                                    ));
                                }
                                Ok(UpdateOperationInput::Clear { target })
                            }
                            api::UpdateOperationKind::ArithmeticUpdate => {
                                let arithmetic = operation
                                    .operation_as_arithmetic_update()
                                    .ok_or_else(|| {
                                        invalid(
                                            "invalid_update_operation",
                                            Some("operations"),
                                            "update operation must contain an arithmetic payload",
                                        )
                                    })?;
                                let operand = arithmetic.operand().ok_or_else(|| {
                                    invalid(
                                        "missing_value",
                                        Some("operations.operand"),
                                        "arithmetic operation requires an operand",
                                    )
                                })?;
                                let operator = match arithmetic.operator() {
                                    api::ArithmeticUpdateOperator::Increment => {
                                        UpdateArithmeticOperatorIr::Increment
                                    }
                                    api::ArithmeticUpdateOperator::Decrement => {
                                        UpdateArithmeticOperatorIr::Decrement
                                    }
                                    api::ArithmeticUpdateOperator::Multiply => {
                                        UpdateArithmeticOperatorIr::Multiply
                                    }
                                    api::ArithmeticUpdateOperator::Divide => {
                                        UpdateArithmeticOperatorIr::Divide
                                    }
                                    _ => {
                                        return Err(invalid(
                                            "invalid_update_operator",
                                            Some("operations.operator"),
                                            "arithmetic operation requires a supported operator",
                                        ));
                                    }
                                };
                                Ok(UpdateOperationInput::Arithmetic {
                                    target,
                                    operator,
                                    operand: RequestValueIr::from_api(
                                        operand,
                                        "operations.operand".to_owned(),
                                    )?,
                                })
                            }
                            api::UpdateOperationKind::AddUpdate => {
                                operation.operation_as_add_update().ok_or_else(|| {
                                    invalid(
                                        "invalid_update_operation",
                                        Some("operations"),
                                        "update operation must contain an add payload",
                                    )
                                })?;
                                Err(PortableError::unsupported(
                                    "update_operation_not_supported",
                                    Some("operations.operation"),
                                    "the current native DocumentUpdate adapter supports assign, clear, and arithmetic operations only",
                                ))
                            }
                            api::UpdateOperationKind::RemoveUpdate => {
                                operation.operation_as_remove_update().ok_or_else(|| {
                                    invalid(
                                        "invalid_update_operation",
                                        Some("operations"),
                                        "update operation must contain a remove payload",
                                    )
                                })?;
                                Err(PortableError::unsupported(
                                    "update_operation_not_supported",
                                    Some("operations.operation"),
                                    "the current native DocumentUpdate adapter supports assign, clear, and arithmetic operations only",
                                ))
                            }
                            api::UpdateOperationKind::MatchUpdate => {
                                operation.operation_as_match_update().ok_or_else(|| {
                                    invalid(
                                        "invalid_update_operation",
                                        Some("operations"),
                                        "update operation must contain a match payload",
                                    )
                                })?;
                                Err(PortableError::unsupported(
                                    "update_operation_not_supported",
                                    Some("operations.operation"),
                                    "the current native DocumentUpdate adapter supports assign, clear, and arithmetic operations only",
                                ))
                            }
                            api::UpdateOperationKind::TensorModifyUpdate => {
                                operation.operation_as_tensor_modify_update().ok_or_else(|| {
                                    invalid(
                                        "invalid_update_operation",
                                        Some("operations"),
                                        "update operation must contain a tensor-modify payload",
                                    )
                                })?;
                                Err(PortableError::unsupported(
                                    "update_operation_not_supported",
                                    Some("operations.operation"),
                                    "the current native DocumentUpdate adapter supports assign, clear, and arithmetic operations only",
                                ))
                            }
                            _ => Err(invalid(
                                "invalid_update_operation",
                                Some("operations"),
                                "update operation must select an operation",
                            )),
                        }
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_default();
        resolve_update(
            self,
            request.id().unwrap_or_default(),
            &operations,
            request.condition(),
            request.create_().map(|create| create.value()),
        )
    }
    pub(crate) fn delete_to_ir(&self, request: api::DeleteRequest<'_>) -> Result<DeleteRequestIr> {
        resolve_delete(request.id().unwrap_or_default(), request.condition())
    }
    pub(crate) fn get_to_ir(&self, request: api::GetRequest<'_>) -> Result<GetRequestIr> {
        let projection = request.projection().map(|projection| {
            projection
                .fields()
                .map(|fields| fields.iter().map(ToOwned::to_owned).collect())
                .unwrap_or_default()
        });
        resolve_get(
            self,
            GetInput {
                id: request.id().unwrap_or_default().to_owned(),
                projection,
            },
        )
    }
    pub(crate) fn value_to_ir(&self, field: &str, value: api::Value<'_>) -> Result<FieldValueIr> {
        let field = self.field(field, "field".to_owned())?;
        self.value_to_ir_at(field, value, "value".to_owned())
    }
    pub(crate) fn document_to_ir(&self, document: api::Document<'_>) -> Result<DocumentIr> {
        resolve_document(
            self,
            document_input_from_optional(Some(document), "document")?,
        )
    }
    pub(crate) fn write_to_ir(&self, request: WriteRequestRef<'_>) -> Result<WriteRequestIr> {
        match request {
            WriteRequestRef::Insert(r) => self.insert_to_ir(r).map(WriteRequestIr::Insert),
            WriteRequestRef::Upsert(r) => self.upsert_to_ir(r).map(WriteRequestIr::Upsert),
            WriteRequestRef::Patch(r) => self.patch_to_ir(r).map(WriteRequestIr::Patch),
            WriteRequestRef::Update(r) => self.update_to_ir(r).map(WriteRequestIr::Update),
            WriteRequestRef::Delete(r) => self.delete_to_ir(r).map(WriteRequestIr::Delete),
        }
    }
    pub(crate) fn bulk_write_to_ir<'a>(
        &self,
        requests: impl IntoIterator<Item = WriteRequestRef<'a>>,
    ) -> Result<Vec<WriteRequestIr>> {
        requests.into_iter().map(|r| self.write_to_ir(r)).collect()
    }
}

fn document_input_from_optional(
    document: Option<api::Document<'_>>,
    path: &str,
) -> Result<DocumentInput> {
    let document = document.ok_or_else(|| {
        invalid(
            "missing_value",
            Some(path),
            "document operation requires a document",
        )
    })?;
    let id = document.id().unwrap_or_default().to_owned();
    let mut fields = BTreeMap::new();
    if let Some(entries) = document.fields() {
        for (index, entry) in entries.iter().enumerate() {
            let name = entry.key();
            let value = entry.value().ok_or_else(|| {
                invalid(
                    "missing_value",
                    Some(format!("document.fields.{name}")),
                    "value must select a scalar kind",
                )
            })?;
            if fields
                .insert(
                    name.to_owned(),
                    RequestValueIr::from_api(value, format!("document.fields.{name}"))?,
                )
                .is_some()
            {
                return Err(invalid(
                    "duplicate_document_field",
                    Some(format!("document.fields[{index}].key")),
                    format!("document field `{name}` occurs more than once"),
                ));
            }
        }
    }
    Ok(DocumentInput { id, fields })
}

pub(crate) fn condition_to_ir(value: Option<&str>) -> Option<String> {
    value
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}
