//! Lower V2 grouping into either the existing facet path or the supported
//! recursive native grouping-plan subset.

use searchkernel_ir::{PortableError, Result};
use searchkernel_vespa_engine::{
    FacetRequest, GroupingAggregation, GroupingBinaryOperator, GroupingExpression, GroupingFilter,
    GroupingLevel, GroupingLiteral, GroupingPlan, SearchRequest,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FacetLowering {
    pub(crate) field: String,
    pub(crate) limit: u32,
    pub(crate) order: searchkernel_vespa_engine::GroupingOrder,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeGroupingLowering {
    pub(crate) plan: GroupingPlan,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum GroupingLowering {
    Facet(FacetLowering),
    Native(NativeGroupingLowering),
}

pub(crate) fn lower_facets_into_request(
    request: SearchRequest,
    facets: &[FacetLowering],
) -> SearchRequest {
    facets.iter().fold(request, |request, facet| {
        request.facet(FacetRequest::new(&facet.field, facet.limit, facet.order))
    })
}

/// Recognize the exact structured grouping emitted by the Rust facet facade:
/// `all(group(attribute(field)), each(max(limit), order(...), output(count)))`.
/// Other valid V2 grouping programs remain unsupported until they have a
/// faithful native lowering.
#[cfg(test)]
pub(crate) fn lower_facet_groupings(
    grouping: &[searchkernel_ir::GroupingRequestIr],
) -> Result<Vec<FacetLowering>> {
    grouping
        .iter()
        .enumerate()
        .map(|(index, value)| {
            facet_lowering(value).ok_or_else(|| {
                PortableError::unsupported(
                    "grouping_not_supported",
                    Some(format!("grouping[{index}]")),
                    "the current native adapter supports only the structured grouping emitted by the facet convenience API",
                )
            })
        })
        .collect()
}

pub(crate) fn lower_groupings(
    grouping: &[searchkernel_ir::GroupingRequestIr],
) -> Result<Vec<GroupingLowering>> {
    grouping
        .iter()
        .enumerate()
        .map(|(index, value)| {
            if let Some(facet) = facet_lowering(value) {
                return Ok(GroupingLowering::Facet(facet));
            }
            grouping_plan(value)
                .map(|plan| GroupingLowering::Native(NativeGroupingLowering { plan }))
                .map_err(|error| {
                    if error.details.is_empty() {
                        PortableError::unsupported(
                            "grouping_not_supported",
                            Some(format!("grouping[{index}]")),
                            "the V2 grouping program cannot be represented by the supported native grouping primitives",
                        )
                    } else {
                        error
                    }
                })
        })
        .collect()
}

fn grouping_plan(value: &searchkernel_ir::GroupingRequestIr) -> Result<GroupingPlan> {
    if !value.continuations.is_empty() {
        return Err(unsupported_grouping(
            "grouping_continuations_not_supported",
            "native grouping plans do not support continuations",
        ));
    }
    let mut compiler = GroupingCompiler {
        levels: Vec::new(),
        group_all: false,
        root_aggregations: Vec::new(),
        root_output_labels: Vec::new(),
    };
    compiler.lower_root(&value.root)?;
    if compiler.levels.is_empty() && compiler.root_aggregations.is_empty() {
        return Err(unsupported_grouping(
            "grouping_level_missing_classifier",
            "a native grouping plan requires an attribute classifier",
        ));
    }
    Ok(GroupingPlan {
        levels: compiler.levels,
        group_all: compiler.group_all,
        root_label: value.root.label.clone(),
        root_aggregations: compiler.root_aggregations,
        root_output_labels: compiler.root_output_labels,
    })
}

struct GroupingCompiler {
    levels: Vec<GroupingLevel>,
    group_all: bool,
    root_aggregations: Vec<GroupingAggregation>,
    root_output_labels: Vec<Option<String>>,
}

impl GroupingCompiler {
    fn lower_root(&mut self, value: &searchkernel_ir::GroupingAllIr) -> Result<()> {
        self.lower_grouping_scope(value)
    }

    fn lower_grouping_scope(&mut self, value: &searchkernel_ir::GroupingAllIr) -> Result<()> {
        let mut classifier = None;
        let mut materializer = None;
        for operation in &value.operations {
            match operation {
                searchkernel_ir::GroupingOperationIr::Group(expression) => {
                    if classifier.is_some() {
                        return Err(unsupported_grouping(
                            "multiple_group_classifiers",
                            "a native grouping level has exactly one classifier",
                        ));
                    }
                    classifier = Some(lower_grouping_expression(expression)?);
                }
                searchkernel_ir::GroupingOperationIr::Each(child) => {
                    if materializer.is_some() {
                        return Err(unsupported_grouping(
                            "parallel_grouping_not_supported",
                            "the linear native grouping plan supports one child branch",
                        ));
                    }
                    materializer = Some(child);
                }
                searchkernel_ir::GroupingOperationIr::All(_) => {
                    return Err(unsupported_grouping(
                        "grouping_all_topology_not_supported",
                        "All cannot materialize a native grouping level; use Each",
                    ));
                }
                searchkernel_ir::GroupingOperationIr::Output(_)
                | searchkernel_ir::GroupingOperationIr::Alias { .. }
                | searchkernel_ir::GroupingOperationIr::Max(_)
                | searchkernel_ir::GroupingOperationIr::Precision(_)
                | searchkernel_ir::GroupingOperationIr::Order(_)
                | searchkernel_ir::GroupingOperationIr::Filter(_)
                | searchkernel_ir::GroupingOperationIr::Keep(_)
                | searchkernel_ir::GroupingOperationIr::Where(_) => {}
            }
        }

        let Some(classifier) = classifier else {
            if materializer.is_some() {
                return Err(unsupported_grouping(
                    "grouping_output_before_each",
                    "root outputs must be inside the materialized grouping level",
                ));
            }
            self.lower_root_operations(&value.operations)?;
            return Ok(());
        };

        let label = value
            .label
            .clone()
            .or_else(|| materializer.as_ref().and_then(|child| child.label.clone()));
        let level = self.levels.len();
        self.levels.push(GroupingLevel {
            classifier,
            label,
            aggregations: Vec::new(),
            output_labels: Vec::new(),
            output_visible: Vec::new(),
            max_groups: None,
            precision: None,
            filter: None,
            order_by: Vec::new(),
        });

        let mut aliases = BTreeMap::new();
        let mut output_names = BTreeSet::new();
        let materializer_index = value.operations.iter().position(|operation| {
            matches!(operation, searchkernel_ir::GroupingOperationIr::Each(_))
        });
        if let Some(index) = materializer_index {
            if value.operations[..index].iter().any(|operation| {
                matches!(
                    operation,
                    searchkernel_ir::GroupingOperationIr::Output(_)
                        | searchkernel_ir::GroupingOperationIr::Alias { .. }
                )
            }) {
                return Err(unsupported_grouping(
                    "grouping_output_before_each",
                    "group outputs must be inside the materialized grouping level",
                ));
            }
        }
        let local_operations = value.operations.iter().filter(|operation| {
            !matches!(
                operation,
                searchkernel_ir::GroupingOperationIr::Each(_)
                    | searchkernel_ir::GroupingOperationIr::All(_)
            )
        });
        self.lower_level_operations(local_operations, level, &mut aliases, &mut output_names)?;
        if let Some(child) = materializer {
            for nested in self.lower_level_operations(
                child.operations.iter(),
                level,
                &mut aliases,
                &mut output_names,
            )? {
                self.lower_grouping_scope(nested)?;
            }
        }
        if self.levels[level].max_groups.is_some()
            && self.levels[level]
                .aggregations
                .iter()
                .any(|aggregation| matches!(aggregation, GroupingAggregation::Summary { .. }))
        {
            return Err(unsupported_grouping(
                "grouping_summary_max_ambiguous",
                "summary() cannot share a grouping level with max(); Vespa requires a separate per-group hit limit",
            ));
        }
        Ok(())
    }

    fn lower_root_operations(
        &mut self,
        operations: &[searchkernel_ir::GroupingOperationIr],
    ) -> Result<()> {
        let mut aliases = BTreeMap::<String, (GroupingAggregation, usize)>::new();
        let mut output_names = BTreeSet::new();
        for operation in operations {
            match operation {
                searchkernel_ir::GroupingOperationIr::Output(outputs) => {
                    for output in outputs {
                        let aggregation =
                            lower_aggregation_with_aliases(&output.expression, &aliases)?;
                        let name = output
                            .label
                            .clone()
                            .unwrap_or_else(|| grouping_aggregation_name(&aggregation));
                        if !output_names.insert(name) {
                            return Err(unsupported_grouping(
                                "grouping_output_name_collision",
                                "native grouping outputs must have unique portable field names",
                            ));
                        }
                        self.root_aggregations.push(aggregation);
                        self.root_output_labels.push(output.label.clone());
                    }
                }
                searchkernel_ir::GroupingOperationIr::Alias { name, expression } => {
                    let aggregation = lower_aggregation(expression)?;
                    aliases.insert(name.clone(), (aggregation, usize::MAX));
                }
                searchkernel_ir::GroupingOperationIr::Group(_)
                | searchkernel_ir::GroupingOperationIr::All(_)
                | searchkernel_ir::GroupingOperationIr::Each(_) => {}
                searchkernel_ir::GroupingOperationIr::Max(_)
                | searchkernel_ir::GroupingOperationIr::Precision(_)
                | searchkernel_ir::GroupingOperationIr::Order(_)
                | searchkernel_ir::GroupingOperationIr::Filter(_)
                | searchkernel_ir::GroupingOperationIr::Keep(_) => {
                    return Err(unsupported_grouping(
                        "grouping_operation_without_level",
                        "this grouping operation requires a materialized group level",
                    ));
                }
                searchkernel_ir::GroupingOperationIr::Where(expression) => {
                    self.lower_where(expression, true)?;
                }
            }
        }
        Ok(())
    }

    fn lower_level_operations<'a>(
        &mut self,
        operations: impl IntoIterator<Item = &'a searchkernel_ir::GroupingOperationIr>,
        level: usize,
        aliases: &mut BTreeMap<String, (GroupingAggregation, usize)>,
        output_names: &mut BTreeSet<String>,
    ) -> Result<Vec<&'a searchkernel_ir::GroupingAllIr>> {
        let mut nested = None;
        for operation in operations {
            match operation {
                searchkernel_ir::GroupingOperationIr::Group(_) => {}
                searchkernel_ir::GroupingOperationIr::Output(outputs) => {
                    for output in outputs {
                        let aggregation =
                            lower_aggregation_with_aliases(&output.expression, aliases)?;
                        let name = output
                            .label
                            .clone()
                            .unwrap_or_else(|| grouping_aggregation_name(&aggregation));
                        if !output_names.insert(name) {
                            return Err(unsupported_grouping(
                                "grouping_output_name_collision",
                                "native grouping outputs must have unique portable field names",
                            ));
                        }
                        self.levels[level].aggregations.push(aggregation);
                        self.levels[level].output_labels.push(output.label.clone());
                        self.levels[level].output_visible.push(true);
                    }
                }
                searchkernel_ir::GroupingOperationIr::Alias { name, expression } => {
                    let aggregation = lower_aggregation(expression)?;
                    let index = self.levels[level].aggregations.len();
                    self.levels[level].aggregations.push(aggregation.clone());
                    self.levels[level].output_labels.push(None);
                    self.levels[level].output_visible.push(false);
                    aliases.insert(name.clone(), (aggregation, index));
                }
                searchkernel_ir::GroupingOperationIr::Max(value) => {
                    self.levels[level].max_groups = Some(value.map_or(u64::MAX, u64::from));
                }
                searchkernel_ir::GroupingOperationIr::Precision(value) => {
                    self.levels[level].precision = Some(u64::from(*value));
                }
                searchkernel_ir::GroupingOperationIr::Order(expressions) => {
                    for expression in expressions {
                        let (expression, ascending) = match expression {
                            searchkernel_ir::GroupingExpressionIr::Positive(value) => (
                                lower_order_expression(value, aliases, &mut self.levels[level])?,
                                true,
                            ),
                            searchkernel_ir::GroupingExpressionIr::Negative(value) => (
                                lower_order_expression(value, aliases, &mut self.levels[level])?,
                                false,
                            ),
                            _ => (
                                lower_order_expression(
                                    expression,
                                    aliases,
                                    &mut self.levels[level],
                                )?,
                                true,
                            ),
                        };
                        self.levels[level].order_by.push(
                            searchkernel_vespa_engine::GroupingOrderBy {
                                expression,
                                ascending,
                            },
                        );
                    }
                }
                searchkernel_ir::GroupingOperationIr::Filter(filter)
                | searchkernel_ir::GroupingOperationIr::Keep(filter) => {
                    if self.levels[level].filter.is_some() {
                        return Err(unsupported_grouping(
                            "grouping_multiple_filters_not_supported",
                            "the native grouping plan supports one filter per level",
                        ));
                    }
                    self.levels[level].filter = Some(lower_filter(filter)?);
                }
                searchkernel_ir::GroupingOperationIr::Where(expression) => {
                    self.lower_where(expression, level == 0)?;
                }
                searchkernel_ir::GroupingOperationIr::All(child) => {
                    if nested.is_some() {
                        return Err(unsupported_grouping(
                            "parallel_grouping_not_supported",
                            "the linear native grouping plan supports one child branch",
                        ));
                    }
                    nested = Some(child);
                }
                searchkernel_ir::GroupingOperationIr::Each(_) => {
                    return Err(unsupported_grouping(
                        "grouping_each_topology_not_supported",
                        "each cannot materialize another level without a structural all scope",
                    ));
                }
            }
        }
        Ok(nested.into_iter().collect())
    }

    fn lower_where(
        &mut self,
        expression: &searchkernel_ir::GroupingExpressionIr,
        is_root: bool,
    ) -> Result<()> {
        if !is_root {
            return Err(unsupported_grouping(
                "grouping_where_not_root",
                "Vespa where(...) is only valid on the root grouping operation",
            ));
        }
        match expression {
            searchkernel_ir::GroupingExpressionIr::Literal(
                searchkernel_ir::RequestValueIr::String(value),
            ) if value == "true" => {
                self.group_all = true;
                Ok(())
            }
            searchkernel_ir::GroupingExpressionIr::Literal(
                searchkernel_ir::RequestValueIr::String(value),
            ) if value == "$query" => Ok(()),
            _ => Err(unsupported_grouping_with_detail(
                "grouping_where_not_supported",
                "Vespa where(...) supports only true and $query in the embedded engine",
                format!("where expression `{expression:?}` cannot be lowered"),
            )),
        }
    }
}

fn lower_grouping_expression(
    expression: &searchkernel_ir::GroupingExpressionIr,
) -> Result<GroupingExpression> {
    match expression {
        searchkernel_ir::GroupingExpressionIr::Attribute(field) => {
            Ok(GroupingExpression::attribute(field))
        }
        searchkernel_ir::GroupingExpressionIr::Literal(value) => {
            Ok(GroupingExpression::Literal(match value {
                searchkernel_ir::RequestValueIr::String(value) => {
                    GroupingLiteral::String(value.clone())
                }
                searchkernel_ir::RequestValueIr::Int64(value) => GroupingLiteral::Integer(*value),
                searchkernel_ir::RequestValueIr::UInt64(value) => {
                    GroupingLiteral::Integer(i64::try_from(*value).map_err(|_| {
                        unsupported_grouping(
                            "grouping_literal_out_of_range",
                            "unsigned grouping literals must fit in a signed native integer",
                        )
                    })?)
                }
                searchkernel_ir::RequestValueIr::Float64(value) if value.is_finite() => {
                    GroupingLiteral::Float(*value)
                }
                searchkernel_ir::RequestValueIr::Bool(value) => GroupingLiteral::Boolean(*value),
                _ => return Err(unsupported_grouping(
                    "grouping_literal_not_supported",
                    "native grouping literals support finite numeric, boolean, and string values",
                )),
            }))
        }
        searchkernel_ir::GroupingExpressionIr::Negative(value) => Ok(GroupingExpression::Negative(
            Box::new(lower_grouping_expression(value)?),
        )),
        searchkernel_ir::GroupingExpressionIr::Positive(value) => lower_grouping_expression(value),
        searchkernel_ir::GroupingExpressionIr::Binary {
            operator,
            left,
            right,
        } => Ok(GroupingExpression::Binary {
            operator: match operator {
                searchkernel_ir::GroupingBinaryOperatorIr::Add => GroupingBinaryOperator::Add,
                searchkernel_ir::GroupingBinaryOperatorIr::Subtract => {
                    GroupingBinaryOperator::Subtract
                }
                searchkernel_ir::GroupingBinaryOperatorIr::Multiply => {
                    GroupingBinaryOperator::Multiply
                }
                searchkernel_ir::GroupingBinaryOperatorIr::Divide => GroupingBinaryOperator::Divide,
                searchkernel_ir::GroupingBinaryOperatorIr::Modulo => GroupingBinaryOperator::Modulo,
            },
            left: Box::new(lower_grouping_expression(left)?),
            right: Box::new(lower_grouping_expression(right)?),
        }),
        searchkernel_ir::GroupingExpressionIr::Function { name, arguments } => {
            Ok(GroupingExpression::Function {
                name: name.clone(),
                arguments: arguments
                    .iter()
                    .map(lower_grouping_expression)
                    .collect::<Result<Vec<_>>>()?,
            })
        }
        searchkernel_ir::GroupingExpressionIr::Alias(name) => {
            Err(unsupported_grouping_with_detail(
                "grouping_alias_not_materialized",
                "an alias cannot be used as a classifier without a native expression handle",
                format!("classifier alias `{name}`"),
            ))
        }
        searchkernel_ir::GroupingExpressionIr::Member { .. }
        | searchkernel_ir::GroupingExpressionIr::MapLookup { .. }
        | searchkernel_ir::GroupingExpressionIr::Bucket(_)
        | searchkernel_ir::GroupingExpressionIr::Aggregation(_) => Err(unsupported_grouping(
            "grouping_classifier_not_supported",
            "this grouping expression requires a native function or bucket constructor",
        )),
    }
}

fn lower_order_expression(
    expression: &searchkernel_ir::GroupingExpressionIr,
    aliases: &BTreeMap<String, (GroupingAggregation, usize)>,
    level: &mut GroupingLevel,
) -> Result<GroupingExpression> {
    match expression {
        searchkernel_ir::GroupingExpressionIr::Alias(name) => aliases
            .get(name)
            .map(|(_, index)| GroupingExpression::AggregationReference(*index))
            .ok_or_else(|| {
                unsupported_grouping("grouping_alias_unknown", "grouping alias is not defined")
            }),
        searchkernel_ir::GroupingExpressionIr::Aggregation(_) => {
            let aggregation = lower_aggregation(expression)?;
            if let Some(index) = level
                .aggregations
                .iter()
                .position(|candidate| candidate == &aggregation)
            {
                return Ok(GroupingExpression::AggregationReference(index));
            }
            let index = level.aggregations.len();
            level.aggregations.push(aggregation);
            level.output_labels.push(None);
            level.output_visible.push(false);
            Ok(GroupingExpression::AggregationReference(index))
        }
        _ => lower_grouping_expression(expression),
    }
}

fn lower_aggregation_with_aliases(
    expression: &searchkernel_ir::GroupingExpressionIr,
    aliases: &BTreeMap<String, (GroupingAggregation, usize)>,
) -> Result<GroupingAggregation> {
    if let searchkernel_ir::GroupingExpressionIr::Alias(name) = expression {
        return aliases
            .get(name)
            .map(|(aggregation, _)| aggregation.clone())
            .ok_or_else(|| {
                unsupported_grouping("grouping_alias_unknown", "grouping alias is not defined")
            });
    }
    lower_aggregation(expression)
}

fn lower_aggregation(
    expression: &searchkernel_ir::GroupingExpressionIr,
) -> Result<GroupingAggregation> {
    let searchkernel_ir::GroupingExpressionIr::Aggregation(aggregation) = expression else {
        return Err(unsupported_grouping(
            "grouping_output_not_supported",
            "the expression is not a native grouping aggregation",
        ));
    };
    match aggregation.as_ref() {
        searchkernel_ir::GroupingAggregationIr::Count => Ok(GroupingAggregation::Count),
        searchkernel_ir::GroupingAggregationIr::Avg(expression) => {
            reject_non_numeric_literal(expression)?;
            Ok(GroupingAggregation::Average(lower_aggregation_expression(
                expression,
            )?))
        }
        searchkernel_ir::GroupingAggregationIr::Sum(value) => Ok(GroupingAggregation::Sum(
            lower_aggregation_expression(value)?,
        )),
        searchkernel_ir::GroupingAggregationIr::Min(value) => Ok(GroupingAggregation::Min(
            lower_aggregation_expression(value)?,
        )),
        searchkernel_ir::GroupingAggregationIr::Max(value) => Ok(GroupingAggregation::Max(
            lower_aggregation_expression(value)?,
        )),
        searchkernel_ir::GroupingAggregationIr::Xor(value) => Ok(GroupingAggregation::Xor(
            lower_aggregation_expression(value)?,
        )),
        searchkernel_ir::GroupingAggregationIr::StdDev(value) => Ok(
            GroupingAggregation::StandardDeviation(lower_aggregation_expression(value)?),
        ),
        searchkernel_ir::GroupingAggregationIr::Quantiles {
            quantiles,
            expression,
        } => Ok(GroupingAggregation::Quantiles {
            expression: lower_aggregation_expression(expression)?,
            quantiles: quantiles.clone(),
        }),
        searchkernel_ir::GroupingAggregationIr::Summary(summary_class) => {
            Ok(GroupingAggregation::Summary {
                summary_class: summary_class.clone().unwrap_or_default(),
                max_hits: 0,
            })
        }
    }
}

fn reject_non_numeric_literal(expression: &searchkernel_ir::GroupingExpressionIr) -> Result<()> {
    if let searchkernel_ir::GroupingExpressionIr::Literal(value) = expression {
        if matches!(
            value,
            searchkernel_ir::RequestValueIr::String(_) | searchkernel_ir::RequestValueIr::Bool(_)
        ) {
            return Err(unsupported_grouping(
                "grouping_classifier_not_supported",
                "numeric grouping aggregations require a numeric expression",
            ));
        }
    }
    Ok(())
}

fn grouping_aggregation_name(aggregation: &GroupingAggregation) -> String {
    let expression_name = |expression: &GroupingExpression| match expression {
        GroupingExpression::Attribute(field) => field.clone(),
        GroupingExpression::AggregationReference(index) => format!("aggregation({index})"),
        expression => format!("{expression:?}"),
    };
    match aggregation {
        GroupingAggregation::Count => "count".to_owned(),
        GroupingAggregation::Average(expression) => {
            format!("average({})", expression_name(expression))
        }
        GroupingAggregation::Sum(expression) => format!("sum({})", expression_name(expression)),
        GroupingAggregation::Min(expression) => format!("min({})", expression_name(expression)),
        GroupingAggregation::Max(expression) => format!("max({})", expression_name(expression)),
        GroupingAggregation::StandardDeviation(expression) => {
            format!("stddev({})", expression_name(expression))
        }
        GroupingAggregation::Xor(expression) => format!("xor({})", expression_name(expression)),
        GroupingAggregation::Quantiles { expression, .. } => {
            format!("quantiles({})", expression_name(expression))
        }
        GroupingAggregation::Summary { .. } => "summary".to_owned(),
    }
}

fn lower_aggregation_expression(
    expression: &searchkernel_ir::GroupingExpressionIr,
) -> Result<GroupingExpression> {
    lower_grouping_expression(expression)
}

fn lower_filter(value: &searchkernel_ir::GroupingFilterIr) -> Result<GroupingFilter> {
    match value {
        searchkernel_ir::GroupingFilterIr::Regex {
            expression,
            pattern,
        } => Ok(GroupingFilter::RegexExpression {
            expression: lower_grouping_expression(expression)?,
            pattern: pattern.clone(),
        }),
        searchkernel_ir::GroupingFilterIr::And(filters) => Ok(GroupingFilter::And(
            filters
                .iter()
                .map(lower_filter)
                .collect::<Result<Vec<_>>>()?,
        )),
        searchkernel_ir::GroupingFilterIr::Or(filters) => Ok(GroupingFilter::Or(
            filters
                .iter()
                .map(lower_filter)
                .collect::<Result<Vec<_>>>()?,
        )),
        searchkernel_ir::GroupingFilterIr::Not(filter) => {
            Ok(GroupingFilter::Not(Box::new(lower_filter(filter)?)))
        }
        searchkernel_ir::GroupingFilterIr::Range {
            expression,
            lower,
            upper,
            include_lower,
            include_upper,
        } => Ok(GroupingFilter::Range {
            expression: lower_grouping_expression(expression)?,
            lower: grouping_filter_bound(lower)?,
            upper: grouping_filter_bound(upper)?,
            include_lower: *include_lower,
            include_upper: *include_upper,
        }),
        searchkernel_ir::GroupingFilterIr::IsTrue(expression) => Ok(GroupingFilter::IsTrue(
            lower_grouping_expression(expression)?,
        )),
    }
}

fn grouping_filter_bound(value: &searchkernel_ir::GroupingExpressionIr) -> Result<f64> {
    let searchkernel_ir::GroupingExpressionIr::Literal(value) = value else {
        return Err(unsupported_grouping(
            "grouping_filter_bound_not_supported",
            "native Vespa range predicates require numeric literal bounds",
        ));
    };
    match value {
        searchkernel_ir::RequestValueIr::Int64(value) => Ok(*value as f64),
        searchkernel_ir::RequestValueIr::UInt64(value) => Ok(*value as f64),
        searchkernel_ir::RequestValueIr::Float64(value) if value.is_finite() => Ok(*value),
        _ => Err(unsupported_grouping(
            "grouping_filter_bound_not_supported",
            "native Vespa range predicates require finite numeric literal bounds",
        )),
    }
}

fn unsupported_grouping(code: &'static str, message: &'static str) -> PortableError {
    PortableError::unsupported(code, Some("grouping"), message)
}

fn unsupported_grouping_with_detail(
    code: &'static str,
    message: &'static str,
    detail: String,
) -> PortableError {
    let mut error = unsupported_grouping(code, message);
    error.details[0].message = format!("{message}: {detail}");
    error
}

pub(crate) fn facet_lowering(value: &searchkernel_ir::GroupingRequestIr) -> Option<FacetLowering> {
    use searchkernel_ir::GroupingOperationIr as Operation;
    use searchkernel_ir::{GroupingAggregationIr as Aggregation, GroupingExpressionIr as Expr};

    if !value.continuations.is_empty() || value.root.label.is_some() {
        return None;
    }
    let [Operation::Group(Expr::Attribute(field)), Operation::Each(each)] =
        value.root.operations.as_slice()
    else {
        return None;
    };
    if each.label.is_some() {
        return None;
    }
    let [Operation::Max(Some(limit)), Operation::Order(order), Operation::Output(output)] =
        each.operations.as_slice()
    else {
        return None;
    };
    let [order] = order.as_slice() else {
        return None;
    };
    let [output] = output.as_slice() else {
        return None;
    };
    if output.label.is_some()
        || !matches!(output.expression, Expr::Aggregation(ref value) if matches!(value.as_ref(), Aggregation::Count))
    {
        return None;
    }
    let order = match order {
        Expr::Negative(value) if matches!(value.as_ref(), Expr::Aggregation(value) if matches!(value.as_ref(), Aggregation::Count)) => {
            searchkernel_vespa_engine::GroupingOrder::CountDescending
        }
        Expr::Aggregation(value) if matches!(value.as_ref(), Aggregation::Count) => {
            searchkernel_vespa_engine::GroupingOrder::CountAscending
        }
        Expr::Negative(value) if matches!(value.as_ref(), Expr::Attribute(value) if value == field) => {
            searchkernel_vespa_engine::GroupingOrder::ValueDescending
        }
        Expr::Attribute(value) if value == field => {
            searchkernel_vespa_engine::GroupingOrder::ValueAscending
        }
        _ => return None,
    };
    Some(FacetLowering {
        field: field.clone(),
        limit: *limit,
        order,
    })
}
