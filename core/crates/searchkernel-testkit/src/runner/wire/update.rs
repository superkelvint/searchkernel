use super::*;

impl WireBuilder {
    pub(in crate::runner) fn patch_operation(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::PatchOperation<'static>>, String> {
        let object = input
            .as_object()
            .ok_or("patch operation must be an object")?;
        if object.get("_union").and_then(JsonValue::as_str) == Some("NONE") {
            return Ok(api::PatchOperation::create(
                &mut self.builder,
                &api::PatchOperationArgs::default(),
            ));
        }
        if let Some(value) = object.get("set") {
            let field = value
                .get("field")
                .and_then(JsonValue::as_str)
                .map(|v| self.text(v));
            let value = value.get("value").map(|v| self.value(v)).transpose()?;
            let set = api::SetField::create(&mut self.builder, &api::SetFieldArgs { field, value });
            Ok(api::PatchOperation::create(
                &mut self.builder,
                &api::PatchOperationArgs {
                    operation_type: api::PatchOperationKind::SetField,
                    operation: Some(set.as_union_value()),
                },
            ))
        } else if let Some(value) = object.get("unset") {
            let field = value
                .get("field")
                .and_then(JsonValue::as_str)
                .map(|v| self.text(v));
            let unset = api::UnsetField::create(&mut self.builder, &api::UnsetFieldArgs { field });
            Ok(api::PatchOperation::create(
                &mut self.builder,
                &api::PatchOperationArgs {
                    operation_type: api::PatchOperationKind::UnsetField,
                    operation: Some(unset.as_union_value()),
                },
            ))
        } else {
            Ok(api::PatchOperation::create(
                &mut self.builder,
                &api::PatchOperationArgs::default(),
            ))
        }
    }

    pub(in crate::runner) fn update_request(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::UpdateRequest<'static>>, String> {
        let id = input
            .get("id")
            .and_then(JsonValue::as_str)
            .map(|value| self.text(value));
        let operations = input
            .get("operations")
            .and_then(JsonValue::as_array)
            .map(|values| {
                values
                    .iter()
                    .map(|value| self.update_operation(value))
                    .collect::<StdResult<Vec<_>, _>>()
            })
            .transpose()?
            .map(|values| self.builder.create_vector(&values));
        let condition = input
            .get("condition")
            .and_then(JsonValue::as_str)
            .map(|value| self.text(value));
        let create = input
            .get("create")
            .and_then(JsonValue::as_bool)
            .map(api::OptionalBool::new);
        Ok(api::UpdateRequest::create(
            &mut self.builder,
            &api::UpdateRequestArgs {
                id,
                operations,
                condition,
                create: create.as_ref(),
            },
        ))
    }

    pub(in crate::runner) fn update_operation(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::UpdateOperation<'static>>, String> {
        let object = input
            .as_object()
            .ok_or("update operation must be an object")?;
        if object.get("_union").and_then(JsonValue::as_str) == Some("NONE") {
            return Ok(api::UpdateOperation::create(
                &mut self.builder,
                &api::UpdateOperationArgs::default(),
            ));
        }
        let (target, operation_type, operation) = if let Some(value) = object.get("set") {
            let target = value
                .get("field")
                .and_then(JsonValue::as_str)
                .map(|value| self.field_path(value));
            let payload = value
                .get("value")
                .map(|value| self.value(value))
                .transpose()?;
            let assign = api::AssignUpdate::create(
                &mut self.builder,
                &api::AssignUpdateArgs { value: payload },
            );
            (
                target,
                api::UpdateOperationKind::AssignUpdate,
                assign.as_union_value(),
            )
        } else if let Some(value) = object.get("unset") {
            let target = value
                .get("field")
                .and_then(JsonValue::as_str)
                .map(|value| self.field_path(value));
            let clear = api::ClearUpdate::create(&mut self.builder, &api::ClearUpdateArgs {});
            (
                target,
                api::UpdateOperationKind::ClearUpdate,
                clear.as_union_value(),
            )
        } else if let Some(value) = object.get("add") {
            let target = value
                .get("field")
                .and_then(JsonValue::as_str)
                .map(|value| self.field_path(value));
            let payload = value
                .get("value")
                .map(|value| self.value(value))
                .transpose()?;
            let add =
                api::AddUpdate::create(&mut self.builder, &api::AddUpdateArgs { value: payload });
            (
                target,
                api::UpdateOperationKind::AddUpdate,
                add.as_union_value(),
            )
        } else if let Some(value) = object.get("remove") {
            let target = value
                .get("field")
                .and_then(JsonValue::as_str)
                .map(|value| self.field_path(value));
            let payload = value
                .get("value")
                .map(|value| self.value(value))
                .transpose()?;
            let remove = api::RemoveUpdate::create(
                &mut self.builder,
                &api::RemoveUpdateArgs { value: payload },
            );
            (
                target,
                api::UpdateOperationKind::RemoveUpdate,
                remove.as_union_value(),
            )
        } else if let Some(value) = object.get("match") {
            let target = value
                .get("field")
                .and_then(JsonValue::as_str)
                .map(|value| self.field_path(value));
            let payload =
                api::MatchUpdate::create(&mut self.builder, &api::MatchUpdateArgs::default());
            (
                target,
                api::UpdateOperationKind::MatchUpdate,
                payload.as_union_value(),
            )
        } else if let Some(value) = object.get("tensor_modify") {
            let target = value
                .get("field")
                .and_then(JsonValue::as_str)
                .map(|value| self.field_path(value));
            let payload = api::TensorModifyUpdate::create(
                &mut self.builder,
                &api::TensorModifyUpdateArgs::default(),
            );
            (
                target,
                api::UpdateOperationKind::TensorModifyUpdate,
                payload.as_union_value(),
            )
        } else {
            let (name, operator) = [
                ("increment", api::ArithmeticUpdateOperator::Increment),
                ("decrement", api::ArithmeticUpdateOperator::Decrement),
                ("multiply", api::ArithmeticUpdateOperator::Multiply),
                ("divide", api::ArithmeticUpdateOperator::Divide),
            ]
            .iter()
            .find_map(|(name, operator)| object.get(*name).map(|_| (*name, *operator)))
            .ok_or("unknown update operation")?;
            let value = object.get(name).ok_or("missing arithmetic update")?;
            let target = value
                .get("field")
                .and_then(JsonValue::as_str)
                .map(|value| self.field_path(value));
            let operand = value
                .get("value")
                .map(|value| self.value(value))
                .transpose()?;
            let arithmetic = api::ArithmeticUpdate::create(
                &mut self.builder,
                &api::ArithmeticUpdateArgs { operator, operand },
            );
            (
                target,
                api::UpdateOperationKind::ArithmeticUpdate,
                arithmetic.as_union_value(),
            )
        };
        Ok(api::UpdateOperation::create(
            &mut self.builder,
            &api::UpdateOperationArgs {
                target,
                operation_type,
                operation: Some(operation),
            },
        ))
    }

    pub(in crate::runner) fn field_path(
        &mut self,
        value: &str,
    ) -> flatbuffers::WIPOffset<api::FieldPath<'static>> {
        let path = self.text(value);
        api::FieldPath::create(&mut self.builder, &api::FieldPathArgs { path: Some(path) })
    }
}
