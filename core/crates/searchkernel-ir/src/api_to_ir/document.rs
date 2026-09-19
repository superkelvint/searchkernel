use super::*;
use crate::model::*;
use searchkernel_protocol::v2 as api;
use std::collections::BTreeMap;

impl IndexContext {
    pub fn insert_to_ir(&self, request: api::InsertRequest<'_>) -> Result<InsertRequestIr> {
        Ok(InsertRequestIr {
            document: resolve_document(
                self,
                document_input_from_optional(request.document(), "document")?,
            )?,
        })
    }
    pub fn upsert_to_ir(&self, request: api::UpsertRequest<'_>) -> Result<UpsertRequestIr> {
        Ok(UpsertRequestIr {
            document: resolve_document(
                self,
                document_input_from_optional(request.document(), "document")?,
            )?,
            condition: condition_to_ir(request.condition()),
        })
    }
    pub fn patch_to_ir(&self, request: api::PatchRequest<'_>) -> Result<PatchRequestIr> {
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
    pub fn delete_to_ir(&self, request: api::DeleteRequest<'_>) -> Result<DeleteRequestIr> {
        resolve_delete(request.id().unwrap_or_default(), request.condition())
    }
    pub fn get_to_ir(&self, request: api::GetRequest<'_>) -> Result<GetRequestIr> {
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
    pub fn value_to_ir(&self, field: &str, value: api::Value<'_>) -> Result<FieldValueIr> {
        let field = self.field(field, "field".to_owned())?;
        self.value_to_ir_at(field, value, "value".to_owned())
    }
    pub fn document_to_ir(&self, document: api::Document<'_>) -> Result<DocumentIr> {
        resolve_document(
            self,
            document_input_from_optional(Some(document), "document")?,
        )
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
        for entry in entries.iter() {
            let name = entry.key();
            let value = entry.value().ok_or_else(|| {
                invalid(
                    "missing_value",
                    Some(format!("document.fields.{name}")),
                    "value must select a scalar kind",
                )
            })?;
            fields.insert(
                name.to_owned(),
                RequestValueIr::from_api(value, format!("document.fields.{name}"))?,
            );
        }
    }
    Ok(DocumentInput { id, fields })
}

pub(crate) fn condition_to_ir(value: Option<&str>) -> Option<String> {
    value
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}
