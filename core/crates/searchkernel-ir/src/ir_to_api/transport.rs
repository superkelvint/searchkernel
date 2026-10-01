use super::*;
use searchkernel_protocol::{protocol::v2 as wire, PROTOCOL_VERSION};
use std::collections::BTreeMap;

fn finish_request(
    builder: &mut flatbuffers::FlatBufferBuilder<'_>,
    payload_type: wire::RequestPayload,
    payload: flatbuffers::WIPOffset<flatbuffers::UnionWIPOffset>,
    target: Option<&str>,
    expected_index_generation: Option<&str>,
) -> Vec<u8> {
    let target = target.map(|name| {
        let name = builder.create_string(name);
        api::IndexTarget::create(builder, &api::IndexTargetArgs { name: Some(name) })
    });
    let expected_index_generation =
        expected_index_generation.map(|generation| builder.create_string(generation));
    let root = wire::RequestEnvelope::create(
        builder,
        &wire::RequestEnvelopeArgs {
            protocol_version: PROTOCOL_VERSION,
            target,
            payload_type,
            payload: Some(payload),
            expected_index_generation,
        },
    );
    wire::finish_request_envelope_buffer(builder, root);
    builder.finished_data().to_vec()
}

fn scalar_request_value(value: &ir::ScalarValueIr) -> ir::RequestValueIr {
    match value {
        ir::ScalarValueIr::Bool(value) => ir::RequestValueIr::Bool(*value),
        ir::ScalarValueIr::Int64(value) => ir::RequestValueIr::Int64(*value),
        ir::ScalarValueIr::Float64(value) => ir::RequestValueIr::Float64(*value),
        ir::ScalarValueIr::String(value) => ir::RequestValueIr::String(value.clone()),
    }
}

fn field_request_value(value: &ir::FieldValueIr) -> Result<ir::RequestValueIr, PortableError> {
    Ok(match value {
        ir::FieldValueIr::Single(value) => scalar_request_value(value),
        ir::FieldValueIr::Multi(values) => {
            ir::RequestValueIr::List(values.iter().map(scalar_request_value).collect())
        }
        ir::FieldValueIr::Structured(values) => ir::RequestValueIr::List(
            values
                .iter()
                .map(|fields| {
                    ir::RequestValueIr::Object(
                        fields
                            .iter()
                            .map(|(name, value)| (name.clone(), scalar_request_value(value)))
                            .collect(),
                    )
                })
                .collect(),
        ),
        ir::FieldValueIr::Position {
            latitude,
            longitude,
        } => ir::RequestValueIr::Object(BTreeMap::from([
            (
                "latitude".to_owned(),
                ir::RequestValueIr::Float64(*latitude),
            ),
            (
                "longitude".to_owned(),
                ir::RequestValueIr::Float64(*longitude),
            ),
        ])),
        ir::FieldValueIr::Predicate { features } => ir::RequestValueIr::Object(BTreeMap::from([(
            "features".to_owned(),
            ir::RequestValueIr::Object(
                features
                    .iter()
                    .map(|(name, value)| (name.clone(), ir::RequestValueIr::String(value.clone())))
                    .collect(),
            ),
        )])),
        ir::FieldValueIr::PredicateExpression { expression } => {
            let json: serde_json::Value = serde_json::from_str(expression).map_err(|error| {
                PortableError::new(
                    ErrorCode::Internal,
                    format!("canonical predicate expression is not valid JSON: {error}"),
                )
            })?;
            crate::request_value_from_json(&json, "document.predicate".to_owned())?
        }
        ir::FieldValueIr::Tensor(value) => ir::RequestValueIr::Tensor(value.clone()),
    })
}

fn document_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    document: &ir::DocumentIr,
) -> Result<flatbuffers::WIPOffset<api::Document<'a>>, PortableError> {
    let id = builder.create_string(document.id().as_str());
    let mut entries = Vec::with_capacity(document.fields().len());
    for (name, field_value) in document.fields() {
        let name = builder.create_string(name.as_str());
        let value = field_request_value(field_value)?;
        let value = request_value_offset(builder, &value);
        entries.push(api::ValueEntry::create(
            builder,
            &api::ValueEntryArgs {
                key: Some(name),
                value: Some(value),
            },
        ));
    }
    let fields = builder.create_vector(&entries);
    Ok(api::Document::create(
        builder,
        &api::DocumentArgs {
            id: Some(id),
            fields: Some(fields),
        },
    ))
}

pub fn encode_insert_request(request: &ir::InsertRequestIr) -> Result<Vec<u8>, PortableError> {
    encode_insert_request_for_target(request, None)
}

pub fn encode_insert_request_for_target(
    request: &ir::InsertRequestIr,
    target: Option<&str>,
) -> Result<Vec<u8>, PortableError> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let document = document_offset(&mut builder, request.document())?;
    let request = api::InsertRequest::create(
        &mut builder,
        &api::InsertRequestArgs {
            document: Some(document),
        },
    );
    Ok(finish_request(
        &mut builder,
        wire::RequestPayload::SearchKernel_V2_InsertRequest,
        request.as_union_value(),
        target,
        None,
    ))
}

pub fn encode_insert_request_for_target_generation(
    request: &ir::InsertRequestIr,
    target: Option<&str>,

    expected_index_generation: Option<&str>,
) -> Result<Vec<u8>, PortableError> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let document = document_offset(&mut builder, request.document())?;
    let request = api::InsertRequest::create(
        &mut builder,
        &api::InsertRequestArgs {
            document: Some(document),
        },
    );
    Ok(finish_request(
        &mut builder,
        wire::RequestPayload::SearchKernel_V2_InsertRequest,
        request.as_union_value(),
        target,
        expected_index_generation,
    ))
}

pub fn encode_upsert_request(request: &ir::UpsertRequestIr) -> Result<Vec<u8>, PortableError> {
    encode_upsert_request_for_target(request, None)
}

pub fn encode_upsert_request_for_target(
    request: &ir::UpsertRequestIr,
    target: Option<&str>,
) -> Result<Vec<u8>, PortableError> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let document = document_offset(&mut builder, request.document())?;
    let condition = request
        .condition()
        .map(|condition| builder.create_string(condition));
    let request = api::UpsertRequest::create(
        &mut builder,
        &api::UpsertRequestArgs {
            document: Some(document),
            condition,
        },
    );
    Ok(finish_request(
        &mut builder,
        wire::RequestPayload::SearchKernel_V2_UpsertRequest,
        request.as_union_value(),
        target,
        None,
    ))
}

pub fn encode_upsert_request_for_target_generation(
    request: &ir::UpsertRequestIr,
    target: Option<&str>,

    expected_index_generation: Option<&str>,
) -> Result<Vec<u8>, PortableError> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let document = document_offset(&mut builder, request.document())?;
    let condition = request
        .condition()
        .map(|condition| builder.create_string(condition));
    let request = api::UpsertRequest::create(
        &mut builder,
        &api::UpsertRequestArgs {
            document: Some(document),
            condition,
        },
    );
    Ok(finish_request(
        &mut builder,
        wire::RequestPayload::SearchKernel_V2_UpsertRequest,
        request.as_union_value(),
        target,
        expected_index_generation,
    ))
}

pub fn encode_patch_request(request: &ir::PatchRequestIr) -> Result<Vec<u8>, PortableError> {
    encode_patch_request_for_target(request, None)
}

pub fn encode_patch_request_for_target(
    request: &ir::PatchRequestIr,
    logical_target: Option<&str>,
) -> Result<Vec<u8>, PortableError> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string(request.id().as_str());
    let mut operations = Vec::with_capacity(request.operations().len());
    for operation in request.operations() {
        let (operation_type, operation) = match operation {
            ir::PatchOperationIr::Set { field, value } => {
                let field = builder.create_string(field.as_str());
                let value = field_request_value(value)?;
                let value = request_value_offset(&mut builder, &value);
                let set = api::SetField::create(
                    &mut builder,
                    &api::SetFieldArgs {
                        field: Some(field),
                        value: Some(value),
                    },
                );
                (api::PatchOperationKind::SetField, set.as_union_value())
            }
            ir::PatchOperationIr::Unset { field } => {
                let field = builder.create_string(field.as_str());
                let unset = api::UnsetField::create(
                    &mut builder,
                    &api::UnsetFieldArgs { field: Some(field) },
                );
                (api::PatchOperationKind::UnsetField, unset.as_union_value())
            }
        };
        operations.push(api::PatchOperation::create(
            &mut builder,
            &api::PatchOperationArgs {
                operation_type,
                operation: Some(operation),
            },
        ));
    }
    let operations = builder.create_vector(&operations);
    let request = api::PatchRequest::create(
        &mut builder,
        &api::PatchRequestArgs {
            id: Some(id),
            operations: Some(operations),
        },
    );
    Ok(finish_request(
        &mut builder,
        wire::RequestPayload::SearchKernel_V2_PatchRequest,
        request.as_union_value(),
        logical_target,
        None,
    ))
}

pub fn encode_patch_request_for_target_generation(
    request: &ir::PatchRequestIr,
    logical_target: Option<&str>,

    expected_index_generation: Option<&str>,
) -> Result<Vec<u8>, PortableError> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string(request.id().as_str());
    let mut operations = Vec::with_capacity(request.operations().len());
    for operation in request.operations() {
        let (operation_type, operation) = match operation {
            ir::PatchOperationIr::Set { field, value } => {
                let field = builder.create_string(field.as_str());
                let value = field_request_value(value)?;
                let value = request_value_offset(&mut builder, &value);
                let set = api::SetField::create(
                    &mut builder,
                    &api::SetFieldArgs {
                        field: Some(field),
                        value: Some(value),
                    },
                );
                (api::PatchOperationKind::SetField, set.as_union_value())
            }
            ir::PatchOperationIr::Unset { field } => {
                let field = builder.create_string(field.as_str());
                let unset = api::UnsetField::create(
                    &mut builder,
                    &api::UnsetFieldArgs { field: Some(field) },
                );
                (api::PatchOperationKind::UnsetField, unset.as_union_value())
            }
        };
        operations.push(api::PatchOperation::create(
            &mut builder,
            &api::PatchOperationArgs {
                operation_type,
                operation: Some(operation),
            },
        ));
    }
    let operations = builder.create_vector(&operations);
    let request = api::PatchRequest::create(
        &mut builder,
        &api::PatchRequestArgs {
            id: Some(id),
            operations: Some(operations),
        },
    );
    Ok(finish_request(
        &mut builder,
        wire::RequestPayload::SearchKernel_V2_PatchRequest,
        request.as_union_value(),
        logical_target,
        expected_index_generation,
    ))
}

pub fn encode_update_request(request: &ir::UpdateRequestIr) -> Result<Vec<u8>, PortableError> {
    encode_update_request_for_target(request, None)
}

pub fn encode_update_request_for_target(
    request: &ir::UpdateRequestIr,
    logical_target: Option<&str>,
) -> Result<Vec<u8>, PortableError> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string(request.id().as_str());
    let mut operations = Vec::with_capacity(request.operations().len());
    for operation in request.operations() {
        let (target, operation_type, operation) = match operation {
            ir::UpdateOperationIr::Assign { target, value } => {
                let path = builder.create_string(target.as_str());
                let target =
                    api::FieldPath::create(&mut builder, &api::FieldPathArgs { path: Some(path) });
                let value = field_request_value(value)?;
                let value = request_value_offset(&mut builder, &value);
                let operation = api::AssignUpdate::create(
                    &mut builder,
                    &api::AssignUpdateArgs { value: Some(value) },
                );
                (
                    target,
                    api::UpdateOperationKind::AssignUpdate,
                    operation.as_union_value(),
                )
            }
            ir::UpdateOperationIr::Clear { target } => {
                let path = builder.create_string(target.as_str());
                let target =
                    api::FieldPath::create(&mut builder, &api::FieldPathArgs { path: Some(path) });
                let operation = api::ClearUpdate::create(&mut builder, &api::ClearUpdateArgs {});
                (
                    target,
                    api::UpdateOperationKind::ClearUpdate,
                    operation.as_union_value(),
                )
            }
            ir::UpdateOperationIr::Arithmetic {
                target,
                operator,
                operand,
            } => {
                let path = builder.create_string(target.as_str());
                let target =
                    api::FieldPath::create(&mut builder, &api::FieldPathArgs { path: Some(path) });
                let operator = match operator {
                    ir::UpdateArithmeticOperatorIr::Add
                    | ir::UpdateArithmeticOperatorIr::Increment => {
                        api::ArithmeticUpdateOperator::Increment
                    }
                    ir::UpdateArithmeticOperatorIr::Subtract
                    | ir::UpdateArithmeticOperatorIr::Decrement => {
                        api::ArithmeticUpdateOperator::Decrement
                    }
                    ir::UpdateArithmeticOperatorIr::Multiply => {
                        api::ArithmeticUpdateOperator::Multiply
                    }
                    ir::UpdateArithmeticOperatorIr::Divide => api::ArithmeticUpdateOperator::Divide,
                };
                let operand =
                    request_value_offset(&mut builder, &ir::RequestValueIr::Float64(*operand));
                let operation = api::ArithmeticUpdate::create(
                    &mut builder,
                    &api::ArithmeticUpdateArgs {
                        operator,
                        operand: Some(operand),
                    },
                );
                (
                    target,
                    api::UpdateOperationKind::ArithmeticUpdate,
                    operation.as_union_value(),
                )
            }
        };
        operations.push(api::UpdateOperation::create(
            &mut builder,
            &api::UpdateOperationArgs {
                target: Some(target),
                operation_type,
                operation: Some(operation),
            },
        ));
    }
    let operations = builder.create_vector(&operations);
    let condition = request
        .condition()
        .map(|condition| builder.create_string(condition));
    let create = request.create().map(api::OptionalBool::new);
    let request = api::UpdateRequest::create(
        &mut builder,
        &api::UpdateRequestArgs {
            id: Some(id),
            operations: Some(operations),
            condition,
            create: create.as_ref(),
        },
    );
    Ok(finish_request(
        &mut builder,
        wire::RequestPayload::SearchKernel_V2_UpdateRequest,
        request.as_union_value(),
        logical_target,
        None,
    ))
}

pub fn encode_update_request_for_target_generation(
    request: &ir::UpdateRequestIr,
    logical_target: Option<&str>,

    expected_index_generation: Option<&str>,
) -> Result<Vec<u8>, PortableError> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string(request.id().as_str());
    let mut operations = Vec::with_capacity(request.operations().len());
    for operation in request.operations() {
        let (target, operation_type, operation) = match operation {
            ir::UpdateOperationIr::Assign { target, value } => {
                let path = builder.create_string(target.as_str());
                let target =
                    api::FieldPath::create(&mut builder, &api::FieldPathArgs { path: Some(path) });
                let value = field_request_value(value)?;
                let value = request_value_offset(&mut builder, &value);
                let operation = api::AssignUpdate::create(
                    &mut builder,
                    &api::AssignUpdateArgs { value: Some(value) },
                );
                (
                    target,
                    api::UpdateOperationKind::AssignUpdate,
                    operation.as_union_value(),
                )
            }
            ir::UpdateOperationIr::Clear { target } => {
                let path = builder.create_string(target.as_str());
                let target =
                    api::FieldPath::create(&mut builder, &api::FieldPathArgs { path: Some(path) });
                let operation = api::ClearUpdate::create(&mut builder, &api::ClearUpdateArgs {});
                (
                    target,
                    api::UpdateOperationKind::ClearUpdate,
                    operation.as_union_value(),
                )
            }
            ir::UpdateOperationIr::Arithmetic {
                target,
                operator,
                operand,
            } => {
                let path = builder.create_string(target.as_str());
                let target =
                    api::FieldPath::create(&mut builder, &api::FieldPathArgs { path: Some(path) });
                let operator = match operator {
                    ir::UpdateArithmeticOperatorIr::Add
                    | ir::UpdateArithmeticOperatorIr::Increment => {
                        api::ArithmeticUpdateOperator::Increment
                    }
                    ir::UpdateArithmeticOperatorIr::Subtract
                    | ir::UpdateArithmeticOperatorIr::Decrement => {
                        api::ArithmeticUpdateOperator::Decrement
                    }
                    ir::UpdateArithmeticOperatorIr::Multiply => {
                        api::ArithmeticUpdateOperator::Multiply
                    }
                    ir::UpdateArithmeticOperatorIr::Divide => api::ArithmeticUpdateOperator::Divide,
                };
                let operand =
                    request_value_offset(&mut builder, &ir::RequestValueIr::Float64(*operand));
                let operation = api::ArithmeticUpdate::create(
                    &mut builder,
                    &api::ArithmeticUpdateArgs {
                        operator,
                        operand: Some(operand),
                    },
                );
                (
                    target,
                    api::UpdateOperationKind::ArithmeticUpdate,
                    operation.as_union_value(),
                )
            }
        };
        operations.push(api::UpdateOperation::create(
            &mut builder,
            &api::UpdateOperationArgs {
                target: Some(target),
                operation_type,
                operation: Some(operation),
            },
        ));
    }
    let operations = builder.create_vector(&operations);
    let condition = request
        .condition()
        .map(|condition| builder.create_string(condition));
    let create = request.create().map(api::OptionalBool::new);
    let request = api::UpdateRequest::create(
        &mut builder,
        &api::UpdateRequestArgs {
            id: Some(id),
            operations: Some(operations),
            condition,
            create: create.as_ref(),
        },
    );
    Ok(finish_request(
        &mut builder,
        wire::RequestPayload::SearchKernel_V2_UpdateRequest,
        request.as_union_value(),
        logical_target,
        expected_index_generation,
    ))
}

pub fn encode_delete_request(request: &ir::DeleteRequestIr) -> Result<Vec<u8>, PortableError> {
    encode_delete_request_for_target(request, None)
}

pub fn encode_delete_request_for_target(
    request: &ir::DeleteRequestIr,
    logical_target: Option<&str>,
) -> Result<Vec<u8>, PortableError> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string(request.id().as_str());
    let condition = request
        .condition()
        .map(|condition| builder.create_string(condition));
    let request = api::DeleteRequest::create(
        &mut builder,
        &api::DeleteRequestArgs {
            id: Some(id),
            condition,
        },
    );
    Ok(finish_request(
        &mut builder,
        wire::RequestPayload::SearchKernel_V2_DeleteRequest,
        request.as_union_value(),
        logical_target,
        None,
    ))
}

pub fn encode_delete_request_for_target_generation(
    request: &ir::DeleteRequestIr,
    logical_target: Option<&str>,

    expected_index_generation: Option<&str>,
) -> Result<Vec<u8>, PortableError> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string(request.id().as_str());
    let condition = request
        .condition()
        .map(|condition| builder.create_string(condition));
    let request = api::DeleteRequest::create(
        &mut builder,
        &api::DeleteRequestArgs {
            id: Some(id),
            condition,
        },
    );
    Ok(finish_request(
        &mut builder,
        wire::RequestPayload::SearchKernel_V2_DeleteRequest,
        request.as_union_value(),
        logical_target,
        expected_index_generation,
    ))
}

pub fn encode_get_request(request: &ir::GetRequestIr) -> Result<Vec<u8>, PortableError> {
    encode_get_request_for_target(request, None)
}

pub fn encode_get_request_for_target(
    request: &ir::GetRequestIr,
    logical_target: Option<&str>,
) -> Result<Vec<u8>, PortableError> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string(request.id().as_str());
    // Resolved IR always carries an explicit effective projection. Encoding it
    // as present is intentional: API -> IR -> API -> IR stability, not original
    // caller presence, is the invariant at this boundary.
    let fields = request
        .projection()
        .fields()
        .iter()
        .map(|field| builder.create_string(field.as_str()))
        .collect::<Vec<_>>();
    let fields = builder.create_vector(&fields);
    let projection = api::FieldProjection::create(
        &mut builder,
        &api::FieldProjectionArgs {
            fields: Some(fields),
        },
    );
    let request = api::GetRequest::create(
        &mut builder,
        &api::GetRequestArgs {
            id: Some(id),
            projection: Some(projection),
        },
    );
    Ok(finish_request(
        &mut builder,
        wire::RequestPayload::SearchKernel_V2_GetRequest,
        request.as_union_value(),
        logical_target,
        None,
    ))
}

pub fn encode_get_request_for_target_generation(
    request: &ir::GetRequestIr,
    logical_target: Option<&str>,

    expected_index_generation: Option<&str>,
) -> Result<Vec<u8>, PortableError> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string(request.id().as_str());
    // Resolved IR always carries an explicit effective projection. Encoding it
    // as present is intentional: API -> IR -> API -> IR stability, not original
    // caller presence, is the invariant at this boundary.
    let fields = request
        .projection()
        .fields()
        .iter()
        .map(|field| builder.create_string(field.as_str()))
        .collect::<Vec<_>>();
    let fields = builder.create_vector(&fields);
    let projection = api::FieldProjection::create(
        &mut builder,
        &api::FieldProjectionArgs {
            fields: Some(fields),
        },
    );
    let request = api::GetRequest::create(
        &mut builder,
        &api::GetRequestArgs {
            id: Some(id),
            projection: Some(projection),
        },
    );
    Ok(finish_request(
        &mut builder,
        wire::RequestPayload::SearchKernel_V2_GetRequest,
        request.as_union_value(),
        logical_target,
        expected_index_generation,
    ))
}

pub fn encode_search_request(request: &ir::SearchRequestIr) -> Result<Vec<u8>, PortableError> {
    encode_search_request_for_target(request, None)
}

pub fn encode_search_request_for_target(
    request: &ir::SearchRequestIr,
    logical_target: Option<&str>,
) -> Result<Vec<u8>, PortableError> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let request = super::search::search_request_offset(&mut builder, request);
    Ok(finish_request(
        &mut builder,
        wire::RequestPayload::SearchKernel_V2_SearchRequest,
        request.as_union_value(),
        logical_target,
        None,
    ))
}

pub fn encode_search_request_for_target_generation(
    request: &ir::SearchRequestIr,
    logical_target: Option<&str>,

    expected_index_generation: Option<&str>,
) -> Result<Vec<u8>, PortableError> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let request = super::search::search_request_offset(&mut builder, request);
    Ok(finish_request(
        &mut builder,
        wire::RequestPayload::SearchKernel_V2_SearchRequest,
        request.as_union_value(),
        logical_target,
        expected_index_generation,
    ))
}

pub fn encode_analyze_request(request: &ir::AnalyzeRequestIr) -> Result<Vec<u8>, PortableError> {
    encode_analyze_request_for_target(request, None)
}

pub fn encode_analyze_request_for_target(
    request: &ir::AnalyzeRequestIr,
    logical_target: Option<&str>,
) -> Result<Vec<u8>, PortableError> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let name = builder.create_string("standard");
    let analyzer = api::Analyzer::create(
        &mut builder,
        &api::AnalyzerArgs {
            name: Some(name),
            version: 1,
            options: None,
        },
    );
    let text = builder.create_string(request.text());
    let language = request
        .language()
        .map(|language| builder.create_string(language.as_str()));
    let request = api::AnalyzeRequest::create(
        &mut builder,
        &api::AnalyzeRequestArgs {
            target_type: api::AnalyzeTarget::Analyzer,
            target: Some(analyzer.as_union_value()),
            text: Some(text),
            language,
        },
    );
    Ok(finish_request(
        &mut builder,
        wire::RequestPayload::SearchKernel_V2_AnalyzeRequest,
        request.as_union_value(),
        logical_target,
        None,
    ))
}

pub fn encode_analyze_request_for_target_generation(
    request: &ir::AnalyzeRequestIr,
    logical_target: Option<&str>,

    expected_index_generation: Option<&str>,
) -> Result<Vec<u8>, PortableError> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let name = builder.create_string("standard");
    let analyzer = api::Analyzer::create(
        &mut builder,
        &api::AnalyzerArgs {
            name: Some(name),
            version: 1,
            options: None,
        },
    );
    let text = builder.create_string(request.text());
    let language = request
        .language()
        .map(|language| builder.create_string(language.as_str()));
    let request = api::AnalyzeRequest::create(
        &mut builder,
        &api::AnalyzeRequestArgs {
            target_type: api::AnalyzeTarget::Analyzer,
            target: Some(analyzer.as_union_value()),
            text: Some(text),
            language,
        },
    );
    Ok(finish_request(
        &mut builder,
        wire::RequestPayload::SearchKernel_V2_AnalyzeRequest,
        request.as_union_value(),
        logical_target,
        expected_index_generation,
    ))
}

pub fn encode_describe_request() -> Vec<u8> {
    encode_describe_request_for_target(None)
}

pub fn encode_describe_request_for_target(logical_target: Option<&str>) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let request = api::DescribeRequest::create(&mut builder, &api::DescribeRequestArgs {});
    finish_request(
        &mut builder,
        wire::RequestPayload::SearchKernel_V2_DescribeRequest,
        request.as_union_value(),
        logical_target,
        None,
    )
}

pub fn encode_describe_request_for_target_generation(logical_target: Option<&str>,
    expected_index_generation: Option<&str>,
) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let request = api::DescribeRequest::create(&mut builder, &api::DescribeRequestArgs {});
    finish_request(
        &mut builder,
        wire::RequestPayload::SearchKernel_V2_DescribeRequest,
        request.as_union_value(),
        logical_target,
        expected_index_generation,
    )
}

pub fn encode_create_index_request(
    target: &str,
    definition: &crate::IndexDefinitionInput,
) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let definition = super::schema::index_definition_input_offset(&mut builder, definition);
    let request = api::CreateIndexRequest::create(
        &mut builder,
        &api::CreateIndexRequestArgs {
            definition: Some(definition),
        },
    );
    finish_request(
        &mut builder,
        wire::RequestPayload::SearchKernel_V2_CreateIndexRequest,
        request.as_union_value(),
        Some(target),
        None,
    )
}

pub fn encode_open_index_request(target: &str) -> Vec<u8> {
    encode_lifecycle_empty_request(
        target,
        wire::RequestPayload::SearchKernel_V2_OpenIndexRequest,
        |builder| {
            api::OpenIndexRequest::create(builder, &api::OpenIndexRequestArgs {}).as_union_value()
        },
    )
}

pub fn encode_delete_index_request(target: &str) -> Vec<u8> {
    encode_lifecycle_empty_request(
        target,
        wire::RequestPayload::SearchKernel_V2_DeleteIndexRequest,
        |builder| {
            api::DeleteIndexRequest::create(builder, &api::DeleteIndexRequestArgs {})
                .as_union_value()
        },
    )
}

fn encode_lifecycle_empty_request(
    target: &str,
    payload_type: wire::RequestPayload,
    create: impl FnOnce(
        &mut flatbuffers::FlatBufferBuilder<'_>,
    ) -> flatbuffers::WIPOffset<flatbuffers::UnionWIPOffset>,
) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let payload = create(&mut builder);
    finish_request(&mut builder, payload_type, payload, Some(target), None)
}
