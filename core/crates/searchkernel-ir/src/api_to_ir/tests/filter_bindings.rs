use super::*;

#[test]
fn request_level_textual_and_is_flattened() {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let first = textual_clause(&mut builder, "price > 10 AND price < 100");
    let second = textual_clause(&mut builder, "in_stock = true");
    let textual = builder.create_vector(&[first, second]);
    let request = api::FilterRequest::create(
        &mut builder,
        &api::FilterRequestArgs {
            textual: Some(textual),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let request = crate::__flatbuffers::root::<api::FilterRequest>(builder.finished_data())
        .expect("valid filter request");

    assert_flattened_and(
        context()
            .filters_to_ir(Some(request))
            .expect("textual filters resolve")
            .expect("filter is present"),
        3,
    );
}

#[test]
fn request_level_structured_and_textual_filters_are_flattened() {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let lower = int_value(&mut builder, 10);
    let upper = int_value(&mut builder, 100);
    let lower = structured_comparison(&mut builder, "price", api::ComparisonOp::Gt, lower);
    let upper = structured_comparison(&mut builder, "price", api::ComparisonOp::Lt, upper);
    let structured_and = builder.create_vector(&[lower, upper]);
    let structured = api::FilterAnd::create(
        &mut builder,
        &api::FilterAndArgs {
            clauses: Some(structured_and),
        },
    );
    let structured = api::FilterExpr::create(
        &mut builder,
        &api::FilterExprArgs {
            expr_type: api::FilterExprKind::FilterAnd,
            expr: Some(structured.as_union_value()),
        },
    );
    let structured = builder.create_vector(&[structured]);
    let textual_clause = textual_clause(&mut builder, "in_stock = true");
    let textual = builder.create_vector(&[textual_clause]);
    let request = api::FilterRequest::create(
        &mut builder,
        &api::FilterRequestArgs {
            structured: Some(structured),
            textual: Some(textual),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let request = crate::__flatbuffers::root::<api::FilterRequest>(builder.finished_data())
        .expect("valid filter request");

    assert_flattened_and(
        context()
            .filters_to_ir(Some(request))
            .expect("structured and textual filters resolve")
            .expect("filter is present"),
        3,
    );
}

#[test]
fn request_level_multiple_structured_and_textual_clauses_are_flattened() {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let lower = int_value(&mut builder, 10);
    let in_stock = bool_value(&mut builder, true);
    let lower = structured_comparison(&mut builder, "price", api::ComparisonOp::Gt, lower);
    let in_stock = structured_comparison(&mut builder, "in_stock", api::ComparisonOp::Eq, in_stock);
    let structured = builder.create_vector(&[lower, in_stock]);
    let price_clause = textual_clause(&mut builder, "price < 100");
    let stock_clause = textual_clause(&mut builder, "in_stock = true");
    let textual = builder.create_vector(&[price_clause, stock_clause]);
    let request = api::FilterRequest::create(
        &mut builder,
        &api::FilterRequestArgs {
            structured: Some(structured),
            textual: Some(textual),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let request = crate::__flatbuffers::root::<api::FilterRequest>(builder.finished_data())
        .expect("valid filter request");

    assert_flattened_and(
        context()
            .filters_to_ir(Some(request))
            .expect("multiple filters resolve")
            .expect("filter is present"),
        4,
    );
}

#[test]
fn textual_bindings_are_local_to_each_clause() {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();

    let price_key = builder.create_string("value");
    let price_value = api::Int64Value::create(&mut builder, &api::Int64ValueArgs { value: 120 });
    let price_value = api::Value::create(
        &mut builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::Int64Value,
            kind: Some(price_value.as_union_value()),
        },
    );
    let price_binding = api::ValueEntry::create(
        &mut builder,
        &api::ValueEntryArgs {
            key: Some(price_key),
            value: Some(price_value),
        },
    );
    let price_bindings = builder.create_vector(&[price_binding]);
    let price_source = builder.create_string("price <= :value");
    let price_clause = api::FilterClause::create(
        &mut builder,
        &api::FilterClauseArgs {
            source: Some(price_source),
            bindings: Some(price_bindings),
        },
    );

    let rating_key = builder.create_string("value");
    let rating_value = api::Int64Value::create(&mut builder, &api::Int64ValueArgs { value: 4 });
    let rating_value = api::Value::create(
        &mut builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::Int64Value,
            kind: Some(rating_value.as_union_value()),
        },
    );
    let rating_binding = api::ValueEntry::create(
        &mut builder,
        &api::ValueEntryArgs {
            key: Some(rating_key),
            value: Some(rating_value),
        },
    );
    let rating_bindings = builder.create_vector(&[rating_binding]);
    let rating_source = builder.create_string("rating >= :value");
    let rating_clause = api::FilterClause::create(
        &mut builder,
        &api::FilterClauseArgs {
            source: Some(rating_source),
            bindings: Some(rating_bindings),
        },
    );

    let textual = builder.create_vector(&[price_clause, rating_clause]);
    let request = api::FilterRequest::create(
        &mut builder,
        &api::FilterRequestArgs {
            textual: Some(textual),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let request = crate::__flatbuffers::root::<api::FilterRequest>(builder.finished_data())
        .expect("valid filter request");

    let filter = context()
        .filters_to_ir(Some(request))
        .expect("clause-local bindings resolve independently")
        .expect("textual filters produce a filter");
    assert!(matches!(filter, FilterIr::And(clauses) if clauses.len() == 2));
}

#[test]
fn binding_from_one_clause_cannot_satisfy_another_clause() {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let key = builder.create_string("value");
    let value = api::Int64Value::create(&mut builder, &api::Int64ValueArgs { value: 120 });
    let value = api::Value::create(
        &mut builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::Int64Value,
            kind: Some(value.as_union_value()),
        },
    );
    let binding = api::ValueEntry::create(
        &mut builder,
        &api::ValueEntryArgs {
            key: Some(key),
            value: Some(value),
        },
    );
    let bindings = builder.create_vector(&[binding]);
    let first_source = builder.create_string("price <= :value");
    let first = api::FilterClause::create(
        &mut builder,
        &api::FilterClauseArgs {
            source: Some(first_source),
            bindings: Some(bindings),
        },
    );
    let second_source = builder.create_string("rating >= :value");
    let second = api::FilterClause::create(
        &mut builder,
        &api::FilterClauseArgs {
            source: Some(second_source),
            ..Default::default()
        },
    );
    let textual = builder.create_vector(&[first, second]);
    let request = api::FilterRequest::create(
        &mut builder,
        &api::FilterRequestArgs {
            textual: Some(textual),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let request = crate::__flatbuffers::root::<api::FilterRequest>(builder.finished_data())
        .expect("valid filter request");

    let error = context().filters_to_ir(Some(request)).unwrap_err();
    assert_eq!(error.details[0].code, "unknown_parameter");
    assert_eq!(
        error.details[0].metadata.get("clause_index"),
        Some(&"1".to_owned())
    );
    assert_eq!(
        error.details[0].path.as_deref(),
        Some("filters.textual[1].source")
    );
}

#[test]
fn filter_bindings_are_bounded_per_clause() {
    fn entries(
        builder: &mut crate::__flatbuffers::FlatBufferBuilder<'static>,
        count: usize,
    ) -> crate::__flatbuffers::WIPOffset<
        crate::__flatbuffers::Vector<
            'static,
            crate::__flatbuffers::ForwardsUOffset<api::ValueEntry<'static>>,
        >,
    > {
        let value = int_value(builder, 1);
        let values = (0..count)
            .map(|index| {
                let key = builder.create_string(&format!("binding_{index}"));
                api::ValueEntry::create(
                    builder,
                    &api::ValueEntryArgs {
                        key: Some(key),
                        value: Some(value),
                    },
                )
            })
            .collect::<Vec<_>>();
        builder.create_vector(&values)
    }

    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let at_limit = entries(&mut builder, MAX_FILTER_BINDINGS);
    let clause = api::FilterClause::create(
        &mut builder,
        &api::FilterClauseArgs {
            bindings: Some(at_limit),
            ..Default::default()
        },
    );
    let textual = builder.create_vector(&[clause]);
    let request = api::FilterRequest::create(
        &mut builder,
        &api::FilterRequestArgs {
            textual: Some(textual),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let request = crate::__flatbuffers::root::<api::FilterRequest>(builder.finished_data())
        .expect("valid filter request");
    let bindings = super::filter::filter_bindings(
        request
            .textual()
            .and_then(|values| values.get(0).bindings()),
        "filters.textual[0].bindings",
    )
    .expect("the binding limit itself is valid");
    assert_eq!(bindings.len(), MAX_FILTER_BINDINGS);

    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let over_limit = entries(&mut builder, MAX_FILTER_BINDINGS + 1);
    let clause = api::FilterClause::create(
        &mut builder,
        &api::FilterClauseArgs {
            bindings: Some(over_limit),
            ..Default::default()
        },
    );
    let textual = builder.create_vector(&[clause]);
    let request = api::FilterRequest::create(
        &mut builder,
        &api::FilterRequestArgs {
            textual: Some(textual),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let request = crate::__flatbuffers::root::<api::FilterRequest>(builder.finished_data())
        .expect("valid filter request");
    let error = super::filter::filter_bindings(
        request
            .textual()
            .and_then(|values| values.get(0).bindings()),
        "filters.textual[0].bindings",
    )
    .expect_err("one binding over the limit must fail");
    assert_eq!(error.details[0].code, "expression_too_complex");
    assert_eq!(
        error.details[0].path.as_deref(),
        Some("filters.textual[0].bindings")
    );
}

#[test]
fn duplicate_bindings_in_one_clause_are_rejected() {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let key_a = builder.create_string("value");
    let key_b = builder.create_string("value");
    let value_a = api::Int64Value::create(&mut builder, &api::Int64ValueArgs { value: 120 });
    let value_b = api::Int64Value::create(&mut builder, &api::Int64ValueArgs { value: 121 });
    let value_a = api::Value::create(
        &mut builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::Int64Value,
            kind: Some(value_a.as_union_value()),
        },
    );
    let value_b = api::Value::create(
        &mut builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::Int64Value,
            kind: Some(value_b.as_union_value()),
        },
    );
    let binding_a = api::ValueEntry::create(
        &mut builder,
        &api::ValueEntryArgs {
            key: Some(key_a),
            value: Some(value_a),
        },
    );
    let binding_b = api::ValueEntry::create(
        &mut builder,
        &api::ValueEntryArgs {
            key: Some(key_b),
            value: Some(value_b),
        },
    );
    let bindings = builder.create_vector(&[binding_a, binding_b]);
    let source = builder.create_string("price <= :value");
    let clause = api::FilterClause::create(
        &mut builder,
        &api::FilterClauseArgs {
            source: Some(source),
            bindings: Some(bindings),
        },
    );
    let textual = builder.create_vector(&[clause]);
    let request = api::FilterRequest::create(
        &mut builder,
        &api::FilterRequestArgs {
            textual: Some(textual),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let request = crate::__flatbuffers::root::<api::FilterRequest>(builder.finished_data())
        .expect("valid filter request");

    let error = context().filters_to_ir(Some(request)).unwrap_err();
    assert_eq!(error.details[0].code, "duplicate_parameter");
}
