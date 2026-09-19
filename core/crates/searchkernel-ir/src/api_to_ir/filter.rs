use super::*;
use crate::filter_expression::{bind_filter_expression_with_references, FilterParameters};
use crate::model::*;
use searchkernel_protocol::v2 as api;
use std::collections::{BTreeMap, BTreeSet};

impl IndexContext {
    pub fn filter_to_ir(&self, filter: api::FilterExpr<'_>) -> Result<FilterIr> {
        self.filter_to_ir_at(filter, "filter")
    }
    pub fn comparison_to_ir(&self, comparison: api::FilterComparison<'_>) -> Result<ComparisonIr> {
        self.comparison_to_ir_at(comparison, "comparison")
    }
    pub fn range_to_ir(&self, range: api::FilterRange<'_>) -> Result<RangeIr> {
        self.range_to_ir_at(range, "range")
    }
    pub(crate) fn filters_to_ir(
        &self,
        filters: Option<api::FilterRequest<'_>>,
    ) -> Result<Option<FilterIr>> {
        let Some(filters) = filters else {
            return Ok(None);
        };
        let mut clauses = filters
            .structured()
            .map(|values| {
                values
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        self.filter_to_ir_at(value, &format!("filters.structured[{index}]"))
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_default();
        let parameters = filter_parameters(filters.parameters())?;
        let mut referenced_parameters = BTreeSet::new();
        if let Some(sources) = filters.sources() {
            for (clause_index, source) in sources.iter().enumerate() {
                let bound =
                    bind_filter_expression_with_references(source, &self.definition, &parameters)
                        .map_err(|error| decorate_filter_source_error(error, clause_index))?;
                referenced_parameters.extend(bound.referenced_parameters().iter().cloned());
                clauses.push(bound.expression().clone());
            }
        }
        if let Some((name, _)) = parameters
            .iter()
            .find(|(name, _)| !referenced_parameters.contains(*name))
        {
            return Err(invalid(
                "unused_parameter",
                Some(format!("filters.parameters.{name}")),
                format!("parameter `:{name}` is not referenced by a filter source"),
            ));
        }
        Ok(match clauses.len() {
            0 => None,
            1 => clauses.into_iter().next(),
            _ => Some(FilterIr::And(clauses)),
        })
    }

    pub(crate) fn filter_to_ir_at(
        &self,
        filter: api::FilterExpr<'_>,
        path: &str,
    ) -> Result<FilterIr> {
        let tag = filter.expr_type();
        match tag {
            api::FilterExprKind::FilterAnd => {
                let n = filter.expr_as_filter_and().ok_or_else(|| {
                    invalid(
                        "invalid_filter",
                        Some(path),
                        "filter expression must select an operation",
                    )
                })?;
                let clauses = filter_children(n.clauses(), self, path, "all")?;
                if clauses.is_empty() {
                    return Err(invalid(
                        "empty_boolean_filter",
                        Some(format!("{path}.all")),
                        "all requires at least one clause",
                    ));
                }
                Ok(flatten_and(clauses))
            }
            api::FilterExprKind::FilterOr => {
                let n = filter.expr_as_filter_or().ok_or_else(|| {
                    invalid(
                        "invalid_filter",
                        Some(path),
                        "filter expression must select an operation",
                    )
                })?;
                let clauses = filter_children(n.clauses(), self, path, "any")?;
                if clauses.is_empty() {
                    return Err(invalid(
                        "empty_boolean_filter",
                        Some(format!("{path}.any")),
                        "any requires at least one clause",
                    ));
                }
                Ok(flatten_or(clauses))
            }
            api::FilterExprKind::FilterNot => {
                let n = filter.expr_as_filter_not().ok_or_else(|| {
                    invalid(
                        "invalid_filter",
                        Some(path),
                        "filter expression must select an operation",
                    )
                })?;
                let c = n.clause().ok_or_else(|| {
                    invalid(
                        "invalid_filter",
                        Some(format!("{path}.negation.clause")),
                        "negation requires a clause",
                    )
                })?;
                Ok(FilterIr::Not(Box::new(
                    self.filter_to_ir_at(c, &format!("{path}.negation.clause"))?,
                )))
            }
            api::FilterExprKind::FilterComparison => self
                .comparison_to_ir_at(
                    filter.expr_as_filter_comparison().ok_or_else(|| {
                        invalid(
                            "invalid_filter",
                            Some(path),
                            "filter expression must select an operation",
                        )
                    })?,
                    &format!("{path}.comparison"),
                )
                .map(FilterIr::Comparison),
            api::FilterExprKind::FilterRange => self
                .range_to_ir_at(
                    filter.expr_as_filter_range().ok_or_else(|| {
                        invalid(
                            "invalid_filter",
                            Some(path),
                            "filter expression must select an operation",
                        )
                    })?,
                    &format!("{path}.range"),
                )
                .map(FilterIr::Range),
            api::FilterExprKind::FilterExists => {
                let n = filter.expr_as_filter_exists().ok_or_else(|| {
                    invalid(
                        "invalid_filter",
                        Some(path),
                        "filter expression must select an operation",
                    )
                })?;
                let f = self.query_field(
                    n.field().and_then(|field| field.path()).unwrap_or_default(),
                    format!("{path}.exists.field"),
                )?;
                if !f.capabilities.filterable {
                    return Err(invalid(
                        "field_not_filterable",
                        Some(format!("{path}.exists.field")),
                        "exists requires a filterable field",
                    ));
                }
                Ok(FilterIr::Exists {
                    field: f.name.clone(),
                })
            }
            api::FilterExprKind::FilterIn => self
                .in_to_ir_at(
                    filter.expr_as_filter_in().ok_or_else(|| {
                        invalid(
                            "invalid_filter",
                            Some(path),
                            "filter expression must select an operation",
                        )
                    })?,
                    &format!("{path}.in"),
                )
                .map(|(field, values)| FilterIr::In { field, values }),
            _ => Err(invalid(
                "invalid_filter",
                Some(path),
                "filter expression must select an operation",
            )),
        }
    }
    pub(crate) fn comparison_to_ir_at(
        &self,
        c: api::FilterComparison<'_>,
        path: &str,
    ) -> Result<ComparisonIr> {
        let f = self.query_field(
            c.field().and_then(|field| field.path()).unwrap_or_default(),
            format!("{path}.field"),
        )?;
        if !f.capabilities.filterable {
            return Err(invalid(
                "field_not_filterable",
                Some(format!("{path}.field")),
                "comparison requires a filterable field",
            ));
        }
        let op = match c.op().0 {
            1 => ComparisonOp::Eq,
            2 => ComparisonOp::Ne,
            3 => ComparisonOp::Lt,
            4 => ComparisonOp::Lte,
            5 => ComparisonOp::Gt,
            6 => ComparisonOp::Gte,
            _ => {
                return Err(invalid(
                    "invalid_filter",
                    Some(format!("{path}.op")),
                    "comparison operator is unspecified or unknown",
                ))
            }
        };
        if matches!(
            op,
            ComparisonOp::Lt | ComparisonOp::Lte | ComparisonOp::Gt | ComparisonOp::Gte
        ) && !matches!(f.kind(), FieldKind::Int64 | FieldKind::Float64)
        {
            return Err(invalid(
                "invalid_filter",
                Some(format!("{path}.op")),
                "ordered comparison requires a numeric field",
            ));
        }
        let v = c.value().ok_or_else(|| {
            invalid(
                "missing_value",
                Some(format!("{path}.value")),
                "comparison requires a value",
            )
        })?;
        Ok(ComparisonIr {
            field: f.name.clone(),
            op,
            value: scalar_to_ir(v, f.kind(), format!("{path}.value"))?,
        })
    }
    pub(crate) fn in_to_ir_at(
        &self,
        input: api::FilterIn<'_>,
        path: &str,
    ) -> Result<(FieldName, Vec<ScalarValueIr>)> {
        let f = self.query_field(
            input
                .field()
                .and_then(|field| field.path())
                .unwrap_or_default(),
            format!("{path}.field"),
        )?;
        if !f.capabilities.filterable {
            return Err(invalid(
                "field_not_filterable",
                Some(format!("{path}.field")),
                "IN requires a filterable field",
            ));
        }
        if !matches!(f.kind(), FieldKind::Keyword | FieldKind::Int64) {
            return Err(invalid(
                "invalid_filter",
                Some(path),
                "IN requires a Keyword or Int64 field",
            ));
        }
        let values = input
            .values()
            .map(|values| {
                values
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        scalar_to_ir(value, f.kind(), format!("{path}.values[{index}]"))
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_default();
        if values.is_empty() {
            return Err(invalid(
                "empty_in",
                Some(path),
                "IN requires at least one value",
            ));
        }
        Ok((f.name.clone(), values))
    }
    pub(crate) fn range_to_ir_at(&self, r: api::FilterRange<'_>, path: &str) -> Result<RangeIr> {
        let f = self.query_field(
            r.field().and_then(|field| field.path()).unwrap_or_default(),
            format!("{path}.field"),
        )?;
        if !f.capabilities.filterable {
            return Err(invalid(
                "field_not_filterable",
                Some(format!("{path}.field")),
                "range requires a filterable field",
            ));
        }
        if !matches!(f.kind(), FieldKind::Int64 | FieldKind::Float64) {
            return Err(invalid(
                "invalid_filter",
                Some(path),
                "range requires an Int64 or Float64 field",
            ));
        }
        let lower = filter_range_bound_to_ir(r.lower(), f.kind(), &format!("{path}.lower"))?;
        let upper = filter_range_bound_to_ir(r.upper(), f.kind(), &format!("{path}.upper"))?;
        if matches!(lower, FilterRangeBoundIr::Unbounded)
            && matches!(upper, FilterRangeBoundIr::Unbounded)
        {
            return Err(invalid(
                "invalid_range",
                Some(path),
                "range requires at least one bound",
            ));
        }
        if matches!(lower, FilterRangeBoundIr::Unbounded)
            && r.include_lower().is_some_and(|v| v.value())
        {
            return Err(invalid(
                "invalid_range",
                Some(format!("{path}.include_lower")),
                "lower inclusion requires a lower bound",
            ));
        }
        if matches!(upper, FilterRangeBoundIr::Unbounded)
            && r.include_upper().is_some_and(|v| v.value())
        {
            return Err(invalid(
                "invalid_range",
                Some(format!("{path}.include_upper")),
                "upper inclusion requires an upper bound",
            ));
        }
        if filter_bound_greater(&lower, &upper) {
            return Err(invalid(
                "invalid_range",
                Some(path),
                "range lower bound must not exceed upper bound",
            ));
        }
        Ok(RangeIr {
            field: f.name.clone(),
            lower,
            upper,
            include_lower: r.include_lower().is_none_or(|v| v.value()),
            include_upper: r.include_upper().is_none_or(|v| v.value()),
        })
    }
}

pub(crate) fn decorate_filter_source_error(
    mut error: PortableError,
    clause_index: usize,
) -> PortableError {
    for detail in &mut error.details {
        detail
            .metadata
            .insert("clause_index".to_owned(), clause_index.to_string());
        // Parser and binder offsets are local to the source clause.
        if let Some(start) = detail.metadata.remove("span_start") {
            detail.metadata.insert("start_offset".to_owned(), start);
        }
        if let Some(end) = detail.metadata.remove("span_end") {
            detail.metadata.insert("end_offset".to_owned(), end);
        }
    }
    error
}

pub(crate) fn filter_range_bound_to_ir<'a>(
    bound: Option<api::RangeBound<'a>>,
    kind: FieldKind,
    path: &str,
) -> Result<FilterRangeBoundIr> {
    let Some(bound) = bound else {
        return Ok(FilterRangeBoundIr::Unbounded);
    };
    match bound.bound_type() {
        api::RangeBoundKind::FiniteRangeBound => bound
            .bound_as_finite_range_bound()
            .and_then(|value| value.value())
            .ok_or_else(|| invalid("invalid_range", Some(path), "finite range bound is missing"))
            .and_then(|value| {
                RequestValueIr::from_api(value, path.to_owned())
                    .and_then(|value| scalar_request_value_to_ir(value, kind, path))
                    .map(FilterRangeBoundIr::Finite)
            }),
        api::RangeBoundKind::NegativeInfinityBound => Ok(FilterRangeBoundIr::NegativeInfinity),
        api::RangeBoundKind::PositiveInfinityBound => Ok(FilterRangeBoundIr::PositiveInfinity),
        api::RangeBoundKind::UnboundedRangeBound => Ok(FilterRangeBoundIr::Unbounded),
        _ => Err(invalid("invalid_range", Some(path), "unknown range bound")),
    }
}

pub(crate) fn scalar_request_value_to_ir(
    value: RequestValueIr,
    kind: FieldKind,
    path: &str,
) -> Result<ScalarValueIr> {
    let scalar = match value {
        RequestValueIr::Bool(value) => ScalarValueIr::Bool(value),
        RequestValueIr::Int64(value) => ScalarValueIr::Int64(value),
        RequestValueIr::Float64(value) if value.is_finite() => ScalarValueIr::Float64(value),
        RequestValueIr::String(value) => ScalarValueIr::String(value),
        _ => {
            return Err(invalid(
                "type_mismatch",
                Some(path),
                "range bound must be a scalar value",
            ))
        }
    };
    if !matches!(
        (&scalar, kind),
        (ScalarValueIr::Int64(_), FieldKind::Int64)
            | (ScalarValueIr::Float64(_), FieldKind::Float64)
    ) {
        return Err(invalid(
            "type_mismatch",
            Some(path),
            "range bound type does not match field kind",
        ));
    }
    Ok(scalar)
}

pub(crate) fn filter_bound_greater(lower: &FilterRangeBoundIr, upper: &FilterRangeBoundIr) -> bool {
    match (lower, upper) {
        (FilterRangeBoundIr::Unbounded, _) => false,
        (_, FilterRangeBoundIr::Unbounded) => false,
        (FilterRangeBoundIr::PositiveInfinity, FilterRangeBoundIr::PositiveInfinity)
        | (FilterRangeBoundIr::NegativeInfinity, FilterRangeBoundIr::NegativeInfinity) => false,
        (FilterRangeBoundIr::PositiveInfinity, _) => true,
        (_, FilterRangeBoundIr::NegativeInfinity) => true,
        (FilterRangeBoundIr::Finite(a), FilterRangeBoundIr::Finite(b)) => numeric_gt(a, b),
        (FilterRangeBoundIr::NegativeInfinity, _) => false,
        (_, FilterRangeBoundIr::PositiveInfinity) => false,
    }
}

pub(crate) fn filter_parameters(
    entries: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::ValueEntry<'_>>>>,
) -> Result<FilterParameters> {
    let mut parameters = BTreeMap::new();
    let Some(entries) = entries else {
        return Ok(parameters);
    };
    for (index, entry) in entries.iter().enumerate() {
        let name = entry.key().to_owned();
        if parameters.contains_key(&name) {
            return Err(invalid(
                "duplicate_parameter",
                Some(format!("filters.parameters[{index}]")),
                format!("parameter `:{name}` occurs more than once"),
            ));
        }
        let value = entry.value().ok_or_else(|| {
            invalid(
                "missing_value",
                Some(format!("filters.parameters[{index}].value")),
                "parameter value is required",
            )
        })?;
        parameters.insert(
            name,
            value_from_api(value, format!("filters.parameters[{index}].value"))?,
        );
    }
    Ok(parameters)
}
pub(crate) fn filter_children(
    children: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::FilterExpr<'_>>>>,
    context: &IndexContext,
    parent_path: &str,
    operation_name: &str,
) -> Result<Vec<FilterIr>> {
    let mut converted = Vec::new();
    if let Some(children) = children {
        for (index, child) in children.iter().enumerate() {
            let child_path = format!("{parent_path}.{operation_name}.clauses[{index}]");
            converted.push(context.filter_to_ir_at(child, &child_path)?);
        }
    }
    Ok(converted)
}
pub(crate) fn flatten_and(clauses: Vec<FilterIr>) -> FilterIr {
    let mut flattened = Vec::new();
    for clause in clauses {
        if let FilterIr::And(nested_clauses) = clause {
            flattened.extend(nested_clauses)
        } else {
            flattened.push(clause)
        }
    }
    if flattened.len() == 1 {
        flattened.pop().expect("one clause was checked")
    } else {
        FilterIr::And(flattened)
    }
}
pub(crate) fn flatten_or(clauses: Vec<FilterIr>) -> FilterIr {
    let mut flattened = Vec::new();
    for clause in clauses {
        if let FilterIr::Or(nested_clauses) = clause {
            flattened.extend(nested_clauses)
        } else {
            flattened.push(clause)
        }
    }
    if flattened.len() == 1 {
        flattened.pop().expect("one clause was checked")
    } else {
        FilterIr::Or(flattened)
    }
}
pub(crate) fn numeric_gt(a: &ScalarValueIr, b: &ScalarValueIr) -> bool {
    match (a, b) {
        (ScalarValueIr::Int64(a), ScalarValueIr::Int64(b)) => a > b,
        (ScalarValueIr::Float64(a), ScalarValueIr::Float64(b)) => a > b,
        _ => false,
    }
}
