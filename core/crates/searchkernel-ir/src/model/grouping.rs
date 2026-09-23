//! Grouping and highlighting settings.

use super::*;
#[derive(Clone, Debug, PartialEq)]
pub struct GroupingRequestIr {
    pub root: GroupingAllIr,
    pub continuations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GroupingAllIr {
    pub operations: Vec<GroupingOperationIr>,
    pub label: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GroupingOperationIr {
    All(GroupingAllIr),
    Each(GroupingAllIr),
    Group(GroupingExpressionIr),
    Output(Vec<GroupingOutputIr>),
    Max(Option<u32>),
    Precision(u32),
    Order(Vec<GroupingExpressionIr>),
    Alias {
        name: String,
        expression: GroupingExpressionIr,
    },
    Filter(GroupingFilterIr),
    Keep(GroupingFilterIr),
    Where(GroupingExpressionIr),
}

#[derive(Clone, Debug, PartialEq)]
pub struct GroupingOutputIr {
    pub label: Option<String>,
    pub expression: GroupingExpressionIr,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GroupingExpressionIr {
    Attribute(String),
    Literal(RequestValueIr),
    Function {
        name: String,
        arguments: Vec<GroupingExpressionIr>,
    },
    Positive(Box<GroupingExpressionIr>),
    Negative(Box<GroupingExpressionIr>),
    Binary {
        operator: GroupingBinaryOperatorIr,
        left: Box<GroupingExpressionIr>,
        right: Box<GroupingExpressionIr>,
    },
    Alias(String),
    Member {
        base: Box<GroupingExpressionIr>,
        member: String,
    },
    MapLookup {
        base: Box<GroupingExpressionIr>,
        key: Box<GroupingExpressionIr>,
    },
    Bucket(GroupingBucketIr),
    Aggregation(Box<GroupingAggregationIr>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupingBinaryOperatorIr {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GroupingBucketIr {
    pub lower: GroupingRangeBoundIr,
    pub upper: GroupingRangeBoundIr,
    pub include_lower: bool,
    pub include_upper: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GroupingRangeBoundIr {
    Finite(RequestValueIr),
    NegativeInfinity,
    PositiveInfinity,
    Unbounded,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GroupingAggregationIr {
    Count,
    Sum(GroupingExpressionIr),
    Avg(GroupingExpressionIr),
    Min(GroupingExpressionIr),
    Max(GroupingExpressionIr),
    Xor(GroupingExpressionIr),
    StdDev(GroupingExpressionIr),
    Quantiles {
        quantiles: Vec<f64>,
        expression: GroupingExpressionIr,
    },
    Summary(Option<String>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum GroupingFilterIr {
    And(Vec<GroupingFilterIr>),
    Or(Vec<GroupingFilterIr>),
    Not(Box<GroupingFilterIr>),
    Regex {
        expression: Box<GroupingExpressionIr>,
        pattern: String,
    },
    Range {
        expression: Box<GroupingExpressionIr>,
        lower: Box<GroupingExpressionIr>,
        upper: Box<GroupingExpressionIr>,
        include_lower: bool,
        include_upper: bool,
    },
    IsTrue(Box<GroupingExpressionIr>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HighlightingIr {
    Disabled,
    Requested {
        fields: Vec<FieldName>,
        /// Keep resolved cardinality with the request so the Vespa adapter
        /// can reject shapes its scalar-only dynamic-teaser path cannot run.
        multi_valued_fields: Vec<FieldName>,
    },
}

/// Return the canonical portable result-field name for a grouping output.
///
/// Explicit labels win. Unlabeled aggregation outputs use one backend-independent
/// name derived from canonical IR so semantic validation and result projection
/// cannot disagree after native lowering changes enum/type spellings.
pub fn effective_grouping_output_name(
    output: &GroupingOutputIr,
    aliases: &std::collections::BTreeMap<String, GroupingExpressionIr>,
) -> Option<String> {
    output
        .label
        .clone()
        .or_else(|| grouping_expression_output_name(&output.expression, aliases))
}

fn grouping_expression_output_name(
    value: &GroupingExpressionIr,
    aliases: &std::collections::BTreeMap<String, GroupingExpressionIr>,
) -> Option<String> {
    match value {
        GroupingExpressionIr::Alias(name) => aliases
            .get(name)
            .and_then(|value| grouping_expression_output_name(value, aliases)),
        GroupingExpressionIr::Aggregation(aggregation) => {
            Some(grouping_aggregation_output_name(aggregation))
        }
        _ => None,
    }
}

fn grouping_aggregation_output_name(value: &GroupingAggregationIr) -> String {
    let expression_name = |expression: &GroupingExpressionIr| match expression {
        GroupingExpressionIr::Attribute(field) => field.clone(),
        expression => format!("{expression:?}"),
    };
    match value {
        GroupingAggregationIr::Count => "count".to_owned(),
        GroupingAggregationIr::Avg(expression) => {
            format!("average({})", expression_name(expression))
        }
        GroupingAggregationIr::Sum(expression) => format!("sum({})", expression_name(expression)),
        GroupingAggregationIr::Min(expression) => format!("min({})", expression_name(expression)),
        GroupingAggregationIr::Max(expression) => format!("max({})", expression_name(expression)),
        GroupingAggregationIr::Xor(expression) => format!("xor({})", expression_name(expression)),
        GroupingAggregationIr::StdDev(expression) => {
            format!("stddev({})", expression_name(expression))
        }
        GroupingAggregationIr::Quantiles { expression, .. } => {
            format!("quantiles({})", expression_name(expression))
        }
        GroupingAggregationIr::Summary(_) => "summary".to_owned(),
    }
}
