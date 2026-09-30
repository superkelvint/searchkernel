use crate::error::{Error, Result};
use searchkernel_ir::{
    GroupingAggregationIr, GroupingAllIr, GroupingExpressionIr, GroupingOperationIr,
    GroupingOutputIr, GroupingRequestIr,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupingExpression(String);

impl GroupingExpression {
    pub fn attribute(field: impl Into<String>) -> Self {
        Self(field.into())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum GroupingOrder {
    Field(GroupingExpression, bool),
    Aggregation(GroupingAggregation, bool),
}

impl GroupingOrder {
    pub fn field(field: impl Into<String>, descending: bool) -> Self {
        Self::Field(GroupingExpression::attribute(field), descending)
    }

    pub fn aggregation(aggregation: GroupingAggregation, descending: bool) -> Self {
        Self::Aggregation(aggregation, descending)
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
    AliasRef(String),
    Named(String, Box<GroupingAggregation>),
}

/// One root grouping output, optionally named for stable result lookup.
#[derive(Clone, Debug, PartialEq)]
pub struct GroupingOutput {
    aggregation: GroupingAggregation,
    label: Option<String>,
}

impl GroupingOutput {
    /// Assign a stable result label to this aggregation output.
    pub fn as_(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl From<GroupingAggregation> for GroupingOutput {
    fn from(aggregation: GroupingAggregation) -> Self {
        Self {
            aggregation,
            // Python and TypeScript encode an unlabeled ergonomic aggregate
            // as an explicit empty label. Preserve that canonical intent so
            // equivalent first-party facade calls resolve identically.
            label: Some(String::new()),
        }
    }
}

impl GroupingAggregation {
    pub fn count() -> Self {
        Self::Count
    }
    pub fn average(field: impl Into<String>) -> Self {
        Self::Average(GroupingExpression::attribute(field))
    }
    pub fn avg(field: impl Into<String>) -> Self {
        Self::average(field)
    }
    pub fn sum(field: impl Into<String>) -> Self {
        Self::Sum(GroupingExpression::attribute(field))
    }
    pub fn min(field: impl Into<String>) -> Self {
        Self::Min(GroupingExpression::attribute(field))
    }
    pub fn max(field: impl Into<String>) -> Self {
        Self::Max(GroupingExpression::attribute(field))
    }
    pub fn xor(field: impl Into<String>) -> Self {
        Self::Xor(GroupingExpression::attribute(field))
    }
    pub fn stddev(field: impl Into<String>) -> Self {
        Self::StdDev(GroupingExpression::attribute(field))
    }
    pub fn quantiles<I>(field: impl Into<String>, quantiles: I) -> Self
    where
        I: IntoIterator<Item = f64>,
    {
        Self::Quantiles {
            expression: GroupingExpression::attribute(field),
            quantiles: quantiles.into_iter().collect(),
        }
    }
    pub fn summary() -> Self {
        Self::Summary(None)
    }
    pub fn summary_named(name: impl Into<String>) -> Self {
        Self::Summary(Some(name.into()))
    }

    /// Name this aggregation when it is used as a root-level output.
    pub fn as_(self, label: impl Into<String>) -> GroupingOutput {
        GroupingOutput::from(self).as_(label)
    }
    pub fn alias_ref(name: impl Into<String>) -> Self {
        Self::AliasRef(name.into())
    }
    pub fn named(name: impl Into<String>, aggregation: GroupingAggregation) -> Self {
        Self::Named(name.into(), Box::new(aggregation))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GroupingLevel {
    classifier: GroupingExpression,
    outputs: Vec<GroupingAggregation>,
    nested: Option<Box<GroupingLevel>>,
    max_groups: Option<u32>,
    precision: Option<u32>,
    aliases: Vec<(String, GroupingAggregation)>,
    keep_regex: Vec<(String, String)>,
    order_by_alias: Vec<(String, bool)>,
    order: Vec<GroupingOrder>,
}

impl GroupingLevel {
    pub fn new(classifier: GroupingExpression) -> Self {
        Self {
            classifier,
            outputs: Vec::new(),
            nested: None,
            max_groups: None,
            precision: None,
            aliases: Vec::new(),
            keep_regex: Vec::new(),
            order_by_alias: Vec::new(),
            order: Vec::new(),
        }
    }

    pub fn output(mut self, aggregation: GroupingAggregation) -> Self {
        self.outputs.push(aggregation);
        self
    }
    pub fn nested(mut self, level: GroupingLevel) -> Self {
        self.nested = Some(Box::new(level));
        self
    }
    pub fn max_groups(mut self, value: u32) -> Self {
        self.max_groups = Some(value);
        self
    }
    pub fn precision(mut self, value: u32) -> Self {
        self.precision = Some(value);
        self
    }
    /// Define a named aggregation that can be referenced by an alias output.
    pub fn alias(mut self, name: impl Into<String>, aggregation: GroupingAggregation) -> Self {
        self.aliases.push((name.into(), aggregation));
        self
    }
    /// Keep only groups whose field value matches the regular expression.
    pub fn keep_regex(mut self, field: impl Into<String>, pattern: impl Into<String>) -> Self {
        self.keep_regex.push((field.into(), pattern.into()));
        self
    }
    /// Order groups by a declared aggregation alias.
    pub fn order_by_alias(mut self, name: impl Into<String>, descending: bool) -> Self {
        self.order_by_alias.push((name.into(), descending));
        self
    }
    pub fn order_by(mut self, order: impl IntoIterator<Item = GroupingOrder>) -> Self {
        self.order.extend(order);
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GroupingPlan {
    levels: Vec<GroupingLevel>,
    root_outputs: Vec<GroupingAggregation>,
}

impl GroupingPlan {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn level(mut self, level: GroupingLevel) -> Self {
        self.levels.push(level);
        self
    }
    /// Add an aggregation directly to the root grouping scope.
    pub fn root_output(mut self, aggregation: GroupingAggregation) -> Self {
        self.root_outputs.push(aggregation);
        self
    }
    pub fn by(field: impl Into<String>) -> GroupingLevel {
        GroupingLevel::new(GroupingExpression::attribute(field))
    }

    pub(crate) fn into_ir(self) -> Result<GroupingRequestIr> {
        if self.levels.is_empty() && !self.root_outputs.is_empty() {
            return Ok(GroupingRequestIr {
                root: GroupingAllIr {
                    operations: vec![GroupingOperationIr::Output(
                        self.root_outputs
                            .into_iter()
                            .map(output_to_ir)
                            .collect::<Result<Vec<_>>>()?,
                    )],
                    label: None,
                },
                continuations: Vec::new(),
            });
        }
        let levels = self.levels;
        if levels.len() > 1 {
            return Err(Error::search(
                crate::ErrorCode::InvalidArgument,
                "grouping levels must be nested with GroupingLevel::nested",
            ));
        }
        let first = levels.into_iter().next().ok_or_else(|| {
            Error::search(
                crate::ErrorCode::InvalidArgument,
                "grouping requires at least one level",
            )
        })?;
        let mut root = level_to_scope(first)?;
        if !self.root_outputs.is_empty() {
            root.operations.insert(
                0,
                GroupingOperationIr::Output(
                    self.root_outputs
                        .into_iter()
                        .map(output_to_ir)
                        .collect::<Result<Vec<_>>>()?,
                ),
            );
        }
        Ok(GroupingRequestIr {
            root,
            continuations: Vec::new(),
        })
    }
}

fn level_to_scope(level: GroupingLevel) -> Result<GroupingAllIr> {
    let mut each = Vec::new();
    if let Some(max) = level.max_groups {
        each.push(GroupingOperationIr::Max(Some(max)));
    }
    if let Some(precision) = level.precision {
        each.push(GroupingOperationIr::Precision(precision));
    }
    for (name, aggregation) in level.aliases {
        each.push(GroupingOperationIr::Alias {
            name,
            expression: aggregation_expression(aggregation)?,
        });
    }
    for (field, pattern) in level.keep_regex {
        each.push(GroupingOperationIr::Keep(
            searchkernel_ir::GroupingFilterIr::Regex {
                expression: Box::new(GroupingExpressionIr::Attribute(field)),
                pattern,
            },
        ));
    }
    if !level.order_by_alias.is_empty() {
        each.push(GroupingOperationIr::Order(
            level
                .order_by_alias
                .into_iter()
                .map(|(name, descending)| {
                    let expression = GroupingExpressionIr::Alias(name);
                    if descending {
                        GroupingExpressionIr::Negative(Box::new(expression))
                    } else {
                        expression
                    }
                })
                .collect(),
        ));
    }
    if !level.order.is_empty() {
        each.push(GroupingOperationIr::Order(
            level
                .order
                .into_iter()
                .map(|order| {
                    let (expression, descending) = match order {
                        GroupingOrder::Field(expression, descending) => {
                            (expression_to_ir(expression)?, descending)
                        }
                        GroupingOrder::Aggregation(aggregation, descending) => {
                            (aggregation_expression(aggregation)?, descending)
                        }
                    };
                    Ok(if descending {
                        GroupingExpressionIr::Negative(Box::new(expression))
                    } else {
                        expression
                    })
                })
                .collect::<Result<Vec<_>>>()?,
        ));
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
    Ok(GroupingExpressionIr::Attribute(expression.0))
}

fn output_to_ir(output: GroupingAggregation) -> Result<GroupingOutputIr> {
    let (label, expression) = match output {
        GroupingAggregation::Named(name, aggregation) => {
            (Some(name), aggregation_expression(*aggregation)?)
        }
        aggregation => (None, aggregation_expression(aggregation)?),
    };
    Ok(GroupingOutputIr { label, expression })
}

fn aggregation_expression(output: GroupingAggregation) -> Result<GroupingExpressionIr> {
    let aggregation = match output {
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
                "grouping aliases cannot be nested as aggregations",
            ));
        }
    };
    Ok(GroupingExpressionIr::Aggregation(Box::new(aggregation)))
}

fn grouping_output_to_ir(output: GroupingOutput) -> Result<GroupingOutputIr> {
    let mut resolved = output_to_ir(output.aggregation)?;
    resolved.label = output.label;
    Ok(resolved)
}

pub(crate) fn aggregate_request(outputs: &[GroupingOutput]) -> Result<GroupingRequestIr> {
    Ok(GroupingRequestIr {
        root: GroupingAllIr {
            operations: vec![GroupingOperationIr::Output(
                outputs
                    .iter()
                    .cloned()
                    .map(grouping_output_to_ir)
                    .collect::<Result<Vec<_>>>()?,
            )],
            label: None,
        },
        continuations: Vec::new(),
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
    fn by_helper_matches_the_explicit_canonical_grouping_tree() {
        let actual = GroupingPlan::new()
            .level(
                GroupingPlan::by("category")
                    .max_groups(5)
                    .output(GroupingAggregation::count())
                    .nested(GroupingPlan::by("brand")),
            )
            .into_ir()
            .expect("grouping helper should lower");
        let expected = GroupingRequestIr {
            root: GroupingAllIr {
                operations: vec![
                    GroupingOperationIr::Group(GroupingExpressionIr::Attribute(
                        "category".to_owned(),
                    )),
                    GroupingOperationIr::Each(GroupingAllIr {
                        operations: vec![
                            GroupingOperationIr::Max(Some(5)),
                            GroupingOperationIr::Output(vec![GroupingOutputIr {
                                label: None,
                                expression: GroupingExpressionIr::Aggregation(Box::new(
                                    GroupingAggregationIr::Count,
                                )),
                            }]),
                            GroupingOperationIr::All(GroupingAllIr {
                                operations: vec![
                                    GroupingOperationIr::Group(GroupingExpressionIr::Attribute(
                                        "brand".to_owned(),
                                    )),
                                    GroupingOperationIr::Each(GroupingAllIr {
                                        operations: Vec::new(),
                                        label: None,
                                    }),
                                ],
                                label: None,
                            }),
                        ],
                        label: None,
                    }),
                ],
                label: None,
            },
            continuations: Vec::new(),
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn grouping_alias_defines_orders_and_emits_a_named_aggregate() {
        let request = GroupingPlan::new()
            .level(
                GroupingPlan::by("brand")
                    .alias("revenue", GroupingAggregation::sum("price"))
                    .order_by_alias("revenue", true)
                    .output(GroupingAggregation::named(
                        "products",
                        GroupingAggregation::count(),
                    ))
                    .output(GroupingAggregation::named(
                        "revenue",
                        GroupingAggregation::alias_ref("revenue"),
                    )),
            )
            .into_ir()
            .expect("alias grouping should lower");

        let [GroupingOperationIr::Group(GroupingExpressionIr::Attribute(field)), GroupingOperationIr::Each(each)] =
            request.root.operations.as_slice()
        else {
            panic!("grouping level should lower to group and each operations");
        };
        assert_eq!(field, "brand");
        assert!(matches!(
            each.operations.as_slice(),
            [
                GroupingOperationIr::Alias { name, expression: GroupingExpressionIr::Aggregation(aggregation) },
                GroupingOperationIr::Order(order),
                GroupingOperationIr::Output(outputs),
            ] if name == "revenue"
                && **aggregation == GroupingAggregationIr::Sum(GroupingExpressionIr::Attribute("price".to_owned()))
                && order == &vec![GroupingExpressionIr::Negative(Box::new(GroupingExpressionIr::Alias("revenue".to_owned())))]
                && outputs == &vec![
                    GroupingOutputIr {
                        label: Some("products".to_owned()),
                        expression: GroupingExpressionIr::Aggregation(Box::new(GroupingAggregationIr::Count)),
                    },
                    GroupingOutputIr {
                        label: Some("revenue".to_owned()),
                        expression: GroupingExpressionIr::Alias("revenue".to_owned()),
                    },
                ]
        ));
    }

    #[test]
    fn grouping_order_lowers_aggregation_then_field_order() {
        let grouping = GroupingPlan::new().level(
            GroupingPlan::by("brand")
                .precision(17)
                .order_by([
                    GroupingOrder::aggregation(GroupingAggregation::count(), true),
                    GroupingOrder::field("brand", false),
                ])
                .output(GroupingAggregation::count()),
        );

        let ir = grouping.into_ir().unwrap();
        let GroupingOperationIr::Each(level) = &ir.root.operations[1] else {
            panic!("group plan should contain its level operations");
        };
        assert!(level
            .operations
            .contains(&GroupingOperationIr::Precision(17)));
        let order = level
            .operations
            .iter()
            .find_map(|operation| match operation {
                GroupingOperationIr::Order(expressions) => Some(expressions),
                _ => None,
            })
            .expect("order operation should be present");
        assert_eq!(
            order,
            &[
                GroupingExpressionIr::Negative(Box::new(GroupingExpressionIr::Aggregation(
                    Box::new(GroupingAggregationIr::Count),
                ))),
                GroupingExpressionIr::Attribute("brand".to_owned()),
            ]
        );
    }
}
