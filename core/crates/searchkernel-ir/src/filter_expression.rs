//! Schema-aware binding for the standalone filter expression language.

use crate::{
    ComparisonIr, ComparisonOp, FieldIr, FieldKind, FilterIr, IndexDefinitionIr, PortableError,
    Result, ScalarValueIr, Value,
};
use searchkernel_filter::{
    CompareOp, ErrorKind, Expr, InValues, ParseError, Predicate, Span, Spanned, ValueExpr,
};
use std::collections::{BTreeMap, BTreeSet};

/// Typed request parameters available to a filter expression.
pub type FilterParameters = BTreeMap<String, Value>;

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

fn bind_parsed_filter(
    parsed: &Spanned<Expr>,
    schema: &IndexDefinitionIr,
    params: &FilterParameters,
) -> Result<BoundFilter> {
    let mut referenced_parameters = BTreeSet::new();
    let expression = bind_expr(parsed, schema, params, &mut referenced_parameters)?;
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

fn parse_error_to_portable(error: ParseError) -> PortableError {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        api, AnalyzerIr, DataTypeIr, FieldCapabilities, FieldIr, FieldName, IndexContext,
        PrefixPolicy, PrimitiveDataTypeIr, RankingPolicyIr, TypoPolicy,
    };
    use std::collections::BTreeSet;

    fn schema() -> IndexDefinitionIr {
        let field = |name, kind, filterable| {
            let data_type = match kind {
                FieldKind::Text | FieldKind::Keyword => {
                    DataTypeIr::Primitive(PrimitiveDataTypeIr::String)
                }
                FieldKind::Int64 => DataTypeIr::Primitive(PrimitiveDataTypeIr::Int64),
                FieldKind::Float64 => DataTypeIr::Primitive(PrimitiveDataTypeIr::Float64),
                FieldKind::Bool => DataTypeIr::Primitive(PrimitiveDataTypeIr::Bool),
            };
            let mut field = FieldIr::new(
                FieldName::parse(name).unwrap(),
                data_type,
                (kind == FieldKind::Text).then(|| AnalyzerIr {
                    name: "standard".to_owned(),
                    version: 1,
                    options: std::collections::BTreeMap::new(),
                }),
                FieldCapabilities::new(false, false, filterable, false, false),
            );
            if kind == FieldKind::Keyword {
                field.matching = Some(crate::MatchingConfigIr {
                    mode: crate::MatchingModeIr::Exact,
                    options: std::collections::BTreeMap::new(),
                });
            }
            field
        };
        IndexDefinitionIr::new(
            None,
            vec![
                field("brand", FieldKind::Keyword, true),
                field("price", FieldKind::Int64, true),
                field("rating", FieldKind::Float64, true),
                field("in_stock", FieldKind::Bool, true),
                field("title", FieldKind::Text, true),
                field("hidden", FieldKind::Keyword, false),
            ],
            Vec::new(),
            RankingPolicyIr::DefaultRelevanceV1,
            PrefixPolicy::Disabled,
            TypoPolicy::Disabled,
        )
        .unwrap()
    }

    fn parameters(values: impl IntoIterator<Item = (&'static str, Value)>) -> FilterParameters {
        values
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value))
            .collect()
    }

    fn comparison(field: &str, op: ComparisonOp, value: ScalarValueIr) -> FilterIr {
        FilterIr::Comparison(ComparisonIr::new(
            FieldName::parse(field).unwrap(),
            op,
            value,
        ))
    }

    fn structured_context() -> IndexContext {
        let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
        let filterable = api::OptionalBool::new(true);
        let fields = [
            structured_field(
                &mut builder,
                "brand",
                api::PrimitiveDataTypeKind::String,
                &filterable,
            ),
            structured_field(
                &mut builder,
                "price",
                api::PrimitiveDataTypeKind::Int64,
                &filterable,
            ),
            structured_field(
                &mut builder,
                "rating",
                api::PrimitiveDataTypeKind::Float64,
                &filterable,
            ),
            structured_field(
                &mut builder,
                "in_stock",
                api::PrimitiveDataTypeKind::Bool,
                &filterable,
            ),
        ];
        let fields = builder.create_vector(&fields);
        let definition = api::IndexDefinition::create(
            &mut builder,
            &api::IndexDefinitionArgs {
                fields: Some(fields),
                ..Default::default()
            },
        );
        builder.finish(definition, None);
        let definition =
            crate::__flatbuffers::root::<api::IndexDefinition>(builder.finished_data())
                .expect("valid structured test schema");
        IndexContext::from_api(definition).expect("structured test schema converts")
    }

    fn structured_field<'bldr, 'args, A: crate::__flatbuffers::Allocator + 'bldr>(
        builder: &'args mut crate::__flatbuffers::FlatBufferBuilder<'bldr, A>,
        name: &str,
        kind: api::PrimitiveDataTypeKind,
        filterable: &api::OptionalBool,
    ) -> crate::__flatbuffers::WIPOffset<api::FieldDefinition<'bldr>>
    where
        'bldr: 'args,
    {
        let name = builder.create_string(name);
        let primitive =
            api::PrimitiveDataType::create(builder, &api::PrimitiveDataTypeArgs { kind });
        let data_type = api::DataType::create(
            builder,
            &api::DataTypeArgs {
                kind_type: api::DataTypeKind::PrimitiveDataType,
                kind: Some(primitive.as_union_value()),
            },
        );
        let matching = (kind == api::PrimitiveDataTypeKind::String).then(|| {
            api::MatchingConfig::create(
                builder,
                &api::MatchingConfigArgs {
                    mode: api::MatchingMode::Exact,
                    ..Default::default()
                },
            )
        });
        api::FieldDefinition::create(
            builder,
            &api::FieldDefinitionArgs {
                name: Some(name),
                data_type: Some(data_type),
                filterable: Some(filterable),
                matching,
                ..Default::default()
            },
        )
    }

    enum StructuredValue {
        String(&'static str),
        Int64(i64),
        Float64(f64),
        Bool(bool),
    }

    enum StructuredFilter {
        Compare {
            field: &'static str,
            op: api::ComparisonOp,
            value: StructuredValue,
        },
        In {
            field: &'static str,
            values: Vec<StructuredValue>,
        },
        And(Vec<StructuredFilter>),
        Or(Vec<StructuredFilter>),
        Not(Box<StructuredFilter>),
    }

    fn structured_value<'bldr, 'args, A: crate::__flatbuffers::Allocator + 'bldr>(
        builder: &'args mut crate::__flatbuffers::FlatBufferBuilder<'bldr, A>,
        value: StructuredValue,
    ) -> crate::__flatbuffers::WIPOffset<api::Value<'bldr>>
    where
        'bldr: 'args,
    {
        let (kind_type, kind) = match value {
            StructuredValue::String(value) => {
                let value = builder.create_string(value);
                let value =
                    api::StringValue::create(builder, &api::StringValueArgs { value: Some(value) });
                (api::ValueKind::StringValue, value.as_union_value())
            }
            StructuredValue::Int64(value) => {
                let value = api::Int64Value::create(builder, &api::Int64ValueArgs { value });
                (api::ValueKind::Int64Value, value.as_union_value())
            }
            StructuredValue::Float64(value) => {
                let value = api::Float64Value::create(builder, &api::Float64ValueArgs { value });
                (api::ValueKind::Float64Value, value.as_union_value())
            }
            StructuredValue::Bool(value) => {
                let value = api::BoolValue::create(builder, &api::BoolValueArgs { value });
                (api::ValueKind::BoolValue, value.as_union_value())
            }
        };
        api::Value::create(
            builder,
            &api::ValueArgs {
                kind_type,
                kind: Some(kind),
            },
        )
    }

    fn structured_filter<'bldr, 'args, A: crate::__flatbuffers::Allocator + 'bldr>(
        builder: &'args mut crate::__flatbuffers::FlatBufferBuilder<'bldr, A>,
        filter: StructuredFilter,
    ) -> crate::__flatbuffers::WIPOffset<api::FilterExpr<'bldr>>
    where
        'bldr: 'args,
    {
        let (expr_type, expr) = match filter {
            StructuredFilter::Compare { field, op, value } => {
                let field = field_ref(builder, field);
                let value = structured_value(builder, value);
                let comparison = api::FilterComparison::create(
                    builder,
                    &api::FilterComparisonArgs {
                        field: Some(field),
                        op,
                        value: Some(value),
                    },
                );
                (
                    api::FilterExprKind::FilterComparison,
                    comparison.as_union_value(),
                )
            }
            StructuredFilter::In { field, values } => {
                let field = field_ref(builder, field);
                let values = values
                    .into_iter()
                    .map(|value| structured_value(builder, value))
                    .collect::<Vec<_>>();
                let values = builder.create_vector(&values);
                let input = api::FilterIn::create(
                    builder,
                    &api::FilterInArgs {
                        field: Some(field),
                        values: Some(values),
                    },
                );
                (api::FilterExprKind::FilterIn, input.as_union_value())
            }
            StructuredFilter::And(filters) => {
                let clauses = filters
                    .into_iter()
                    .map(|filter| structured_filter(builder, filter))
                    .collect::<Vec<_>>();
                let clauses = builder.create_vector(&clauses);
                let input = api::FilterAnd::create(
                    builder,
                    &api::FilterAndArgs {
                        clauses: Some(clauses),
                    },
                );
                (api::FilterExprKind::FilterAnd, input.as_union_value())
            }
            StructuredFilter::Or(filters) => {
                let clauses = filters
                    .into_iter()
                    .map(|filter| structured_filter(builder, filter))
                    .collect::<Vec<_>>();
                let clauses = builder.create_vector(&clauses);
                let input = api::FilterOr::create(
                    builder,
                    &api::FilterOrArgs {
                        clauses: Some(clauses),
                    },
                );
                (api::FilterExprKind::FilterOr, input.as_union_value())
            }
            StructuredFilter::Not(filter) => {
                let clause = structured_filter(builder, *filter);
                let input = api::FilterNot::create(
                    builder,
                    &api::FilterNotArgs {
                        clause: Some(clause),
                    },
                );
                (api::FilterExprKind::FilterNot, input.as_union_value())
            }
        };
        api::FilterExpr::create(
            builder,
            &api::FilterExprArgs {
                expr_type,
                expr: Some(expr),
            },
        )
    }

    fn field_ref<'bldr, A: crate::__flatbuffers::Allocator + 'bldr>(
        builder: &mut crate::__flatbuffers::FlatBufferBuilder<'bldr, A>,
        value: &str,
    ) -> crate::__flatbuffers::WIPOffset<api::QueryFieldRef<'bldr>> {
        let path = builder.create_string(value);
        api::QueryFieldRef::create(
            builder,
            &api::QueryFieldRefArgs {
                path: Some(path),
                element_index: None,
            },
        )
    }

    fn resolve_structured(context: &IndexContext, filter: StructuredFilter) -> FilterIr {
        let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
        let expression = structured_filter(&mut builder, filter);
        builder.finish(expression, None);
        let expression = crate::__flatbuffers::root::<api::FilterExpr>(builder.finished_data())
            .expect("valid structured filter");
        context
            .filter_to_ir(expression)
            .expect("structured filter converts")
    }

    fn structured_string_filter(context: &IndexContext) -> FilterIr {
        let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
        let field = field_ref(&mut builder, "brand");
        let text = builder.create_string("Acme");
        let string =
            api::StringValue::create(&mut builder, &api::StringValueArgs { value: Some(text) });
        let scalar = api::Value::create(
            &mut builder,
            &api::ValueArgs {
                kind_type: api::ValueKind::StringValue,
                kind: Some(string.as_union_value()),
            },
        );
        let comparison = api::FilterComparison::create(
            &mut builder,
            &api::FilterComparisonArgs {
                field: Some(field),
                op: api::ComparisonOp::Eq,
                value: Some(scalar),
            },
        );
        let expression = api::FilterExpr::create(
            &mut builder,
            &api::FilterExprArgs {
                expr_type: api::FilterExprKind::FilterComparison,
                expr: Some(comparison.as_union_value()),
            },
        );
        builder.finish(expression, None);
        let expression = crate::__flatbuffers::root::<api::FilterExpr>(builder.finished_data())
            .expect("valid structured comparison");
        context
            .filter_to_ir(expression)
            .expect("comparison converts")
    }

    fn structured_in_filter(context: &IndexContext) -> FilterIr {
        let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
        let field = field_ref(&mut builder, "brand");
        let acme_text = builder.create_string("Acme");
        let acme_string = api::StringValue::create(
            &mut builder,
            &api::StringValueArgs {
                value: Some(acme_text),
            },
        );
        let acme = api::Value::create(
            &mut builder,
            &api::ValueArgs {
                kind_type: api::ValueKind::StringValue,
                kind: Some(acme_string.as_union_value()),
            },
        );
        let trailco_text = builder.create_string("TrailCo");
        let trailco_string = api::StringValue::create(
            &mut builder,
            &api::StringValueArgs {
                value: Some(trailco_text),
            },
        );
        let trailco = api::Value::create(
            &mut builder,
            &api::ValueArgs {
                kind_type: api::ValueKind::StringValue,
                kind: Some(trailco_string.as_union_value()),
            },
        );
        let values = builder.create_vector(&[acme, trailco]);
        let input = api::FilterIn::create(
            &mut builder,
            &api::FilterInArgs {
                field: Some(field),
                values: Some(values),
            },
        );
        let expression = api::FilterExpr::create(
            &mut builder,
            &api::FilterExprArgs {
                expr_type: api::FilterExprKind::FilterIn,
                expr: Some(input.as_union_value()),
            },
        );
        builder.finish(expression, None);
        let expression = crate::__flatbuffers::root::<api::FilterExpr>(builder.finished_data())
            .expect("valid structured IN");
        context.filter_to_ir(expression).expect("IN converts")
    }

    fn assert_detail(source: &str, params: &FilterParameters, code: &str) {
        let error = bind_filter_expression(source, &schema(), params).unwrap_err();
        assert_eq!(error.code, crate::ErrorCode::InvalidArgument);
        assert_eq!(error.details[0].code, code, "{error:?}");
        assert!(error.details[0].metadata.contains_key("start_offset"));
        assert!(error.details[0].metadata.contains_key("end_offset"));
    }

    #[test]
    fn textual_filters_match_the_structured_ir() {
        assert_eq!(
            bind_filter_expression("brand = 'Acme'", &schema(), &FilterParameters::new()).unwrap(),
            comparison(
                "brand",
                ComparisonOp::Eq,
                ScalarValueIr::String("Acme".to_owned())
            )
        );
        assert_eq!(
            bind_filter_expression(
                "price <= 120 AND in_stock = true",
                &schema(),
                &FilterParameters::new(),
            )
            .unwrap(),
            FilterIr::And(vec![
                comparison("price", ComparisonOp::Lte, ScalarValueIr::Int64(120)),
                comparison("in_stock", ComparisonOp::Eq, ScalarValueIr::Bool(true)),
            ])
        );
        assert_eq!(
            bind_filter_expression(
                "rating > 4.5 OR NOT (brand = 'Acme')",
                &schema(),
                &FilterParameters::new(),
            )
            .unwrap(),
            FilterIr::Or(vec![
                comparison("rating", ComparisonOp::Gt, ScalarValueIr::Float64(4.5)),
                FilterIr::Not(Box::new(comparison(
                    "brand",
                    ComparisonOp::Eq,
                    ScalarValueIr::String("Acme".to_owned()),
                ))),
            ])
        );
        assert_eq!(
            bind_filter_expression(
                "brand IN ('Acme', 'TrailCo')",
                &schema(),
                &FilterParameters::new(),
            )
            .unwrap(),
            FilterIr::In {
                field: FieldName::parse("brand").unwrap(),
                values: vec![
                    ScalarValueIr::String("Acme".to_owned()),
                    ScalarValueIr::String("TrailCo".to_owned()),
                ],
            }
        );
    }

    #[test]
    fn textual_filters_equal_the_existing_structured_conversion() {
        let context = structured_context();
        assert_eq!(
            bind_filter_expression(
                "brand = 'Acme'",
                context.definition(),
                &FilterParameters::new(),
            )
            .unwrap(),
            structured_string_filter(&context)
        );
        assert_eq!(
            bind_filter_expression(
                "brand IN ('Acme', 'TrailCo')",
                context.definition(),
                &FilterParameters::new(),
            )
            .unwrap(),
            structured_in_filter(&context)
        );
    }

    #[test]
    fn all_textual_filter_operators_equal_structured_conversion() {
        let context = structured_context();
        let cases = [
            (
                "price = 4",
                StructuredFilter::Compare {
                    field: "price",
                    op: api::ComparisonOp::Eq,
                    value: StructuredValue::Int64(4),
                },
            ),
            (
                "price < 4",
                StructuredFilter::Compare {
                    field: "price",
                    op: api::ComparisonOp::Lt,
                    value: StructuredValue::Int64(4),
                },
            ),
            (
                "price <= 4",
                StructuredFilter::Compare {
                    field: "price",
                    op: api::ComparisonOp::Lte,
                    value: StructuredValue::Int64(4),
                },
            ),
            (
                "price > 4",
                StructuredFilter::Compare {
                    field: "price",
                    op: api::ComparisonOp::Gt,
                    value: StructuredValue::Int64(4),
                },
            ),
            (
                "price >= 4",
                StructuredFilter::Compare {
                    field: "price",
                    op: api::ComparisonOp::Gte,
                    value: StructuredValue::Int64(4),
                },
            ),
            (
                "rating = 4.5",
                StructuredFilter::Compare {
                    field: "rating",
                    op: api::ComparisonOp::Eq,
                    value: StructuredValue::Float64(4.5),
                },
            ),
            (
                "rating < 4.5",
                StructuredFilter::Compare {
                    field: "rating",
                    op: api::ComparisonOp::Lt,
                    value: StructuredValue::Float64(4.5),
                },
            ),
            (
                "rating <= 4.5",
                StructuredFilter::Compare {
                    field: "rating",
                    op: api::ComparisonOp::Lte,
                    value: StructuredValue::Float64(4.5),
                },
            ),
            (
                "rating > 4.5",
                StructuredFilter::Compare {
                    field: "rating",
                    op: api::ComparisonOp::Gt,
                    value: StructuredValue::Float64(4.5),
                },
            ),
            (
                "rating >= 4.5",
                StructuredFilter::Compare {
                    field: "rating",
                    op: api::ComparisonOp::Gte,
                    value: StructuredValue::Float64(4.5),
                },
            ),
            (
                "in_stock = true",
                StructuredFilter::Compare {
                    field: "in_stock",
                    op: api::ComparisonOp::Eq,
                    value: StructuredValue::Bool(true),
                },
            ),
            (
                "brand IN ('Acme', 'TrailCo')",
                StructuredFilter::In {
                    field: "brand",
                    values: vec![
                        StructuredValue::String("Acme"),
                        StructuredValue::String("TrailCo"),
                    ],
                },
            ),
            (
                "price IN (1, 2, 3)",
                StructuredFilter::In {
                    field: "price",
                    values: vec![
                        StructuredValue::Int64(1),
                        StructuredValue::Int64(2),
                        StructuredValue::Int64(3),
                    ],
                },
            ),
            (
                "price <= 120 AND in_stock = true",
                StructuredFilter::And(vec![
                    StructuredFilter::Compare {
                        field: "price",
                        op: api::ComparisonOp::Lte,
                        value: StructuredValue::Int64(120),
                    },
                    StructuredFilter::Compare {
                        field: "in_stock",
                        op: api::ComparisonOp::Eq,
                        value: StructuredValue::Bool(true),
                    },
                ]),
            ),
            (
                "brand = 'Acme' OR brand = 'TrailCo'",
                StructuredFilter::Or(vec![
                    StructuredFilter::Compare {
                        field: "brand",
                        op: api::ComparisonOp::Eq,
                        value: StructuredValue::String("Acme"),
                    },
                    StructuredFilter::Compare {
                        field: "brand",
                        op: api::ComparisonOp::Eq,
                        value: StructuredValue::String("TrailCo"),
                    },
                ]),
            ),
            (
                "NOT (price > 10 AND (in_stock = false OR brand = 'Acme'))",
                StructuredFilter::Not(Box::new(StructuredFilter::And(vec![
                    StructuredFilter::Compare {
                        field: "price",
                        op: api::ComparisonOp::Gt,
                        value: StructuredValue::Int64(10),
                    },
                    StructuredFilter::Or(vec![
                        StructuredFilter::Compare {
                            field: "in_stock",
                            op: api::ComparisonOp::Eq,
                            value: StructuredValue::Bool(false),
                        },
                        StructuredFilter::Compare {
                            field: "brand",
                            op: api::ComparisonOp::Eq,
                            value: StructuredValue::String("Acme"),
                        },
                    ]),
                ]))),
            ),
        ];
        for (source, structured) in cases {
            assert_eq!(
                bind_filter_expression(source, context.definition(), &FilterParameters::new())
                    .unwrap(),
                resolve_structured(&context, structured),
                "source: {source}"
            );
        }
    }

    #[test]
    fn typed_parameters_and_references_are_preserved() {
        let params = parameters([
            ("max", Value::Int64(120)),
            ("brand", Value::String("Acme".to_owned())),
            (
                "brands",
                Value::List(vec![
                    Value::String("Acme".to_owned()),
                    Value::String("TrailCo".to_owned()),
                ]),
            ),
        ]);
        let bound = bind_filter_expression_with_references(
            "price <= :max AND brand = :brand AND brand IN (:brands)",
            &schema(),
            &params,
        )
        .unwrap();
        assert_eq!(
            bound.referenced_parameters(),
            &BTreeSet::from(["brand".to_owned(), "brands".to_owned(), "max".to_owned()])
        );
        assert!(matches!(bound.expression(), FilterIr::And(clauses) if clauses.len() == 3));
    }

    #[test]
    fn binder_rejects_invalid_fields_operators_and_types() {
        let empty = FilterParameters::new();
        assert_detail("missing = 1", &empty, "unknown_field");
        assert_detail("hidden = 'x'", &empty, "field_not_filterable");
        assert_detail("title = 'x'", &empty, "unsupported_operator");
        assert_detail("brand < 'x'", &empty, "unsupported_operator");
        assert_detail("in_stock < true", &empty, "unsupported_operator");
        assert_detail("price <= true", &empty, "type_mismatch");
        assert_detail("price <= 4.5", &empty, "type_mismatch");
        assert_detail("rating IN (1, 2)", &empty, "unsupported_operator");
        assert_detail("price = 9223372036854775808", &empty, "invalid_literal");
        assert_detail("rating = 1e309", &empty, "non_finite_number");
    }

    #[test]
    fn binder_rejects_invalid_parameter_use() {
        assert_detail(
            "price <= :missing",
            &FilterParameters::new(),
            "unknown_parameter",
        );
        assert_detail(
            "price <= :max",
            &parameters([("max", Value::String("120".to_owned()))]),
            "type_mismatch",
        );
        assert_detail(
            "price <= :max",
            &parameters([("max", Value::List(vec![Value::Int64(120)]))]),
            "type_mismatch",
        );
        assert_detail(
            "brand IN (:brands)",
            &parameters([("brands", Value::String("Acme".to_owned()))]),
            "type_mismatch",
        );
        assert_detail(
            "brand IN (:brands)",
            &parameters([("brands", Value::List(vec![Value::Int64(1)]))]),
            "type_mismatch",
        );
        assert_detail(
            "brand IN (:brands)",
            &parameters([("brands", Value::List(Vec::new()))]),
            "empty_in",
        );
    }

    #[test]
    fn numeric_parameters_are_normalized_to_the_field_type() {
        assert_eq!(
            bind_filter_expression(
                "rating <= :max",
                &schema(),
                &parameters([("max", Value::Int64(120))]),
            )
            .unwrap(),
            comparison("rating", ComparisonOp::Lte, ScalarValueIr::Float64(120.0))
        );
        assert_eq!(
            bind_filter_expression(
                "price = :value",
                &schema(),
                &parameters([("value", Value::UInt64(120))]),
            )
            .unwrap(),
            comparison("price", ComparisonOp::Eq, ScalarValueIr::Int64(120))
        );
        assert_eq!(
            bind_filter_expression(
                "rating = :value",
                &schema(),
                &parameters([("value", Value::UInt64(120))]),
            )
            .unwrap(),
            comparison("rating", ComparisonOp::Eq, ScalarValueIr::Float64(120.0))
        );
    }

    #[test]
    fn parser_errors_become_portable_errors() {
        assert_detail("brand =", &FilterParameters::new(), "syntax_error");
        assert_detail("brand IN ()", &FilterParameters::new(), "empty_in");
    }
}
