use super::*;

impl WireBuilder {
    pub(in crate::runner) fn value(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::Value<'static>>, String> {
        let object = input.as_object().ok_or("value must be an object")?;
        if object.get("_union").and_then(JsonValue::as_str) == Some("NONE") {
            return Ok(api::Value::create(
                &mut self.builder,
                &api::ValueArgs::default(),
            ));
        }
        let (kind_type, kind) = if let Some(value) = object.get("bool") {
            (
                api::ValueKind::BoolValue,
                api::BoolValue::create(
                    &mut self.builder,
                    &api::BoolValueArgs {
                        value: value.as_bool().ok_or("invalid bool")?,
                    },
                )
                .as_union_value(),
            )
        } else if let Some(value) = object.get("int64") {
            (
                api::ValueKind::Int64Value,
                api::Int64Value::create(
                    &mut self.builder,
                    &api::Int64ValueArgs {
                        value: value.as_i64().ok_or("invalid int64")?,
                    },
                )
                .as_union_value(),
            )
        } else if let Some(value) = object.get("uint64") {
            (
                api::ValueKind::UInt64Value,
                api::UInt64Value::create(
                    &mut self.builder,
                    &api::UInt64ValueArgs {
                        value: value.as_u64().ok_or("invalid uint64")?,
                    },
                )
                .as_union_value(),
            )
        } else if let Some(value) = object.get("float64") {
            (
                api::ValueKind::Float64Value,
                api::Float64Value::create(
                    &mut self.builder,
                    &api::Float64ValueArgs {
                        value: parse_float(value)?,
                    },
                )
                .as_union_value(),
            )
        } else if let Some(value) = object.get("string") {
            let value = self.text(value.as_str().ok_or("invalid string")?);
            (
                api::ValueKind::StringValue,
                api::StringValue::create(
                    &mut self.builder,
                    &api::StringValueArgs { value: Some(value) },
                )
                .as_union_value(),
            )
        } else if let Some(values) = object.get("list") {
            let values = values
                .as_array()
                .ok_or("invalid list")?
                .iter()
                .map(|v| self.value(v))
                .collect::<StdResult<Vec<_>, _>>()?;
            let values = self.builder.create_vector(&values);
            (
                api::ValueKind::ValueList,
                api::ValueList::create(
                    &mut self.builder,
                    &api::ValueListArgs {
                        values: Some(values),
                    },
                )
                .as_union_value(),
            )
        } else if object.contains_key("null") {
            (
                api::ValueKind::NullValueValue,
                api::NullValueValue::create(
                    &mut self.builder,
                    &api::NullValueValueArgs {
                        value: api::NullValue::Null,
                    },
                )
                .as_union_value(),
            )
        } else {
            return Err("unknown value kind".to_owned());
        };
        Ok(api::Value::create(
            &mut self.builder,
            &api::ValueArgs {
                kind_type,
                kind: Some(kind),
            },
        ))
    }

    pub(in crate::runner) fn entries(
        &mut self,
        input: Option<&JsonValue>,
    ) -> StdResult<
        flatbuffers::WIPOffset<
            flatbuffers::Vector<'static, flatbuffers::ForwardsUOffset<api::ValueEntry<'static>>>,
        >,
        String,
    > {
        let mut result = Vec::new();
        if let Some(fields) = input.and_then(JsonValue::as_object) {
            for (key, value) in fields {
                let key = self.text(key);
                let value = self.value(value)?;
                result.push(api::ValueEntry::create(
                    &mut self.builder,
                    &api::ValueEntryArgs {
                        key: Some(key),
                        value: Some(value),
                    },
                ));
            }
        }
        Ok(self.builder.create_vector(&result))
    }

    pub(in crate::runner) fn document(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::Document<'static>>, String> {
        let id = input
            .get("id")
            .and_then(JsonValue::as_str)
            .map(|v| self.text(v));
        let fields = self.entries(input.get("fields"))?;
        Ok(api::Document::create(
            &mut self.builder,
            &api::DocumentArgs {
                id,
                fields: Some(fields),
            },
        ))
    }
}
