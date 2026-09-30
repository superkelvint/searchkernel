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
}

#[derive(Clone, Debug, PartialEq)]
pub struct GroupingLevel {
    classifier: GroupingExpression,
    outputs: Vec<GroupingAggregation>,
    nested: Option<Box<GroupingLevel>>,
    max_groups: Option<u32>,
}

impl GroupingLevel {
    pub fn new(classifier: GroupingExpression) -> Self {
        Self {
            classifier,
            outputs: Vec::new(),
            nested: None,
            max_groups: None,
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
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GroupingPlan {
    levels: Vec<GroupingLevel>,
}

impl GroupingPlan {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn level(mut self, level: GroupingLevel) -> Self {
        self.levels.push(level);
        self
    }
    pub fn by(field: impl Into<String>) -> GroupingLevel {
        GroupingLevel::new(GroupingExpression::attribute(field))
    }

    pub(crate) fn into_ir(self) -> Result<GroupingRequestIr> {
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
        Ok(GroupingRequestIr {
            root: level_to_scope(first)?,
            continuations: Vec::new(),
        })
    }
}

fn level_to_scope(level: GroupingLevel) -> Result<GroupingAllIr> {
    let mut each = Vec::new();
    if let Some(max) = level.max_groups {
        each.push(GroupingOperationIr::Max(Some(max)));
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
    };
    Ok(GroupingOutputIr {
        label: None,
        expression: GroupingExpressionIr::Aggregation(Box::new(aggregation)),
    })
}

pub(crate) fn aggregate_request(outputs: &[GroupingAggregation]) -> Result<GroupingRequestIr> {
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
}
