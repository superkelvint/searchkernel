use super::*;
use crate::api_to_ir::resolve::resolve_filter;
use crate::filter_expression::FilterBindings;
#[cfg(test)]
use crate::filter_expression::{bind_parsed_filter, parse_error_to_portable};
use crate::model::*;
#[cfg(test)]
use searchkernel_filter::parse_filter;
use searchkernel_protocol::wire::v1 as api;
use std::collections::BTreeMap;

pub(crate) const MAX_FILTER_BINDINGS: usize = 1024;

impl IndexContext {
    pub(crate) fn filters_to_input(
        &self,
        filters: Option<api::FilterRequest<'_>>,
    ) -> Result<(
        Vec<FilterInput>,
        Vec<crate::api_to_ir::resolve::TextualFilterInput>,
    )> {
        let Some(filters) = filters else {
            return Ok((Vec::new(), Vec::new()));
        };
        if filters.sources().is_some() || filters.parameters().is_some() {
            return Err(invalid(
                "invalid_filter",
                Some("filters"),
                "legacy global textual filter fields are unsupported; use filters.textual",
            ));
        }
        let structured = filters
            .structured()
            .map(|values| {
                values
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        filter_to_input_at(value, &format!("filters.structured[{index}]"))
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_default();
        let textual = filters
            .textual()
            .map(|values| {
                values
                    .iter()
                    .enumerate()
                    .map(|(index, clause)| {
                        let path = format!("filters.textual[{index}]");
                        Ok(crate::api_to_ir::resolve::TextualFilterInput {
                            source: clause
                                .source()
                                .ok_or_else(|| {
                                    invalid(
                                        "missing_source",
                                        Some(format!("{path}.source")),
                                        "textual filter clause source is required",
                                    )
                                })?
                                .to_owned(),
                            bindings: filter_bindings(
                                clause.bindings(),
                                &format!("{path}.bindings"),
                            )?,
                        })
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_default();
        Ok((structured, textual))
    }

    pub fn filter_to_ir(&self, filter: api::FilterExpr<'_>) -> Result<FilterIr> {
        self.filter_to_ir_at(filter, "filter")
    }
    pub fn comparison_to_ir(&self, comparison: api::FilterComparison<'_>) -> Result<ComparisonIr> {
        self.comparison_to_ir_at(comparison, "comparison")
    }
    pub fn range_to_ir(&self, range: api::FilterRange<'_>) -> Result<RangeIr> {
        self.range_to_ir_at(range, "range")
    }
    #[cfg(test)]
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

        if filters.sources().is_some() || filters.parameters().is_some() {
            return Err(invalid(
                "invalid_filter",
                Some("filters"),
                "legacy global textual filter fields are unsupported; use filters.textual",
            ));
        }

        if let Some(textual) = filters.textual() {
            for (clause_index, clause) in textual.iter().enumerate() {
                let path = format!("filters.textual[{clause_index}]");
                let source = clause.source().ok_or_else(|| {
                    invalid(
                        "missing_source",
                        Some(format!("{path}.source")),
                        "textual filter clause source is required",
                    )
                })?;
                let parsed = parse_filter(source)
                    .map_err(parse_error_to_portable)
                    .map_err(|error| decorate_filter_clause_error(error, clause_index, &path))?;
                let bindings = filter_bindings(clause.bindings(), &format!("{path}.bindings"))
                    .map_err(|error| decorate_filter_clause_error(error, clause_index, &path))?;
                let bound = bind_parsed_filter(&parsed, &self.definition, &bindings)
                    .map_err(|error| decorate_filter_clause_error(error, clause_index, &path))?;
                clauses.push(bound.expression().clone());
            }
        }
        Ok(match clauses.len() {
            0 => None,
            1 => clauses.into_iter().next(),
            _ => Some(flatten_and(clauses)),
        })
    }

    pub(crate) fn filter_to_ir_at(
        &self,
        filter: api::FilterExpr<'_>,
        path: &str,
    ) -> Result<FilterIr> {
        // Component conversion must use the same input decoder and resolver
        // as complete search requests so boolean structure and error paths
        // cannot drift between ingress paths.
        let input = filter_to_input_at(filter, path)?;
        resolve_filter(self, &input, path)
    }
    pub(crate) fn comparison_to_ir_at(
        &self,
        c: api::FilterComparison<'_>,
        path: &str,
    ) -> Result<ComparisonIr> {
        let op = filter_comparison_op(c.op().0, &format!("{path}.op"))?;
        let v = c.value().ok_or_else(|| {
            invalid(
                "missing_value",
                Some(format!("{path}.value")),
                "comparison requires a value",
            )
        })?;
        let input = FilterInput::Comparison {
            field: c
                .field()
                .and_then(|field| field.path())
                .unwrap_or_default()
                .to_owned(),
            op,
            value: RequestValueIr::from_api(v, format!("{path}.value"))?,
        };
        match resolve_filter(self, &input, path)? {
            FilterIr::Comparison(value) => Ok(value),
            _ => unreachable!("comparison resolver returned a non-comparison filter"),
        }
    }
    pub(crate) fn range_to_ir_at(&self, r: api::FilterRange<'_>, path: &str) -> Result<RangeIr> {
        let input = FilterInput::Range {
            field: r
                .field()
                .and_then(|field| field.path())
                .unwrap_or_default()
                .to_owned(),
            lower: filter_range_bound_to_input(r.lower(), &format!("{path}.lower"))?,
            upper: filter_range_bound_to_input(r.upper(), &format!("{path}.upper"))?,
            include_lower: r.include_lower().map(|value| value.value()),
            include_upper: r.include_upper().map(|value| value.value()),
        };
        match resolve_filter(self, &input, path)? {
            FilterIr::Range(value) => Ok(value),
            _ => unreachable!("range resolver returned a non-range filter"),
        }
    }
}

fn filter_comparison_op(value: u8, path: &str) -> Result<ComparisonOp> {
    match value {
        1 => Ok(ComparisonOp::Eq),
        2 => Ok(ComparisonOp::Ne),
        3 => Ok(ComparisonOp::Lt),
        4 => Ok(ComparisonOp::Lte),
        5 => Ok(ComparisonOp::Gt),
        6 => Ok(ComparisonOp::Gte),
        _ => Err(invalid(
            "invalid_filter",
            Some(path),
            "comparison operator is unspecified or unknown",
        )),
    }
}

fn filter_to_input_at(filter: api::FilterExpr<'_>, path: &str) -> Result<FilterInput> {
    match filter.expr_type() {
        api::FilterExprKind::FilterAnd => {
            let value = filter.expr_as_filter_and().ok_or_else(|| {
                invalid(
                    "invalid_filter",
                    Some(path),
                    "filter expression must select an operation",
                )
            })?;
            Ok(FilterInput::All(
                value
                    .clauses()
                    .map(|clauses| {
                        clauses
                            .iter()
                            .enumerate()
                            .map(|(index, child)| {
                                filter_to_input_at(child, &format!("{path}.all[{index}]"))
                            })
                            .collect::<Result<Vec<_>>>()
                    })
                    .transpose()?
                    .unwrap_or_default(),
            ))
        }
        api::FilterExprKind::FilterOr => {
            let value = filter.expr_as_filter_or().ok_or_else(|| {
                invalid(
                    "invalid_filter",
                    Some(path),
                    "filter expression must select an operation",
                )
            })?;
            Ok(FilterInput::Any(
                value
                    .clauses()
                    .map(|clauses| {
                        clauses
                            .iter()
                            .enumerate()
                            .map(|(index, child)| {
                                filter_to_input_at(child, &format!("{path}.any[{index}]"))
                            })
                            .collect::<Result<Vec<_>>>()
                    })
                    .transpose()?
                    .unwrap_or_default(),
            ))
        }
        api::FilterExprKind::FilterNot => {
            let value = filter.expr_as_filter_not().ok_or_else(|| {
                invalid(
                    "invalid_filter",
                    Some(path),
                    "filter expression must select an operation",
                )
            })?;
            let child = value.clause().ok_or_else(|| {
                invalid(
                    "invalid_filter",
                    Some(format!("{path}.negation.clause")),
                    "negation requires a clause",
                )
            })?;
            Ok(FilterInput::Not(Box::new(filter_to_input_at(
                child,
                &format!("{path}.negation.clause"),
            )?)))
        }
        api::FilterExprKind::FilterExists => {
            let value = filter.expr_as_filter_exists().ok_or_else(|| {
                invalid(
                    "invalid_filter",
                    Some(path),
                    "filter expression must select an operation",
                )
            })?;
            Ok(FilterInput::Exists {
                field: value
                    .field()
                    .and_then(|field| field.path())
                    .unwrap_or_default()
                    .to_owned(),
            })
        }
        api::FilterExprKind::FilterComparison => {
            let value = filter.expr_as_filter_comparison().ok_or_else(|| {
                invalid(
                    "invalid_filter",
                    Some(path),
                    "filter expression must select an operation",
                )
            })?;
            Ok(FilterInput::Comparison {
                field: value
                    .field()
                    .and_then(|field| field.path())
                    .unwrap_or_default()
                    .to_owned(),
                op: filter_comparison_op(value.op().0, &format!("{path}.comparison.op"))?,
                value: RequestValueIr::from_api(
                    value.value().ok_or_else(|| {
                        invalid(
                            "missing_value",
                            Some(format!("{path}.comparison.value")),
                            "comparison requires a value",
                        )
                    })?,
                    format!("{path}.comparison.value"),
                )?,
            })
        }
        api::FilterExprKind::FilterRange => {
            let value = filter.expr_as_filter_range().ok_or_else(|| {
                invalid(
                    "invalid_filter",
                    Some(path),
                    "filter expression must select an operation",
                )
            })?;
            Ok(FilterInput::Range {
                field: value
                    .field()
                    .and_then(|field| field.path())
                    .unwrap_or_default()
                    .to_owned(),
                lower: filter_range_bound_to_input(value.lower(), &format!("{path}.range.lower"))?,
                upper: filter_range_bound_to_input(value.upper(), &format!("{path}.range.upper"))?,
                include_lower: value.include_lower().map(|value| value.value()),
                include_upper: value.include_upper().map(|value| value.value()),
            })
        }
        api::FilterExprKind::FilterIn => {
            let value = filter.expr_as_filter_in().ok_or_else(|| {
                invalid(
                    "invalid_filter",
                    Some(path),
                    "filter expression must select an operation",
                )
            })?;
            Ok(FilterInput::In {
                field: value
                    .field()
                    .and_then(|field| field.path())
                    .unwrap_or_default()
                    .to_owned(),
                values: value
                    .values()
                    .map(|values| {
                        values
                            .iter()
                            .enumerate()
                            .map(|(index, value)| {
                                RequestValueIr::from_api(
                                    value,
                                    format!("{path}.in.values[{index}]"),
                                )
                            })
                            .collect::<Result<Vec<_>>>()
                    })
                    .transpose()?
                    .unwrap_or_default(),
            })
        }
        _ => Err(invalid(
            "invalid_filter",
            Some(path),
            "filter expression must select an operation",
        )),
    }
}

fn filter_range_bound_to_input(
    bound: Option<api::RangeBound<'_>>,
    path: &str,
) -> Result<FilterBoundInput> {
    let Some(bound) = bound else {
        return Ok(FilterBoundInput::Unbounded);
    };
    match bound.bound_type() {
        api::RangeBoundKind::FiniteRangeBound => bound
            .bound_as_finite_range_bound()
            .and_then(|value| value.value())
            .ok_or_else(|| invalid("invalid_range", Some(path), "finite range bound is missing"))
            .and_then(|value| {
                RequestValueIr::from_api(value, path.to_owned()).map(FilterBoundInput::Finite)
            }),
        api::RangeBoundKind::NegativeInfinityBound => Ok(FilterBoundInput::NegativeInfinity),
        api::RangeBoundKind::PositiveInfinityBound => Ok(FilterBoundInput::PositiveInfinity),
        api::RangeBoundKind::UnboundedRangeBound => Ok(FilterBoundInput::Unbounded),
        _ => Err(invalid("invalid_range", Some(path), "unknown range bound")),
    }
}

pub(crate) fn decorate_filter_clause_error(
    mut error: PortableError,
    clause_index: usize,
    clause_path: &str,
) -> PortableError {
    for detail in &mut error.details {
        detail
            .metadata
            .insert("clause_index".to_owned(), clause_index.to_string());
        if let Some(path) = detail.path.take() {
            detail.path = Some(
                if let Some(binding_path) = path.strip_prefix("filter.bindings") {
                    format!("{clause_path}.bindings{binding_path}")
                } else if path == "filter" || path.starts_with("filter.") {
                    format!("{clause_path}.source")
                } else {
                    path
                },
            );
        }
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

pub(crate) fn filter_bindings(
    entries: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::ValueEntry<'_>>>>,
    path: &str,
) -> Result<FilterBindings> {
    let Some(entries) = entries else {
        return Ok(BTreeMap::new());
    };
    if entries.len() > MAX_FILTER_BINDINGS {
        return Err(invalid(
            "expression_too_complex",
            Some(path),
            format!("filter clause contains too many bindings; maximum is {MAX_FILTER_BINDINGS}"),
        ));
    }
    let mut bindings = BTreeMap::new();
    for (index, entry) in entries.iter().enumerate() {
        let name = entry.key().to_owned();
        if bindings.contains_key(&name) {
            return Err(invalid(
                "duplicate_parameter",
                Some(format!("{path}[{index}]")),
                format!("binding `:{name}` occurs more than once"),
            ));
        }
        let value = entry.value().ok_or_else(|| {
            invalid(
                "missing_value",
                Some(format!("{path}[{index}].value")),
                "binding value is required",
            )
        })?;
        bindings.insert(
            name,
            value_from_api(value, format!("{path}[{index}].value"))?,
        );
    }
    Ok(bindings)
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
