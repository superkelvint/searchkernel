//! Lower portable filters into Vespa attribute query nodes.

use crate::unsupported;
use searchkernel_ir::{FilterIr, RangeIr, Result, ScalarValueIr};
use searchkernel_vespa_engine::{InQueryValue, Query, Value as VespaValue};

pub(crate) fn lower_filter(filter: &FilterIr) -> Result<Query> {
    match filter {
        FilterIr::And(children) => Ok(Query::And(
            children
                .iter()
                .map(lower_filter)
                .collect::<Result<Vec<_>>>()?,
        )),
        FilterIr::Or(children) => Ok(Query::Or(
            children
                .iter()
                .map(lower_filter)
                .collect::<Result<Vec<_>>>()?,
        )),
        FilterIr::Not(child) => Ok(Query::Not(Box::new(lower_filter(child)?))),
        FilterIr::Exists { field } => Ok(Query::AttributeRange {
            field: field.to_string(),
            lower: searchkernel_vespa_engine::AttributeBound::Unbounded,
            upper: searchkernel_vespa_engine::AttributeBound::Unbounded,
        }),
        FilterIr::Comparison(comparison) => lower_comparison(comparison),
        FilterIr::Range(range) => lower_range(range),
        FilterIr::In { field, values } => Ok(Query::In {
            field: field.to_string(),
            values: values
                .iter()
                .map(lower_in_value)
                .collect::<Result<Vec<_>>>()?,
        }),
    }
}

pub(crate) fn lower_comparison(comparison: &searchkernel_ir::ComparisonIr) -> Result<Query> {
    use searchkernel_ir::ComparisonOp;
    let field = comparison.field().to_string();
    let value = lower_attribute_value(comparison.value())?;
    match comparison.op() {
        ComparisonOp::Eq => Ok(Query::AttributeEquals { field, value }),
        ComparisonOp::Ne => Ok(Query::Not(Box::new(Query::AttributeEquals {
            field,
            value,
        }))),
        ComparisonOp::Lt | ComparisonOp::Lte | ComparisonOp::Gt | ComparisonOp::Gte => {
            let lower_bound = matches!(comparison.op(), ComparisonOp::Gt | ComparisonOp::Gte);
            let included = matches!(comparison.op(), ComparisonOp::Gte | ComparisonOp::Lte);
            if !matches!(value, VespaValue::Integer(_) | VespaValue::Float(_)) {
                return Err(unsupported("ordered comparisons require a numeric field"));
            }
            let bound = if included {
                searchkernel_vespa_engine::AttributeBound::Included(value)
            } else {
                searchkernel_vespa_engine::AttributeBound::Excluded(value)
            };
            Ok(Query::AttributeRange {
                field,
                lower: if lower_bound {
                    bound.clone()
                } else {
                    searchkernel_vespa_engine::AttributeBound::Unbounded
                },
                upper: if lower_bound {
                    searchkernel_vespa_engine::AttributeBound::Unbounded
                } else {
                    bound
                },
            })
        }
    }
}

pub(crate) fn lower_range(range: &RangeIr) -> Result<Query> {
    Ok(Query::AttributeRange {
        field: range.field().to_string(),
        lower: attribute_bound(range.lower_bound(), range.include_lower())?,
        upper: attribute_bound(range.upper_bound(), range.include_upper())?,
    })
}

pub(crate) fn attribute_bound(
    value: &searchkernel_ir::FilterRangeBoundIr,
    included: bool,
) -> Result<searchkernel_vespa_engine::AttributeBound> {
    match value {
        searchkernel_ir::FilterRangeBoundIr::Unbounded => {
            Ok(searchkernel_vespa_engine::AttributeBound::Unbounded)
        }
        searchkernel_ir::FilterRangeBoundIr::NegativeInfinity
        | searchkernel_ir::FilterRangeBoundIr::PositiveInfinity => Err(unsupported(
            "infinite filter bounds are not supported by the native adapter",
        )),
        searchkernel_ir::FilterRangeBoundIr::Finite(value) => {
            let value = lower_attribute_value(value)?;
            Ok(if included {
                searchkernel_vespa_engine::AttributeBound::Included(value)
            } else {
                searchkernel_vespa_engine::AttributeBound::Excluded(value)
            })
        }
    }
}

pub(crate) fn lower_attribute_value(value: &ScalarValueIr) -> Result<VespaValue> {
    match value {
        ScalarValueIr::Int64(value) => Ok(VespaValue::Integer(integer_query_value(*value)?)),
        ScalarValueIr::Float64(value) if value.is_finite() => Ok(VespaValue::Float(*value)),
        ScalarValueIr::String(value) => Ok(VespaValue::String(value.clone())),
        ScalarValueIr::Bool(value) => Ok(VespaValue::Boolean(*value)),
        ScalarValueIr::Float64(_) => Err(unsupported("float comparison value must be finite")),
    }
}

// Both filter membership and typed-query IN must preserve native InTerm's
// string/integer representation, including the undefined integer sentinel rule.
pub(super) fn lower_in_value(value: &ScalarValueIr) -> Result<InQueryValue> {
    match lower_attribute_value(value)? {
        VespaValue::String(value) => Ok(InQueryValue::String(value)),
        VespaValue::Integer(value) => Ok(InQueryValue::Integer(value)),
        _ => Err(unsupported(
            "in query supports only string and integer values",
        )),
    }
}

pub(crate) fn integer_query_value(value: i64) -> Result<i64> {
    if value == i64::MIN {
        return Err(unsupported(
            "INT64_MIN is reserved as Vespa's undefined integer attribute sentinel",
        ));
    }
    Ok(value)
}
