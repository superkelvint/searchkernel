use super::*;

#[test]
fn component_filter_resolution_matches_nested_search_error_path() {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let empty_clauses =
        builder.create_vector::<crate::__flatbuffers::WIPOffset<api::FilterExpr>>(&[]);
    let empty_or = api::FilterOr::create(
        &mut builder,
        &api::FilterOrArgs {
            clauses: Some(empty_clauses),
        },
    );
    let nested_or = api::FilterExpr::create(
        &mut builder,
        &api::FilterExprArgs {
            expr_type: api::FilterExprKind::FilterOr,
            expr: Some(empty_or.as_union_value()),
        },
    );
    let outer_clauses = builder.create_vector(&[nested_or]);
    let outer_and = api::FilterAnd::create(
        &mut builder,
        &api::FilterAndArgs {
            clauses: Some(outer_clauses),
        },
    );
    let filter = api::FilterExpr::create(
        &mut builder,
        &api::FilterExprArgs {
            expr_type: api::FilterExprKind::FilterAnd,
            expr: Some(outer_and.as_union_value()),
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
        .expect("valid nested filter search request");
    let filter = request
        .filters()
        .and_then(|filters| filters.structured())
        .expect("structured filter is present")
        .get(0);
    let context = context();
    let component_error = context
        .filter_to_ir(filter)
        .expect_err("component conversion should reject the empty nested OR");
    let search_error = context
        .search_to_ir(request)
        .expect_err("full search conversion should reject the empty nested OR");

    let relative_path = |path: Option<&str>| {
        path.map(|path| {
            path.strip_prefix("filters.structured[0]")
                .or_else(|| path.strip_prefix("filter"))
                .unwrap_or(path)
                .to_owned()
        })
    };
    assert_eq!(
        component_error.details[0].code,
        search_error.details[0].code
    );
    assert_eq!(
        relative_path(component_error.details[0].path.as_deref()),
        relative_path(search_error.details[0].path.as_deref()),
        "component and full-search filter paths must describe the same nested clause"
    );
}

#[test]
fn component_filter_resolution_matches_full_search_ir() {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let lower = int_value(&mut builder, 10);
    let upper = int_value(&mut builder, 100);
    let lower = structured_comparison(&mut builder, "price", api::ComparisonOp::Gt, lower);
    let upper = structured_comparison(&mut builder, "price", api::ComparisonOp::Lt, upper);
    let clauses = builder.create_vector(&[lower, upper]);
    let all = api::FilterAnd::create(
        &mut builder,
        &api::FilterAndArgs {
            clauses: Some(clauses),
        },
    );
    let filter = api::FilterExpr::create(
        &mut builder,
        &api::FilterExprArgs {
            expr_type: api::FilterExprKind::FilterAnd,
            expr: Some(all.as_union_value()),
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
        .expect("valid nested filter search request");
    let filter = request
        .filters()
        .and_then(|filters| filters.structured())
        .expect("structured filter is present")
        .get(0);
    let context = context();
    let component = context
        .filter_to_ir(filter)
        .expect("component conversion should resolve the nested AND");
    let search = context
        .search_to_ir(request)
        .expect("full search conversion should resolve the nested AND");
    assert_eq!(search.filter(), Some(&component));
}
