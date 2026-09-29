use super::*;

impl WireBuilder {
    pub(in crate::runner) fn analyzer(
        &mut self,
        input: Option<&JsonValue>,
    ) -> StdResult<flatbuffers::WIPOffset<api::Analyzer<'static>>, String> {
        let input = input.unwrap_or(&JsonValue::Null);
        let name = input
            .get("name")
            .and_then(JsonValue::as_str)
            .map(|v| self.text(v));
        let options = self.entries(input.get("options"))?;
        Ok(api::Analyzer::create(
            &mut self.builder,
            &api::AnalyzerArgs {
                name,
                version: input
                    .get("version")
                    .and_then(JsonValue::as_u64)
                    .unwrap_or(0) as u32,
                options: Some(options),
            },
        ))
    }

    pub(in crate::runner) fn field(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::FieldDefinition<'static>>, String> {
        let kind = input
            .get("kind")
            .and_then(JsonValue::as_str)
            .unwrap_or("string");
        let matching_mode = match kind {
            "keyword" => api::MatchingMode::Exact,
            _ => api::MatchingMode::Text,
        };
        let kind = match kind {
            "int64" => api::PrimitiveDataTypeKind::Int64,
            "float64" => api::PrimitiveDataTypeKind::Float64,
            "bool" => api::PrimitiveDataTypeKind::Bool,
            _ => api::PrimitiveDataTypeKind::String,
        };
        let is_string = matches!(kind, api::PrimitiveDataTypeKind::String);
        let primitive =
            api::PrimitiveDataType::create(&mut self.builder, &api::PrimitiveDataTypeArgs { kind });
        let element_type = api::DataType::create(
            &mut self.builder,
            &api::DataTypeArgs {
                kind_type: api::DataTypeKind::PrimitiveDataType,
                kind: Some(primitive.as_union_value()),
            },
        );
        let data_type = if input.get("cardinality").and_then(JsonValue::as_str) == Some("multi") {
            let array = api::ArrayDataType::create(
                &mut self.builder,
                &api::ArrayDataTypeArgs {
                    element_type: Some(element_type),
                },
            );
            api::DataType::create(
                &mut self.builder,
                &api::DataTypeArgs {
                    kind_type: api::DataTypeKind::ArrayDataType,
                    kind: Some(array.as_union_value()),
                },
            )
        } else {
            element_type
        };
        let matching = is_string.then(|| {
            api::MatchingConfig::create(
                &mut self.builder,
                &api::MatchingConfigArgs {
                    mode: matching_mode,
                    options: None,
                },
            )
        });
        let stored = input
            .get("stored")
            .and_then(JsonValue::as_bool)
            .map(api::OptionalBool::new);
        let searchable = input
            .get("searchable")
            .and_then(JsonValue::as_bool)
            .map(api::OptionalBool::new);
        let filterable = input
            .get("filterable")
            .and_then(JsonValue::as_bool)
            .map(api::OptionalBool::new);
        let sortable = input
            .get("sortable")
            .and_then(JsonValue::as_bool)
            .map(api::OptionalBool::new);
        let facetable = input
            .get("facetable")
            .and_then(JsonValue::as_bool)
            .map(api::OptionalBool::new);
        let analyzer = input
            .get("analyzer")
            .map(|v| self.analyzer(Some(v)))
            .transpose()?;
        let indexing_script = input
            .get("indexing_script")
            .and_then(JsonValue::as_str)
            .map(|value| self.text(value));
        let properties = input
            .get("properties")
            .map(|value| self.entries(Some(value)))
            .transpose()?;
        let name = input
            .get("name")
            .and_then(JsonValue::as_str)
            .map(|v| self.text(v));
        Ok(api::FieldDefinition::create(
            &mut self.builder,
            &api::FieldDefinitionArgs {
                name,
                data_type: Some(data_type),
                stored: stored.as_ref(),
                searchable: searchable.as_ref(),
                filterable: filterable.as_ref(),
                sortable: sortable.as_ref(),
                facetable: facetable.as_ref(),
                analyzer,
                matching,
                indexing_script,
                properties,
                ..Default::default()
            },
        ))
    }

    pub(in crate::runner) fn selection(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::SearchFieldSelection<'static>>, String> {
        let fields = input
            .get("fields")
            .and_then(JsonValue::as_array)
            .map(|values| {
                values
                    .iter()
                    .map(|v| self.search_field(v))
                    .collect::<StdResult<Vec<_>, _>>()
            })
            .transpose()?;
        let fields = fields.map(|values| self.builder.create_vector(&values));
        Ok(api::SearchFieldSelection::create(
            &mut self.builder,
            &api::SearchFieldSelectionArgs { fields },
        ))
    }

    pub(in crate::runner) fn index(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<flatbuffers::WIPOffset<api::IndexDefinition<'static>>, String> {
        let fields = input
            .get("fields")
            .and_then(JsonValue::as_array)
            .map(|values| {
                values
                    .iter()
                    .map(|v| self.field(v))
                    .collect::<StdResult<Vec<_>, _>>()
            })
            .transpose()?;
        let fields = fields.map(|values| self.builder.create_vector(&values));
        let default_search_fields = input
            .get("default_search_fields")
            .map(|v| self.selection(v))
            .transpose()?;
        let name = input
            .get("name")
            .and_then(JsonValue::as_str)
            .map(|v| self.text(v));
        let properties = input
            .get("properties")
            .map(|value| self.entries(Some(value)))
            .transpose()?;
        let document_type = input
            .get("document_type")
            .and_then(JsonValue::as_str)
            .map(|value| self.text(value));
        let schema_inherits = input
            .get("schema_inherits")
            .map(|value| self.string_vector(value))
            .transpose()?;
        let document_inherits = input
            .get("document_inherits")
            .map(|value| self.string_vector(value))
            .transpose()?;
        let schema_fields = input
            .get("schema_fields")
            .map(|value| self.schema_fields(value))
            .transpose()?
            .map(|value| self.builder.create_vector(&value));
        let fieldsets = input
            .get("fieldsets")
            .map(|value| self.fieldsets(value))
            .transpose()?
            .map(|value| self.builder.create_vector(&value));
        Ok(api::IndexDefinition::create(
            &mut self.builder,
            &api::IndexDefinitionArgs {
                name,
                fields,
                default_search_fields,
                properties,
                document_type,
                schema_inherits,
                document_inherits,
                fieldsets,
                schema_fields,
                ..Default::default()
            },
        ))
    }

    pub(in crate::runner) fn schema_fields(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<Vec<flatbuffers::WIPOffset<api::SearchOnlyFieldDefinition<'static>>>, String>
    {
        let values = input.as_array().ok_or("schema_fields must be an array")?;
        values
            .iter()
            .map(|value| {
                let field = value.get("field").unwrap_or(value);
                let field = self.field(field)?;
                Ok(api::SearchOnlyFieldDefinition::create(
                    &mut self.builder,
                    &api::SearchOnlyFieldDefinitionArgs { field: Some(field) },
                ))
            })
            .collect()
    }

    pub(in crate::runner) fn fieldsets(
        &mut self,
        input: &JsonValue,
    ) -> StdResult<Vec<flatbuffers::WIPOffset<api::FieldSetDefinition<'static>>>, String> {
        let values = input.as_array().ok_or("fieldsets must be an array")?;
        values
            .iter()
            .map(|value| {
                let value = value.as_object().ok_or("fieldset must be an object")?;
                let name = value
                    .get("name")
                    .and_then(JsonValue::as_str)
                    .ok_or("fieldset name is required")?;
                let fields = value
                    .get("fields")
                    .map(|value| self.string_vector(value))
                    .transpose()?;
                let query_commands = value
                    .get("query_commands")
                    .map(|value| self.string_vector(value))
                    .transpose()?;
                let properties = value
                    .get("properties")
                    .map(|value| self.entries(Some(value)))
                    .transpose()?;
                let name = self.text(name);
                Ok(api::FieldSetDefinition::create(
                    &mut self.builder,
                    &api::FieldSetDefinitionArgs {
                        name: Some(name),
                        fields,
                        matching: None,
                        query_commands,
                        properties,
                    },
                ))
            })
            .collect()
    }
}
