//! Schema-aware binding for the standalone filter expression language.

use crate::{
    ComparisonIr, ComparisonOp, FieldIr, FieldKind, FilterIr, IndexDefinitionIr, PortableError,
    Result, ScalarValueIr, Value,
};
use searchkernel_filter::{
    CompareOp, ErrorKind, Expr, InValues, ParseError, Predicate, Span, Spanned, ValueExpr,
};
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
#[path = "filter_expression_tests.rs"]
mod tests;

/// Typed bindings available to one textual filter clause.
pub type FilterBindings = BTreeMap<String, Value>;

/// Compatibility name for callers of the pre-clause-local binder API.
pub type FilterParameters = FilterBindings;

/// A bound filter and the parameter references it consumed.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundFilter {
    expression: FilterIr,
    referenced_parameters: BTreeSet<String>,
}

impl BoundFilter {
    pub fn expression(&self) -> &FilterIr {
        &self.expression
    }

    pub fn referenced_parameters(&self) -> &BTreeSet<String> {
        &self.referenced_parameters
    }

    pub fn into_parts(self) -> (FilterIr, BTreeSet<String>) {
        (self.expression, self.referenced_parameters)
    }
}

/// Parse and bind a filter source against a resolved schema and typed parameters.
pub fn bind_filter_expression(
    source: &str,
    schema: &IndexDefinitionIr,
    params: &FilterParameters,
) -> Result<FilterIr> {
    Ok(bind_filter_expression_with_references(source, schema, params)?.expression)
}

/// Parse and bind a filter source while retaining its referenced parameter names.
pub fn bind_filter_expression_with_references(
    source: &str,
    schema: &IndexDefinitionIr,
    params: &FilterParameters,
) -> Result<BoundFilter> {
    let parsed = searchkernel_filter::parse_filter(source).map_err(parse_error_to_portable)?;
    bind_parsed_filter(&parsed, schema, params)
}

pub(crate) fn bind_parsed_filter(
    parsed: &Spanned<Expr>,
    schema: &IndexDefinitionIr,
    bindings: &FilterBindings,
) -> Result<BoundFilter> {
    let mut referenced_parameters = BTreeSet::new();
    let expression = bind_expr(parsed, schema, bindings, &mut referenced_parameters)?;
    if let Some(name) = bindings
        .keys()
        .find(|name| !referenced_parameters.contains(*name))
    {
        return Err(PortableError::invalid(
            "unused_parameter",
            Some(format!("filter.bindings.{name}")),
            format!("binding `:{name}` is not referenced by the filter expression"),
        ));
    }
    Ok(BoundFilter {
        expression,
        referenced_parameters,
    })
}

fn bind_expr(
    expression: &Spanned<Expr>,
    schema: &IndexDefinitionIr,
    params: &FilterParameters,
    referenced_parameters: &mut BTreeSet<String>,
) -> Result<FilterIr> {
    match &expression.node {
        Expr::Predicate(predicate) => bind_predicate(
            predicate,
            expression.span,
            schema,
            params,
            referenced_parameters,
        ),
        Expr::And(children) => {
            let children = children
                .iter()
                .map(|child| bind_expr(child, schema, params, referenced_parameters))
                .collect::<Result<Vec<_>>>()?;
            if children.is_empty() {
                return Err(filter_error(
                    "empty_boolean_filter",
                    expression.span,
                    "AND requires at least one clause",
                ));
            }
            Ok(flatten_and(children))
        }
        Expr::Or(children) => {
            let children = children
                .iter()
                .map(|child| bind_expr(child, schema, params, referenced_parameters))
                .collect::<Result<Vec<_>>>()?;
            if children.is_empty() {
                return Err(filter_error(
                    "empty_boolean_filter",
                    expression.span,
                    "OR requires at least one clause",
                ));
            }
            Ok(flatten_or(children))
        }
        Expr::Not(child) => Ok(FilterIr::Not(Box::new(bind_expr(
            child,
            schema,
            params,
            referenced_parameters,
        )?))),
    }
}

fn bind_predicate(
    predicate: &Predicate,
    expression_span: Span,
    schema: &IndexDefinitionIr,
    params: &FilterParameters,
    referenced_parameters: &mut BTreeSet<String>,
) -> Result<FilterIr> {
    match predicate {
        Predicate::Compare { field, op, value } => {
            let field_ir = resolve_field(field.node.0.as_str(), field.span, schema)?;
            ensure_filterable(field_ir, field.span)?;
            ensure_comparison_operator(field_ir.kind(), *op, expression_span)?;
            let value = bind_scalar(value, field_ir.kind(), params, referenced_parameters)?;
            Ok(FilterIr::Comparison(ComparisonIr {
                field: field_ir.name().clone(),
                op: comparison_op(*op),
                value,
            }))
        }
        Predicate::In { field, values } => {
            let field_ir = resolve_field(field.node.0.as_str(), field.span, schema)?;
            ensure_filterable(field_ir, field.span)?;
            ensure_in_operator(field_ir.kind(), expression_span)?;
            let values = bind_in_values(values, field_ir.kind(), params, referenced_parameters)?;
            Ok(FilterIr::In {
                field: field_ir.name().clone(),
                values,
            })
        }
    }
}

fn resolve_field<'a>(name: &str, span: Span, schema: &'a IndexDefinitionIr) -> Result<&'a FieldIr> {
    if let Some(field) = schema
        .query_fields()
        .find(|field| field.name().as_str() == name)
    {
        return Ok(field);
    }
    if schema.imported_alias(name) {
        return Err(filter_error(
            "imported_field_query_not_supported",
            span,
            format!("imported field alias `{name}` has no local type for query binding"),
        ));
    }
    Err(filter_error(
        "unknown_field",
        span,
        format!("unknown field `{name}`"),
    ))
}

fn ensure_filterable(field: &FieldIr, span: Span) -> Result<()> {
    if field.capabilities().filterable() {
        Ok(())
    } else {
        Err(filter_error(
            "field_not_filterable",
            span,
            format!("field `{}` is not filterable", field.name().as_str()),
        ))
    }
}

fn ensure_comparison_operator(kind: FieldKind, op: CompareOp, span: Span) -> Result<()> {
    let supported = match kind {
        FieldKind::Keyword | FieldKind::Bool => matches!(op, CompareOp::Eq),
        FieldKind::Int64 | FieldKind::Float64 => true,
        FieldKind::Text => false,
    };
    if supported {
        Ok(())
    } else {
        Err(filter_error(
            "unsupported_operator",
            span,
            format!("operator is not supported for {kind:?} fields"),
        ))
    }
}

fn ensure_in_operator(kind: FieldKind, span: Span) -> Result<()> {
    if matches!(kind, FieldKind::Keyword | FieldKind::Int64) {
        Ok(())
    } else {
        Err(filter_error(
            "unsupported_operator",
            span,
            format!("IN is not supported for {kind:?} fields"),
        ))
    }
}

fn comparison_op(op: CompareOp) -> ComparisonOp {
    match op {
        CompareOp::Eq => ComparisonOp::Eq,
        CompareOp::Lt => ComparisonOp::Lt,
        CompareOp::Lte => ComparisonOp::Lte,
        CompareOp::Gt => ComparisonOp::Gt,
        CompareOp::Gte => ComparisonOp::Gte,
    }
}

fn bind_scalar(
    value: &Spanned<ValueExpr>,
    kind: FieldKind,
    params: &FilterParameters,
    referenced_parameters: &mut BTreeSet<String>,
) -> Result<ScalarValueIr> {
    match &value.node {
        ValueExpr::String(text) => bind_string(text, kind, value.span),
        ValueExpr::Boolean(boolean) => bind_boolean(*boolean, kind, value.span),
        ValueExpr::Number(number) => bind_number(number.source.as_str(), kind, value.span),
        ValueExpr::Parameter(parameter) => {
            referenced_parameters.insert(parameter.0.clone());
            let parameter_value = params.get(&parameter.0).ok_or_else(|| {
                filter_error(
                    "unknown_parameter",
                    value.span,
                    format!("unknown parameter `:{}`", parameter.0),
                )
            })?;
            bind_runtime_scalar(parameter_value, kind, value.span)
        }
    }
}

fn bind_in_values(
    values: &InValues,
    kind: FieldKind,
    params: &FilterParameters,
    referenced_parameters: &mut BTreeSet<String>,
) -> Result<Vec<ScalarValueIr>> {
    match values {
        InValues::Values(values) => values
            .iter()
            .map(|value| bind_scalar(value, kind, params, referenced_parameters))
            .collect(),
        InValues::Parameter(parameter) => {
            referenced_parameters.insert(parameter.node.0.clone());
            let parameter_value = params.get(&parameter.node.0).ok_or_else(|| {
                filter_error(
                    "unknown_parameter",
                    parameter.span,
                    format!("unknown parameter `:{}`", parameter.node.0),
                )
            })?;
            let Value::List(values) = parameter_value else {
                return Err(filter_error(
                    "type_mismatch",
                    parameter.span,
                    format!("parameter `:{}` must be a list for IN", parameter.node.0),
                ));
            };
            if values.is_empty() {
                return Err(filter_error(
                    "empty_in",
                    parameter.span,
                    "IN parameter list must not be empty",
                ));
            }
            values
                .iter()
                .map(|value| bind_runtime_scalar(value, kind, parameter.span))
                .collect()
        }
    }
}

fn bind_string(value: &str, kind: FieldKind, span: Span) -> Result<ScalarValueIr> {
    if matches!(kind, FieldKind::Keyword) {
        Ok(ScalarValueIr::String(value.to_owned()))
    } else {
        Err(filter_error(
            "type_mismatch",
            span,
            format!("expected a {kind:?} value"),
        ))
    }
}

fn bind_boolean(value: bool, kind: FieldKind, span: Span) -> Result<ScalarValueIr> {
    if kind == FieldKind::Bool {
        Ok(ScalarValueIr::Bool(value))
    } else {
        Err(filter_error(
            "type_mismatch",
            span,
            format!("expected a {kind:?} value"),
        ))
    }
}

fn bind_number(source: &str, kind: FieldKind, span: Span) -> Result<ScalarValueIr> {
    match kind {
        FieldKind::Int64 if is_decimal_literal(source) => Err(filter_error(
            "type_mismatch",
            span,
            format!("decimal literal `{source}` is not valid for Int64 fields"),
        )),
        FieldKind::Int64 => source
            .parse::<i64>()
            .map(ScalarValueIr::Int64)
            .map_err(|_| {
                filter_error(
                    "invalid_literal",
                    span,
                    format!("`{source}` is not a valid Int64 literal or is out of range"),
                )
            }),
        FieldKind::Float64 => {
            let value = source.parse::<f64>().map_err(|_| {
                filter_error(
                    "invalid_literal",
                    span,
                    format!("`{source}` is not a valid Float64 literal"),
                )
            })?;
            if value.is_finite() {
                Ok(ScalarValueIr::Float64(value))
            } else {
                Err(filter_error(
                    "non_finite_number",
                    span,
                    "floating-point literals must be finite",
                ))
            }
        }
        _ => Err(filter_error(
            "type_mismatch",
            span,
            format!("a numeric literal is not valid for {kind:?} fields"),
        )),
    }
}

fn is_decimal_literal(source: &str) -> bool {
    source
        .bytes()
        .any(|byte| matches!(byte, b'.' | b'e' | b'E'))
}

fn bind_runtime_scalar(value: &Value, kind: FieldKind, span: Span) -> Result<ScalarValueIr> {
    match value {
        Value::Bool(value) => bind_boolean(*value, kind, span),
        Value::Int64(value) => normalize_int64_parameter(*value, kind, span),
        Value::UInt64(value) => normalize_uint64_parameter(*value, kind, span),
        Value::Float64(value) => normalize_float64_parameter(*value, kind, span),
        Value::String(value) => bind_string(value, kind, span),
        Value::List(_) => Err(filter_error(
            "type_mismatch",
            span,
            "a list parameter cannot be used as a scalar",
        )),
        _ => Err(filter_error(
            "type_mismatch",
            span,
            format!("parameter value is not compatible with {kind:?} fields"),
        )),
    }
}

fn normalize_int64_parameter(value: i64, kind: FieldKind, span: Span) -> Result<ScalarValueIr> {
    match kind {
        FieldKind::Int64 => Ok(ScalarValueIr::Int64(value)),
        FieldKind::Float64 => {
            let converted = value as f64;
            if (value < 0 || converted < 2_f64.powi(63)) && converted as i64 == value {
                Ok(ScalarValueIr::Float64(converted))
            } else {
                Err(filter_error(
                    "type_mismatch",
                    span,
                    "integer parameter cannot be represented exactly as Float64",
                ))
            }
        }
        _ => Err(filter_error(
            "type_mismatch",
            span,
            format!("parameter value is not compatible with {kind:?} fields"),
        )),
    }
}

fn normalize_uint64_parameter(value: u64, kind: FieldKind, span: Span) -> Result<ScalarValueIr> {
    match kind {
        FieldKind::Int64 => i64::try_from(value).map(ScalarValueIr::Int64).map_err(|_| {
            filter_error(
                "type_mismatch",
                span,
                "UInt64 parameter is out of Int64 range",
            )
        }),
        FieldKind::Float64 => {
            let converted = value as f64;
            if converted < 2_f64.powi(64) && converted as u64 == value {
                Ok(ScalarValueIr::Float64(converted))
            } else {
                Err(filter_error(
                    "type_mismatch",
                    span,
                    "UInt64 parameter cannot be represented exactly as Float64",
                ))
            }
        }
        _ => Err(filter_error(
            "type_mismatch",
            span,
            format!("parameter value is not compatible with {kind:?} fields"),
        )),
    }
}

fn normalize_float64_parameter(value: f64, kind: FieldKind, span: Span) -> Result<ScalarValueIr> {
    if !value.is_finite() {
        return Err(filter_error(
            "non_finite_number",
            span,
            "floating-point parameters must be finite",
        ));
    }
    match kind {
        FieldKind::Float64 => Ok(ScalarValueIr::Float64(value)),
        FieldKind::Int64
            if value.fract() == 0.0 && value >= i64::MIN as f64 && value < 2_f64.powi(63) =>
        {
            let converted = value as i64;
            if converted as f64 == value {
                Ok(ScalarValueIr::Int64(converted))
            } else {
                Err(filter_error(
                    "type_mismatch",
                    span,
                    "Float64 parameter cannot be represented exactly as Int64",
                ))
            }
        }
        _ => Err(filter_error(
            "type_mismatch",
            span,
            format!("parameter value is not compatible with {kind:?} fields"),
        )),
    }
}

fn flatten_and(children: Vec<FilterIr>) -> FilterIr {
    let mut flattened = Vec::new();
    for child in children {
        if let FilterIr::And(nested) = child {
            flattened.extend(nested);
        } else {
            flattened.push(child);
        }
    }
    if flattened.len() == 1 {
        flattened.pop().expect("one child was checked")
    } else {
        FilterIr::And(flattened)
    }
}

fn flatten_or(children: Vec<FilterIr>) -> FilterIr {
    let mut flattened = Vec::new();
    for child in children {
        if let FilterIr::Or(nested) = child {
            flattened.extend(nested);
        } else {
            flattened.push(child);
        }
    }
    if flattened.len() == 1 {
        flattened.pop().expect("one child was checked")
    } else {
        FilterIr::Or(flattened)
    }
}

pub(crate) fn parse_error_to_portable(error: ParseError) -> PortableError {
    let (code, reason) = match error.kind {
        ErrorKind::SyntaxError => (
            "syntax_error",
            error.reason.map(|reason| format!("{reason:?}")),
        ),
        ErrorKind::EmptyIn => ("empty_in", None),
        ErrorKind::ExpressionTooComplex => (
            "expression_too_complex",
            error.limit.map(|limit| format!("{limit:?}")),
        ),
    };
    let mut converted = filter_error(code, error.span, error.to_string());
    if let Some(reason) = reason {
        if let Some(detail) = converted.details.first_mut() {
            let key = if error.kind == ErrorKind::SyntaxError {
                "reason"
            } else {
                "limit"
            };
            detail.metadata.insert(key.to_owned(), reason);
        }
    }
    converted
}

fn filter_error(code: impl Into<String>, span: Span, message: impl Into<String>) -> PortableError {
    let mut error = PortableError::invalid(code, Some("filter"), message);
    if let Some(detail) = error.details.first_mut() {
        detail
            .metadata
            .insert("start_offset".to_owned(), span.start.to_string());
        detail
            .metadata
            .insert("end_offset".to_owned(), span.end.to_string());
    }
    error
}
