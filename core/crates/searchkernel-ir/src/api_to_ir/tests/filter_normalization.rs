use super::*;

#[test]
fn equivalent_filter_representations_resolve_to_one_owned_ir() {
    let structured = FilterInput::Comparison {
        field: "price".to_owned(),
        op: ComparisonOp::Lte,
        value: RequestValueIr::Int64(120),
    };
    let direct = resolve_search(
        &context(),
        filter_search_input(vec![structured], Vec::new()),
    )
    .expect("structured filter should resolve");

    let bytes = portable_search_with_textual_filter("price <= 120");
    let portable = crate::__flatbuffers::root::<api::SearchRequest>(&bytes)
        .expect("textual filter request should decode");
    let portable = context()
        .search_to_ir(portable)
        .expect("textual filter should resolve");

    assert_eq!(direct, portable);
    assert_eq!(
        crate::snapshot::search_request(&direct),
        crate::snapshot::search_request(&portable)
    );

    let round_trip_bytes = crate::wire::v1::search_request_ir_to_api(&portable);
    let round_trip = crate::__flatbuffers::root::<api::SearchRequest>(&round_trip_bytes)
        .expect("IR-to-API search request should decode");
    let round_trip = context()
        .search_to_ir(round_trip)
        .expect("API-to-IR round trip should resolve");
    assert_eq!(
        crate::snapshot::search_request(&portable),
        crate::snapshot::search_request(&round_trip)
    );
}

#[test]
fn repeated_filters_equal_one_nested_all_after_canonical_flattening() {
    let first = FilterInput::Comparison {
        field: "price".to_owned(),
        op: ComparisonOp::Gt,
        value: RequestValueIr::Int64(10),
    };
    let second = FilterInput::Comparison {
        field: "in_stock".to_owned(),
        op: ComparisonOp::Eq,
        value: RequestValueIr::Bool(true),
    };
    let repeated = resolve_search(
        &context(),
        filter_search_input(vec![first.clone(), second.clone()], Vec::new()),
    )
    .expect("repeated filters should resolve");
    let nested = resolve_search(
        &context(),
        filter_search_input(
            vec![FilterInput::All(vec![
                first,
                FilterInput::All(vec![second]),
            ])],
            Vec::new(),
        ),
    )
    .expect("nested all filter should resolve");

    assert_eq!(repeated, nested);
    assert_eq!(
        crate::snapshot::search_request(&repeated),
        crate::snapshot::search_request(&nested)
    );
}

#[test]
fn equivalent_invalid_filters_share_portable_error_identity() {
    let structured = resolve_search(
        &context(),
        filter_search_input(
            vec![FilterInput::Comparison {
                field: "unknown".to_owned(),
                op: ComparisonOp::Eq,
                value: RequestValueIr::Int64(1),
            }],
            Vec::new(),
        ),
    )
    .expect_err("unknown structured field must fail");

    let bytes = portable_search_with_textual_filter("unknown = 1");
    let request = crate::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let textual = context()
        .search_to_ir(request)
        .expect_err("unknown textual field must fail");
    assert_same_error(&structured, &textual);

    let mut non_filterable_context = context();
    non_filterable_context.definition.fields[0]
        .capabilities
        .filterable = false;
    let structured = resolve_search(
        &non_filterable_context,
        filter_search_input(
            vec![FilterInput::Comparison {
                field: "brand".to_owned(),
                op: ComparisonOp::Eq,
                value: RequestValueIr::String("acme".to_owned()),
            }],
            Vec::new(),
        ),
    )
    .expect_err("non-filterable structured field must fail");
    let bytes = portable_search_with_textual_filter("brand = 'acme'");
    let request = crate::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let textual = non_filterable_context
        .search_to_ir(request)
        .expect_err("non-filterable textual field must fail");
    assert_same_error(&structured, &textual);

    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let empty = api::FilterAnd::create(&mut builder, &api::FilterAndArgs { clauses: None });
    let empty = api::FilterExpr::create(
        &mut builder,
        &api::FilterExprArgs {
            expr_type: api::FilterExprKind::FilterAnd,
            expr: Some(empty.as_union_value()),
        },
    );
    let structured_filters = builder.create_vector(&[empty]);
    let source = match_all_source(&mut builder);
    let filters = api::FilterRequest::create(
        &mut builder,
        &api::FilterRequestArgs {
            structured: Some(structured_filters),
            ..Default::default()
        },
    );
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(source),
            filters: Some(filters),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let bytes = builder.finished_data().to_vec();
    let request = crate::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let wire = context()
        .search_to_ir(request)
        .expect_err("empty boolean filter must fail");
    let direct = resolve_search(
        &context(),
        filter_search_input(vec![FilterInput::All(Vec::new())], Vec::new()),
    )
    .expect_err("empty structured all must fail");
    assert_same_error(&direct, &wire);
}

#[test]
fn filter_comparison_invalid_operator_uses_filter_error_domain_across_ingress() {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let value = int_value(&mut builder, 10);
    let field = builder.create_string("price");
    let field = api::QueryFieldRef::create(
        &mut builder,
        &api::QueryFieldRefArgs {
            path: Some(field),
            element_index: None,
        },
    );
    let comparison = api::FilterComparison::create(
        &mut builder,
        &api::FilterComparisonArgs {
            field: Some(field),
            op: api::ComparisonOp(255),
            value: Some(value),
        },
    );
    let filter = api::FilterExpr::create(
        &mut builder,
        &api::FilterExprArgs {
            expr_type: api::FilterExprKind::FilterComparison,
            expr: Some(comparison.as_union_value()),
        },
    );
    let structured = builder.create_vector(&[filter]);
    let filters = api::FilterRequest::create(
        &mut builder,
        &api::FilterRequestArgs {
            structured: Some(structured),
            ..Default::default()
        },
    );
    let match_all = api::MatchAllQuery::create(&mut builder, &api::MatchAllQueryArgs {});
    let source = api::QuerySource::create(
        &mut builder,
        &api::QuerySourceArgs {
            source_type: api::QuerySourceKind::MatchAllQuery,
            source: Some(match_all.as_union_value()),
        },
    );
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(source),
            filters: Some(filters),
            ..Default::default()
        },
    );
    builder.finish(request, None);

    let request = crate::__flatbuffers::root::<api::SearchRequest>(builder.finished_data())
        .expect("valid malformed-comparison search request");
    let filter = request
        .filters()
        .and_then(|filters| filters.structured())
        .expect("structured filter is present")
        .get(0);
    let comparison = filter
        .expr_as_filter_comparison()
        .expect("comparison payload is present");
    let context = context();

    let component_error = context
        .filter_to_ir(filter)
        .expect_err("whole filter conversion must reject an unknown comparison operator");
    let leaf_error = context
        .comparison_to_ir(comparison)
        .expect_err("leaf comparison conversion must reject an unknown comparison operator");
    let search_error = context
        .search_to_ir(request)
        .expect_err("full search conversion must reject an unknown comparison operator");

    assert_eq!(component_error.details[0].code, "invalid_filter");
    assert_eq!(leaf_error.details[0].code, "invalid_filter");
    assert_eq!(search_error.details[0].code, "invalid_filter");
    assert_eq!(
        component_error.details[0].path.as_deref(),
        Some("filter.comparison.op")
    );
    assert_eq!(leaf_error.details[0].path.as_deref(), Some("comparison.op"));
    assert_eq!(
        search_error.details[0].path.as_deref(),
        Some("filters.structured[0].comparison.op")
    );
}

#[test]
fn positive_infinity_filter_lower_bound_is_not_treated_as_unbounded() {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let field = builder.create_string("price");
    let field = api::QueryFieldRef::create(
        &mut builder,
        &api::QueryFieldRefArgs {
            path: Some(field),
            element_index: None,
        },
    );
    let lower =
        api::PositiveInfinityBound::create(&mut builder, &api::PositiveInfinityBoundArgs {});
    let lower = api::RangeBound::create(
        &mut builder,
        &api::RangeBoundArgs {
            bound_type: api::RangeBoundKind::PositiveInfinityBound,
            bound: Some(lower.as_union_value()),
        },
    );
    let upper_value = api::Int64Value::create(&mut builder, &api::Int64ValueArgs { value: 10 });
    let upper_value = api::Value::create(
        &mut builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::Int64Value,
            kind: Some(upper_value.as_union_value()),
        },
    );
    let upper = api::FiniteRangeBound::create(
        &mut builder,
        &api::FiniteRangeBoundArgs {
            value: Some(upper_value),
        },
    );
    let upper = api::RangeBound::create(
        &mut builder,
        &api::RangeBoundArgs {
            bound_type: api::RangeBoundKind::FiniteRangeBound,
            bound: Some(upper.as_union_value()),
        },
    );
    let range = api::FilterRange::create(
        &mut builder,
        &api::FilterRangeArgs {
            field: Some(field),
            lower: Some(lower),
            upper: Some(upper),
            ..Default::default()
        },
    );
    let expression = api::FilterExpr::create(
        &mut builder,
        &api::FilterExprArgs {
            expr_type: api::FilterExprKind::FilterRange,
            expr: Some(range.as_union_value()),
        },
    );
    builder.finish(expression, None);
    let expression =
        crate::__flatbuffers::root::<api::FilterExpr>(builder.finished_data()).unwrap();

    assert!(
        context().filter_to_ir(expression).is_err(),
        "a lower +infinity bound must not collapse into an absent lower bound"
    );
}
