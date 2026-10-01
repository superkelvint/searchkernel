use super::*;
use crate::model::*;
use searchkernel_protocol::wire::v1 as api;

pub(crate) fn grouping_to_input(
    values: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::GroupingRequest<'_>>>>,
) -> Result<Vec<crate::api_to_ir::resolve::GroupingInput>> {
    let mut output = Vec::new();
    if let Some(values) = values {
        for (index, value) in values.iter().enumerate() {
            let structured = match value.source_type() {
                api::GroupingSourceKind::StructuredGroupingSource => value
                    .source_as_structured_grouping_source()
                    .ok_or_else(|| {
                        invalid(
                            "invalid_grouping",
                            Some(format!("grouping[{index}].source")),
                            "structured grouping source is missing",
                        )
                    })?,
                api::GroupingSourceKind::RawGroupingSource => {
                    let raw = value
                        .source_as_raw_grouping_source()
                        .and_then(|value| value.expression())
                        .unwrap_or_default()
                        .to_owned();
                    output.push(crate::api_to_ir::resolve::GroupingInput::Raw(raw));
                    continue;
                }
                _ => {
                    return Err(invalid(
                        "invalid_grouping",
                        Some(format!("grouping[{index}].source")),
                        "grouping request must select a source",
                    ))
                }
            };
            let root = structured.root().ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(format!("grouping[{index}].source.root")),
                    "structured grouping requires an all() root",
                )
            })?;
            output.push(crate::api_to_ir::resolve::GroupingInput::Structured(
                GroupingRequestIr {
                    root: grouping_all_to_ir(root, &format!("grouping[{index}].source.root"))?,
                    continuations: value
                        .continuations()
                        .map(|values| values.iter().map(ToOwned::to_owned).collect())
                        .unwrap_or_default(),
                },
            ));
        }
    }
    Ok(output)
}

pub(crate) fn grouping_all_to_ir(
    value: api::GroupingAllOperation<'_>,
    path: &str,
) -> Result<GroupingAllIr> {
    let operations = value
        .operations()
        .map(|values| {
            values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    grouping_operation_to_ir(value, &format!("{path}.operations[{index}]"))
                })
                .collect()
        })
        .transpose()?
        .unwrap_or_default();
    Ok(GroupingAllIr {
        operations,
        label: normalized_optional_string(value.label()),
    })
}

pub(crate) fn grouping_operation_to_ir(
    value: api::GroupingOperation<'_>,
    path: &str,
) -> Result<GroupingOperationIr> {
    match value.operation_type() {
        api::GroupingOperationKind::GroupingAllOperation => value
            .operation_as_grouping_all_operation()
            .ok_or_else(|| invalid("invalid_grouping", Some(path), "all operation is missing"))
            .and_then(|value| grouping_all_to_ir(value, path))
            .map(GroupingOperationIr::All),
        api::GroupingOperationKind::GroupingEachOperation => {
            let value = value
                .operation_as_grouping_each_operation()
                .ok_or_else(|| {
                    invalid("invalid_grouping", Some(path), "each operation is missing")
                })?;
            let operations = value
                .operations()
                .map(|values| {
                    values
                        .iter()
                        .enumerate()
                        .map(|(index, value)| {
                            grouping_operation_to_ir(value, &format!("{path}.operations[{index}]"))
                        })
                        .collect()
                })
                .transpose()?
                .unwrap_or_default();
            Ok(GroupingOperationIr::Each(GroupingAllIr {
                operations,
                label: normalized_optional_string(value.label()),
            }))
        }
        api::GroupingOperationKind::GroupingGroupOperation => value
            .operation_as_grouping_group_operation()
            .and_then(|value| value.expression())
            .ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "group operation requires an expression",
                )
            })
            .and_then(|value| grouping_expression_to_ir(value, path))
            .map(GroupingOperationIr::Group),
        api::GroupingOperationKind::GroupingOutputOperation => {
            let value = value
                .operation_as_grouping_output_operation()
                .ok_or_else(|| {
                    invalid(
                        "invalid_grouping",
                        Some(path),
                        "output operation is missing",
                    )
                })?;
            let outputs = value
                .outputs()
                .map(|values| {
                    values
                        .iter()
                        .enumerate()
                        .map(|(index, output)| {
                            Ok(GroupingOutputIr {
                                label: normalized_optional_string(output.label()),
                                expression: grouping_expression_to_ir(
                                    output.expression().ok_or_else(|| {
                                        invalid(
                                            "invalid_grouping",
                                            Some(format!("{path}.outputs[{index}].expression")),
                                            "output requires an expression",
                                        )
                                    })?,
                                    &format!("{path}.outputs[{index}].expression"),
                                )?,
                            })
                        })
                        .collect()
                })
                .transpose()?
                .unwrap_or_default();
            Ok(GroupingOperationIr::Output(outputs))
        }
        api::GroupingOperationKind::GroupingMaxOperation => {
            let value = value.operation_as_grouping_max_operation().ok_or_else(|| {
                invalid("invalid_grouping", Some(path), "max operation is missing")
            })?;
            let max = match value.value_type() {
                api::GroupingMaxKind::FiniteGroupingMax => Some(
                    value
                        .value_as_finite_grouping_max()
                        .ok_or_else(|| {
                            invalid("invalid_grouping", Some(path), "finite max is missing")
                        })?
                        .value(),
                ),
                api::GroupingMaxKind::InfiniteGroupingMax => None,
                _ => {
                    return Err(invalid(
                        "invalid_grouping",
                        Some(path),
                        "max requires a value",
                    ))
                }
            };
            Ok(GroupingOperationIr::Max(max))
        }
        api::GroupingOperationKind::GroupingPrecisionOperation => value
            .operation_as_grouping_precision_operation()
            .map(|value| GroupingOperationIr::Precision(value.value()))
            .ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "precision operation is missing",
                )
            }),
        api::GroupingOperationKind::GroupingOrderOperation => {
            let value = value
                .operation_as_grouping_order_operation()
                .ok_or_else(|| {
                    invalid("invalid_grouping", Some(path), "order operation is missing")
                })?;
            let expressions = value
                .expressions()
                .map(|values| {
                    values
                        .iter()
                        .enumerate()
                        .map(|(index, value)| {
                            grouping_expression_to_ir(
                                value.expression().ok_or_else(|| {
                                    invalid(
                                        "invalid_grouping",
                                        Some(format!("{path}.expressions[{index}].expression")),
                                        "order expression is missing",
                                    )
                                })?,
                                &format!("{path}.expressions[{index}]"),
                            )
                        })
                        .collect()
                })
                .transpose()?
                .unwrap_or_default();
            Ok(GroupingOperationIr::Order(expressions))
        }
        api::GroupingOperationKind::GroupingAliasOperation => {
            let value = value
                .operation_as_grouping_alias_operation()
                .ok_or_else(|| {
                    invalid("invalid_grouping", Some(path), "alias operation is missing")
                })?;
            Ok(GroupingOperationIr::Alias {
                name: value
                    .name()
                    .ok_or_else(|| {
                        invalid("invalid_grouping", Some(path), "alias requires a name")
                    })?
                    .to_owned(),
                expression: grouping_expression_to_ir(
                    value.expression().ok_or_else(|| {
                        invalid(
                            "invalid_grouping",
                            Some(path),
                            "alias requires an expression",
                        )
                    })?,
                    path,
                )?,
            })
        }
        api::GroupingOperationKind::GroupingFilterOperation => value
            .operation_as_grouping_filter_operation()
            .and_then(|value| value.filter())
            .ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "filter operation requires a filter",
                )
            })
            .and_then(|value| grouping_filter_to_ir(value, path))
            .map(GroupingOperationIr::Filter),
        api::GroupingOperationKind::GroupingKeepOperation => value
            .operation_as_grouping_keep_operation()
            .and_then(|value| value.filter())
            .ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "keep operation requires a filter",
                )
            })
            .and_then(|value| grouping_filter_to_ir(value, path))
            .map(GroupingOperationIr::Keep),
        api::GroupingOperationKind::GroupingWhereOperation => value
            .operation_as_grouping_where_operation()
            .and_then(|value| value.expression())
            .ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "where operation requires an expression",
                )
            })
            .and_then(|value| grouping_expression_to_ir(value, path))
            .map(GroupingOperationIr::Where),
        _ => Err(invalid(
            "invalid_grouping",
            Some(path),
            "grouping operation must select a kind",
        )),
    }
}

pub(crate) fn grouping_expression_to_ir(
    value: api::GroupingExpression<'_>,
    path: &str,
) -> Result<GroupingExpressionIr> {
    match value.expr_type() {
        api::GroupingExpressionKind::GroupingAttributeExpression => value
            .expr_as_grouping_attribute_expression()
            .and_then(|value| value.field())
            .map(|value| GroupingExpressionIr::Attribute(value.to_owned()))
            .ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "attribute expression requires a field",
                )
            }),
        api::GroupingExpressionKind::GroupingLiteralExpression => value
            .expr_as_grouping_literal_expression()
            .and_then(|value| value.value())
            .ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "literal expression requires a value",
                )
            })
            .and_then(|value| RequestValueIr::from_api(value, path.to_owned()))
            .map(GroupingExpressionIr::Literal),
        api::GroupingExpressionKind::GroupingFunctionExpression => {
            let value = value
                .expr_as_grouping_function_expression()
                .ok_or_else(|| {
                    invalid(
                        "invalid_grouping",
                        Some(path),
                        "function expression is missing",
                    )
                })?;
            let arguments = value
                .arguments()
                .map(|values| {
                    values
                        .iter()
                        .enumerate()
                        .map(|(index, value)| {
                            grouping_expression_to_ir(value, &format!("{path}.arguments[{index}]"))
                        })
                        .collect()
                })
                .transpose()?
                .unwrap_or_default();
            let name = value.name().ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "function expression requires a name",
                )
            })?;
            Ok(GroupingExpressionIr::Function {
                name: name.to_owned(),
                arguments,
            })
        }
        api::GroupingExpressionKind::GroupingUnaryExpression => {
            let value = value.expr_as_grouping_unary_expression().ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "unary expression is missing",
                )
            })?;
            let expression = Box::new(grouping_expression_to_ir(
                value.expression().ok_or_else(|| {
                    invalid(
                        "invalid_grouping",
                        Some(path),
                        "unary expression requires an operand",
                    )
                })?,
                path,
            )?);
            match value.op() {
                api::GroupingUnaryOperator::Positive => {
                    Ok(GroupingExpressionIr::Positive(expression))
                }
                api::GroupingUnaryOperator::Negative => {
                    Ok(GroupingExpressionIr::Negative(expression))
                }
                _ => Err(invalid(
                    "invalid_grouping",
                    Some(path),
                    "unary expression requires an operator",
                )),
            }
        }
        api::GroupingExpressionKind::GroupingBinaryExpression => {
            let value = value.expr_as_grouping_binary_expression().ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "binary expression is missing",
                )
            })?;
            let operator = match value.op() {
                api::GroupingBinaryOperator::Add => GroupingBinaryOperatorIr::Add,
                api::GroupingBinaryOperator::Subtract => GroupingBinaryOperatorIr::Subtract,
                api::GroupingBinaryOperator::Multiply => GroupingBinaryOperatorIr::Multiply,
                api::GroupingBinaryOperator::Divide => GroupingBinaryOperatorIr::Divide,
                api::GroupingBinaryOperator::Modulo => GroupingBinaryOperatorIr::Modulo,
                _ => {
                    return Err(invalid(
                        "invalid_grouping",
                        Some(path),
                        "binary expression requires an operator",
                    ))
                }
            };
            Ok(GroupingExpressionIr::Binary {
                operator,
                left: Box::new(grouping_expression_to_ir(
                    value.left().ok_or_else(|| {
                        invalid(
                            "invalid_grouping",
                            Some(path),
                            "binary expression requires a left operand",
                        )
                    })?,
                    path,
                )?),
                right: Box::new(grouping_expression_to_ir(
                    value.right().ok_or_else(|| {
                        invalid(
                            "invalid_grouping",
                            Some(path),
                            "binary expression requires a right operand",
                        )
                    })?,
                    path,
                )?),
            })
        }
        api::GroupingExpressionKind::GroupingAliasReference => value
            .expr_as_grouping_alias_reference()
            .and_then(|value| value.name())
            .map(|value| GroupingExpressionIr::Alias(value.to_owned()))
            .ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "alias reference requires a name",
                )
            }),
        api::GroupingExpressionKind::GroupingMemberExpression => {
            let value = value.expr_as_grouping_member_expression().ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "member expression is missing",
                )
            })?;
            Ok(GroupingExpressionIr::Member {
                base: Box::new(grouping_expression_to_ir(
                    value.base().ok_or_else(|| {
                        invalid(
                            "invalid_grouping",
                            Some(path),
                            "member expression requires a base",
                        )
                    })?,
                    path,
                )?),
                member: value
                    .member()
                    .ok_or_else(|| {
                        invalid(
                            "invalid_grouping",
                            Some(path),
                            "member expression requires a member",
                        )
                    })?
                    .to_owned(),
            })
        }
        api::GroupingExpressionKind::GroupingMapLookupExpression => {
            let value = value
                .expr_as_grouping_map_lookup_expression()
                .ok_or_else(|| {
                    invalid(
                        "invalid_grouping",
                        Some(path),
                        "map lookup expression is missing",
                    )
                })?;
            Ok(GroupingExpressionIr::MapLookup {
                base: Box::new(grouping_expression_to_ir(
                    value.base().ok_or_else(|| {
                        invalid("invalid_grouping", Some(path), "map lookup requires a base")
                    })?,
                    path,
                )?),
                key: Box::new(grouping_expression_to_ir(
                    value.key().ok_or_else(|| {
                        invalid("invalid_grouping", Some(path), "map lookup requires a key")
                    })?,
                    path,
                )?),
            })
        }
        api::GroupingExpressionKind::GroupingBucketExpression => {
            let value = value.expr_as_grouping_bucket_expression().ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "bucket expression is missing",
                )
            })?;
            Ok(GroupingExpressionIr::Bucket(GroupingBucketIr {
                lower: grouping_range_bound_to_ir(value.lower(), &format!("{path}.lower"))?,
                upper: grouping_range_bound_to_ir(value.upper(), &format!("{path}.upper"))?,
                include_lower: value.include_lower().is_none_or(|value| value.value()),
                include_upper: value.include_upper().is_some_and(|value| value.value()),
            }))
        }
        api::GroupingExpressionKind::GroupingAggregationExpression => value
            .expr_as_grouping_aggregation_expression()
            .ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "aggregation expression is missing",
                )
            })
            .and_then(|value| grouping_aggregation_to_ir(value, path))
            .map(|value| GroupingExpressionIr::Aggregation(Box::new(value))),
        _ => Err(invalid(
            "invalid_grouping",
            Some(path),
            "grouping expression must select a kind",
        )),
    }
}

pub(crate) fn grouping_range_bound_to_ir(
    value: Option<api::RangeBound<'_>>,
    path: &str,
) -> Result<GroupingRangeBoundIr> {
    let value =
        value.ok_or_else(|| invalid("invalid_grouping", Some(path), "bucket bound is required"))?;
    match value.bound_type() {
        api::RangeBoundKind::FiniteRangeBound => value
            .bound_as_finite_range_bound()
            .and_then(|value| value.value())
            .ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "finite bucket bound requires a value",
                )
            })
            .and_then(|value| RequestValueIr::from_api(value, path.to_owned()))
            .map(GroupingRangeBoundIr::Finite),
        api::RangeBoundKind::NegativeInfinityBound => Ok(GroupingRangeBoundIr::NegativeInfinity),
        api::RangeBoundKind::PositiveInfinityBound => Ok(GroupingRangeBoundIr::PositiveInfinity),
        api::RangeBoundKind::UnboundedRangeBound => Ok(GroupingRangeBoundIr::Unbounded),
        _ => Err(invalid(
            "invalid_grouping",
            Some(path),
            "unknown bucket bound",
        )),
    }
}

pub(crate) fn supported_grouping_function(name: &str) -> bool {
    matches!(
        name,
        "cat"
            | "md5"
            | "xor"
            | "fixedwidth"
            | "predefined"
            | "interpolatedlookup"
            | "reverse"
            | "normalize"
            | "strlen"
            | "lowercase"
            | "uppercase"
            | "now"
            | "relevance"
            | "array.at"
            | "map.at"
            | "math.sqrt"
            | "math.log"
            | "math.exp"
            | "math.floor"
            | "math.ceil"
            | "time.year"
            | "time.month"
            | "time.dayofmonth"
            | "time.dayofweek"
            | "time.hour"
            | "time.minute"
            | "time.second"
    )
}

pub(crate) fn grouping_aggregation_to_ir(
    value: api::GroupingAggregationExpression<'_>,
    path: &str,
) -> Result<GroupingAggregationIr> {
    let expression = |value: Option<api::GroupingExpression<'_>>| {
        value
            .ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "aggregation requires an expression",
                )
            })
            .and_then(|value| grouping_expression_to_ir(value, path))
    };
    match value.aggregation_type() {
        api::AggregationKind::CountAggregation => value
            .aggregation_as_count_aggregation()
            .map(|_| GroupingAggregationIr::Count)
            .ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "count aggregation is missing",
                )
            }),
        api::AggregationKind::SumAggregation => expression(
            value
                .aggregation_as_sum_aggregation()
                .and_then(|value| value.expression()),
        )
        .map(GroupingAggregationIr::Sum),
        api::AggregationKind::AvgAggregation => expression(
            value
                .aggregation_as_avg_aggregation()
                .and_then(|value| value.expression()),
        )
        .map(GroupingAggregationIr::Avg),
        api::AggregationKind::MinAggregation => expression(
            value
                .aggregation_as_min_aggregation()
                .and_then(|value| value.expression()),
        )
        .map(GroupingAggregationIr::Min),
        api::AggregationKind::MaxAggregation => expression(
            value
                .aggregation_as_max_aggregation()
                .and_then(|value| value.expression()),
        )
        .map(GroupingAggregationIr::Max),
        api::AggregationKind::XorAggregation => expression(
            value
                .aggregation_as_xor_aggregation()
                .and_then(|value| value.expression()),
        )
        .map(GroupingAggregationIr::Xor),
        api::AggregationKind::StdDevAggregation => expression(
            value
                .aggregation_as_std_dev_aggregation()
                .and_then(|value| value.expression()),
        )
        .map(GroupingAggregationIr::StdDev),
        api::AggregationKind::QuantilesAggregation => {
            let value = value
                .aggregation_as_quantiles_aggregation()
                .ok_or_else(|| {
                    invalid(
                        "invalid_grouping",
                        Some(path),
                        "quantiles aggregation is missing",
                    )
                })?;
            let quantiles: Vec<f64> = value
                .quantiles()
                .map(|values| values.iter().collect())
                .unwrap_or_default();
            Ok(GroupingAggregationIr::Quantiles {
                quantiles,
                expression: expression(value.expression())?,
            })
        }
        api::AggregationKind::SummaryAggregation => Ok(GroupingAggregationIr::Summary(
            value
                .aggregation_as_summary_aggregation()
                .map(|value| normalized_optional_string(value.summary()))
                .ok_or_else(|| {
                    invalid(
                        "invalid_grouping",
                        Some(path),
                        "summary aggregation is missing",
                    )
                })?,
        )),
        _ => Err(invalid(
            "invalid_grouping",
            Some(path),
            "aggregation must select a kind",
        )),
    }
}

pub(crate) fn grouping_filter_to_ir(
    value: api::GroupingFilterExpr<'_>,
    path: &str,
) -> Result<GroupingFilterIr> {
    match value.expr_type() {
        api::GroupingFilterExprKind::GroupingFilterAnd => value
            .expr_as_grouping_filter_and()
            .map(|value| {
                value
                    .clauses()
                    .map(|values| {
                        values
                            .iter()
                            .map(|value| grouping_filter_to_ir(value, path))
                            .collect()
                    })
                    .unwrap_or_else(|| Ok(Vec::new()))
            })
            .ok_or_else(|| invalid("invalid_grouping", Some(path), "filter-and is missing"))?
            .map(GroupingFilterIr::And),
        api::GroupingFilterExprKind::GroupingFilterOr => value
            .expr_as_grouping_filter_or()
            .map(|value| {
                value
                    .clauses()
                    .map(|values| {
                        values
                            .iter()
                            .map(|value| grouping_filter_to_ir(value, path))
                            .collect()
                    })
                    .unwrap_or_else(|| Ok(Vec::new()))
            })
            .ok_or_else(|| invalid("invalid_grouping", Some(path), "filter-or is missing"))?
            .map(GroupingFilterIr::Or),
        api::GroupingFilterExprKind::GroupingFilterNot => value
            .expr_as_grouping_filter_not()
            .and_then(|value| value.clause())
            .ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "filter-not requires a clause",
                )
            })
            .and_then(|value| grouping_filter_to_ir(value, path))
            .map(|value| GroupingFilterIr::Not(Box::new(value))),
        api::GroupingFilterExprKind::GroupingFilterRegex => {
            let value = value.expr_as_grouping_filter_regex().ok_or_else(|| {
                invalid("invalid_grouping", Some(path), "regex filter is missing")
            })?;
            Ok(GroupingFilterIr::Regex {
                expression: Box::new(grouping_expression_to_ir(
                    value.expression().ok_or_else(|| {
                        invalid(
                            "invalid_grouping",
                            Some(path),
                            "regex filter requires an expression",
                        )
                    })?,
                    path,
                )?),
                pattern: value
                    .pattern()
                    .ok_or_else(|| {
                        invalid(
                            "invalid_grouping",
                            Some(path),
                            "regex filter requires a pattern",
                        )
                    })?
                    .to_owned(),
            })
        }
        api::GroupingFilterExprKind::GroupingFilterRange => {
            let value = value.expr_as_grouping_filter_range().ok_or_else(|| {
                invalid("invalid_grouping", Some(path), "range filter is missing")
            })?;
            Ok(GroupingFilterIr::Range {
                expression: Box::new(grouping_expression_to_ir(
                    value.expression().ok_or_else(|| {
                        invalid(
                            "invalid_grouping",
                            Some(path),
                            "range filter requires an expression",
                        )
                    })?,
                    path,
                )?),
                lower: Box::new(grouping_expression_to_ir(
                    value.lower().ok_or_else(|| {
                        invalid(
                            "invalid_grouping",
                            Some(path),
                            "range filter requires a lower bound",
                        )
                    })?,
                    path,
                )?),
                upper: Box::new(grouping_expression_to_ir(
                    value.upper().ok_or_else(|| {
                        invalid(
                            "invalid_grouping",
                            Some(path),
                            "range filter requires an upper bound",
                        )
                    })?,
                    path,
                )?),
                include_lower: value.include_lower().is_none_or(|value| value.value()),
                include_upper: value.include_upper().is_some_and(|value| value.value()),
            })
        }
        api::GroupingFilterExprKind::GroupingFilterIsTrue => value
            .expr_as_grouping_filter_is_true()
            .and_then(|value| value.expression())
            .ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "is-true filter requires an expression",
                )
            })
            .and_then(|value| grouping_expression_to_ir(value, path))
            .map(|value| GroupingFilterIr::IsTrue(Box::new(value))),
        _ => Err(invalid(
            "invalid_grouping",
            Some(path),
            "grouping filter must select a kind",
        )),
    }
}
