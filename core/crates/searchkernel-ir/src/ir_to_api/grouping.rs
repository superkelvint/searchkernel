use super::*;

pub(super) fn grouping_offsets<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    grouping: &[ir::GroupingRequestIr],
) -> Option<
    flatbuffers::WIPOffset<
        flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<api::GroupingRequest<'a>>>,
    >,
> {
    (!grouping.is_empty()).then(|| {
        let values = grouping
            .iter()
            .map(|value| grouping_request_offset(builder, value))
            .collect::<Vec<_>>();
        builder.create_vector(&values)
    })
}

pub(super) fn resolved_grouping_offsets<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    grouping: &[ir::GroupingRequestIr],
) -> Option<
    flatbuffers::WIPOffset<
        flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<api::ResolvedGroupingRequest<'a>>>,
    >,
> {
    (!grouping.is_empty()).then(|| {
        let values = grouping
            .iter()
            .map(|value| {
                let root = grouping_all_offset(builder, &value.root);
                let continuations = (!value.continuations.is_empty()).then(|| {
                    let values = value
                        .continuations
                        .iter()
                        .map(|value| builder.create_string(value))
                        .collect::<Vec<_>>();
                    builder.create_vector(&values)
                });
                api::ResolvedGroupingRequest::create(
                    builder,
                    &api::ResolvedGroupingRequestArgs {
                        root: Some(root),
                        continuations,
                    },
                )
            })
            .collect::<Vec<_>>();
        builder.create_vector(&values)
    })
}

pub(super) fn grouping_request_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &ir::GroupingRequestIr,
) -> flatbuffers::WIPOffset<api::GroupingRequest<'a>> {
    let root = grouping_all_offset(builder, &value.root);
    let source = api::StructuredGroupingSource::create(
        builder,
        &api::StructuredGroupingSourceArgs { root: Some(root) },
    );
    let continuations = (!value.continuations.is_empty()).then(|| {
        let values = value
            .continuations
            .iter()
            .map(|value| builder.create_string(value))
            .collect::<Vec<_>>();
        builder.create_vector(&values)
    });
    api::GroupingRequest::create(
        builder,
        &api::GroupingRequestArgs {
            source_type: api::GroupingSourceKind::StructuredGroupingSource,
            source: Some(source.as_union_value()),
            continuations,
        },
    )
}

pub(super) fn grouping_all_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &ir::GroupingAllIr,
) -> flatbuffers::WIPOffset<api::GroupingAllOperation<'a>> {
    let operations = value
        .operations
        .iter()
        .map(|value| grouping_operation_offset(builder, value))
        .collect::<Vec<_>>();
    let operations = (!operations.is_empty()).then(|| builder.create_vector(&operations));
    let label = value
        .label
        .as_ref()
        .map(|value| builder.create_string(value));
    api::GroupingAllOperation::create(
        builder,
        &api::GroupingAllOperationArgs { operations, label },
    )
}

pub(super) fn grouping_operation_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &ir::GroupingOperationIr,
) -> flatbuffers::WIPOffset<api::GroupingOperation<'a>> {
    let (operation_type, operation) = match value {
        ir::GroupingOperationIr::All(value) => {
            let value = grouping_all_offset(builder, value);
            (
                api::GroupingOperationKind::GroupingAllOperation,
                value.as_union_value(),
            )
        }
        ir::GroupingOperationIr::Each(value) => {
            let operations = value
                .operations
                .iter()
                .map(|value| grouping_operation_offset(builder, value))
                .collect::<Vec<_>>();
            let operations = (!operations.is_empty()).then(|| builder.create_vector(&operations));
            let label = value
                .label
                .as_ref()
                .map(|value| builder.create_string(value));
            let value = api::GroupingEachOperation::create(
                builder,
                &api::GroupingEachOperationArgs { operations, label },
            );
            (
                api::GroupingOperationKind::GroupingEachOperation,
                value.as_union_value(),
            )
        }
        ir::GroupingOperationIr::Group(value) => {
            let expression = grouping_expression_offset(builder, value);
            let value = api::GroupingGroupOperation::create(
                builder,
                &api::GroupingGroupOperationArgs {
                    expression: Some(expression),
                },
            );
            (
                api::GroupingOperationKind::GroupingGroupOperation,
                value.as_union_value(),
            )
        }
        ir::GroupingOperationIr::Output(values) => {
            let values = values
                .iter()
                .map(|value| {
                    let label = value
                        .label
                        .as_ref()
                        .map(|value| builder.create_string(value));
                    let expression = grouping_expression_offset(builder, &value.expression);
                    api::GroupingOutput::create(
                        builder,
                        &api::GroupingOutputArgs {
                            label,
                            expression: Some(expression),
                        },
                    )
                })
                .collect::<Vec<_>>();
            let outputs = (!values.is_empty()).then(|| builder.create_vector(&values));
            let value = api::GroupingOutputOperation::create(
                builder,
                &api::GroupingOutputOperationArgs { outputs },
            );
            (
                api::GroupingOperationKind::GroupingOutputOperation,
                value.as_union_value(),
            )
        }
        ir::GroupingOperationIr::Max(value) => {
            let (value_type, value) = match value {
                Some(value) => {
                    let value = api::FiniteGroupingMax::create(
                        builder,
                        &api::FiniteGroupingMaxArgs { value: *value },
                    );
                    (
                        api::GroupingMaxKind::FiniteGroupingMax,
                        value.as_union_value(),
                    )
                }
                None => {
                    let value =
                        api::InfiniteGroupingMax::create(builder, &api::InfiniteGroupingMaxArgs {});
                    (
                        api::GroupingMaxKind::InfiniteGroupingMax,
                        value.as_union_value(),
                    )
                }
            };
            let value = api::GroupingMaxOperation::create(
                builder,
                &api::GroupingMaxOperationArgs {
                    value_type,
                    value: Some(value),
                },
            );
            (
                api::GroupingOperationKind::GroupingMaxOperation,
                value.as_union_value(),
            )
        }
        ir::GroupingOperationIr::Precision(number) => {
            let value = api::GroupingPrecisionOperation::create(
                builder,
                &api::GroupingPrecisionOperationArgs { value: *number },
            );
            (
                api::GroupingOperationKind::GroupingPrecisionOperation,
                value.as_union_value(),
            )
        }
        ir::GroupingOperationIr::Order(values) => {
            let values = values
                .iter()
                .map(|value| {
                    let expression = grouping_expression_offset(builder, value);
                    api::GroupingOrderExpr::create(
                        builder,
                        &api::GroupingOrderExprArgs {
                            expression: Some(expression),
                        },
                    )
                })
                .collect::<Vec<_>>();
            let expressions = (!values.is_empty()).then(|| builder.create_vector(&values));
            let value = api::GroupingOrderOperation::create(
                builder,
                &api::GroupingOrderOperationArgs { expressions },
            );
            (
                api::GroupingOperationKind::GroupingOrderOperation,
                value.as_union_value(),
            )
        }
        ir::GroupingOperationIr::Alias { name, expression } => {
            let name = builder.create_string(name);
            let expression = grouping_expression_offset(builder, expression);
            let value = api::GroupingAliasOperation::create(
                builder,
                &api::GroupingAliasOperationArgs {
                    name: Some(name),
                    expression: Some(expression),
                },
            );
            (
                api::GroupingOperationKind::GroupingAliasOperation,
                value.as_union_value(),
            )
        }
        ir::GroupingOperationIr::Filter(value) => {
            let filter = grouping_filter_offset(builder, value);
            let value = api::GroupingFilterOperation::create(
                builder,
                &api::GroupingFilterOperationArgs {
                    filter: Some(filter),
                },
            );
            (
                api::GroupingOperationKind::GroupingFilterOperation,
                value.as_union_value(),
            )
        }
        ir::GroupingOperationIr::Keep(value) => {
            let filter = grouping_filter_offset(builder, value);
            let value = api::GroupingKeepOperation::create(
                builder,
                &api::GroupingKeepOperationArgs {
                    filter: Some(filter),
                },
            );
            (
                api::GroupingOperationKind::GroupingKeepOperation,
                value.as_union_value(),
            )
        }
        ir::GroupingOperationIr::Where(value) => {
            let expression = grouping_expression_offset(builder, value);
            let value = api::GroupingWhereOperation::create(
                builder,
                &api::GroupingWhereOperationArgs {
                    expression: Some(expression),
                },
            );
            (
                api::GroupingOperationKind::GroupingWhereOperation,
                value.as_union_value(),
            )
        }
    };
    api::GroupingOperation::create(
        builder,
        &api::GroupingOperationArgs {
            operation_type,
            operation: Some(operation),
        },
    )
}

pub(super) fn grouping_aggregation_expression_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &ir::GroupingAggregationIr,
) -> (
    api::GroupingExpressionKind,
    flatbuffers::WIPOffset<flatbuffers::UnionWIPOffset>,
) {
    match value {
        ir::GroupingAggregationIr::Count => {
            let aggregation = api::CountAggregation::create(builder, &api::CountAggregationArgs {});
            let value = api::GroupingAggregationExpression::create(
                builder,
                &api::GroupingAggregationExpressionArgs {
                    aggregation_type: api::AggregationKind::CountAggregation,
                    aggregation: Some(aggregation.as_union_value()),
                },
            );
            (
                api::GroupingExpressionKind::GroupingAggregationExpression,
                value.as_union_value(),
            )
        }
        ir::GroupingAggregationIr::Sum(expression) => {
            let expression = grouping_expression_offset(builder, expression);
            let aggregation = api::SumAggregation::create(
                builder,
                &api::SumAggregationArgs {
                    expression: Some(expression),
                },
            );
            let value = api::GroupingAggregationExpression::create(
                builder,
                &api::GroupingAggregationExpressionArgs {
                    aggregation_type: api::AggregationKind::SumAggregation,
                    aggregation: Some(aggregation.as_union_value()),
                },
            );
            (
                api::GroupingExpressionKind::GroupingAggregationExpression,
                value.as_union_value(),
            )
        }
        ir::GroupingAggregationIr::Avg(expression) => {
            let expression = grouping_expression_offset(builder, expression);
            let aggregation = api::AvgAggregation::create(
                builder,
                &api::AvgAggregationArgs {
                    expression: Some(expression),
                },
            );
            let value = api::GroupingAggregationExpression::create(
                builder,
                &api::GroupingAggregationExpressionArgs {
                    aggregation_type: api::AggregationKind::AvgAggregation,
                    aggregation: Some(aggregation.as_union_value()),
                },
            );
            (
                api::GroupingExpressionKind::GroupingAggregationExpression,
                value.as_union_value(),
            )
        }
        ir::GroupingAggregationIr::Min(expression) => {
            let expression = grouping_expression_offset(builder, expression);
            let aggregation = api::MinAggregation::create(
                builder,
                &api::MinAggregationArgs {
                    expression: Some(expression),
                },
            );
            let value = api::GroupingAggregationExpression::create(
                builder,
                &api::GroupingAggregationExpressionArgs {
                    aggregation_type: api::AggregationKind::MinAggregation,
                    aggregation: Some(aggregation.as_union_value()),
                },
            );
            (
                api::GroupingExpressionKind::GroupingAggregationExpression,
                value.as_union_value(),
            )
        }
        ir::GroupingAggregationIr::Max(expression) => {
            let expression = grouping_expression_offset(builder, expression);
            let aggregation = api::MaxAggregation::create(
                builder,
                &api::MaxAggregationArgs {
                    expression: Some(expression),
                },
            );
            let value = api::GroupingAggregationExpression::create(
                builder,
                &api::GroupingAggregationExpressionArgs {
                    aggregation_type: api::AggregationKind::MaxAggregation,
                    aggregation: Some(aggregation.as_union_value()),
                },
            );
            (
                api::GroupingExpressionKind::GroupingAggregationExpression,
                value.as_union_value(),
            )
        }
        ir::GroupingAggregationIr::Xor(expression) => {
            let expression = grouping_expression_offset(builder, expression);
            let aggregation = api::XorAggregation::create(
                builder,
                &api::XorAggregationArgs {
                    expression: Some(expression),
                },
            );
            let value = api::GroupingAggregationExpression::create(
                builder,
                &api::GroupingAggregationExpressionArgs {
                    aggregation_type: api::AggregationKind::XorAggregation,
                    aggregation: Some(aggregation.as_union_value()),
                },
            );
            (
                api::GroupingExpressionKind::GroupingAggregationExpression,
                value.as_union_value(),
            )
        }
        ir::GroupingAggregationIr::StdDev(expression) => {
            let expression = grouping_expression_offset(builder, expression);
            let aggregation = api::StdDevAggregation::create(
                builder,
                &api::StdDevAggregationArgs {
                    expression: Some(expression),
                },
            );
            let value = api::GroupingAggregationExpression::create(
                builder,
                &api::GroupingAggregationExpressionArgs {
                    aggregation_type: api::AggregationKind::StdDevAggregation,
                    aggregation: Some(aggregation.as_union_value()),
                },
            );
            (
                api::GroupingExpressionKind::GroupingAggregationExpression,
                value.as_union_value(),
            )
        }
        ir::GroupingAggregationIr::Quantiles {
            quantiles,
            expression,
        } => {
            let quantiles = builder.create_vector(quantiles);
            let expression = grouping_expression_offset(builder, expression);
            let aggregation = api::QuantilesAggregation::create(
                builder,
                &api::QuantilesAggregationArgs {
                    quantiles: Some(quantiles),
                    expression: Some(expression),
                },
            );
            let value = api::GroupingAggregationExpression::create(
                builder,
                &api::GroupingAggregationExpressionArgs {
                    aggregation_type: api::AggregationKind::QuantilesAggregation,
                    aggregation: Some(aggregation.as_union_value()),
                },
            );
            (
                api::GroupingExpressionKind::GroupingAggregationExpression,
                value.as_union_value(),
            )
        }
        ir::GroupingAggregationIr::Summary(summary) => {
            let summary = summary.as_ref().map(|value| builder.create_string(value));
            let aggregation =
                api::SummaryAggregation::create(builder, &api::SummaryAggregationArgs { summary });
            let value = api::GroupingAggregationExpression::create(
                builder,
                &api::GroupingAggregationExpressionArgs {
                    aggregation_type: api::AggregationKind::SummaryAggregation,
                    aggregation: Some(aggregation.as_union_value()),
                },
            );
            (
                api::GroupingExpressionKind::GroupingAggregationExpression,
                value.as_union_value(),
            )
        }
    }
}

pub(super) fn grouping_expression_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &ir::GroupingExpressionIr,
) -> flatbuffers::WIPOffset<api::GroupingExpression<'a>> {
    let (expr_type, expr) = match value {
        ir::GroupingExpressionIr::Attribute(value) => {
            let field = builder.create_string(value);
            let value = api::GroupingAttributeExpression::create(
                builder,
                &api::GroupingAttributeExpressionArgs { field: Some(field) },
            );
            (
                api::GroupingExpressionKind::GroupingAttributeExpression,
                value.as_union_value(),
            )
        }
        ir::GroupingExpressionIr::Literal(value) => {
            let value = request_value_offset(builder, value);
            let value = api::GroupingLiteralExpression::create(
                builder,
                &api::GroupingLiteralExpressionArgs { value: Some(value) },
            );
            (
                api::GroupingExpressionKind::GroupingLiteralExpression,
                value.as_union_value(),
            )
        }
        ir::GroupingExpressionIr::Function { name, arguments } => {
            let name = builder.create_string(name);
            let arguments = arguments
                .iter()
                .map(|value| grouping_expression_offset(builder, value))
                .collect::<Vec<_>>();
            let arguments = (!arguments.is_empty()).then(|| builder.create_vector(&arguments));
            let value = api::GroupingFunctionExpression::create(
                builder,
                &api::GroupingFunctionExpressionArgs {
                    name: Some(name),
                    arguments,
                },
            );
            (
                api::GroupingExpressionKind::GroupingFunctionExpression,
                value.as_union_value(),
            )
        }
        ir::GroupingExpressionIr::Positive(value) => {
            let expression = grouping_expression_offset(builder, value);
            let value = api::GroupingUnaryExpression::create(
                builder,
                &api::GroupingUnaryExpressionArgs {
                    op: api::GroupingUnaryOperator::Positive,
                    expression: Some(expression),
                },
            );
            (
                api::GroupingExpressionKind::GroupingUnaryExpression,
                value.as_union_value(),
            )
        }
        ir::GroupingExpressionIr::Negative(value) => {
            let expression = grouping_expression_offset(builder, value);
            let value = api::GroupingUnaryExpression::create(
                builder,
                &api::GroupingUnaryExpressionArgs {
                    op: api::GroupingUnaryOperator::Negative,
                    expression: Some(expression),
                },
            );
            (
                api::GroupingExpressionKind::GroupingUnaryExpression,
                value.as_union_value(),
            )
        }
        ir::GroupingExpressionIr::Aggregation(value) => {
            grouping_aggregation_expression_offset(builder, value)
        }
        ir::GroupingExpressionIr::Binary {
            operator,
            left,
            right,
        } => {
            let op = match operator {
                ir::GroupingBinaryOperatorIr::Add => api::GroupingBinaryOperator::Add,
                ir::GroupingBinaryOperatorIr::Subtract => api::GroupingBinaryOperator::Subtract,
                ir::GroupingBinaryOperatorIr::Multiply => api::GroupingBinaryOperator::Multiply,
                ir::GroupingBinaryOperatorIr::Divide => api::GroupingBinaryOperator::Divide,
                ir::GroupingBinaryOperatorIr::Modulo => api::GroupingBinaryOperator::Modulo,
            };
            let left = grouping_expression_offset(builder, left);
            let right = grouping_expression_offset(builder, right);
            let value = api::GroupingBinaryExpression::create(
                builder,
                &api::GroupingBinaryExpressionArgs {
                    op,
                    left: Some(left),
                    right: Some(right),
                },
            );
            (
                api::GroupingExpressionKind::GroupingBinaryExpression,
                value.as_union_value(),
            )
        }
        ir::GroupingExpressionIr::Alias(name) => {
            let name = builder.create_string(name);
            let value = api::GroupingAliasReference::create(
                builder,
                &api::GroupingAliasReferenceArgs { name: Some(name) },
            );
            (
                api::GroupingExpressionKind::GroupingAliasReference,
                value.as_union_value(),
            )
        }
        ir::GroupingExpressionIr::Member { base, member } => {
            let base = grouping_expression_offset(builder, base);
            let member = builder.create_string(member);
            let value = api::GroupingMemberExpression::create(
                builder,
                &api::GroupingMemberExpressionArgs {
                    base: Some(base),
                    member: Some(member),
                },
            );
            (
                api::GroupingExpressionKind::GroupingMemberExpression,
                value.as_union_value(),
            )
        }
        ir::GroupingExpressionIr::MapLookup { base, key } => {
            let base = grouping_expression_offset(builder, base);
            let key = grouping_expression_offset(builder, key);
            let value = api::GroupingMapLookupExpression::create(
                builder,
                &api::GroupingMapLookupExpressionArgs {
                    base: Some(base),
                    key: Some(key),
                },
            );
            (
                api::GroupingExpressionKind::GroupingMapLookupExpression,
                value.as_union_value(),
            )
        }
        ir::GroupingExpressionIr::Bucket(value) => {
            let lower = grouping_range_bound_offset(builder, &value.lower);
            let upper = grouping_range_bound_offset(builder, &value.upper);
            let include_lower = api::OptionalBool::new(value.include_lower);
            let include_upper = api::OptionalBool::new(value.include_upper);
            let value = api::GroupingBucketExpression::create(
                builder,
                &api::GroupingBucketExpressionArgs {
                    lower: Some(lower),
                    upper: Some(upper),
                    include_lower: Some(&include_lower),
                    include_upper: Some(&include_upper),
                },
            );
            (
                api::GroupingExpressionKind::GroupingBucketExpression,
                value.as_union_value(),
            )
        }
    };
    api::GroupingExpression::create(
        builder,
        &api::GroupingExpressionArgs {
            expr_type,
            expr: Some(expr),
        },
    )
}

pub(super) fn grouping_filter_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &ir::GroupingFilterIr,
) -> flatbuffers::WIPOffset<api::GroupingFilterExpr<'a>> {
    let (expr_type, expr) = match value {
        ir::GroupingFilterIr::And(values) => {
            let values = values
                .iter()
                .map(|value| grouping_filter_offset(builder, value))
                .collect::<Vec<_>>();
            let clauses = (!values.is_empty()).then(|| builder.create_vector(&values));
            let value =
                api::GroupingFilterAnd::create(builder, &api::GroupingFilterAndArgs { clauses });
            (
                api::GroupingFilterExprKind::GroupingFilterAnd,
                value.as_union_value(),
            )
        }
        ir::GroupingFilterIr::Or(values) => {
            let values = values
                .iter()
                .map(|value| grouping_filter_offset(builder, value))
                .collect::<Vec<_>>();
            let clauses = (!values.is_empty()).then(|| builder.create_vector(&values));
            let value =
                api::GroupingFilterOr::create(builder, &api::GroupingFilterOrArgs { clauses });
            (
                api::GroupingFilterExprKind::GroupingFilterOr,
                value.as_union_value(),
            )
        }
        ir::GroupingFilterIr::Not(value) => {
            let clause = grouping_filter_offset(builder, value);
            let value = api::GroupingFilterNot::create(
                builder,
                &api::GroupingFilterNotArgs {
                    clause: Some(clause),
                },
            );
            (
                api::GroupingFilterExprKind::GroupingFilterNot,
                value.as_union_value(),
            )
        }
        ir::GroupingFilterIr::Regex {
            expression,
            pattern,
        } => {
            let expression = grouping_expression_offset(builder, expression);
            let pattern = builder.create_string(pattern);
            let value = api::GroupingFilterRegex::create(
                builder,
                &api::GroupingFilterRegexArgs {
                    expression: Some(expression),
                    pattern: Some(pattern),
                },
            );
            (
                api::GroupingFilterExprKind::GroupingFilterRegex,
                value.as_union_value(),
            )
        }
        ir::GroupingFilterIr::Range {
            expression,
            lower,
            upper,
            include_lower,
            include_upper,
        } => {
            let expression = grouping_expression_offset(builder, expression);
            let lower = grouping_expression_offset(builder, lower);
            let upper = grouping_expression_offset(builder, upper);
            let include_lower = api::OptionalBool::new(*include_lower);
            let include_upper = api::OptionalBool::new(*include_upper);
            let value = api::GroupingFilterRange::create(
                builder,
                &api::GroupingFilterRangeArgs {
                    expression: Some(expression),
                    lower: Some(lower),
                    upper: Some(upper),
                    include_lower: Some(&include_lower),
                    include_upper: Some(&include_upper),
                },
            );
            (
                api::GroupingFilterExprKind::GroupingFilterRange,
                value.as_union_value(),
            )
        }
        ir::GroupingFilterIr::IsTrue(expression) => {
            let expression = grouping_expression_offset(builder, expression);
            let value = api::GroupingFilterIsTrue::create(
                builder,
                &api::GroupingFilterIsTrueArgs {
                    expression: Some(expression),
                },
            );
            (
                api::GroupingFilterExprKind::GroupingFilterIsTrue,
                value.as_union_value(),
            )
        }
    };
    api::GroupingFilterExpr::create(
        builder,
        &api::GroupingFilterExprArgs {
            expr_type,
            expr: Some(expr),
        },
    )
}

pub(super) fn grouping_range_bound_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &ir::GroupingRangeBoundIr,
) -> flatbuffers::WIPOffset<api::RangeBound<'a>> {
    let (bound_type, bound) = match value {
        ir::GroupingRangeBoundIr::Finite(value) => {
            let value = request_value_offset(builder, value);
            let value = api::FiniteRangeBound::create(
                builder,
                &api::FiniteRangeBoundArgs { value: Some(value) },
            );
            (
                api::RangeBoundKind::FiniteRangeBound,
                value.as_union_value(),
            )
        }
        ir::GroupingRangeBoundIr::NegativeInfinity => {
            let value =
                api::NegativeInfinityBound::create(builder, &api::NegativeInfinityBoundArgs {});
            (
                api::RangeBoundKind::NegativeInfinityBound,
                value.as_union_value(),
            )
        }
        ir::GroupingRangeBoundIr::PositiveInfinity => {
            let value =
                api::PositiveInfinityBound::create(builder, &api::PositiveInfinityBoundArgs {});
            (
                api::RangeBoundKind::PositiveInfinityBound,
                value.as_union_value(),
            )
        }
        ir::GroupingRangeBoundIr::Unbounded => {
            let value = api::UnboundedRangeBound::create(builder, &api::UnboundedRangeBoundArgs {});
            (
                api::RangeBoundKind::UnboundedRangeBound,
                value.as_union_value(),
            )
        }
    };
    api::RangeBound::create(
        builder,
        &api::RangeBoundArgs {
            bound_type,
            bound: Some(bound),
        },
    )
}
