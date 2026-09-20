//! Lower V2 grouping into either the existing facet path or the supported
//! recursive native grouping-plan subset.

use searchkernel_ir::{PortableError, Result};
use searchkernel_vespa_engine::{
    FacetRequest, GroupingAggregation, GroupingExpression, GroupingFilter, GroupingLevel,
    GroupingPlan, SearchRequest,
};
use std::collections::BTreeSet;

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
    let mut compiler = GroupingCompiler { levels: Vec::new() };
    compiler.lower_root(&value.root)?;
    let levels = compiler.levels;
    if levels.is_empty() {
        return Err(unsupported_grouping(
            "grouping_level_missing_classifier",
            "a native grouping plan requires an attribute classifier",
        ));
    }
    Ok(GroupingPlan { levels })
}

struct GroupingCompiler {
    levels: Vec<GroupingLevel>,
}

impl GroupingCompiler {
    /// Lower the small linear shape that RequestBuilder can represent as one
    /// backend Grouping: the root/all scope records a classifier and the
    /// following each scope materializes it. Nested all scopes are structural
    /// and must contain the next group/each pair.
    fn lower_root(&mut self, value: &searchkernel_ir::GroupingAllIr) -> Result<()> {
        self.lower_grouping_scope(value)
    }

    fn lower_grouping_scope(&mut self, value: &searchkernel_ir::GroupingAllIr) -> Result<()> {
        if value.label.is_some() {
            return Err(unsupported_grouping(
                "grouping_labels_not_supported",
                "native grouping plans do not expose grouping labels",
            ));
        }

        let mut classifier = None;
        let mut each = None;
        for operation in &value.operations {
            match operation {
                searchkernel_ir::GroupingOperationIr::Group(expression) => {
                    if each.is_some() {
                        return Err(unsupported_grouping(
                            "grouping_group_after_each_not_supported",
                            "group must establish pending state before each materializes it",
                        ));
                    }
                    if classifier.is_some() {
                        return Err(unsupported_grouping(
                            "multiple_group_classifiers",
                            "a native grouping level has exactly one attribute classifier",
                        ));
                    }
                    classifier = Some(lower_classifier(expression)?);
                }
                searchkernel_ir::GroupingOperationIr::Each(child) => {
                    if classifier.is_none() {
                        return Err(unsupported_grouping(
                            "grouping_each_without_group",
                            "each cannot materialize a grouping level without pending group-by state",
                        ));
                    }
                    if each.is_some() {
                        return Err(unsupported_grouping(
                            "parallel_grouping_not_supported",
                            "native grouping plans support one linear each branch",
                        ));
                    }
                    each = Some(child);
                }
                searchkernel_ir::GroupingOperationIr::Output(_) => {
                    return Err(unsupported_grouping(
                        "grouping_output_before_each",
                        "grouping output is attached only after each materializes its pending group-by",
                    ));
                }
                searchkernel_ir::GroupingOperationIr::All(_) => {
                    return Err(unsupported_grouping(
                        "grouping_all_topology_not_supported",
                        "all does not materialize pending group-by state; the supported topology requires each",
                    ));
                }
                _ => {
                    return Err(unsupported_grouping(
                        "grouping_operation_not_supported",
                        "the native grouping plan does not support this grouping operation",
                    ));
                }
            }
        }

        let classifier = classifier.ok_or_else(|| {
            unsupported_grouping(
                "grouping_level_missing_classifier",
                "a native grouping level requires an attribute classifier",
            )
        })?;
        let each = each.ok_or_else(|| {
            unsupported_grouping(
                "grouping_each_required",
                "each is required to materialize pending group-by state",
            )
        })?;
        let level = self.levels.len();
        self.levels.push(GroupingLevel {
            classifier,
            aggregations: Vec::new(),
            max_groups: None,
            precision: None,
            filter: None,
            order_by: Vec::new(),
        });
        self.lower_each_body(each, level)
    }

    fn lower_each_body(
        &mut self,
        value: &searchkernel_ir::GroupingAllIr,
        level: usize,
    ) -> Result<()> {
        if value.label.is_some() {
            return Err(unsupported_grouping(
                "grouping_labels_not_supported",
                "native grouping plans do not expose grouping labels",
            ));
        }

        let mut nested = None;
        let mut output_names = BTreeSet::new();
        for operation in &value.operations {
            match operation {
                searchkernel_ir::GroupingOperationIr::Output(outputs) => {
                    for output in outputs {
                        if output.label.is_some() {
                            return Err(unsupported_grouping(
                                "grouping_output_labels_not_supported",
                                "native grouping outputs do not expose aliases or labels",
                            ));
                        }
                        let aggregation = lower_aggregation(&output.expression)?;
                        if !output_names.insert(grouping_aggregation_name(&aggregation)) {
                            return Err(unsupported_grouping(
                                "grouping_output_name_collision",
                                "native grouping outputs must have unique portable field names",
                            ));
                        }
                        self.levels[level].aggregations.push(aggregation);
                    }
                }
                searchkernel_ir::GroupingOperationIr::Max(value) => {
                    self.levels[level].max_groups = value.map(u64::from);
                }
                searchkernel_ir::GroupingOperationIr::Precision(value) => {
                    self.levels[level].precision = Some(u64::from(*value));
                }
                searchkernel_ir::GroupingOperationIr::Order(expressions) => {
                    for expression in expressions {
                        let (expression, ascending) = match expression {
                            searchkernel_ir::GroupingExpressionIr::Positive(value) => {
                                (lower_classifier(value)?, true)
                            }
                            searchkernel_ir::GroupingExpressionIr::Negative(value) => {
                                (lower_classifier(value)?, false)
                            }
                            _ => (lower_classifier(expression)?, true),
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
                    self.levels[level].filter = Some(lower_filter(filter)?);
                }
                searchkernel_ir::GroupingOperationIr::All(child) => {
                    if nested.is_some() {
                        return Err(unsupported_grouping(
                            "parallel_grouping_not_supported",
                            "native grouping plans support one linear nested branch",
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
                searchkernel_ir::GroupingOperationIr::Group(_) => {
                    return Err(unsupported_grouping(
                        "grouping_group_scope_not_supported",
                        "group must be processed inside an all scope before each",
                    ));
                }
                _ => {
                    return Err(unsupported_grouping(
                        "grouping_operation_not_supported",
                        "the native grouping plan does not support this grouping operation",
                    ));
                }
            }
        }

        if let Some(nested) = nested {
            self.lower_grouping_scope(nested)?;
        }
        Ok(())
    }
}

/*
 * Keep this comment beside the compiler: an output on an all node is not an
 * output for the group that a later each creates. RequestBuilder attaches it
 * to the current Group immediately, so collapsing all/each would change the
 * result scope. The compiler therefore rejects it rather than relocating it.
 */

fn lower_classifier(
    expression: &searchkernel_ir::GroupingExpressionIr,
) -> Result<GroupingExpression> {
    match expression {
        searchkernel_ir::GroupingExpressionIr::Attribute(field) => {
            Ok(GroupingExpression::attribute(field))
        }
        _ => Err(unsupported_grouping(
            "grouping_classifier_not_supported",
            "native grouping classifiers support only attributes",
        )),
    }
}

fn lower_aggregation(
    expression: &searchkernel_ir::GroupingExpressionIr,
) -> Result<GroupingAggregation> {
    let searchkernel_ir::GroupingExpressionIr::Aggregation(aggregation) = expression else {
        return Err(unsupported_grouping(
            "grouping_output_not_supported",
            "native grouping outputs must be count or average aggregations",
        ));
    };
    match aggregation.as_ref() {
        searchkernel_ir::GroupingAggregationIr::Count => Ok(GroupingAggregation::Count),
        searchkernel_ir::GroupingAggregationIr::Avg(expression) => {
            Ok(GroupingAggregation::Average(lower_classifier(expression)?))
        }
        searchkernel_ir::GroupingAggregationIr::Sum(value) => {
            Ok(GroupingAggregation::Sum(lower_classifier(value)?))
        }
        searchkernel_ir::GroupingAggregationIr::Min(value) => {
            Ok(GroupingAggregation::Min(lower_classifier(value)?))
        }
        searchkernel_ir::GroupingAggregationIr::Max(value) => {
            Ok(GroupingAggregation::Max(lower_classifier(value)?))
        }
        searchkernel_ir::GroupingAggregationIr::Xor(value) => {
            Ok(GroupingAggregation::Xor(lower_classifier(value)?))
        }
        searchkernel_ir::GroupingAggregationIr::StdDev(value) => Ok(
            GroupingAggregation::StandardDeviation(lower_classifier(value)?),
        ),
        searchkernel_ir::GroupingAggregationIr::Quantiles {
            quantiles,
            expression,
        } => Ok(GroupingAggregation::Quantiles {
            expression: lower_classifier(expression)?,
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

fn grouping_aggregation_name(aggregation: &GroupingAggregation) -> String {
    match aggregation {
        GroupingAggregation::Count => "count".to_owned(),
        GroupingAggregation::Average(GroupingExpression::Attribute(field)) => {
            format!("average({field})")
        }
        GroupingAggregation::Sum(GroupingExpression::Attribute(field)) => format!("sum({field})"),
        GroupingAggregation::Min(GroupingExpression::Attribute(field)) => format!("min({field})"),
        GroupingAggregation::Max(GroupingExpression::Attribute(field)) => format!("max({field})"),
        GroupingAggregation::StandardDeviation(GroupingExpression::Attribute(field)) => {
            format!("stddev({field})")
        }
        GroupingAggregation::Xor(GroupingExpression::Attribute(field)) => format!("xor({field})"),
        GroupingAggregation::Quantiles {
            expression: GroupingExpression::Attribute(field),
            ..
        } => format!("quantiles({field})"),
        GroupingAggregation::Summary { .. } => "summary".to_owned(),
    }
}

fn lower_filter(value: &searchkernel_ir::GroupingFilterIr) -> Result<GroupingFilter> {
    match value {
        searchkernel_ir::GroupingFilterIr::Regex {
            expression,
            pattern,
        } => {
            let GroupingExpression::Attribute(field) = lower_classifier(expression)?;
            Ok(GroupingFilter::Regex {
                field,
                pattern: pattern.clone(),
            })
        }
        _ => Err(unsupported_grouping(
            "grouping_filter_not_supported",
            "native grouping supports only regex filters over an attribute",
        )),
    }
}

fn unsupported_grouping(code: &'static str, message: &'static str) -> PortableError {
    PortableError::unsupported(code, Some("grouping"), message)
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
