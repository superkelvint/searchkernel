use super::*;

pub(crate) fn generic_search_request_add_grouping_plan(
    request: NonNull<sys::sk_generic_search_request>,
    plan: &NativeGroupingPlan,
) -> Result<(), Error> {
    let mut error = NativeError::new();
    check(
        unsafe {
            sys::sk_generic_search_request_add_grouping_plan(
                request.as_ptr(),
                plan.0.as_ptr(),
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        &error,
    )
}

pub(crate) struct NativeGroupingPlan(pub(crate) NonNull<sys::sk_grouping_plan>);
impl Drop for NativeGroupingPlan {
    fn drop(&mut self) {
        unsafe { sys::sk_grouping_plan_destroy(self.0.as_ptr()) };
    }
}

pub(crate) struct NativeGroupingExpression(pub(crate) NonNull<sys::sk_grouping_expression>);
impl Drop for NativeGroupingExpression {
    fn drop(&mut self) {
        unsafe { sys::sk_grouping_expression_destroy(self.0.as_ptr()) };
    }
}

pub(crate) struct NativeGroupingAggregation(pub(crate) NonNull<sys::sk_grouping_aggregation>);
impl Drop for NativeGroupingAggregation {
    fn drop(&mut self) {
        unsafe { sys::sk_grouping_aggregation_destroy(self.0.as_ptr()) };
    }
}

struct NativeGroupingTreeResult(NonNull<sys::sk_grouping_tree_result>);
impl Drop for NativeGroupingTreeResult {
    fn drop(&mut self) {
        unsafe { sys::sk_grouping_tree_result_destroy(self.0.as_ptr()) };
    }
}

pub(crate) fn grouping_plan_create() -> Result<NativeGroupingPlan, Error> {
    let mut pointer = ptr::null_mut();
    let mut error = NativeError::new();
    let status =
        unsafe { sys::sk_grouping_plan_create(&mut pointer, error.as_mut_ptr(), error.capacity()) };
    check(status, &error)?;
    Ok(NativeGroupingPlan(require_ptr(pointer, "grouping plan")?))
}

pub(crate) fn grouping_plan_set_all(plan: &NativeGroupingPlan, all: bool) -> Result<(), Error> {
    let mut error = NativeError::new();
    check(
        unsafe {
            sys::sk_grouping_plan_set_all(
                plan.0.as_ptr(),
                u8::from(all),
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        &error,
    )
}

pub(crate) fn grouping_expression_attribute(
    field: &str,
) -> Result<NativeGroupingExpression, Error> {
    let field = cstring(field)?;
    let mut pointer = ptr::null_mut();
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_grouping_expression_attribute(
            field.as_ptr(),
            &mut pointer,
            error.as_mut_ptr().cast(),
            error.capacity(),
        )
    };
    check(status, &error)?;
    Ok(NativeGroupingExpression(require_ptr(
        pointer,
        "grouping expression",
    )?))
}

fn grouping_literal_expression(
    status: impl FnOnce(*mut *mut sys::sk_grouping_expression, *mut u8, u32) -> i32,
) -> Result<NativeGroupingExpression, Error> {
    let mut pointer = ptr::null_mut();
    let mut error = NativeError::new();
    check(
        status(&mut pointer, error.as_mut_ptr(), error.capacity()),
        &error,
    )?;
    Ok(NativeGroupingExpression(require_ptr(
        pointer,
        "grouping expression",
    )?))
}

pub(crate) fn grouping_expression_string(value: &str) -> Result<NativeGroupingExpression, Error> {
    let value = cstring(value)?;
    grouping_literal_expression(|pointer, error, capacity| unsafe {
        sys::sk_grouping_expression_string(value.as_ptr(), pointer, error, capacity)
    })
}

pub(crate) fn grouping_expression_integer(value: i64) -> Result<NativeGroupingExpression, Error> {
    grouping_literal_expression(|pointer, error, capacity| unsafe {
        sys::sk_grouping_expression_integer(value, pointer, error, capacity)
    })
}

pub(crate) fn grouping_expression_float(value: f64) -> Result<NativeGroupingExpression, Error> {
    grouping_literal_expression(|pointer, error, capacity| unsafe {
        sys::sk_grouping_expression_float(value, pointer, error, capacity)
    })
}

pub(crate) fn grouping_expression_boolean(value: bool) -> Result<NativeGroupingExpression, Error> {
    grouping_literal_expression(|pointer, error, capacity| unsafe {
        sys::sk_grouping_expression_boolean(u8::from(value), pointer, error, capacity)
    })
}

pub(crate) fn grouping_expression_negative(
    input: &NativeGroupingExpression,
) -> Result<NativeGroupingExpression, Error> {
    let mut pointer = ptr::null_mut();
    let mut error = NativeError::new();
    check(
        unsafe {
            sys::sk_grouping_expression_negative(
                input.0.as_ptr(),
                &mut pointer,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        &error,
    )?;
    Ok(NativeGroupingExpression(require_ptr(
        pointer,
        "grouping expression",
    )?))
}

pub(crate) fn grouping_expression_binary(
    left: &NativeGroupingExpression,
    right: &NativeGroupingExpression,
    operator: GroupingBinaryOperator,
) -> Result<NativeGroupingExpression, Error> {
    let operator = match operator {
        GroupingBinaryOperator::Add => sys::SK_GROUPING_BINARY_ADD,
        GroupingBinaryOperator::Subtract => sys::SK_GROUPING_BINARY_SUBTRACT,
        GroupingBinaryOperator::Multiply => sys::SK_GROUPING_BINARY_MULTIPLY,
        GroupingBinaryOperator::Divide => sys::SK_GROUPING_BINARY_DIVIDE,
        GroupingBinaryOperator::Modulo => sys::SK_GROUPING_BINARY_MODULO,
    };
    let mut pointer = ptr::null_mut();
    let mut error = NativeError::new();
    check(
        unsafe {
            sys::sk_grouping_expression_binary(
                operator,
                left.0.as_ptr(),
                right.0.as_ptr(),
                &mut pointer,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        &error,
    )?;
    Ok(NativeGroupingExpression(require_ptr(
        pointer,
        "grouping expression",
    )?))
}

pub(crate) fn grouping_expression_function(
    name: &str,
    arguments: &[NativeGroupingExpression],
) -> Result<NativeGroupingExpression, Error> {
    let name = cstring(name)?;
    let arguments = arguments
        .iter()
        .map(|argument| argument.0.as_ptr().cast_const())
        .collect::<Vec<_>>();
    let argument_count = u32::try_from(arguments.len())
        .map_err(|_| Error::message("too many grouping function arguments"))?;
    let mut pointer = ptr::null_mut();
    let mut error = NativeError::new();
    check(
        unsafe {
            sys::sk_grouping_expression_function(
                name.as_ptr(),
                arguments.as_ptr(),
                argument_count,
                &mut pointer,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        &error,
    )?;
    Ok(NativeGroupingExpression(require_ptr(
        pointer,
        "grouping expression",
    )?))
}

pub(crate) fn grouping_expression_aggregation_reference(
    index: usize,
) -> Result<NativeGroupingExpression, Error> {
    let index = u32::try_from(index)
        .map_err(|_| Error::message("grouping aggregation reference is too large"))?;
    let mut pointer = ptr::null_mut();
    let mut error = NativeError::new();
    check(
        unsafe {
            sys::sk_grouping_expression_aggregation_reference(
                index,
                &mut pointer,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        &error,
    )?;
    Ok(NativeGroupingExpression(require_ptr(
        pointer,
        "grouping expression",
    )?))
}

pub(crate) fn grouping_aggregation_count() -> Result<NativeGroupingAggregation, Error> {
    let mut pointer = ptr::null_mut();
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_grouping_aggregation_count(
            &mut pointer,
            error.as_mut_ptr().cast(),
            error.capacity(),
        )
    };
    check(status, &error)?;
    Ok(NativeGroupingAggregation(require_ptr(
        pointer,
        "grouping aggregation",
    )?))
}

pub(crate) fn grouping_aggregation_average(
    expression: &NativeGroupingExpression,
) -> Result<NativeGroupingAggregation, Error> {
    let mut pointer = ptr::null_mut();
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_grouping_aggregation_average(
            expression.0.as_ptr(),
            &mut pointer,
            error.as_mut_ptr().cast(),
            error.capacity(),
        )
    };
    check(status, &error)?;
    Ok(NativeGroupingAggregation(require_ptr(
        pointer,
        "grouping aggregation",
    )?))
}

fn grouping_aggregation_expression(
    expression: &NativeGroupingExpression,
    kind: u32,
) -> Result<NativeGroupingAggregation, Error> {
    let mut pointer = ptr::null_mut();
    let mut error = NativeError::new();
    let status = unsafe {
        match kind {
            sys::SK_AGGREGATION_SUM => sys::sk_grouping_aggregation_sum(
                expression.0.as_ptr(),
                &mut pointer,
                error.as_mut_ptr(),
                error.capacity(),
            ),
            sys::SK_AGGREGATION_MIN => sys::sk_grouping_aggregation_min(
                expression.0.as_ptr(),
                &mut pointer,
                error.as_mut_ptr(),
                error.capacity(),
            ),
            sys::SK_AGGREGATION_MAX => sys::sk_grouping_aggregation_max(
                expression.0.as_ptr(),
                &mut pointer,
                error.as_mut_ptr(),
                error.capacity(),
            ),
            sys::SK_AGGREGATION_STDDEV => sys::sk_grouping_aggregation_stddev(
                expression.0.as_ptr(),
                &mut pointer,
                error.as_mut_ptr(),
                error.capacity(),
            ),
            sys::SK_AGGREGATION_XOR => sys::sk_grouping_aggregation_xor(
                expression.0.as_ptr(),
                &mut pointer,
                error.as_mut_ptr(),
                error.capacity(),
            ),
            _ => return Err(Error::message("unknown grouping aggregation kind")),
        }
    };
    check(status, &error)?;
    Ok(NativeGroupingAggregation(require_ptr(
        pointer,
        "grouping aggregation",
    )?))
}

pub(crate) fn grouping_aggregation_for_expression(
    expression: &NativeGroupingExpression,
    kind: u32,
) -> Result<NativeGroupingAggregation, Error> {
    grouping_aggregation_expression(expression, kind)
}

pub(crate) fn grouping_aggregation_quantiles(
    expression: &NativeGroupingExpression,
    quantiles: &[f64],
) -> Result<NativeGroupingAggregation, Error> {
    let mut pointer = ptr::null_mut();
    let mut error = NativeError::new();
    let count = u32::try_from(quantiles.len())
        .map_err(|_| Error::message("too many grouping quantiles"))?;
    check(
        unsafe {
            sys::sk_grouping_aggregation_quantiles(
                expression.0.as_ptr(),
                quantiles.as_ptr(),
                count,
                &mut pointer,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        &error,
    )?;
    Ok(NativeGroupingAggregation(require_ptr(
        pointer,
        "grouping aggregation",
    )?))
}

pub(crate) fn grouping_aggregation_summary(
    summary_class: &str,
    max_hits: u32,
) -> Result<NativeGroupingAggregation, Error> {
    let summary_class = cstring(summary_class)?;
    let mut pointer = ptr::null_mut();
    let mut error = NativeError::new();
    check(
        unsafe {
            sys::sk_grouping_aggregation_summary(
                summary_class.as_ptr(),
                max_hits,
                &mut pointer,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        &error,
    )?;
    Ok(NativeGroupingAggregation(require_ptr(
        pointer,
        "grouping aggregation",
    )?))
}

pub(crate) fn grouping_plan_add_level(
    plan: &NativeGroupingPlan,
    classifier: &NativeGroupingExpression,
) -> Result<NonNull<sys::sk_grouping_level>, Error> {
    let mut pointer = ptr::null_mut();
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_grouping_plan_add_level(
            plan.0.as_ptr(),
            classifier.0.as_ptr(),
            &mut pointer,
            error.as_mut_ptr().cast(),
            error.capacity(),
        )
    };
    check(status, &error)?;
    require_ptr(pointer, "grouping level")
}

pub(crate) fn grouping_level_set_limits(
    level: NonNull<sys::sk_grouping_level>,
    max_groups: Option<u64>,
    precision: Option<u64>,
) -> Result<(), Error> {
    if let Some(value) = max_groups {
        let value =
            i64::try_from(value).map_err(|_| Error::message("grouping max groups is too large"))?;
        let mut error = NativeError::new();
        check(
            unsafe {
                sys::sk_grouping_level_set_max_groups(
                    level.as_ptr(),
                    value,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            },
            &error,
        )?;
    }
    if let Some(value) = precision {
        let value =
            i64::try_from(value).map_err(|_| Error::message("grouping precision is too large"))?;
        let mut error = NativeError::new();
        check(
            unsafe {
                sys::sk_grouping_level_set_precision(
                    level.as_ptr(),
                    value,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            },
            &error,
        )?;
    }
    Ok(())
}

pub(crate) fn grouping_level_add_order_by(
    level: NonNull<sys::sk_grouping_level>,
    expression: &NativeGroupingExpression,
    ascending: bool,
) -> Result<(), Error> {
    let mut error = NativeError::new();
    check(
        unsafe {
            sys::sk_grouping_level_add_order_by(
                level.as_ptr(),
                expression.0.as_ptr(),
                u8::from(ascending),
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        &error,
    )
}

pub(crate) struct NativeGroupingFilter(NonNull<sys::sk_grouping_filter>);
impl Drop for NativeGroupingFilter {
    fn drop(&mut self) {
        unsafe { sys::sk_grouping_filter_destroy(self.0.as_ptr()) };
    }
}

pub(crate) fn grouping_filter_regex(
    pattern: &str,
    field: &str,
) -> Result<NativeGroupingFilter, Error> {
    let pattern = cstring(pattern)?;
    let field = cstring(field)?;
    let mut pointer = ptr::null_mut();
    let mut error = NativeError::new();
    check(
        unsafe {
            sys::sk_grouping_filter_regex(
                pattern.as_ptr(),
                field.as_ptr(),
                &mut pointer,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        &error,
    )?;
    Ok(NativeGroupingFilter(require_ptr(
        pointer,
        "grouping filter",
    )?))
}

pub(crate) fn grouping_filter_regex_expression(
    pattern: &str,
    expression: &NativeGroupingExpression,
) -> Result<NativeGroupingFilter, Error> {
    let pattern = cstring(pattern)?;
    let mut pointer = ptr::null_mut();
    let mut error = NativeError::new();
    check(
        unsafe {
            sys::sk_grouping_filter_regex_expression(
                pattern.as_ptr(),
                expression.0.as_ptr(),
                &mut pointer,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        &error,
    )?;
    Ok(NativeGroupingFilter(require_ptr(
        pointer,
        "grouping filter",
    )?))
}

pub(crate) fn grouping_filter_multi(
    filters: &[NativeGroupingFilter],
    and: bool,
) -> Result<NativeGroupingFilter, Error> {
    let filters = filters
        .iter()
        .map(|filter| filter.0.as_ptr().cast_const())
        .collect::<Vec<_>>();
    let count = u32::try_from(filters.len())
        .map_err(|_| Error::message("too many grouping filter clauses"))?;
    let mut pointer = ptr::null_mut();
    let mut error = NativeError::new();
    let status = unsafe {
        if and {
            sys::sk_grouping_filter_and(
                filters.as_ptr(),
                count,
                &mut pointer,
                error.as_mut_ptr(),
                error.capacity(),
            )
        } else {
            sys::sk_grouping_filter_or(
                filters.as_ptr(),
                count,
                &mut pointer,
                error.as_mut_ptr(),
                error.capacity(),
            )
        }
    };
    check(status, &error)?;
    Ok(NativeGroupingFilter(require_ptr(
        pointer,
        "grouping filter",
    )?))
}

pub(crate) fn grouping_filter_not(
    filter: &NativeGroupingFilter,
) -> Result<NativeGroupingFilter, Error> {
    let mut pointer = ptr::null_mut();
    let mut error = NativeError::new();
    check(
        unsafe {
            sys::sk_grouping_filter_not(
                filter.0.as_ptr(),
                &mut pointer,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        &error,
    )?;
    Ok(NativeGroupingFilter(require_ptr(
        pointer,
        "grouping filter",
    )?))
}

pub(crate) fn grouping_filter_range(
    expression: &NativeGroupingExpression,
    lower: f64,
    upper: f64,
    include_lower: bool,
    include_upper: bool,
) -> Result<NativeGroupingFilter, Error> {
    let mut pointer = ptr::null_mut();
    let mut error = NativeError::new();
    check(
        unsafe {
            sys::sk_grouping_filter_range(
                lower,
                upper,
                expression.0.as_ptr(),
                u8::from(include_lower),
                u8::from(include_upper),
                &mut pointer,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        &error,
    )?;
    Ok(NativeGroupingFilter(require_ptr(
        pointer,
        "grouping filter",
    )?))
}

pub(crate) fn grouping_filter_is_true(
    expression: &NativeGroupingExpression,
) -> Result<NativeGroupingFilter, Error> {
    let mut pointer = ptr::null_mut();
    let mut error = NativeError::new();
    check(
        unsafe {
            sys::sk_grouping_filter_is_true(
                expression.0.as_ptr(),
                &mut pointer,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        &error,
    )?;
    Ok(NativeGroupingFilter(require_ptr(
        pointer,
        "grouping filter",
    )?))
}

pub(crate) fn grouping_level_set_filter(
    level: NonNull<sys::sk_grouping_level>,
    filter: &NativeGroupingFilter,
) -> Result<(), Error> {
    let mut error = NativeError::new();
    check(
        unsafe {
            sys::sk_grouping_level_set_filter(
                level.as_ptr(),
                filter.0.as_ptr(),
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        &error,
    )
}

pub(crate) fn grouping_level_add_aggregation(
    level: NonNull<sys::sk_grouping_level>,
    aggregation: &NativeGroupingAggregation,
) -> Result<(), Error> {
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_grouping_level_add_aggregation_handle(
            level.as_ptr(),
            aggregation.0.as_ptr(),
            error.as_mut_ptr().cast(),
            error.capacity(),
        )
    };
    check(status, &error)
}

pub(crate) fn grouping_plan_add_root_aggregation(
    plan: &NativeGroupingPlan,
    aggregation: &NativeGroupingAggregation,
) -> Result<(), Error> {
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_grouping_plan_add_root_aggregation_handle(
            plan.0.as_ptr(),
            aggregation.0.as_ptr(),
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

pub(crate) fn engine_group_tree(
    engine: NonNull<sys::sk_engine>,
    query: NonNull<sys::sk_query>,
    plan: &NativeGroupingPlan,
) -> Result<GroupingResult, Error> {
    let mut pointer = ptr::null_mut();
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_engine_group_tree(
            engine.as_ptr(),
            query.as_ptr(),
            plan.0.as_ptr(),
            &mut pointer,
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)?;
    read_grouping_tree_result(require_ptr(pointer, "grouping tree result")?)
}

pub(crate) fn engine_group_tree_with_request(
    engine: NonNull<sys::sk_engine>,
    query: NonNull<sys::sk_query>,
    request: NonNull<sys::sk_generic_search_request>,
    plan: &NativeGroupingPlan,
) -> Result<GroupingResult, Error> {
    let mut pointer = ptr::null_mut();
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_engine_group_tree_with_request(
            engine.as_ptr(),
            query.as_ptr(),
            request.as_ptr(),
            plan.0.as_ptr(),
            &mut pointer,
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)?;
    read_grouping_tree_result(require_ptr(pointer, "grouping tree result")?)
}

fn grouping_result_value(
    pointer: *const sys::sk_grouping_result_value,
) -> Result<GroupingResultValue, Error> {
    let pointer = NonNull::new(pointer.cast_mut())
        .ok_or_else(|| Error::corrupt("native grouping returned a null result value"))?;
    let value = pointer.as_ptr();
    match unsafe { sys::sk_grouping_result_value_kind(value) } {
        sys::SK_GROUPING_RESULT_VALUE_STRING => {
            let mut string = ptr::null();
            let mut length = 0;
            let mut error = NativeError::new();
            check(
                unsafe {
                    sys::sk_grouping_result_value_string(
                        value,
                        &mut string,
                        &mut length,
                        error.as_mut_ptr().cast(),
                        error.capacity(),
                    )
                },
                &error,
            )?;
            if length != 0 && string.is_null() {
                return Err(Error::corrupt(
                    "native grouping returned a null string value",
                ));
            }
            let bytes = if length == 0 {
                &[][..]
            } else {
                unsafe { std::slice::from_raw_parts(string.cast::<u8>(), length as usize) }
            };
            Ok(GroupingResultValue::String(
                String::from_utf8(bytes.to_vec())
                    .map_err(|_| Error::corrupt("native grouping returned invalid UTF-8"))?,
            ))
        }
        sys::SK_GROUPING_RESULT_VALUE_INTEGER => {
            let mut integer = 0;
            let mut error = NativeError::new();
            check(
                unsafe {
                    sys::sk_grouping_result_value_integer(
                        value,
                        &mut integer,
                        error.as_mut_ptr().cast(),
                        error.capacity(),
                    )
                },
                &error,
            )?;
            Ok(GroupingResultValue::Integer(integer))
        }
        sys::SK_GROUPING_RESULT_VALUE_FLOAT => {
            let mut floating = 0.0;
            let mut error = NativeError::new();
            check(
                unsafe {
                    sys::sk_grouping_result_value_float(
                        value,
                        &mut floating,
                        error.as_mut_ptr().cast(),
                        error.capacity(),
                    )
                },
                &error,
            )?;
            Ok(GroupingResultValue::Float(floating))
        }
        sys::SK_GROUPING_RESULT_VALUE_BOOLEAN => {
            let mut boolean = 0;
            let mut error = NativeError::new();
            check(
                unsafe {
                    sys::sk_grouping_result_value_boolean(
                        value,
                        &mut boolean,
                        error.as_mut_ptr().cast(),
                        error.capacity(),
                    )
                },
                &error,
            )?;
            Ok(GroupingResultValue::Boolean(boolean != 0))
        }
        sys::SK_GROUPING_RESULT_VALUE_QUANTILES => {
            let count = unsafe { sys::sk_grouping_result_value_quantile_count(value) };
            let mut values = Vec::with_capacity(count as usize);
            for index in 0..count {
                let mut quantile = 0.0;
                let mut result = 0.0;
                let mut error = NativeError::new();
                check(
                    unsafe {
                        sys::sk_grouping_result_value_quantile(
                            value,
                            index,
                            &mut quantile,
                            &mut result,
                            error.as_mut_ptr().cast(),
                            error.capacity(),
                        )
                    },
                    &error,
                )?;
                values.push((quantile, result));
            }
            Ok(GroupingResultValue::Quantiles(values))
        }
        kind => Err(Error::corrupt(format!(
            "native grouping returned unknown result-value kind {kind}"
        ))),
    }
}

fn grouping_output(pointer: *const sys::sk_grouping_tree_output) -> Result<GroupingOutput, Error> {
    let pointer = NonNull::new(pointer.cast_mut())
        .ok_or_else(|| Error::corrupt("native grouping returned a null output"))?;
    let output = pointer.as_ptr();
    match unsafe { sys::sk_grouping_tree_output_kind(output) } {
        sys::SK_GROUPING_TREE_OUTPUT_VALUE => {
            let mut value = ptr::null();
            let mut error = NativeError::new();
            check(
                unsafe {
                    sys::sk_grouping_tree_output_value(
                        output,
                        &mut value,
                        error.as_mut_ptr().cast(),
                        error.capacity(),
                    )
                },
                &error,
            )?;
            Ok(GroupingOutput::Value(grouping_result_value(value)?))
        }
        sys::SK_GROUPING_TREE_OUTPUT_HIT_LIST => {
            let count = unsafe { sys::sk_grouping_tree_output_hit_count(output) };
            let mut hits = Vec::with_capacity(count as usize);
            for index in 0..count {
                let mut id = ptr::null();
                let mut id_length = 0;
                let mut error = NativeError::new();
                check(
                    unsafe {
                        sys::sk_grouping_tree_output_hit_id(
                            output,
                            index,
                            &mut id,
                            &mut id_length,
                            error.as_mut_ptr().cast(),
                            error.capacity(),
                        )
                    },
                    &error,
                )?;
                let id = unsafe { std::slice::from_raw_parts(id.cast::<u8>(), id_length as usize) };
                let document_id = String::from_utf8(id.to_vec())
                    .map_err(|_| Error::corrupt("native grouping returned a non-UTF-8 hit ID"))?;
                let mut score = 0.0;
                let mut error = NativeError::new();
                check(
                    unsafe {
                        sys::sk_grouping_tree_output_hit_score(
                            output,
                            index,
                            &mut score,
                            error.as_mut_ptr().cast(),
                            error.capacity(),
                        )
                    },
                    &error,
                )?;
                let summary_count =
                    unsafe { sys::sk_grouping_tree_output_hit_summary_count(output, index) };
                let mut fields = BTreeMap::new();
                for summary_index in 0..summary_count {
                    let mut name = ptr::null();
                    let mut name_length = 0;
                    let mut error = NativeError::new();
                    check(
                        unsafe {
                            sys::sk_grouping_tree_output_hit_summary_name(
                                output,
                                index,
                                summary_index,
                                &mut name,
                                &mut name_length,
                                error.as_mut_ptr().cast(),
                                error.capacity(),
                            )
                        },
                        &error,
                    )?;
                    if name.is_null() {
                        return Err(Error::corrupt(
                            "native grouping returned a null summary field name",
                        ));
                    }
                    let name = unsafe {
                        std::slice::from_raw_parts(name.cast::<u8>(), name_length as usize)
                    };
                    let name = String::from_utf8(name.to_vec()).map_err(|_| {
                        Error::corrupt("native grouping returned a non-UTF-8 summary field name")
                    })?;
                    let mut kind = 0;
                    let mut string = ptr::null();
                    let mut string_length = 0;
                    let mut integer = 0;
                    let mut float = 0.0;
                    let mut boolean = 0;
                    let mut error = NativeError::new();
                    check(
                        unsafe {
                            sys::sk_grouping_tree_output_hit_summary_value(
                                output,
                                index,
                                summary_index,
                                &mut kind,
                                &mut string,
                                &mut string_length,
                                &mut integer,
                                &mut float,
                                &mut boolean,
                                error.as_mut_ptr().cast(),
                                error.capacity(),
                            )
                        },
                        &error,
                    )?;
                    let value = match kind {
                        sys::SK_VALUE_STRING => {
                            if string.is_null() {
                                return Err(Error::corrupt(
                                    "native grouping returned a null summary string",
                                ));
                            }
                            let value = unsafe {
                                std::slice::from_raw_parts(
                                    string.cast::<u8>(),
                                    string_length as usize,
                                )
                            };
                            FieldValue::Scalar(ScalarValue::String(
                                String::from_utf8(value.to_vec()).map_err(|_| {
                                    Error::corrupt(
                                        "native grouping returned a non-UTF-8 summary string",
                                    )
                                })?,
                            ))
                        }
                        sys::SK_VALUE_INTEGER => FieldValue::Scalar(ScalarValue::Integer(integer)),
                        sys::SK_VALUE_FLOAT => FieldValue::Scalar(ScalarValue::Float(float)),
                        sys::SK_VALUE_BOOLEAN => {
                            FieldValue::Scalar(ScalarValue::Boolean(boolean != 0))
                        }
                        _ => {
                            return Err(Error::corrupt(
                                "native grouping returned an unknown summary value kind",
                            ))
                        }
                    };
                    fields.insert(name, value);
                }
                hits.push(Hit {
                    document_id,
                    score,
                    fields,
                    highlights: Vec::new(),
                    match_features: BTreeMap::new(),
                });
            }
            Ok(GroupingOutput::HitList(hits))
        }
        kind => Err(Error::corrupt(format!(
            "native grouping returned unknown output kind {kind}"
        ))),
    }
}

pub(super) fn grouping_tree_node(
    node: *const sys::sk_grouping_tree_node,
    include_key: bool,
) -> Result<GroupingResult, Error> {
    let node = NonNull::new(node.cast_mut())
        .ok_or_else(|| Error::corrupt("native grouping returned a null tree node"))?;
    let node = node.as_ptr();
    let key = if include_key {
        let mut value = ptr::null();
        let mut error = NativeError::new();
        check(
            unsafe {
                sys::sk_grouping_tree_node_key(
                    node,
                    &mut value,
                    error.as_mut_ptr().cast(),
                    error.capacity(),
                )
            },
            &error,
        )?;
        Some(grouping_result_value(value)?)
    } else {
        None
    };
    let output_count = unsafe { sys::sk_grouping_tree_node_output_count(node) };
    let mut outputs = Vec::with_capacity(output_count as usize);
    for index in 0..output_count {
        let mut output = ptr::null();
        let mut error = NativeError::new();
        check(
            unsafe {
                sys::sk_grouping_tree_node_output(
                    node,
                    index,
                    &mut output,
                    error.as_mut_ptr().cast(),
                    error.capacity(),
                )
            },
            &error,
        )?;
        outputs.push(grouping_output(output)?);
    }
    let group_count = unsafe { sys::sk_grouping_tree_node_group_count(node) };
    let mut groups = Vec::with_capacity(group_count as usize);
    for index in 0..group_count {
        let mut group = ptr::null();
        let mut error = NativeError::new();
        check(
            unsafe {
                sys::sk_grouping_tree_node_group(
                    node,
                    index,
                    &mut group,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            },
            &error,
        )?;
        groups.push(grouping_tree_node(group, true)?);
    }
    Ok(GroupingResult {
        key,
        outputs,
        groups,
    })
}

fn read_grouping_tree_result(
    pointer: NonNull<sys::sk_grouping_tree_result>,
) -> Result<GroupingResult, Error> {
    let result = NativeGroupingTreeResult(pointer);
    let mut root = ptr::null();
    let mut error = NativeError::new();
    check(
        unsafe {
            sys::sk_grouping_tree_result_root(
                result.0.as_ptr(),
                &mut root,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        &error,
    )?;
    grouping_tree_node(root, false)
}
