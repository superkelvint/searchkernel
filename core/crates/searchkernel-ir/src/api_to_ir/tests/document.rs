use super::*;

#[test]
fn update_resolution_preserves_operations_create_and_condition() {
    let request = resolve_update(
        &context(),
        "doc:1",
        &[
            UpdateOperationInput::Assign {
                target: "price".to_owned(),
                value: RequestValueIr::Int64(12),
            },
            UpdateOperationInput::Arithmetic {
                target: "rating".to_owned(),
                operator: UpdateArithmeticOperatorIr::Increment,
                operand: RequestValueIr::Float64(3.0),
            },
        ],
        Some("price == 9"),
        Some(true),
    )
    .expect("valid update resolves");

    assert_eq!(request.id().as_str(), "doc:1");
    assert_eq!(request.operations().len(), 2);
    assert_eq!(request.condition(), Some("price == 9"));
    assert_eq!(request.create(), Some(true));
}

#[test]
fn update_resolution_enforces_target_value_and_arithmetic_policy() {
    fn detail_code(id: &str, operations: &[UpdateOperationInput]) -> String {
        resolve_update(&context(), id, operations, None, None)
            .expect_err("update should be rejected")
            .details[0]
            .code
            .clone()
    }

    assert_eq!(detail_code("doc:1", &[]), "empty_update");
    assert_eq!(
        detail_code(
            "",
            &[UpdateOperationInput::Clear {
                target: "price".to_owned(),
            }],
        ),
        "invalid_document_id"
    );
    assert_eq!(
        detail_code(
            "doc:1",
            &[UpdateOperationInput::Assign {
                target: String::new(),
                value: RequestValueIr::Int64(1),
            }],
        ),
        "invalid_update_target"
    );
    assert_eq!(
        detail_code(
            "doc:1",
            &[UpdateOperationInput::Assign {
                target: "price.value".to_owned(),
                value: RequestValueIr::Int64(1),
            }],
        ),
        "update_field_path_not_supported"
    );
    assert_eq!(
        detail_code(
            "doc:1",
            &[UpdateOperationInput::Assign {
                target: "missing".to_owned(),
                value: RequestValueIr::Int64(1),
            }],
        ),
        "unknown_field"
    );
    assert_eq!(
        detail_code(
            "doc:1",
            &[UpdateOperationInput::Assign {
                target: "price".to_owned(),
                value: RequestValueIr::String("not an integer".to_owned()),
            }],
        ),
        "type_mismatch"
    );
    assert_eq!(
        detail_code(
            "doc:1",
            &[UpdateOperationInput::Arithmetic {
                target: "brand".to_owned(),
                operator: UpdateArithmeticOperatorIr::Increment,
                operand: RequestValueIr::Int64(1),
            }],
        ),
        "arithmetic_requires_numeric_field"
    );
    assert_eq!(
        detail_code(
            "doc:1",
            &[UpdateOperationInput::Arithmetic {
                target: "rating".to_owned(),
                operator: UpdateArithmeticOperatorIr::Divide,
                operand: RequestValueIr::Float64(f64::NAN),
            }],
        ),
        "arithmetic_operand_type"
    );
    let duplicate = resolve_update(
        &context(),
        "doc:1",
        &[
            UpdateOperationInput::Assign {
                target: "price".to_owned(),
                value: RequestValueIr::Int64(1),
            },
            UpdateOperationInput::Clear {
                target: "price".to_owned(),
            },
        ],
        None,
        None,
    )
    .expect_err("multiple updates for one target are unsupported");
    assert_eq!(duplicate.code, ErrorCode::Unsupported);
    assert_eq!(
        duplicate.details[0].code,
        "multiple_updates_per_target_not_supported"
    );

    for operator in [
        UpdateArithmeticOperatorIr::Increment,
        UpdateArithmeticOperatorIr::Decrement,
        UpdateArithmeticOperatorIr::Multiply,
        UpdateArithmeticOperatorIr::Divide,
    ] {
        let request = resolve_update(
            &context(),
            "doc:1",
            &[UpdateOperationInput::Arithmetic {
                target: "rating".to_owned(),
                operator,
                operand: RequestValueIr::Float64(2.0),
            }],
            None,
            None,
        )
        .expect("finite arithmetic update resolves");
        assert_eq!(request.operations().len(), 1);
        assert!(matches!(
            request.operations()[0],
            UpdateOperationIr::Arithmetic { operator: actual, operand, .. }
                if actual == operator && operand == 2.0
        ));
    }
}

#[test]
fn duplicate_document_fields_are_rejected_for_insert_and_upsert() {
    let duplicate_document = |builder: &mut crate::__flatbuffers::FlatBufferBuilder<'static>| {
        let fields = [
            value_entry(
                builder,
                "brand",
                &RequestValueIr::String("first".to_owned()),
            ),
            value_entry(
                builder,
                "brand",
                &RequestValueIr::String("second".to_owned()),
            ),
        ];
        let fields = builder.create_vector(&fields);
        let id = builder.create_string("doc-duplicate");
        api::Document::create(
            builder,
            &api::DocumentArgs {
                id: Some(id),
                fields: Some(fields),
            },
        )
    };

    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let document = duplicate_document(&mut builder);
    let request = api::InsertRequest::create(
        &mut builder,
        &api::InsertRequestArgs {
            document: Some(document),
        },
    );
    builder.finish(request, None);
    let request = crate::__flatbuffers::root::<api::InsertRequest>(builder.finished_data())
        .expect("valid insert request");
    let error = context()
        .insert_to_ir(request)
        .expect_err("duplicate insert fields must be rejected");
    assert_eq!(error.details[0].code, "duplicate_document_field");

    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let document = duplicate_document(&mut builder);
    let request = api::UpsertRequest::create(
        &mut builder,
        &api::UpsertRequestArgs {
            document: Some(document),
            condition: None,
        },
    );
    builder.finish(request, None);
    let request = crate::__flatbuffers::root::<api::UpsertRequest>(builder.finished_data())
        .expect("valid upsert request");
    let error = context()
        .upsert_to_ir(request)
        .expect_err("duplicate upsert fields must be rejected");
    assert_eq!(error.details[0].code, "duplicate_document_field");
}

#[test]
fn canonical_schema_resolves_document_identity_without_changing_public_intent() {
    let mut unnamed = parity_schema();
    unnamed.name = None;
    let unnamed = resolve_rust_index_definition(unnamed).expect("valid unnamed schema");
    assert_eq!(unnamed.document_type(), None);
    assert_eq!(unnamed.resolved_document_type(), "searchkernel");

    let named = resolve_rust_index_definition(parity_schema()).expect("valid named schema");
    assert_eq!(named.document_type(), None);
    assert_eq!(named.resolved_document_type(), "catalog");
}

#[test]
fn tensor_patch_resolution_preserves_tensor_values_and_rejects_scalars() {
    let definition = IndexDefinitionIr::new(
        None,
        vec![FieldIr::new(
            FieldName::parse("embedding").unwrap(),
            DataTypeIr::Tensor {
                type_spec: "tensor<float>(x[2])".to_owned(),
            },
            None,
            FieldCapabilities::new(false, false, true, false, false),
        )],
        Vec::new(),
    )
    .unwrap();
    let context = IndexContext::from_definition_ir(definition);
    let tensor = RequestValueIr::Tensor(RequestTensorIr {
        type_spec: "tensor<float>(x[2])".to_owned(),
        payload: RequestTensorPayloadIr::Dense(vec![1.0, 0.0]),
    });
    let patch = resolve_patch(
        &context,
        "doc-1",
        &[PatchOperationInput::Set {
            field: "embedding".to_owned(),
            value: tensor.clone(),
        }],
    )
    .expect("dense tensor patch resolves");
    assert_eq!(
        patch.operations(),
        &[PatchOperationIr::Set {
            field: FieldName::parse("embedding").unwrap(),
            value: FieldValueIr::Tensor(match tensor {
                RequestValueIr::Tensor(value) => value,
                _ => unreachable!(),
            }),
        }]
    );

    let error = resolve_patch(
        &context,
        "doc-1",
        &[PatchOperationInput::Set {
            field: "embedding".to_owned(),
            value: RequestValueIr::Float64(1.0),
        }],
    )
    .expect_err("scalar tensor patch unexpectedly resolved");
    assert_eq!(error.details[0].code, "type_mismatch");
}
