use super::*;

pub(super) fn ok(value: JsonValue) -> JsonValue {
    json!({"ok": value})
}
pub(super) fn error_json(error: &PortableError) -> JsonValue {
    let bytes = error_to_api(error);
    let encoded = flatbuffers::root::<api::Error>(&bytes)
        .expect("production error encoder must emit a valid Error table");
    let details = encoded
        .details()
        .map(|values| {
            values
                .iter()
                .map(|detail| json!({"code":detail.code(),"path":detail.path()}))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({"code":api_error_code_name(encoded.code()),"details":details,"retryable":encoded.retryable()})
}
pub(super) fn api_error_code_name(code: api::ErrorCode) -> &'static str {
    match code {
        api::ErrorCode::InvalidArgument => "invalid_argument",
        api::ErrorCode::NotFound => "not_found",
        api::ErrorCode::AlreadyExists => "already_exists",
        api::ErrorCode::FailedPrecondition => "failed_precondition",
        api::ErrorCode::Conflict => "conflict",
        api::ErrorCode::Unsupported => "unsupported",
        api::ErrorCode::ResourceExhausted => "resource_exhausted",
        api::ErrorCode::Internal => "internal",
        api::ErrorCode::Unavailable => "unavailable",
        api::ErrorCode::Unspecified => "unspecified",
        _ => "unspecified",
    }
}
pub(super) fn parse_field_kind(value: &str) -> StdResult<FieldKind, String> {
    match value {
        "text" => Ok(FieldKind::Text),
        "keyword" => Ok(FieldKind::Keyword),
        "int64" => Ok(FieldKind::Int64),
        "float64" => Ok(FieldKind::Float64),
        "bool" => Ok(FieldKind::Bool),
        _ => Err(format!("unknown field kind {value}")),
    }
}
pub(super) fn scalar_json(value: &ScalarValueIr) -> JsonValue {
    match value {
        ScalarValueIr::Bool(v) => json!({"bool":v}),
        ScalarValueIr::Int64(v) => json!({"int64":v}),
        ScalarValueIr::Float64(v) => json!({"float64":v}),
        ScalarValueIr::String(v) => json!({"string":v}),
    }
}
pub(super) fn field_value_json(value: &FieldValueIr) -> JsonValue {
    match value {
        FieldValueIr::Single(v) => json!({"single":scalar_json(v)}),
        FieldValueIr::Multi(v) => json!({"multi":v.iter().map(scalar_json).collect::<Vec<_>>()}),
        FieldValueIr::Tensor(value) => {
            let payload = match &value.payload {
                searchkernel_ir::RequestTensorPayloadIr::Dense(values) => json!({"dense": values}),
                searchkernel_ir::RequestTensorPayloadIr::Sparse(values) => json!({
                    "sparse": values.iter().map(|cell| json!({
                        "address": cell.address,
                        "value": cell.value,
                    })).collect::<Vec<_>>()
                }),
                searchkernel_ir::RequestTensorPayloadIr::Mixed(values) => json!({
                    "mixed": values.iter().map(|block| json!({
                        "address": block.address,
                        "values": block.values,
                    })).collect::<Vec<_>>()
                }),
            };
            json!({"tensor":{"type_spec":value.type_spec,"payload":payload}})
        }
        FieldValueIr::Position {
            latitude,
            longitude,
        } => {
            json!({"position":{"latitude":latitude,"longitude":longitude}})
        }
        FieldValueIr::Predicate { features } => json!({"predicate":features}),
        FieldValueIr::PredicateExpression { expression } => {
            json!({"predicate_expression":expression})
        }
        FieldValueIr::Structured(values) => json!({
            "structs": values
                .iter()
                .map(|value| {
                    value
                        .iter()
                        .map(|(name, scalar)| (name.clone(), scalar_json(scalar)))
                        .collect::<serde_json::Map<_, _>>()
                })
                .collect::<Vec<_>>()
        }),
    }
}
pub(super) fn search_fields_json(values: &[SearchFieldIr]) -> JsonValue {
    json!(values
        .iter()
        .map(|v| json!({"field":v.field().as_str(),"weight":v.weight().get()}))
        .collect::<Vec<_>>())
}
pub(super) fn projection_json(value: &ProjectionIr) -> JsonValue {
    json!({"fields":value.fields().iter().map(|v|v.as_str()).collect::<Vec<_>>()})
}
pub(super) fn highlighting_json(value: &HighlightingIr) -> JsonValue {
    match value {
        HighlightingIr::Disabled => json!({"kind":"disabled"}),
        HighlightingIr::Requested { fields, .. } => {
            json!({"kind":"requested","fields":fields.iter().map(|v|v.as_str()).collect::<Vec<_>>()})
        }
    }
}
pub(super) fn analyzer_name(value: Option<AnalyzerIdentity>) -> Option<&'static str> {
    value.map(|value| match value {
        AnalyzerIdentity::StandardV1 => "standard_v1",
    })
}
pub(super) fn field_json(value: &FieldIr) -> JsonValue {
    searchkernel_ir::snapshot::field(value)
}
pub(super) fn index_json_value(value: &IndexDefinitionIr) -> JsonValue {
    searchkernel_ir::snapshot::index_definition(value)
}
pub(super) fn describe_json(value: &IndexDefinitionIr) -> JsonValue {
    json!({"index":searchkernel_ir::snapshot::index_definition(value)})
}
pub(super) fn inspect_json(value: &SearchRequestIr) -> JsonValue {
    json!({"resolved_request":searchkernel_ir::snapshot::search_request(value)})
}
pub(super) fn document_json(value: &DocumentIr) -> JsonValue {
    let fields: Map<String, JsonValue> = value
        .fields()
        .iter()
        .map(|(k, v)| (k.as_str().to_owned(), field_value_json(v)))
        .collect();
    json!({"id":value.id().as_str(),"fields":fields})
}
pub(super) fn insert_json(value: &InsertRequestIr) -> JsonValue {
    json!({"document":document_json(value.document())})
}
pub(super) fn upsert_json(value: &UpsertRequestIr) -> JsonValue {
    json!({"document":document_json(value.document()),"condition":value.condition()})
}
pub(super) fn delete_json(value: &DeleteRequestIr) -> JsonValue {
    json!({"id":value.id().as_str(),"condition":value.condition()})
}
pub(super) fn get_json(value: &GetRequestIr) -> JsonValue {
    json!({"id":value.id().as_str(),"projection":projection_json(value.projection())})
}
pub(super) fn patch_operation_json(value: &PatchOperationIr) -> JsonValue {
    match value {
        PatchOperationIr::Set { field, value } => {
            json!({"set":{"field":field.as_str(),"value":field_value_json(value)}})
        }
        PatchOperationIr::Unset { field } => json!({"unset":{"field":field.as_str()}}),
    }
}
pub(super) fn patch_json(value: &PatchRequestIr) -> JsonValue {
    json!({"id":value.id().as_str(),"operations":value.operations().iter().map(patch_operation_json).collect::<Vec<_>>()})
}
pub(super) fn update_operation_json(value: &UpdateOperationIr) -> JsonValue {
    match value {
        UpdateOperationIr::Assign { target, value } => {
            json!({"set":{"field":target.as_str(),"value":field_value_json(value)}})
        }
        UpdateOperationIr::Clear { target } => json!({"unset":{"field":target.as_str()}}),
        UpdateOperationIr::Arithmetic {
            target,
            operator,
            operand,
        } => {
            let name = match operator {
                UpdateArithmeticOperatorIr::Add | UpdateArithmeticOperatorIr::Increment => {
                    "increment"
                }
                UpdateArithmeticOperatorIr::Subtract | UpdateArithmeticOperatorIr::Decrement => {
                    "decrement"
                }
                UpdateArithmeticOperatorIr::Multiply => "multiply",
                UpdateArithmeticOperatorIr::Divide => "divide",
            };
            json!({name:{"field":target.as_str(),"value":operand}})
        }
    }
}
pub(super) fn update_json(value: &UpdateRequestIr) -> JsonValue {
    json!({
        "id": value.id().as_str(),
        "operations": value.operations().iter().map(update_operation_json).collect::<Vec<_>>(),
        "condition": value.condition(),
        "create": value.create(),
    })
}
pub(super) fn write_json(value: &WriteRequestIr) -> JsonValue {
    match value {
        WriteRequestIr::Insert(value) => json!({"insert":insert_json(value)}),
        WriteRequestIr::Upsert(value) => json!({"upsert":upsert_json(value)}),
        WriteRequestIr::Patch(value) => json!({"patch":patch_json(value)}),
        WriteRequestIr::Update(value) => json!({"update":update_json(value)}),
        WriteRequestIr::Delete(value) => json!({"delete":delete_json(value)}),
    }
}
pub(super) fn comparison_name(value: ComparisonOp) -> &'static str {
    match value {
        ComparisonOp::Eq => "eq",
        ComparisonOp::Ne => "ne",
        ComparisonOp::Lt => "lt",
        ComparisonOp::Lte => "lte",
        ComparisonOp::Gt => "gt",
        ComparisonOp::Gte => "gte",
    }
}
pub(super) fn comparison_json(value: &ComparisonIr) -> JsonValue {
    json!({"field":value.field().as_str(),"op":comparison_name(value.op()),"value":scalar_json(value.value())})
}
pub(super) fn range_json(value: &RangeIr) -> JsonValue {
    fn bound_json(value: &searchkernel_ir::FilterRangeBoundIr) -> JsonValue {
        match value {
            searchkernel_ir::FilterRangeBoundIr::Finite(value) => scalar_json(value),
            searchkernel_ir::FilterRangeBoundIr::NegativeInfinity => {
                json!("negative_infinity")
            }
            searchkernel_ir::FilterRangeBoundIr::PositiveInfinity => {
                json!("positive_infinity")
            }
            searchkernel_ir::FilterRangeBoundIr::Unbounded => JsonValue::Null,
        }
    }
    json!({"field":value.field().as_str(),"lower":bound_json(value.lower_bound()),"upper":bound_json(value.upper_bound()),"include_lower":value.include_lower(),"include_upper":value.include_upper()})
}
pub(super) fn filter_json(value: &FilterIr) -> JsonValue {
    searchkernel_ir::snapshot::filter(value)
}
pub(super) fn query_json(value: &QueryIr) -> JsonValue {
    searchkernel_ir::snapshot::query(value)
}

pub(super) fn sort_json(value: &SortIr) -> JsonValue {
    searchkernel_ir::snapshot::sort(value)
}
pub(super) fn facet_from_json(
    input: &JsonValue,
    context: &IndexContext,
) -> StdResult<JsonValue, PortableError> {
    let (facet, json) = facet_spec_from_json(input, "facet")?;
    resolve_facets(context, vec![facet])?;
    Ok(json)
}

pub(super) fn facet_spec_from_json(
    input: &JsonValue,
    path: &str,
) -> StdResult<(FacetInput, JsonValue), PortableError> {
    let field_name = input
        .get("field")
        .and_then(JsonValue::as_str)
        .unwrap_or_default();
    let (sort_name, sort) = match input.get("sort").and_then(JsonValue::as_str) {
        None | Some("count_desc") => ("count_desc", Some(FacetSortInput::CountDesc)),
        Some("count_asc") => ("count_asc", Some(FacetSortInput::CountAsc)),
        Some("value_asc") => ("value_asc", Some(FacetSortInput::ValueAsc)),
        Some("value_desc") => ("value_desc", Some(FacetSortInput::ValueDesc)),
        Some(_) => {
            return Err(PortableError::invalid(
                "invalid_facet",
                Some(format!("{path}.sort")),
                "facet sort is unknown",
            ))
        }
    };
    let limit = input.get("limit").and_then(JsonValue::as_u64).unwrap_or(10);
    let limit = u32::try_from(limit).map_err(|_| {
        PortableError::invalid(
            "invalid_facet",
            Some(format!("{path}.limit")),
            "facet limit is out of range",
        )
    })?;
    Ok((
        FacetInput {
            field: field_name.to_owned(),
            name: None,
            limit: Some(limit),
            sort,
        },
        json!({"field": field_name, "limit": limit, "sort": sort_name}),
    ))
}
pub(super) fn analyze_json(value: &AnalyzeRequestIr) -> JsonValue {
    json!({"analyzer":analyzer_name(Some(value.analyzer())),"text":value.text(),"language":value.language().map(LanguageHint::as_str)})
}
pub(super) fn search_json(value: &SearchRequestIr) -> JsonValue {
    searchkernel_ir::snapshot::search_request(value)
}

pub(super) fn parse_float(value: &JsonValue) -> StdResult<f64, String> {
    match value {
        JsonValue::Number(value) => value.as_f64().ok_or("invalid float64".to_owned()),
        JsonValue::String(value) if value == "NaN" => Ok(f64::NAN),
        JsonValue::String(value) if value == "Infinity" => Ok(f64::INFINITY),
        JsonValue::String(value) if value == "-Infinity" => Ok(f64::NEG_INFINITY),
        _ => Err("invalid float64".to_owned()),
    }
}
