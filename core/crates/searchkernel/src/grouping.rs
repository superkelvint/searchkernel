use crate::document::{pending, PendingValue};
use crate::error::{Error, Result};
use searchkernel_ir::{
    GroupingAggregationIr, GroupingAllIr, GroupingBinaryOperatorIr, GroupingExpressionIr,
    GroupingFilterIr, GroupingOperationIr, GroupingOutputIr, GroupingRequestIr,
};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupingExpression(GroupingExpressionNode);

#[derive(Clone, Debug, PartialEq, Eq)]
enum GroupingExpressionNode {
    Attribute(String),
    Literal(PendingValue),
    Function {
        name: String,
        arguments: Vec<GroupingExpression>,
    },
    Positive(Box<GroupingExpression>),
    Negative(Box<GroupingExpression>),
    Binary {
        operator: GroupingBinaryOperatorIr,
        left: Box<GroupingExpression>,
        right: Box<GroupingExpression>,
    },
}

impl GroupingExpression {
    pub fn attribute(field: impl Into<String>) -> Self {
        Self(GroupingExpressionNode::Attribute(field.into()))
    }

    pub fn literal<T: Serialize>(value: T) -> Self {
        Self(GroupingExpressionNode::Literal(pending(&value)))
    }

    pub fn function<I, E>(name: impl Into<String>, arguments: I) -> Self
    where
        I: IntoIterator<Item = E>,
        E: Into<GroupingExpression>,
    {
        Self(GroupingExpressionNode::Function {
            name: name.into(),
            arguments: arguments.into_iter().map(Into::into).collect(),
        })
    }

    pub fn positive(self) -> Self {
        Self(GroupingExpressionNode::Positive(Box::new(self)))
    }

    pub fn negative(self) -> Self {
        Self(GroupingExpressionNode::Negative(Box::new(self)))
    }

    pub fn plus(self, right: impl Into<GroupingExpression>) -> Self {
        self.binary(GroupingBinaryOperatorIr::Add, right)
    }
    pub fn subtract(self, right: impl Into<GroupingExpression>) -> Self {
        self.binary(GroupingBinaryOperatorIr::Subtract, right)
    }
    pub fn multiply(self, right: impl Into<GroupingExpression>) -> Self {
        self.binary(GroupingBinaryOperatorIr::Multiply, right)
    }
    pub fn divide(self, right: impl Into<GroupingExpression>) -> Self {
        self.binary(GroupingBinaryOperatorIr::Divide, right)
    }
    pub fn modulo(self, right: impl Into<GroupingExpression>) -> Self {
        self.binary(GroupingBinaryOperatorIr::Modulo, right)
    }

    fn binary(
        self,
        operator: GroupingBinaryOperatorIr,
        right: impl Into<GroupingExpression>,
    ) -> Self {
        Self(GroupingExpressionNode::Binary {
            operator,
            left: Box::new(self),
            right: Box::new(right.into()),
        })
    }
}

impl From<&str> for GroupingExpression {
    fn from(value: &str) -> Self {
        Self::attribute(value)
    }
}
impl From<String> for GroupingExpression {
    fn from(value: String) -> Self {
        Self::attribute(value)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum GroupingAggregation {
    Count,
    Average(GroupingExpression),
    Sum(GroupingExpression),
    Min(GroupingExpression),
    Max(GroupingExpression),
    Xor(GroupingExpression),
    StdDev(GroupingExpression),
    Quantiles {
        expression: GroupingExpression,
        quantiles: Vec<f64>,
    },
    Summary(Option<String>),
    /// Compatibility form for referring to an aggregation alias as an expression.
    AliasRef(String),
    /// Compatibility form for attaching an output label to an aggregation.
    Named(String, Box<GroupingAggregation>),
}

impl GroupingAggregation {
    pub fn count() -> Self {
        Self::Count
    }
    pub fn average(expression: impl Into<GroupingExpression>) -> Self {
        Self::Average(expression.into())
    }
    pub fn avg(field: impl Into<String>) -> Self {
        Self::average(field.into())
    }
    pub fn sum(expression: impl Into<GroupingExpression>) -> Self {
        Self::Sum(expression.into())
    }
    pub fn min(expression: impl Into<GroupingExpression>) -> Self {
        Self::Min(expression.into())
    }
    pub fn max(expression: impl Into<GroupingExpression>) -> Self {
        Self::Max(expression.into())
    }
    pub fn xor(expression: impl Into<GroupingExpression>) -> Self {
        Self::Xor(expression.into())
    }
    pub fn stddev(expression: impl Into<GroupingExpression>) -> Self {
        Self::StdDev(expression.into())
    }
    pub fn quantiles<I>(expression: impl Into<GroupingExpression>, quantiles: I) -> Self
    where
        I: IntoIterator<Item = f64>,
    {
        Self::Quantiles {
            expression: expression.into(),
            quantiles: quantiles.into_iter().collect(),
        }
    }
    pub fn summary() -> Self {
        Self::Summary(None)
    }
    pub fn summary_named(name: impl Into<String>) -> Self {
        Self::Summary(Some(name.into()))
    }
    /// Name this aggregation when it is used as an output.
    pub fn as_(self, label: impl Into<String>) -> GroupingOutput {
        GroupingOutput::from(self).label(label)
    }
    pub fn alias_ref(name: impl Into<String>) -> Self {
        Self::AliasRef(name.into())
    }
    pub fn named(name: impl Into<String>, aggregation: GroupingAggregation) -> Self {
        Self::Named(name.into(), Box::new(aggregation))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GroupingOutput {
    source: GroupingOutputSource,
    label: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
enum GroupingOutputSource {
    Aggregation(GroupingAggregation),
    Alias(String),
}

impl GroupingOutput {
    pub fn aggregation(aggregation: GroupingAggregation) -> Self {
        Self {
            source: GroupingOutputSource::Aggregation(aggregation),
            label: None,
        }
    }
    pub fn alias(name: impl Into<String>) -> Self {
        Self {
            source: GroupingOutputSource::Alias(name.into()),
            label: None,
        }
    }
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
    /// Compatibility alias for assigning an output label.
    pub fn as_(self, label: impl Into<String>) -> Self {
        self.label(label)
    }
}

impl From<GroupingAggregation> for GroupingOutput {
    fn from(value: GroupingAggregation) -> Self {
        Self::aggregation(value).label(String::new())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum GroupingOrder {
    Field(GroupingExpression, bool),
    Aggregation(GroupingAggregation, bool),
}

impl GroupingOrder {
    pub fn ascending(expression: impl Into<GroupingExpression>) -> Self {
        Self::Field(expression.into(), false)
    }
    pub fn descending(expression: impl Into<GroupingExpression>) -> Self {
        Self::Field(expression.into(), true)
    }
    pub fn ascending_aggregation(aggregation: GroupingAggregation) -> Self {
        Self::Aggregation(aggregation, false)
    }
    pub fn descending_aggregation(aggregation: GroupingAggregation) -> Self {
        Self::Aggregation(aggregation, true)
    }
    pub fn ascending_alias(name: impl Into<String>) -> Self {
        Self::Aggregation(GroupingAggregation::AliasRef(name.into()), false)
    }
    pub fn descending_alias(name: impl Into<String>) -> Self {
        Self::Aggregation(GroupingAggregation::AliasRef(name.into()), true)
    }
    pub fn field(field: impl Into<String>, descending: bool) -> Self {
        Self::Field(GroupingExpression::attribute(field), descending)
    }
    pub fn aggregation(aggregation: GroupingAggregation, descending: bool) -> Self {
        Self::Aggregation(aggregation, descending)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GroupingFilter {
    And(Vec<GroupingFilter>),
    Or(Vec<GroupingFilter>),
    Not(Box<GroupingFilter>),
    Regex {
        expression: GroupingExpression,
        pattern: String,
    },
    Range {
        expression: GroupingExpression,
        lower: GroupingExpression,
        upper: GroupingExpression,
        include_lower: bool,
        include_upper: bool,
    },
    IsTrue(GroupingExpression),
}

impl GroupingFilter {
    pub fn regex(expression: impl Into<GroupingExpression>, pattern: impl Into<String>) -> Self {
        Self::Regex {
            expression: expression.into(),
            pattern: pattern.into(),
        }
    }
    pub fn range(
        expression: impl Into<GroupingExpression>,
        lower: GroupingExpression,
        upper: GroupingExpression,
    ) -> Self {
        Self::range_with_inclusion(expression, lower, upper, true, false)
    }

    pub fn range_with_inclusion(
        expression: impl Into<GroupingExpression>,
        lower: GroupingExpression,
        upper: GroupingExpression,
        include_lower: bool,
        include_upper: bool,
    ) -> Self {
        Self::Range {
            expression: expression.into(),
            lower,
            upper,
            include_lower,
            include_upper,
        }
    }
    pub fn is_true(expression: impl Into<GroupingExpression>) -> Self {
        Self::IsTrue(expression.into())
    }
    pub fn all<I>(filters: I) -> Self
    where
        I: IntoIterator<Item = GroupingFilter>,
    {
        Self::And(filters.into_iter().collect())
    }
    pub fn any<I>(filters: I) -> Self
    where
        I: IntoIterator<Item = GroupingFilter>,
    {
        Self::Or(filters.into_iter().collect())
    }
    pub fn negate(filter: GroupingFilter) -> Self {
        Self::Not(Box::new(filter))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GroupingLevel {
    classifier: GroupingExpression,
    aliases: Vec<(String, GroupingAggregation)>,
    outputs: Vec<GroupingOutput>,
    orders: Vec<GroupingOrder>,
    filter: Option<GroupingFilter>,
    keep: Option<GroupingFilter>,
    nested: Option<Box<GroupingLevel>>,
    max_groups: Option<Option<u32>>,
    precision: Option<u32>,
}

impl GroupingLevel {
    pub fn new(classifier: GroupingExpression) -> Self {
        Self {
            classifier,
            aliases: Vec::new(),
            outputs: Vec::new(),
            orders: Vec::new(),
            filter: None,
            keep: None,
            nested: None,
            max_groups: None,
            precision: None,
        }
    }

    pub fn output(mut self, output: impl Into<GroupingOutput>) -> Self {
        self.outputs.push(output.into());
        self
    }
    pub fn alias(mut self, name: impl Into<String>, aggregation: GroupingAggregation) -> Self {
        self.aliases.push((name.into(), aggregation));
        self
    }
    pub fn order_by<I>(mut self, orders: I) -> Self
    where
        I: IntoIterator<Item = GroupingOrder>,
    {
        self.orders.extend(orders);
        self
    }
    pub fn order_by_alias(self, name: impl Into<String>, descending: bool) -> Self {
        let order = if descending {
            GroupingOrder::descending_alias(name)
        } else {
            GroupingOrder::ascending_alias(name)
        };
        self.order_by([order])
    }
    pub fn keep_regex(self, field: impl Into<String>, pattern: impl Into<String>) -> Self {
        let next = GroupingFilter::regex(GroupingExpression::attribute(field), pattern);
        match self.keep.clone() {
            Some(existing) => self.keep(GroupingFilter::all([existing, next])),
            None => self.keep(next),
        }
    }
    pub fn filter(mut self, filter: GroupingFilter) -> Self {
        self.filter = Some(filter);
        self
    }
    pub fn keep(mut self, filter: GroupingFilter) -> Self {
        self.keep = Some(filter);
        self
    }
    pub fn nested(mut self, level: GroupingLevel) -> Self {
        self.nested = Some(Box::new(level));
        self
    }
    pub fn max_groups(mut self, value: u32) -> Self {
        self.max_groups = Some(Some(value));
        self
    }

    pub fn unlimited(mut self) -> Self {
        self.max_groups = Some(None);
        self
    }
    pub fn precision(mut self, value: u32) -> Self {
        self.precision = Some(value);
        self
    }
}

/// A native Rust structured-grouping request.
///
/// This facade exposes only the subset the embedded native grouping adapter can
/// execute without approximation: scalar expressions and verified functions,
/// linear nesting, aggregation aliases/outputs, order, precision, and one filter
/// predicate slot per level. `filter()` and `keep()` are each executable alone;
/// setting both on one level returns `Unsupported` instead of being approximated.
/// The facade also exposes the executable aggregation family. Summary
/// aggregation can be represented for API parity but remains non-executable in
/// the native result path; continuations remain absent. Member/map/bucket classifiers and raw grouping text
/// remain absent because native lowering rejects them. The adapter's narrow
/// root-only `where(true)` / `where($query)` forms are also not surfaced as a
/// general Rust predicate API.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GroupingPlan {
    levels: Vec<GroupingLevel>,
    root_aliases: Vec<(String, GroupingAggregation)>,
    root_outputs: Vec<GroupingOutput>,
}

impl GroupingPlan {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn level(mut self, level: GroupingLevel) -> Self {
        self.levels.push(level);
        self
    }
    pub fn aggregate(mut self, output: impl Into<GroupingOutput>) -> Self {
        self.root_outputs.push(output.into());
        self
    }
    /// Compatibility alias for adding one aggregation directly at root scope.
    pub fn root_output(self, aggregation: GroupingAggregation) -> Self {
        self.aggregate(aggregation)
    }
    pub fn root_alias(mut self, name: impl Into<String>, aggregation: GroupingAggregation) -> Self {
        self.root_aliases.push((name.into(), aggregation));
        self
    }
    pub fn by(field: impl Into<String>) -> GroupingLevel {
        GroupingLevel::new(GroupingExpression::attribute(field))
    }
    pub fn by_expression(expression: GroupingExpression) -> GroupingLevel {
        GroupingLevel::new(expression)
    }

    pub(crate) fn into_ir(self) -> Result<GroupingRequestIr> {
        if self.levels.len() > 1 {
            return Err(Error::search(
                crate::ErrorCode::InvalidArgument,
                "grouping levels must be nested with GroupingLevel::nested",
            ));
        }

        let mut operations = self
            .root_aliases
            .into_iter()
            .map(|(name, aggregation)| {
                Ok(GroupingOperationIr::Alias {
                    name,
                    expression: aggregation_to_expression_ir(aggregation)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        if !self.root_outputs.is_empty() {
            operations.push(GroupingOperationIr::Output(
                self.root_outputs
                    .into_iter()
                    .map(output_to_ir)
                    .collect::<Result<Vec<_>>>()?,
            ));
        }
        if let Some(level) = self.levels.into_iter().next() {
            operations.extend(level_to_scope(level)?.operations);
        }
        if operations.is_empty() {
            return Err(Error::search(
                crate::ErrorCode::InvalidArgument,
                "grouping requires at least one level or root aggregation",
            ));
        }
        Ok(GroupingRequestIr {
            root: GroupingAllIr {
                operations,
                label: None,
            },
            continuations: Vec::new(),
        })
    }
}

fn level_to_scope(level: GroupingLevel) -> Result<GroupingAllIr> {
    let mut each = level
        .aliases
        .into_iter()
        .map(|(name, aggregation)| {
            Ok(GroupingOperationIr::Alias {
                name,
                expression: aggregation_to_expression_ir(aggregation)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    if let Some(max) = level.max_groups {
        each.push(GroupingOperationIr::Max(max));
    }
    if let Some(precision) = level.precision {
        each.push(GroupingOperationIr::Precision(precision));
    }
    if !level.orders.is_empty() {
        each.push(GroupingOperationIr::Order(
            level
                .orders
                .into_iter()
                .map(order_to_ir)
                .collect::<Result<Vec<_>>>()?,
        ));
    }
    if let Some(filter) = level.filter {
        each.push(GroupingOperationIr::Filter(filter_to_ir(filter)?));
    }
    if let Some(keep) = level.keep {
        each.push(GroupingOperationIr::Keep(filter_to_ir(keep)?));
    }

    if !level.outputs.is_empty() {
        each.push(GroupingOperationIr::Output(
            level
                .outputs
                .into_iter()
                .map(output_to_ir)
                .collect::<Result<Vec<_>>>()?,
        ));
    }
    if let Some(nested) = level.nested {
        each.push(GroupingOperationIr::All(level_to_scope(*nested)?));
    }
    Ok(GroupingAllIr {
        operations: vec![
            GroupingOperationIr::Group(expression_to_ir(level.classifier)?),
            GroupingOperationIr::Each(GroupingAllIr {
                operations: each,
                label: None,
            }),
        ],
        label: None,
    })
}

fn expression_to_ir(expression: GroupingExpression) -> Result<GroupingExpressionIr> {
    Ok(match expression.0 {
        GroupingExpressionNode::Attribute(field) => GroupingExpressionIr::Attribute(field),
        GroupingExpressionNode::Literal(value) => {
            let value = crate::filter::pending_value(&value).map_err(Error::Serialization)?;
            GroupingExpressionIr::Literal(
                searchkernel_ir::request_value_from_json(value, "grouping.expression".to_owned())
                    .map_err(Error::from_ir)?,
            )
        }
        GroupingExpressionNode::Function { name, arguments } => GroupingExpressionIr::Function {
            name,
            arguments: arguments
                .into_iter()
                .map(expression_to_ir)
                .collect::<Result<Vec<_>>>()?,
        },
        GroupingExpressionNode::Positive(expression) => {
            GroupingExpressionIr::Positive(Box::new(expression_to_ir(*expression)?))
        }
        GroupingExpressionNode::Negative(expression) => {
            GroupingExpressionIr::Negative(Box::new(expression_to_ir(*expression)?))
        }
        GroupingExpressionNode::Binary {
            operator,
            left,
            right,
        } => GroupingExpressionIr::Binary {
            operator,
            left: Box::new(expression_to_ir(*left)?),
            right: Box::new(expression_to_ir(*right)?),
        },
    })
}

fn order_to_ir(order: GroupingOrder) -> Result<GroupingExpressionIr> {
    let (expression, descending) = match order {
        GroupingOrder::Field(expression, descending) => (expression_to_ir(expression)?, descending),
        GroupingOrder::Aggregation(aggregation, descending) => {
            (aggregation_to_expression_ir(aggregation)?, descending)
        }
    };
    Ok(if descending {
        GroupingExpressionIr::Negative(Box::new(expression))
    } else {
        GroupingExpressionIr::Positive(Box::new(expression))
    })
}

fn aggregation_to_expression_ir(aggregation: GroupingAggregation) -> Result<GroupingExpressionIr> {
    let aggregation = match aggregation {
        GroupingAggregation::Count => GroupingAggregationIr::Count,
        GroupingAggregation::Average(expression) => {
            GroupingAggregationIr::Avg(expression_to_ir(expression)?)
        }
        GroupingAggregation::Sum(expression) => {
            GroupingAggregationIr::Sum(expression_to_ir(expression)?)
        }
        GroupingAggregation::Min(expression) => {
            GroupingAggregationIr::Min(expression_to_ir(expression)?)
        }
        GroupingAggregation::Max(expression) => {
            GroupingAggregationIr::Max(expression_to_ir(expression)?)
        }
        GroupingAggregation::Xor(expression) => {
            GroupingAggregationIr::Xor(expression_to_ir(expression)?)
        }
        GroupingAggregation::StdDev(expression) => {
            GroupingAggregationIr::StdDev(expression_to_ir(expression)?)
        }
        GroupingAggregation::Quantiles {
            expression,
            quantiles,
        } => GroupingAggregationIr::Quantiles {
            quantiles,
            expression: expression_to_ir(expression)?,
        },
        GroupingAggregation::Summary(name) => GroupingAggregationIr::Summary(name),
        GroupingAggregation::AliasRef(name) => return Ok(GroupingExpressionIr::Alias(name)),
        GroupingAggregation::Named(_, _) => {
            return Err(Error::search(
                crate::ErrorCode::InvalidArgument,
                "named grouping outputs cannot be nested as aggregation expressions",
            ));
        }
    };
    Ok(GroupingExpressionIr::Aggregation(Box::new(aggregation)))
}

fn output_to_ir(output: GroupingOutput) -> Result<GroupingOutputIr> {
    let mut label = output.label;
    let expression = match output.source {
        GroupingOutputSource::Aggregation(GroupingAggregation::Named(name, aggregation)) => {
            if label.is_none() {
                label = Some(name);
            }
            aggregation_to_expression_ir(*aggregation)?
        }
        GroupingOutputSource::Aggregation(aggregation) => {
            aggregation_to_expression_ir(aggregation)?
        }
        GroupingOutputSource::Alias(name) => GroupingExpressionIr::Alias(name),
    };
    Ok(GroupingOutputIr { label, expression })
}

pub(crate) fn aggregate_request(outputs: &[GroupingOutput]) -> Result<GroupingRequestIr> {
    Ok(GroupingRequestIr {
        root: GroupingAllIr {
            operations: vec![GroupingOperationIr::Output(
                outputs
                    .iter()
                    .cloned()
                    .map(output_to_ir)
                    .collect::<Result<Vec<_>>>()?,
            )],
            label: None,
        },
        continuations: Vec::new(),
    })
}

fn filter_to_ir(filter: GroupingFilter) -> Result<GroupingFilterIr> {
    Ok(match filter {
        GroupingFilter::And(filters) => GroupingFilterIr::And(
            filters
                .into_iter()
                .map(filter_to_ir)
                .collect::<Result<Vec<_>>>()?,
        ),
        GroupingFilter::Or(filters) => GroupingFilterIr::Or(
            filters
                .into_iter()
                .map(filter_to_ir)
                .collect::<Result<Vec<_>>>()?,
        ),
        GroupingFilter::Not(filter) => GroupingFilterIr::Not(Box::new(filter_to_ir(*filter)?)),
        GroupingFilter::Regex {
            expression,
            pattern,
        } => GroupingFilterIr::Regex {
            expression: Box::new(expression_to_ir(expression)?),
            pattern,
        },
        GroupingFilter::Range {
            expression,
            lower,
            upper,
            include_lower,
            include_upper,
        } => GroupingFilterIr::Range {
            expression: Box::new(expression_to_ir(expression)?),
            lower: Box::new(expression_to_ir(lower)?),
            upper: Box::new(expression_to_ir(upper)?),
            include_lower,
            include_upper,
        },
        GroupingFilter::IsTrue(expression) => {
            GroupingFilterIr::IsTrue(Box::new(expression_to_ir(expression)?))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_plan_materializes_each_before_nested_all() {
        let plan = GroupingPlan::new().level(
            GroupingPlan::by("category")
                .output(GroupingAggregation::count())
                .nested(GroupingPlan::by("rating").output(GroupingAggregation::count())),
        );
        let request = plan.into_ir().expect("grouping plan should convert");
        let [GroupingOperationIr::Group(GroupingExpressionIr::Attribute(field)), GroupingOperationIr::Each(each)] =
            request.root.operations.as_slice()
        else {
            panic!("nested grouping must materialize its group with each");
        };
        assert_eq!(field, "category");
        assert!(matches!(
            each.operations.first(),
            Some(GroupingOperationIr::Output(_))
        ));
        assert!(matches!(
            each.operations.get(1),
            Some(GroupingOperationIr::All(_))
        ));
    }

    #[test]
    fn advanced_plan_materializes_executable_grouping_union() {
        let classifier =
            GroupingExpression::function("lowercase", [GroupingExpression::attribute("category")]);
        let plan = GroupingPlan::new().level(
            GroupingPlan::by_expression(classifier)
                .alias("total", GroupingAggregation::sum("price"))
                .max_groups(5)
                .precision(9)
                .order_by([GroupingOrder::descending_alias("total")])
                .filter(GroupingFilter::range(
                    "price",
                    GroupingExpression::literal(10_i64),
                    GroupingExpression::literal(100_i64),
                ))
                .output(GroupingOutput::alias("total").label("revenue"))
                .output(GroupingAggregation::stddev("price"))
                .output(GroupingAggregation::xor("inventory"))
                .output(GroupingAggregation::quantiles("price", [0.5, 0.9])),
        );
        let request = plan.into_ir().expect("advanced grouping should convert");
        let [GroupingOperationIr::Group(GroupingExpressionIr::Function { name, .. }), GroupingOperationIr::Each(each)] =
            request.root.operations.as_slice()
        else {
            panic!("advanced grouping must retain function classifier and each scope");
        };
        assert_eq!(name, "lowercase");
        assert!(matches!(
            each.operations[0],
            GroupingOperationIr::Alias { .. }
        ));
        assert_eq!(each.operations[1], GroupingOperationIr::Max(Some(5)));
        assert_eq!(each.operations[2], GroupingOperationIr::Precision(9));
        assert!(matches!(each.operations[3], GroupingOperationIr::Order(_)));
        assert!(matches!(each.operations[4], GroupingOperationIr::Filter(_)));
        let GroupingOperationIr::Output(outputs) = &each.operations[5] else {
            panic!("advanced grouping must retain outputs");
        };
        assert_eq!(outputs.len(), 4);
        assert_eq!(outputs[0].label.as_deref(), Some("revenue"));
    }

    #[test]
    fn root_aggregations_do_not_require_a_classifier() {
        let request = GroupingPlan::new()
            .root_alias("docs", GroupingAggregation::count())
            .aggregate(GroupingOutput::alias("docs").label("docs"))
            .aggregate(GroupingAggregation::sum(
                GroupingExpression::attribute("price").multiply(GroupingExpression::literal(2_i64)),
            ))
            .into_ir()
            .expect("root aggregations should convert");
        assert!(matches!(
            request.root.operations.first(),
            Some(GroupingOperationIr::Alias { name, .. }) if name == "docs"
        ));
        assert!(request.root.operations.iter().any(|operation| matches!(
            operation,
            GroupingOperationIr::Output(outputs) if outputs.len() == 2
        )));
    }

    #[test]
    fn keep_unlimited_and_aggregation_order_reach_canonical_operations() {
        let request = GroupingPlan::new()
            .level(
                GroupingPlan::by("category")
                    .unlimited()
                    .keep(GroupingFilter::is_true("active"))
                    .order_by([GroupingOrder::ascending_aggregation(
                        GroupingAggregation::count(),
                    )]),
            )
            .into_ir()
            .expect("grouping controls should convert");
        let [GroupingOperationIr::Group(_), GroupingOperationIr::Each(each)] =
            request.root.operations.as_slice()
        else {
            panic!("expected group/each grouping topology");
        };
        assert_eq!(each.operations[0], GroupingOperationIr::Max(None));
        assert!(matches!(each.operations[1], GroupingOperationIr::Order(_)));
        assert!(matches!(each.operations[2], GroupingOperationIr::Keep(_)));
    }

    #[test]
    fn by_helper_preserves_existing_canonical_tree() {
        let actual = GroupingPlan::new()
            .level(
                GroupingPlan::by("category")
                    .max_groups(5)
                    .output(GroupingAggregation::count())
                    .nested(GroupingPlan::by("brand")),
            )
            .into_ir()
            .expect("grouping helper should lower");
        let [GroupingOperationIr::Group(GroupingExpressionIr::Attribute(field)), GroupingOperationIr::Each(each)] =
            actual.root.operations.as_slice()
        else {
            panic!("existing helper topology changed");
        };
        assert_eq!(field, "category");
        assert_eq!(each.operations[0], GroupingOperationIr::Max(Some(5)));
        assert!(matches!(each.operations[1], GroupingOperationIr::Output(_)));
        assert!(matches!(each.operations[2], GroupingOperationIr::All(_)));
    }
}
