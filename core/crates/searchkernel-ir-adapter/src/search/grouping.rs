//! Lower V2 grouping into either the existing facet path or the supported
//! recursive native grouping-plan subset.

use searchkernel_ir::{PortableError, Result};
use searchkernel_vespa_engine::{
    FacetRequest, GroupingAggregation, GroupingExpression, GroupingLevel, GroupingPlan,
    SearchRequest,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FacetLowering {
    pub(crate) field: String,
    pub(crate) limit: u32,
    pub(crate) order: searchkernel_vespa_engine::GroupingOrder,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NativeGroupingLowering {
    pub(crate) plan: GroupingPlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
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
    let mut levels = Vec::new();
    lower_level(&value.root, &mut levels)?;
    if levels.is_empty() {
        return Err(unsupported_grouping(
            "grouping_level_missing_classifier",
            "a native grouping plan requires an attribute classifier",
        ));
    }
    Ok(GroupingPlan { levels })
}

fn lower_level(
    value: &searchkernel_ir::GroupingAllIr,
    levels: &mut Vec<GroupingLevel>,
) -> Result<()> {
    if value.label.is_some() {
        return Err(unsupported_grouping(
            "grouping_labels_not_supported",
            "native grouping plans do not expose grouping labels",
        ));
    }

    let mut classifier = None;
    let mut aggregations = Vec::new();
    let mut nested = None;
    for operation in &value.operations {
        match operation {
            searchkernel_ir::GroupingOperationIr::Group(expression) => {
                if classifier.is_some() {
                    return Err(unsupported_grouping(
                        "multiple_group_classifiers",
                        "a native grouping level has exactly one attribute classifier",
                    ));
                }
                classifier = Some(lower_classifier(expression)?);
            }
            searchkernel_ir::GroupingOperationIr::Output(outputs) => {
                for output in outputs {
                    if output.label.is_some() {
                        return Err(unsupported_grouping(
                            "grouping_output_labels_not_supported",
                            "native grouping outputs do not expose aliases or labels",
                        ));
                    }
                    aggregations.push(lower_aggregation(&output.expression)?);
                }
            }
            searchkernel_ir::GroupingOperationIr::All(child)
            | searchkernel_ir::GroupingOperationIr::Each(child) => {
                if nested.is_some() {
                    return Err(unsupported_grouping(
                        "branching_grouping_not_supported",
                        "native grouping plans support only linear nested levels",
                    ));
                }
                nested = Some(child);
            }
            _ => {
                return Err(unsupported_grouping(
                    "grouping_operation_not_supported",
                    "the native grouping plan does not support this grouping operation",
                ));
            }
        }
    }

    if let Some(classifier) = classifier {
        levels.push(GroupingLevel {
            classifier,
            aggregations,
        });
    } else {
        if !aggregations.is_empty() {
            return Err(unsupported_grouping(
                "grouping_output_without_classifier",
                "native grouping cannot preserve outputs on a structural grouping wrapper",
            ));
        }
        if nested.is_none() {
            return Err(unsupported_grouping(
                "grouping_level_missing_classifier",
                "a native grouping level requires an attribute classifier",
            ));
        }
    }
    if let Some(nested) = nested {
        lower_level(nested, levels)?;
    }
    Ok(())
}

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
        _ => Err(unsupported_grouping(
            "grouping_aggregation_not_supported",
            "native grouping supports only count and average(attribute)",
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
