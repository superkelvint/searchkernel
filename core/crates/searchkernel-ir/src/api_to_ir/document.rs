use super::*;
use crate::model::*;
use searchkernel_protocol::v2 as api;
use std::collections::{BTreeMap, BTreeSet};

impl IndexContext {
    pub fn insert_to_ir(&self, request: api::InsertRequest<'_>) -> Result<InsertRequestIr> {
        Ok(InsertRequestIr {
            document: self.document_from_optional(request.document(), "document")?,
        })
    }
    pub fn upsert_to_ir(&self, request: api::UpsertRequest<'_>) -> Result<UpsertRequestIr> {
        Ok(UpsertRequestIr {
            document: self.document_from_optional(request.document(), "document")?,
            condition: condition_to_ir(request.condition()),
        })
    }
    pub fn patch_to_ir(&self, request: api::PatchRequest<'_>) -> Result<PatchRequestIr> {
        let id = self.document_id(request.id(), "id")?;
        let operations = request.operations();
        let count = operations.map_or(0, |v| v.len());
        if count == 0 {
            return Err(invalid(
                "empty_patch",
                Some("operations"),
                "patch must contain at least one operation",
            ));
        }
        let mut seen = BTreeSet::new();
        let mut converted_operations = Vec::with_capacity(count);
        if let Some(operations) = operations {
            for (index, operation) in operations.iter().enumerate() {
                let path = format!("operations[{index}]");
                let converted = self.patch_operation_to_ir(operation, &path, &mut seen)?;
                converted_operations.push(converted);
            }
        }
        Ok(PatchRequestIr {
            id,
            operations: converted_operations,
        })
    }
    pub fn delete_to_ir(&self, request: api::DeleteRequest<'_>) -> Result<DeleteRequestIr> {
        Ok(DeleteRequestIr {
            id: self.document_id(request.id(), "id")?,
            condition: condition_to_ir(request.condition()),
        })
    }
    pub fn get_to_ir(&self, request: api::GetRequest<'_>) -> Result<GetRequestIr> {
        Ok(GetRequestIr {
            id: self.document_id(request.id(), "id")?,
            projection: self.projection_to_ir(request.projection())?,
        })
    }
    pub fn value_to_ir(&self, field: &str, value: api::Value<'_>) -> Result<FieldValueIr> {
        let field = self.field(field, "field".to_owned())?;
        self.value_to_ir_at(field, value, "value".to_owned())
    }
    pub fn document_to_ir(&self, document: api::Document<'_>) -> Result<DocumentIr> {
        self.document_from_optional(Some(document), "document")
    }
    pub fn write_to_ir(&self, request: WriteRequestRef<'_>) -> Result<WriteRequestIr> {
        match request {
            WriteRequestRef::Insert(r) => self.insert_to_ir(r).map(WriteRequestIr::Insert),
            WriteRequestRef::Upsert(r) => self.upsert_to_ir(r).map(WriteRequestIr::Upsert),
            WriteRequestRef::Patch(r) => self.patch_to_ir(r).map(WriteRequestIr::Patch),
            WriteRequestRef::Delete(r) => self.delete_to_ir(r).map(WriteRequestIr::Delete),
        }
    }
    pub fn bulk_write_to_ir<'a>(
        &self,
        requests: impl IntoIterator<Item = WriteRequestRef<'a>>,
    ) -> Result<Vec<WriteRequestIr>> {
        requests.into_iter().map(|r| self.write_to_ir(r)).collect()
    }
    pub(crate) fn patch_operation_to_ir(
        &self,
        operation: api::PatchOperation<'_>,
        path: &str,
        seen_fields: &mut BTreeSet<FieldName>,
    ) -> Result<PatchOperationIr> {
        match operation.operation_type() {
            api::PatchOperationKind::SetField => {
                let set = operation.operation_as_set_field().ok_or_else(|| {
                    invalid(
                        "invalid_patch_operation",
                        Some(path),
                        "patch operation must select set or unset",
                    )
                })?;
                let field =
                    self.field(set.field().unwrap_or_default(), format!("{path}.set.field"))?;
                ensure_field_is_not_repeated(seen_fields, field, path)?;
                let value = set.value().ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(format!("{path}.set.value")),
                        "set operation requires a value",
                    )
                })?;
                Ok(PatchOperationIr::Set {
                    field: field.name.clone(),
                    value: self.value_to_ir_at(field, value, format!("{path}.set.value"))?,
                })
            }
            api::PatchOperationKind::UnsetField => {
                let unset = operation.operation_as_unset_field().ok_or_else(|| {
                    invalid(
                        "invalid_patch_operation",
                        Some(path),
                        "patch operation must select set or unset",
                    )
                })?;
                let field = self.field(
                    unset.field().unwrap_or_default(),
                    format!("{path}.unset.field"),
                )?;
                ensure_field_is_not_repeated(seen_fields, field, path)?;
                Ok(PatchOperationIr::Unset {
                    field: field.name.clone(),
                })
            }
            _ => Err(invalid(
                "invalid_patch_operation",
                Some(path),
                "patch operation must select set or unset",
            )),
        }
    }
    pub(crate) fn document_id(&self, value: Option<&str>, path: &str) -> Result<DocumentId> {
        DocumentId::parse(value.unwrap_or_default().to_owned()).map_err(|_| {
            invalid(
                "invalid_document_id",
                Some(path),
                "document ID must not be empty",
            )
        })
    }
    pub(crate) fn document_from_optional(
        &self,
        document: Option<api::Document<'_>>,
        path: &str,
    ) -> Result<DocumentIr> {
        let document = document.ok_or_else(|| {
            invalid(
                "missing_value",
                Some(path),
                "document operation requires a document",
            )
        })?;
        let id = self.document_id(document.id(), "document.id")?;
        let mut fields = BTreeMap::new();
        if let Some(entries) = document.fields() {
            for entry in entries.iter() {
                let name = entry.key();
                let field = self.field(name, format!("document.fields.{name}"))?;
                let value = entry.value().ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(format!("document.fields.{name}")),
                        "value must select a scalar kind",
                    )
                })?;
                fields.insert(
                    field.name.clone(),
                    self.value_to_ir_at(field, value, format!("document.fields.{name}"))?,
                );
            }
        }
        Ok(DocumentIr { id, fields })
    }
}

pub(crate) fn condition_to_ir(value: Option<&str>) -> Option<String> {
    value
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(crate) fn ensure_field_is_not_repeated(
    seen_fields: &mut BTreeSet<FieldName>,
    field: &FieldIr,
    path: &str,
) -> Result<()> {
    if !seen_fields.insert(field.name.clone()) {
        return Err(invalid(
            "duplicate_patch_field",
            Some(path),
            "a patch may mention each field at most once",
        ));
    }
    Ok(())
}
