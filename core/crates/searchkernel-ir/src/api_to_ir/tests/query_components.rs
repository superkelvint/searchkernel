use super::*;

#[test]
fn component_query_resolution_does_not_simplify_typed_search_semantics() {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let value = builder.create_string("Acme");
    let string_value =
        api::StringValue::create(&mut builder, &api::StringValueArgs { value: Some(value) });
    let term_value = api::Value::create(
        &mut builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::StringValue,
            kind: Some(string_value.as_union_value()),
        },
    );
    let term = api::TermQuery::create(
        &mut builder,
        &api::TermQueryArgs {
            value: Some(term_value),
        },
    );
    let field = builder.create_string("brand");
    let field = api::QueryFieldRef::create(
        &mut builder,
        &api::QueryFieldRefArgs {
            path: Some(field),
            element_index: None,
        },
    );
    let item = api::QueryExpr::create(
        &mut builder,
        &api::QueryExprArgs {
            expr_type: api::QueryExprKind::TermQuery,
            expr: Some(term.as_union_value()),
            annotations: None,
        },
    );
    let contains = api::QueryContains::create(
        &mut builder,
        &api::QueryContainsArgs {
            field: Some(field),
            item: Some(item),
        },
    );
    let expression = api::QueryExpr::create(
        &mut builder,
        &api::QueryExprArgs {
            expr_type: api::QueryExprKind::QueryContains,
            expr: Some(contains.as_union_value()),
            annotations: None,
        },
    );
    builder.finish(expression, None);
    let expression_bytes = builder.finished_data().to_vec();
    let expression = crate::__flatbuffers::root::<api::QueryExpr>(&expression_bytes).unwrap();

    let mut context = context();
    context.definition.fields[0].capabilities.searchable = true;
    let direct = context
        .query_to_ir("", Some(expression))
        .expect("component query conversion should succeed");

    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let value = builder.create_string("Acme");
    let string_value =
        api::StringValue::create(&mut builder, &api::StringValueArgs { value: Some(value) });
    let term_value = api::Value::create(
        &mut builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::StringValue,
            kind: Some(string_value.as_union_value()),
        },
    );
    let term = api::TermQuery::create(
        &mut builder,
        &api::TermQueryArgs {
            value: Some(term_value),
        },
    );
    let field = builder.create_string("brand");
    let field = api::QueryFieldRef::create(
        &mut builder,
        &api::QueryFieldRefArgs {
            path: Some(field),
            element_index: None,
        },
    );
    let item = api::QueryExpr::create(
        &mut builder,
        &api::QueryExprArgs {
            expr_type: api::QueryExprKind::TermQuery,
            expr: Some(term.as_union_value()),
            annotations: None,
        },
    );
    let contains = api::QueryContains::create(
        &mut builder,
        &api::QueryContainsArgs {
            field: Some(field),
            item: Some(item),
        },
    );
    let expression = api::QueryExpr::create(
        &mut builder,
        &api::QueryExprArgs {
            expr_type: api::QueryExprKind::QueryContains,
            expr: Some(contains.as_union_value()),
            annotations: None,
        },
    );
    let typed = api::TypedQuery::create(
        &mut builder,
        &api::TypedQueryArgs {
            query: Some(expression),
        },
    );
    let source = api::QuerySource::create(
        &mut builder,
        &api::QuerySourceArgs {
            source_type: api::QuerySourceKind::TypedQuery,
            source: Some(typed.as_union_value()),
        },
    );
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(source),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let request = crate::__flatbuffers::root::<api::SearchRequest>(builder.finished_data())
        .expect("valid search request");
    let canonical = context.search_to_ir(request).unwrap();

    assert!(matches!(direct, QueryIr::Typed(_)));
    assert!(matches!(canonical.query(), QueryIr::Typed(_)));
}

#[test]
fn query_component_matches_full_search_for_contains_term() {
    fn contains_expression_bytes() -> Vec<u8> {
        let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
        let field_name = builder.create_string("brand");
        let field = api::QueryFieldRef::create(
            &mut builder,
            &api::QueryFieldRefArgs {
                path: Some(field_name),
                element_index: None,
            },
        );
        let value = builder.create_string("Acme");
        let value =
            api::StringValue::create(&mut builder, &api::StringValueArgs { value: Some(value) });
        let value = api::Value::create(
            &mut builder,
            &api::ValueArgs {
                kind_type: api::ValueKind::StringValue,
                kind: Some(value.as_union_value()),
            },
        );
        let term = api::TermQuery::create(&mut builder, &api::TermQueryArgs { value: Some(value) });
        let item = api::QueryExpr::create(
            &mut builder,
            &api::QueryExprArgs {
                expr_type: api::QueryExprKind::TermQuery,
                expr: Some(term.as_union_value()),
                annotations: None,
            },
        );
        let contains = api::QueryContains::create(
            &mut builder,
            &api::QueryContainsArgs {
                field: Some(field),
                item: Some(item),
            },
        );
        let expression = api::QueryExpr::create(
            &mut builder,
            &api::QueryExprArgs {
                expr_type: api::QueryExprKind::QueryContains,
                expr: Some(contains.as_union_value()),
                annotations: None,
            },
        );
        builder.finish(expression, None);
        builder.finished_data().to_vec()
    }

    fn search_request_bytes() -> Vec<u8> {
        let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
        let field_name = builder.create_string("brand");
        let field = api::QueryFieldRef::create(
            &mut builder,
            &api::QueryFieldRefArgs {
                path: Some(field_name),
                element_index: None,
            },
        );
        let value = builder.create_string("Acme");
        let value =
            api::StringValue::create(&mut builder, &api::StringValueArgs { value: Some(value) });
        let value = api::Value::create(
            &mut builder,
            &api::ValueArgs {
                kind_type: api::ValueKind::StringValue,
                kind: Some(value.as_union_value()),
            },
        );
        let term = api::TermQuery::create(&mut builder, &api::TermQueryArgs { value: Some(value) });
        let item = api::QueryExpr::create(
            &mut builder,
            &api::QueryExprArgs {
                expr_type: api::QueryExprKind::TermQuery,
                expr: Some(term.as_union_value()),
                annotations: None,
            },
        );
        let contains = api::QueryContains::create(
            &mut builder,
            &api::QueryContainsArgs {
                field: Some(field),
                item: Some(item),
            },
        );
        let expression = api::QueryExpr::create(
            &mut builder,
            &api::QueryExprArgs {
                expr_type: api::QueryExprKind::QueryContains,
                expr: Some(contains.as_union_value()),
                annotations: None,
            },
        );
        let typed = api::TypedQuery::create(
            &mut builder,
            &api::TypedQueryArgs {
                query: Some(expression),
            },
        );
        let source = api::QuerySource::create(
            &mut builder,
            &api::QuerySourceArgs {
                source_type: api::QuerySourceKind::TypedQuery,
                source: Some(typed.as_union_value()),
            },
        );
        let request = api::SearchRequest::create(
            &mut builder,
            &api::SearchRequestArgs {
                query: Some(source),
                ..Default::default()
            },
        );
        builder.finish(request, None);
        builder.finished_data().to_vec()
    }

    let mut query_context = context();
    query_context.definition.fields[0].capabilities.searchable = true;

    let expression_bytes = contains_expression_bytes();
    let expression = crate::__flatbuffers::root::<api::QueryExpr>(&expression_bytes).unwrap();
    let component = query_context
        .query_to_ir("", Some(expression))
        .expect("component query should resolve");

    let request_bytes = search_request_bytes();
    let request = crate::__flatbuffers::root::<api::SearchRequest>(&request_bytes).unwrap();
    let complete = query_context
        .search_to_ir(request)
        .expect("complete search should resolve");

    assert_eq!(component, complete.query().clone());
}
