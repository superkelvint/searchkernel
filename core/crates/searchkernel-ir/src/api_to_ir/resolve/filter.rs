use super::*;

pub(super) fn resolve_filters(
    context: &IndexContext,
    input: &SearchInput,
) -> Result<Option<FilterIr>> {
    let mut clauses = input
        .filters
        .iter()
        .enumerate()
        .map(|(index, filter)| {
            resolve_filter(context, filter, &format!("filters.structured[{index}]"))
        })
        .collect::<Result<Vec<_>>>()?;
    for (index, textual) in input.textual_filters.iter().enumerate() {
        let parsed = searchkernel_filter::parse_filter(&textual.source)
            .map_err(crate::filter_expression::parse_error_to_portable)
            .map_err(|error| {
                crate::api_to_ir::filter::decorate_filter_clause_error(
                    error,
                    index,
                    &format!("filters.textual[{index}]"),
                )
            })?;
        let bound = bind_parsed_filter(&parsed, context.definition(), &textual.bindings).map_err(
            |error| {
                crate::api_to_ir::filter::decorate_filter_clause_error(
                    error,
                    index,
                    &format!("filters.textual[{index}]"),
                )
            },
        )?;
        clauses.push(bound.expression().clone());
    }
    Ok(match clauses.len() {
        0 => None,
        1 => clauses.into_iter().next(),
        _ => Some(flatten_and(clauses)),
    })
}

fn filter_operation_path(path: &str, operation: &str) -> String {
    if path == operation || path.ends_with(&format!(".{operation}")) {
        path.to_owned()
    } else {
        format!("{path}.{operation}")
    }
}

pub(crate) fn resolve_filter(
    context: &IndexContext,
    input: &FilterInput,
    path: &str,
) -> Result<FilterIr> {
    match input {
        FilterInput::All(filters) => {
            if filters.is_empty() {
                return Err(invalid(
                    "empty_boolean_filter",
                    Some(format!("{path}.all")),
                    "all requires at least one clause",
                ));
            }
            Ok(flatten_and(
                filters
                    .iter()
                    .enumerate()
                    .map(|(index, filter)| {
                        resolve_filter(context, filter, &format!("{path}.all[{index}]"))
                    })
                    .collect::<Result<Vec<_>>>()?,
            ))
        }
        FilterInput::Any(filters) => {
            if filters.is_empty() {
                return Err(invalid(
                    "empty_boolean_filter",
                    Some(format!("{path}.any")),
                    "any requires at least one clause",
                ));
            }
            Ok(flatten_or(
                filters
                    .iter()
                    .enumerate()
                    .map(|(index, filter)| {
                        resolve_filter(context, filter, &format!("{path}.any[{index}]"))
                    })
                    .collect::<Result<Vec<_>>>()?,
            ))
        }
        FilterInput::Not(filter) => Ok(FilterIr::Not(Box::new(resolve_filter(
            context,
            filter,
            &format!("{path}.negation.clause"),
        )?))),
        FilterInput::Exists { field } => {
            let exists_path = filter_operation_path(path, "exists");
            let field = context.query_field_for_resolver(field, format!("{exists_path}.field"))?;
            if !field.capabilities().filterable() {
                return Err(invalid(
                    "field_not_filterable",
                    Some(format!("{exists_path}.field")),
                    "exists requires a filterable field",
                ));
            }
            Ok(FilterIr::Exists {
                field: field.name().clone(),
            })
        }
        FilterInput::Comparison { field, op, value } => {
            let comparison_path = filter_operation_path(path, "comparison");
            let schema_field =
                context.query_field_for_resolver(field, format!("{comparison_path}.field"))?;
            if !schema_field.capabilities().filterable() {
                return Err(invalid(
                    "field_not_filterable",
                    Some(format!("{comparison_path}.field")),
                    "comparison requires a filterable field",
                ));
            }
            if matches!(
                op,
                crate::model::ComparisonOp::Lt
                    | crate::model::ComparisonOp::Lte
                    | crate::model::ComparisonOp::Gt
                    | crate::model::ComparisonOp::Gte
            ) && !matches!(schema_field.kind(), FieldKind::Int64 | FieldKind::Float64)
            {
                return Err(invalid(
                    "invalid_filter",
                    Some(format!("{comparison_path}.op")),
                    "ordered comparison requires a numeric field",
                ));
            }
            Ok(FilterIr::Comparison(crate::model::ComparisonIr {
                field: schema_field.name().clone(),
                op: *op,
                value: resolve_scalar(
                    value.clone(),
                    schema_field.kind(),
                    format!("{comparison_path}.value"),
                )?,
            }))
        }
        FilterInput::In { field, values } => {
            let in_path = filter_operation_path(path, "in");
            let schema_field =
                context.query_field_for_resolver(field, format!("{in_path}.field"))?;
            if !schema_field.capabilities().filterable() {
                return Err(invalid(
                    "field_not_filterable",
                    Some(format!("{in_path}.field")),
                    "IN requires a filterable field",
                ));
            }
            if !matches!(schema_field.kind(), FieldKind::Keyword | FieldKind::Int64) {
                return Err(invalid(
                    "invalid_filter",
                    Some(&in_path),
                    "IN requires a Keyword or Int64 field",
                ));
            }
            let values = values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    resolve_scalar(
                        value.clone(),
                        schema_field.kind(),
                        format!("{in_path}.values[{index}]"),
                    )
                })
                .collect::<Result<Vec<_>>>()?;
            if values.is_empty() {
                return Err(invalid(
                    "empty_in",
                    Some(&in_path),
                    "IN requires at least one value",
                ));
            }
            Ok(FilterIr::In {
                field: schema_field.name().clone(),
                values,
            })
        }
        FilterInput::Range {
            field,
            lower,
            upper,
            include_lower,
            include_upper,
        } => {
            // Structured FilterExpr paths name the selected union operation
            // (for example `filter.range`). Leaf range conversion already uses
            // `range` as its root, so avoid duplicating the operation segment.
            let range_path = filter_operation_path(path, "range");
            let schema_field =
                context.query_field_for_resolver(field, format!("{range_path}.field"))?;
            if !schema_field.capabilities().filterable() {
                return Err(invalid(
                    "field_not_filterable",
                    Some(format!("{range_path}.field")),
                    "range requires a filterable field",
                ));
            }
            if !matches!(schema_field.kind(), FieldKind::Int64 | FieldKind::Float64) {
                return Err(invalid(
                    "invalid_filter",
                    Some(&range_path),
                    "range requires an Int64 or Float64 field",
                ));
            }
            let lower =
                resolve_range_bound(lower, schema_field.kind(), &format!("{range_path}.lower"))?;
            let upper =
                resolve_range_bound(upper, schema_field.kind(), &format!("{range_path}.upper"))?;
            if matches!(lower, crate::model::FilterRangeBoundIr::Unbounded)
                && matches!(upper, crate::model::FilterRangeBoundIr::Unbounded)
            {
                return Err(invalid(
                    "invalid_range",
                    Some(&range_path),
                    "range requires at least one bound",
                ));
            }
            let explicit_lower_inclusion = *include_lower == Some(true);
            let explicit_upper_inclusion = *include_upper == Some(true);
            let include_lower = (*include_lower).unwrap_or(true);
            let include_upper = (*include_upper).unwrap_or(true);
            if matches!(lower, crate::model::FilterRangeBoundIr::Unbounded)
                && include_lower
                && explicit_lower_inclusion
            {
                return Err(invalid(
                    "invalid_range",
                    Some(format!("{range_path}.include_lower")),
                    "lower inclusion requires a lower bound",
                ));
            }
            if matches!(upper, crate::model::FilterRangeBoundIr::Unbounded)
                && include_upper
                && explicit_upper_inclusion
            {
                return Err(invalid(
                    "invalid_range",
                    Some(format!("{range_path}.include_upper")),
                    "upper inclusion requires an upper bound",
                ));
            }
            if crate::api_to_ir::filter::filter_bound_greater(&lower, &upper) {
                return Err(invalid(
                    "invalid_range",
                    Some(&range_path),
                    "range lower bound must not exceed upper bound",
                ));
            }
            Ok(FilterIr::Range(crate::model::RangeIr {
                field: schema_field.name().clone(),
                lower,
                upper,
                include_lower,
                include_upper,
            }))
        }
    }
}

fn resolve_scalar(value: RequestValueIr, kind: FieldKind, path: String) -> Result<ScalarValueIr> {
    let scalar = match value {
        RequestValueIr::Bool(value) => ScalarValueIr::Bool(value),
        RequestValueIr::Int64(value) => ScalarValueIr::Int64(value),
        RequestValueIr::Float64(value) if value.is_finite() => ScalarValueIr::Float64(value),
        RequestValueIr::String(value) => ScalarValueIr::String(value),
        _ => {
            return Err(invalid(
                "type_mismatch",
                Some(path),
                "filter value must be scalar",
            ))
        }
    };
    let valid = matches!(
        (&scalar, kind),
        (
            ScalarValueIr::String(_),
            FieldKind::Text | FieldKind::Keyword
        ) | (ScalarValueIr::Int64(_), FieldKind::Int64)
            | (ScalarValueIr::Float64(_), FieldKind::Float64)
            | (ScalarValueIr::Bool(_), FieldKind::Bool)
    );
    if !valid {
        return Err(invalid(
            "type_mismatch",
            Some(path),
            "filter value type does not match field kind",
        ));
    }
    Ok(scalar)
}

fn resolve_range_bound(
    value: &FilterBoundInput,
    kind: FieldKind,
    path: &str,
) -> Result<crate::model::FilterRangeBoundIr> {
    match value {
        FilterBoundInput::Finite(value) => resolve_scalar(value.clone(), kind, path.to_owned())
            .map(crate::model::FilterRangeBoundIr::Finite),
        FilterBoundInput::NegativeInfinity => {
            Ok(crate::model::FilterRangeBoundIr::NegativeInfinity)
        }
        FilterBoundInput::PositiveInfinity => {
            Ok(crate::model::FilterRangeBoundIr::PositiveInfinity)
        }
        FilterBoundInput::Unbounded => Ok(crate::model::FilterRangeBoundIr::Unbounded),
    }
}
