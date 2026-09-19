use super::*;

pub(super) fn value_object_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    values: &std::collections::BTreeMap<String, ir::RequestValueIr>,
) -> flatbuffers::WIPOffset<api::ValueObject<'a>> {
    let fields = request_value_entries_offset(builder, values);
    api::ValueObject::create(builder, &api::ValueObjectArgs { fields })
}

pub(super) fn comparison_op_to_api(value: ir::ComparisonOp) -> api::ComparisonOp {
    comparison_op(value)
}

pub(super) fn filter_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    filter: &ir::FilterIr,
) -> flatbuffers::WIPOffset<api::FilterExpr<'a>> {
    let (expr_type, expr) = match filter {
        ir::FilterIr::And(clauses) => {
            let clauses = clauses
                .iter()
                .map(|clause| filter_offset(builder, clause))
                .collect::<Vec<_>>();
            let clauses = builder.create_vector(&clauses);
            let value = api::FilterAnd::create(
                builder,
                &api::FilterAndArgs {
                    clauses: Some(clauses),
                },
            );
            (api::FilterExprKind::FilterAnd, value.as_union_value())
        }
        ir::FilterIr::Or(clauses) => {
            let clauses = clauses
                .iter()
                .map(|clause| filter_offset(builder, clause))
                .collect::<Vec<_>>();
            let clauses = builder.create_vector(&clauses);
            let value = api::FilterOr::create(
                builder,
                &api::FilterOrArgs {
                    clauses: Some(clauses),
                },
            );
            (api::FilterExprKind::FilterOr, value.as_union_value())
        }
        ir::FilterIr::Not(clause) => {
            let clause = filter_offset(builder, clause);
            let value = api::FilterNot::create(
                builder,
                &api::FilterNotArgs {
                    clause: Some(clause),
                },
            );
            (api::FilterExprKind::FilterNot, value.as_union_value())
        }
        ir::FilterIr::Comparison(comparison) => {
            let field = query_field_ref_offset(builder, comparison.field.as_str());
            let value = scalar_value_offset(builder, &comparison.value);
            let value = api::FilterComparison::create(
                builder,
                &api::FilterComparisonArgs {
                    field: Some(field),
                    op: comparison_op(comparison.op),
                    value: Some(value),
                },
            );
            (
                api::FilterExprKind::FilterComparison,
                value.as_union_value(),
            )
        }
        ir::FilterIr::Range(range) => {
            let field = query_field_ref_offset(builder, range.field.as_str());
            let lower = range_bound_offset(builder, &range.lower);
            let upper = range_bound_offset(builder, &range.upper);
            let include_lower = api::OptionalBool::new(range.include_lower);
            let include_upper = api::OptionalBool::new(range.include_upper);
            let value = api::FilterRange::create(
                builder,
                &api::FilterRangeArgs {
                    field: Some(field),
                    lower: Some(lower),
                    upper: Some(upper),
                    include_lower: Some(&include_lower),
                    include_upper: Some(&include_upper),
                },
            );
            (api::FilterExprKind::FilterRange, value.as_union_value())
        }
        ir::FilterIr::In { field, values } => {
            let field = query_field_ref_offset(builder, field.as_str());
            let values = values
                .iter()
                .map(|value| scalar_value_offset(builder, value))
                .collect::<Vec<_>>();
            let values = builder.create_vector(&values);
            let value = api::FilterIn::create(
                builder,
                &api::FilterInArgs {
                    field: Some(field),
                    values: Some(values),
                },
            );
            (api::FilterExprKind::FilterIn, value.as_union_value())
        }
        ir::FilterIr::Exists { field } => {
            let field = query_field_ref_offset(builder, field.as_str());
            let value =
                api::FilterExists::create(builder, &api::FilterExistsArgs { field: Some(field) });
            (api::FilterExprKind::FilterExists, value.as_union_value())
        }
    };
    api::FilterExpr::create(
        builder,
        &api::FilterExprArgs {
            expr_type,
            expr: Some(expr),
        },
    )
}

pub(super) fn range_bound_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &ir::FilterRangeBoundIr,
) -> flatbuffers::WIPOffset<api::RangeBound<'a>> {
    let (bound_type, bound) = match value {
        ir::FilterRangeBoundIr::Finite(value) => (api::RangeBoundKind::FiniteRangeBound, {
            let value = scalar_value_offset(builder, value);
            api::FiniteRangeBound::create(
                builder,
                &api::FiniteRangeBoundArgs { value: Some(value) },
            )
            .as_union_value()
        }),
        ir::FilterRangeBoundIr::Unbounded => (
            api::RangeBoundKind::UnboundedRangeBound,
            api::UnboundedRangeBound::create(builder, &api::UnboundedRangeBoundArgs {})
                .as_union_value(),
        ),
        ir::FilterRangeBoundIr::NegativeInfinity => (
            api::RangeBoundKind::NegativeInfinityBound,
            api::NegativeInfinityBound::create(builder, &api::NegativeInfinityBoundArgs {})
                .as_union_value(),
        ),
        ir::FilterRangeBoundIr::PositiveInfinity => (
            api::RangeBoundKind::PositiveInfinityBound,
            api::PositiveInfinityBound::create(builder, &api::PositiveInfinityBoundArgs {})
                .as_union_value(),
        ),
    };
    api::RangeBound::create(
        builder,
        &api::RangeBoundArgs {
            bound_type,
            bound: Some(bound),
        },
    )
}

pub(super) fn scalar_value_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &ir::ScalarValueIr,
) -> flatbuffers::WIPOffset<api::Value<'a>> {
    let (kind_type, kind) = match value {
        ir::ScalarValueIr::Bool(value) => (
            api::ValueKind::BoolValue,
            api::BoolValue::create(builder, &api::BoolValueArgs { value: *value }).as_union_value(),
        ),
        ir::ScalarValueIr::Int64(value) => (
            api::ValueKind::Int64Value,
            api::Int64Value::create(builder, &api::Int64ValueArgs { value: *value })
                .as_union_value(),
        ),
        ir::ScalarValueIr::Float64(value) => (
            api::ValueKind::Float64Value,
            api::Float64Value::create(builder, &api::Float64ValueArgs { value: *value })
                .as_union_value(),
        ),
        ir::ScalarValueIr::String(value) => {
            let value = builder.create_string(value);
            let value =
                api::StringValue::create(builder, &api::StringValueArgs { value: Some(value) });
            (api::ValueKind::StringValue, value.as_union_value())
        }
    };
    api::Value::create(
        builder,
        &api::ValueArgs {
            kind_type,
            kind: Some(kind),
        },
    )
}

pub(super) fn comparison_op(op: ir::ComparisonOp) -> api::ComparisonOp {
    match op {
        ir::ComparisonOp::Eq => api::ComparisonOp::Eq,
        ir::ComparisonOp::Ne => api::ComparisonOp::Ne,
        ir::ComparisonOp::Lt => api::ComparisonOp::Lt,
        ir::ComparisonOp::Lte => api::ComparisonOp::Lte,
        ir::ComparisonOp::Gt => api::ComparisonOp::Gt,
        ir::ComparisonOp::Gte => api::ComparisonOp::Gte,
    }
}

pub(super) fn query_field_ref_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    path: &str,
) -> flatbuffers::WIPOffset<api::QueryFieldRef<'a>> {
    let path = builder.create_string(path);
    api::QueryFieldRef::create(
        builder,
        &api::QueryFieldRefArgs {
            path: Some(path),
            element_index: None,
        },
    )
}

pub(super) fn missing_policy_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    missing: ir::MissingPlacement,
) -> flatbuffers::WIPOffset<api::MissingPolicy<'a>> {
    let (policy_type, policy) = match missing {
        ir::MissingPlacement::First => {
            let policy = api::MissingFirst::create(builder, &api::MissingFirstArgs {});
            (
                api::MissingPolicyKind::MissingFirst,
                policy.as_union_value(),
            )
        }
        ir::MissingPlacement::Last => {
            let policy = api::MissingLast::create(builder, &api::MissingLastArgs {});
            (api::MissingPolicyKind::MissingLast, policy.as_union_value())
        }
        ir::MissingPlacement::As(value) => {
            let value = request_value_offset(builder, &value);
            let policy =
                api::MissingAs::create(builder, &api::MissingAsArgs { value: Some(value) });
            (api::MissingPolicyKind::MissingAs, policy.as_union_value())
        }
    };
    api::MissingPolicy::create(
        builder,
        &api::MissingPolicyArgs {
            policy_type,
            policy: Some(policy),
        },
    )
}

pub(super) fn sort_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    sort: &ir::SortIr,
) -> flatbuffers::WIPOffset<api::SortExpr<'a>> {
    let target = sort_target_offset(builder, &sort.target);
    let missing = missing_policy_offset(builder, sort.missing.clone());
    let annotations = request_value_entries_offset(builder, &sort.annotations);
    api::SortExpr::create(
        builder,
        &api::SortExprArgs {
            target: Some(target),
            direction: match sort.direction {
                ir::SortDirection::Asc => api::SortDirection::Asc,
                ir::SortDirection::Desc => api::SortDirection::Desc,
            },
            missing: Some(missing),
            annotations,
        },
    )
}

pub(super) fn resolved_sort_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    sort: &ir::SortIr,
) -> flatbuffers::WIPOffset<api::ResolvedSortExpr<'a>> {
    let target = resolved_sort_target_offset(builder, &sort.target);
    let missing = missing_policy_offset(builder, sort.missing.clone());
    let annotations = request_value_entries_offset(builder, &sort.annotations);
    api::ResolvedSortExpr::create(
        builder,
        &api::ResolvedSortExprArgs {
            target: Some(target),
            direction: match sort.direction {
                ir::SortDirection::Asc => api::SortDirection::Asc,
                ir::SortDirection::Desc => api::SortDirection::Desc,
            },
            missing: Some(missing),
            annotations,
        },
    )
}

pub(super) fn sort_target_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    target: &ir::SortTargetIr,
) -> flatbuffers::WIPOffset<api::SortTarget<'a>> {
    let (target_type, target) = match target {
        ir::SortTargetIr::Field(field) => {
            let field = builder.create_string(field.as_str());
            let target = api::FieldSortTarget::create(
                builder,
                &api::FieldSortTargetArgs { field: Some(field) },
            );
            (
                api::SortTargetKind::FieldSortTarget,
                target.as_union_value(),
            )
        }
        ir::SortTargetIr::Relevance => {
            let target =
                api::RelevanceSortTarget::create(builder, &api::RelevanceSortTargetArgs {});
            (
                api::SortTargetKind::RelevanceSortTarget,
                target.as_union_value(),
            )
        }
        ir::SortTargetIr::RankFeature(feature) => {
            let feature = builder.create_string(feature);
            let target = api::RankFeatureSortTarget::create(
                builder,
                &api::RankFeatureSortTargetArgs {
                    feature: Some(feature),
                },
            );
            (
                api::SortTargetKind::RankFeatureSortTarget,
                target.as_union_value(),
            )
        }
    };
    api::SortTarget::create(
        builder,
        &api::SortTargetArgs {
            target_type,
            target: Some(target),
        },
    )
}

pub(super) fn resolved_sort_target_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    target: &ir::SortTargetIr,
) -> flatbuffers::WIPOffset<api::ResolvedSortTarget<'a>> {
    let (target_type, target) = match target {
        ir::SortTargetIr::Field(field) => {
            let field = builder.create_string(field.as_str());
            let target = api::FieldSortTarget::create(
                builder,
                &api::FieldSortTargetArgs { field: Some(field) },
            );
            (
                api::ResolvedSortTargetKind::FieldSortTarget,
                target.as_union_value(),
            )
        }
        ir::SortTargetIr::Relevance => {
            let target =
                api::RelevanceSortTarget::create(builder, &api::RelevanceSortTargetArgs {});
            (
                api::ResolvedSortTargetKind::RelevanceSortTarget,
                target.as_union_value(),
            )
        }
        ir::SortTargetIr::RankFeature(feature) => {
            let feature = builder.create_string(feature);
            let target = api::RankFeatureSortTarget::create(
                builder,
                &api::RankFeatureSortTargetArgs {
                    feature: Some(feature),
                },
            );
            (
                api::ResolvedSortTargetKind::RankFeatureSortTarget,
                target.as_union_value(),
            )
        }
    };
    api::ResolvedSortTarget::create(
        builder,
        &api::ResolvedSortTargetArgs {
            target_type,
            target: Some(target),
        },
    )
}
