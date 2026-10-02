use super::*;

#[test]
fn component_sort_resolution_uses_search_error_semantics() {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let field = builder.create_string("price");
    let field = api::FieldSortTarget::create(
        &mut builder,
        &api::FieldSortTargetArgs { field: Some(field) },
    );
    let target = api::SortTarget::create(
        &mut builder,
        &api::SortTargetArgs {
            target_type: api::SortTargetKind::FieldSortTarget,
            target: Some(field.as_union_value()),
        },
    );
    let sort = api::SortExpr::create(
        &mut builder,
        &api::SortExprArgs {
            target: Some(target),
            direction: api::SortDirection(3),
            ..Default::default()
        },
    );
    builder.finish(sort, None);
    let sort_bytes = builder.finished_data().to_vec();
    let sort = crate::__flatbuffers::root::<api::SortExpr>(&sort_bytes).unwrap();
    let direct_error = context()
        .sort_to_ir(sort)
        .expect_err("component sort conversion should reject the invalid direction");
    assert_eq!(direct_error.details[0].code, "invalid_sort");

    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let match_all = api::MatchAllQuery::create(&mut builder, &api::MatchAllQueryArgs {});
    let source = api::QuerySource::create(
        &mut builder,
        &api::QuerySourceArgs {
            source_type: api::QuerySourceKind::MatchAllQuery,
            source: Some(match_all.as_union_value()),
        },
    );
    let field = builder.create_string("price");
    let field = api::FieldSortTarget::create(
        &mut builder,
        &api::FieldSortTargetArgs { field: Some(field) },
    );
    let target = api::SortTarget::create(
        &mut builder,
        &api::SortTargetArgs {
            target_type: api::SortTargetKind::FieldSortTarget,
            target: Some(field.as_union_value()),
        },
    );
    let sort = api::SortExpr::create(
        &mut builder,
        &api::SortExprArgs {
            target: Some(target),
            direction: api::SortDirection(3),
            ..Default::default()
        },
    );
    let sorts = builder.create_vector(&[sort]);
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(source),
            sort: Some(sorts),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let request = crate::__flatbuffers::root::<api::SearchRequest>(builder.finished_data())
        .expect("valid search request");
    let full_error = context()
        .search_to_ir(request)
        .expect_err("full search conversion should reject the invalid direction");
    assert_eq!(full_error.details[0].code, "invalid_sort");
}

#[test]
fn sort_component_uses_the_same_error_semantics_as_full_search() {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();

    let match_all = api::MatchAllQuery::create(&mut builder, &api::MatchAllQueryArgs {});
    let query = api::QuerySource::create(
        &mut builder,
        &api::QuerySourceArgs {
            source_type: api::QuerySourceKind::MatchAllQuery,
            source: Some(match_all.as_union_value()),
        },
    );

    let field = builder.create_string("price");
    let field = api::FieldSortTarget::create(
        &mut builder,
        &api::FieldSortTargetArgs { field: Some(field) },
    );
    let target = api::SortTarget::create(
        &mut builder,
        &api::SortTargetArgs {
            target_type: api::SortTargetKind::FieldSortTarget,
            target: Some(field.as_union_value()),
        },
    );
    let sort = api::SortExpr::create(
        &mut builder,
        &api::SortExprArgs {
            target: Some(target),
            direction: api::SortDirection(99),
            ..Default::default()
        },
    );
    let sorts = builder.create_vector(&[sort]);
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(query),
            sort: Some(sorts),
            ..Default::default()
        },
    );
    builder.finish(request, None);

    let request =
        crate::__flatbuffers::root::<api::SearchRequest>(builder.finished_data()).unwrap();
    let sort = request.sort().unwrap().get(0);
    let context = context();

    let component_error = context
        .sort_to_ir(sort)
        .expect_err("invalid sort direction must fail component conversion");
    let search_error = context
        .search_to_ir(request)
        .expect_err("invalid sort direction must fail full search conversion");

    assert_eq!(
        component_error.details[0].code,
        search_error.details[0].code
    );
    assert_eq!(component_error.details[0].code, "invalid_sort");
}

#[test]
fn sort_missing_as_is_not_rejected_or_dropped() {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let field = builder.create_string("price");
    let field = api::FieldSortTarget::create(
        &mut builder,
        &api::FieldSortTargetArgs { field: Some(field) },
    );
    let target = api::SortTarget::create(
        &mut builder,
        &api::SortTargetArgs {
            target_type: api::SortTargetKind::FieldSortTarget,
            target: Some(field.as_union_value()),
        },
    );
    let value = api::Int64Value::create(&mut builder, &api::Int64ValueArgs { value: -1 });
    let value = api::Value::create(
        &mut builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::Int64Value,
            kind: Some(value.as_union_value()),
        },
    );
    let missing = api::MissingAs::create(&mut builder, &api::MissingAsArgs { value: Some(value) });
    let missing = api::MissingPolicy::create(
        &mut builder,
        &api::MissingPolicyArgs {
            policy_type: api::MissingPolicyKind::MissingAs,
            policy: Some(missing.as_union_value()),
        },
    );
    let key = builder.create_string("locale");
    let annotation_value = builder.create_string("en-US");
    let annotation_value = api::StringValue::create(
        &mut builder,
        &api::StringValueArgs {
            value: Some(annotation_value),
        },
    );
    let annotation_value = api::Value::create(
        &mut builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::StringValue,
            kind: Some(annotation_value.as_union_value()),
        },
    );
    let annotation = api::ValueEntry::create(
        &mut builder,
        &api::ValueEntryArgs {
            key: Some(key),
            value: Some(annotation_value),
        },
    );
    let annotations = builder.create_vector(&[annotation]);
    let sort = api::SortExpr::create(
        &mut builder,
        &api::SortExprArgs {
            target: Some(target),
            direction: api::SortDirection::Asc,
            missing: Some(missing),
            annotations: Some(annotations),
        },
    );
    builder.finish(sort, None);
    let sort = crate::__flatbuffers::root::<api::SortExpr>(builder.finished_data()).unwrap();

    let mut query_context = context();
    query_context.definition.fields[1].capabilities.sortable = true;
    let converted = query_context
        .sort_to_ir(sort)
        .expect("MissingAs and sort annotations must have an IR representation");
    assert_eq!(
        converted.missing(),
        MissingPlacement::As(RequestValueIr::Int64(-1))
    );
    assert_eq!(
        converted.annotations().get("locale"),
        Some(&RequestValueIr::String("en-US".to_owned()))
    );

    let request = SearchRequestIr::new(
        QueryIr::MatchAll,
        Vec::new(),
        None,
        vec![converted],
        10,
        0,
        ProjectionIr::new(Vec::new()),
        HighlightingIr::Disabled,
        None,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    );
    let bytes = crate::wire::v1::search_request_ir_to_api(&request);
    let encoded = crate::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let encoded_sort = encoded.sort().unwrap().get(0);
    assert_eq!(
        encoded_sort
            .missing()
            .and_then(|value| value.policy_as_missing_as())
            .and_then(|value| value.value())
            .and_then(|value| value.kind_as_int_64_value())
            .map(|value| value.value()),
        Some(-1)
    );
    assert_eq!(encoded_sort.annotations().unwrap().get(0).key(), "locale");
}
