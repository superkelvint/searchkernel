use searchkernel_ir::wire::v1::schema as api;
use searchkernel_ir::wire::v1::*;
use searchkernel_ir::*;
use std::collections::BTreeMap;

fn index_context() -> IndexContext {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let name = builder.create_string("title");
    let primitive = api::PrimitiveDataType::create(
        &mut builder,
        &api::PrimitiveDataTypeArgs {
            kind: api::PrimitiveDataTypeKind::String,
        },
    );
    let data_type = api::DataType::create(
        &mut builder,
        &api::DataTypeArgs {
            kind_type: api::DataTypeKind::PrimitiveDataType,
            kind: Some(primitive.as_union_value()),
        },
    );
    let stored = api::OptionalBool::new(true);
    let searchable = api::OptionalBool::new(true);
    let analyzer_name = builder.create_string("standard");
    let analyzer = api::Analyzer::create(
        &mut builder,
        &api::AnalyzerArgs {
            name: Some(analyzer_name),
            version: 1,
            ..Default::default()
        },
    );
    let field = api::FieldDefinition::create(
        &mut builder,
        &api::FieldDefinitionArgs {
            name: Some(name),
            data_type: Some(data_type),
            stored: Some(&stored),
            searchable: Some(&searchable),
            analyzer: Some(analyzer),
            ..Default::default()
        },
    );
    let brand_name = builder.create_string("brand");
    let filterable = api::OptionalBool::new(true);
    let sortable = api::OptionalBool::new(true);
    let matching = api::MatchingConfig::create(
        &mut builder,
        &api::MatchingConfigArgs {
            mode: api::MatchingMode::Exact,
            ..Default::default()
        },
    );
    let brand = api::FieldDefinition::create(
        &mut builder,
        &api::FieldDefinitionArgs {
            name: Some(brand_name),
            data_type: Some(data_type),
            stored: Some(&stored),
            filterable: Some(&filterable),
            sortable: Some(&sortable),
            matching: Some(matching),
            ..Default::default()
        },
    );
    let fields = builder.create_vector(&[field, brand]);
    let configured_profile_name = builder.create_string("configured-profile");
    let configured_profile = api::RankProfileDefinition::create(
        &mut builder,
        &api::RankProfileDefinitionArgs {
            name: Some(configured_profile_name),
            ..Default::default()
        },
    );
    let configured_profiles = builder.create_vector(&[configured_profile]);
    let title = builder.create_string("title");
    let weight = api::OptionalDouble::new(1.0);
    let search_field = api::SearchField::create(
        &mut builder,
        &api::SearchFieldArgs {
            field: Some(title),
            weight: Some(&weight),
        },
    );
    let search_fields = builder.create_vector(&[search_field]);
    let default_search_fields = api::SearchFieldSelection::create(
        &mut builder,
        &api::SearchFieldSelectionArgs {
            fields: Some(search_fields),
        },
    );
    let definition = api::IndexDefinition::create(
        &mut builder,
        &api::IndexDefinitionArgs {
            fields: Some(fields),
            default_search_fields: Some(default_search_fields),
            rank_profiles: Some(configured_profiles),
            ..Default::default()
        },
    );
    builder.finish(definition, None);
    let definition =
        searchkernel_ir::__flatbuffers::root::<api::IndexDefinition>(builder.finished_data())
            .expect("valid index definition");
    IndexContext::from_api(definition).expect("valid index context")
}

fn text_query<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    text: &str,
) -> flatbuffers::WIPOffset<api::QuerySource<'a>> {
    let text = builder.create_string(text);
    let query = api::SearchKernelTextQuery::create(
        builder,
        &api::SearchKernelTextQueryArgs { text: Some(text) },
    );
    api::QuerySource::create(
        builder,
        &api::QuerySourceArgs {
            source_type: api::QuerySourceKind::SearchKernelTextQuery,
            source: Some(query.as_union_value()),
        },
    )
}

fn search_request_for_source(
    source_kind: api::QuerySourceKind,
    with_search_fields: bool,
) -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let source = match source_kind {
        api::QuerySourceKind::SearchKernelTextQuery => text_query(&mut builder, "headphones"),
        api::QuerySourceKind::TypedQuery => {
            let value = api::QueryTrue::create(&mut builder, &api::QueryTrueArgs {});
            let expression = api::QueryExpr::create(
                &mut builder,
                &api::QueryExprArgs {
                    expr_type: api::QueryExprKind::QueryTrue,
                    expr: Some(value.as_union_value()),
                    annotations: None,
                },
            );
            let typed = api::TypedQuery::create(
                &mut builder,
                &api::TypedQueryArgs {
                    query: Some(expression),
                },
            );
            api::QuerySource::create(
                &mut builder,
                &api::QuerySourceArgs {
                    source_type: api::QuerySourceKind::TypedQuery,
                    source: Some(typed.as_union_value()),
                },
            )
        }
        api::QuerySourceKind::YqlQuery => {
            let source = builder.create_string("select * from sources * where true");
            let yql = api::YqlQuery::create(
                &mut builder,
                &api::YqlQueryArgs {
                    source: Some(source),
                    parameters: None,
                },
            );
            api::QuerySource::create(
                &mut builder,
                &api::QuerySourceArgs {
                    source_type: api::QuerySourceKind::YqlQuery,
                    source: Some(yql.as_union_value()),
                },
            )
        }
        api::QuerySourceKind::MatchAllQuery => {
            let match_all = api::MatchAllQuery::create(&mut builder, &api::MatchAllQueryArgs {});
            api::QuerySource::create(
                &mut builder,
                &api::QuerySourceArgs {
                    source_type: api::QuerySourceKind::MatchAllQuery,
                    source: Some(match_all.as_union_value()),
                },
            )
        }
        _ => unreachable!("unknown query source kind"),
    };
    let search_fields = if with_search_fields {
        let field = builder.create_string("title");
        let weight = api::OptionalDouble::new(1.0);
        let search_field = api::SearchField::create(
            &mut builder,
            &api::SearchFieldArgs {
                field: Some(field),
                weight: Some(&weight),
            },
        );
        let fields = builder.create_vector(&[search_field]);
        Some(api::SearchFieldSelection::create(
            &mut builder,
            &api::SearchFieldSelectionArgs {
                fields: Some(fields),
            },
        ))
    } else {
        None
    };
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(source),
            search_fields,
            ..Default::default()
        },
    );
    builder.finish(request, None);
    builder.finished_data().to_vec()
}

#[test]
fn search_fields_are_source_specific() {
    let context = index_context();
    let source_kinds = [
        api::QuerySourceKind::SearchKernelTextQuery,
        api::QuerySourceKind::TypedQuery,
        api::QuerySourceKind::YqlQuery,
        api::QuerySourceKind::MatchAllQuery,
    ];

    for source_kind in source_kinds {
        let bytes = search_request_for_source(source_kind, false);
        let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
        let converted = context
            .search_to_ir(request)
            .expect("source without search_fields should convert");
        if source_kind == api::QuerySourceKind::SearchKernelTextQuery {
            assert_eq!(converted.search_fields().len(), 1);
        } else {
            assert!(converted.search_fields().is_empty());
        }
    }

    for source_kind in source_kinds {
        let bytes = search_request_for_source(source_kind, true);
        let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
        let converted = context.search_to_ir(request);
        if source_kind == api::QuerySourceKind::SearchKernelTextQuery {
            assert!(converted.is_ok());
        } else {
            assert!(converted.is_err());
        }
    }
}

fn standalone_value<'a>(
    builder: &mut searchkernel_ir::__flatbuffers::FlatBufferBuilder<'a>,
    value: &str,
) -> flatbuffers::WIPOffset<api::Value<'a>> {
    let value = builder.create_string(value);
    let string = api::StringValue::create(builder, &api::StringValueArgs { value: Some(value) });
    api::Value::create(
        builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::StringValue,
            kind: Some(string.as_union_value()),
        },
    )
}

fn rust_search(query: QuerySourceInput) -> SearchInput {
    SearchInput {
        query,
        search_fields: None,
        filters: Vec::new(),
        textual_filters: Vec::new(),
        sorts: Vec::new(),
        grouping: Vec::new(),
        ranking: None,
        query_inputs: BTreeMap::new(),
        rank_properties: BTreeMap::new(),
        query_model: QueryModelInput::default(),
        projection: None,
        highlighting: None,
        presentation: PresentationInput::default(),
        properties: BTreeMap::new(),
        query_profile: None,
        search_chain: None,
        timeout: None,
        limit: None,
        offset: None,
    }
}

fn rust_typed_contains(field: &str, value: &str) -> QuerySourceInput {
    QuerySourceInput::Typed(QueryExprIr {
        kind: QueryExprKindIr::Contains {
            field: QueryFieldRefIr {
                path: FieldName::parse_query_path(field.to_owned()).unwrap(),
                element_index: None,
            },
            item: Box::new(QueryExprIr {
                kind: QueryExprKindIr::Term {
                    value: RequestValueIr::String(value.to_owned()),
                },
                annotations: BTreeMap::new(),
            }),
        },
        annotations: BTreeMap::new(),
    })
}

fn rust_typed_comparison() -> QuerySourceInput {
    QuerySourceInput::Typed(QueryExprIr {
        kind: QueryExprKindIr::Comparison {
            field: QueryFieldRefIr {
                path: FieldName::parse("title").unwrap(),
                element_index: None,
            },
            op: ComparisonOp::Gt,
            value: RequestValueIr::String("fox".to_owned()),
        },
        annotations: BTreeMap::new(),
    })
}

fn rust_typed_nearest_neighbor() -> QuerySourceInput {
    QuerySourceInput::Typed(QueryExprIr {
        kind: QueryExprKindIr::NearestNeighbor {
            document_field: QueryFieldRefIr {
                path: FieldName::parse("title").unwrap(),
                element_index: None,
            },
            query_tensor: "q".to_owned(),
        },
        annotations: BTreeMap::new(),
    })
}

fn typed_comparison_request() -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let field_name = builder.create_string("title");
    let field = api::QueryFieldRef::create(
        &mut builder,
        &api::QueryFieldRefArgs {
            path: Some(field_name),
            element_index: None,
        },
    );
    let value = standalone_value(&mut builder, "fox");
    let comparison = api::QueryComparison::create(
        &mut builder,
        &api::QueryComparisonArgs {
            field: Some(field),
            op: api::ComparisonOp::Gt,
            value: Some(value),
        },
    );
    let expression = api::QueryExpr::create(
        &mut builder,
        &api::QueryExprArgs {
            expr_type: api::QueryExprKind::QueryComparison,
            expr: Some(comparison.as_union_value()),
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

fn typed_nearest_neighbor_request(with_input: bool) -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let field_name = builder.create_string("title");
    let field = api::QueryFieldRef::create(
        &mut builder,
        &api::QueryFieldRefArgs {
            path: Some(field_name),
            element_index: None,
        },
    );
    let query_tensor = builder.create_string("q");
    let nearest = api::QueryNearestNeighbor::create(
        &mut builder,
        &api::QueryNearestNeighborArgs {
            document_field: Some(field),
            query_tensor: Some(query_tensor),
        },
    );
    let expression = api::QueryExpr::create(
        &mut builder,
        &api::QueryExprArgs {
            expr_type: api::QueryExprKind::QueryNearestNeighbor,
            expr: Some(nearest.as_union_value()),
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
    let inputs = if with_input {
        let name = builder.create_string("q");
        let value = standalone_value(&mut builder, "tensor");
        let literal = api::LiteralQueryInput::create(
            &mut builder,
            &api::LiteralQueryInputArgs { value: Some(value) },
        );
        let input = api::QueryInput::create(
            &mut builder,
            &api::QueryInputArgs {
                name: Some(name),
                input_type: api::QueryInputKind::LiteralQueryInput,
                input: Some(literal.as_union_value()),
            },
        );
        Some(builder.create_vector(&[input]))
    } else {
        None
    };
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(source),
            inputs,
            ..Default::default()
        },
    );
    builder.finish(request, None);
    builder.finished_data().to_vec()
}

fn text_request_with_profile(profile: &str) -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let query = text_query(&mut builder, "fox");
    let profile = builder.create_string(profile);
    let ranking = api::RankingRequest::create(
        &mut builder,
        &api::RankingRequestArgs {
            profile: Some(profile),
            ..Default::default()
        },
    );
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(query),
            ranking: Some(ranking),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    builder.finished_data().to_vec()
}

fn typed_contains_request(field_name: &str, value: &str) -> Vec<u8> {
    typed_contains_request_with_search_fields(field_name, value, false)
}

fn typed_contains_request_with_search_fields(
    field_name: &str,
    value: &str,
    with_search_fields: bool,
) -> Vec<u8> {
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let field_name = builder.create_string(field_name);
    let field = api::QueryFieldRef::create(
        &mut builder,
        &api::QueryFieldRefArgs {
            path: Some(field_name),
            element_index: None,
        },
    );
    let term_value = standalone_value(&mut builder, value);
    let term = api::TermQuery::create(
        &mut builder,
        &api::TermQueryArgs {
            value: Some(term_value),
        },
    );
    let term = api::QueryExpr::create(
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
            item: Some(term),
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
    let search_fields = if with_search_fields {
        let field = builder.create_string("title");
        let weight = api::OptionalDouble::new(1.0);
        let search_field = api::SearchField::create(
            &mut builder,
            &api::SearchFieldArgs {
                field: Some(field),
                weight: Some(&weight),
            },
        );
        let fields = builder.create_vector(&[search_field]);
        Some(api::SearchFieldSelection::create(
            &mut builder,
            &api::SearchFieldSelectionArgs {
                fields: Some(fields),
            },
        ))
    } else {
        None
    };
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(source),
            search_fields,
            ..Default::default()
        },
    );
    builder.finish(request, None);
    builder.finished_data().to_vec()
}

fn assert_ingress_equivalent(context: &IndexContext, rust: SearchInput, portable: &[u8]) {
    let rust_result = resolve_search(context, rust);
    let request = searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(portable).unwrap();
    let portable_result = context.search_to_ir(request);
    assert_eq!(rust_result, portable_result);
}

#[test]
fn rust_and_portable_ingress_normalize_empty_summary_identically() {
    let context = index_context();

    let mut rust = rust_search(QuerySourceInput::Text("fox".to_owned()));
    rust.presentation.summary = Some(String::new());

    let mut portable = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let source = text_query(&mut portable, "fox");
    let summary = portable.create_string("");
    let presentation = api::PresentationRequest::create(
        &mut portable,
        &api::PresentationRequestArgs {
            summary: Some(summary),
            ..Default::default()
        },
    );
    let request = api::SearchRequest::create(
        &mut portable,
        &api::SearchRequestArgs {
            query: Some(source),
            presentation: Some(presentation),
            ..Default::default()
        },
    );
    portable.finish(request, None);

    let rust_result = resolve_search(&context, rust);
    let request =
        searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(portable.finished_data())
            .unwrap();
    let portable_result = context.search_to_ir(request);

    assert_eq!(rust_result, portable_result);
    assert_eq!(
        rust_result
            .expect("empty summary should resolve successfully")
            .presentation()
            .summary,
        None
    );
}

#[test]
fn rust_and_portable_ingress_share_typed_query_validation() {
    let context = index_context();
    assert_ingress_equivalent(
        &context,
        rust_search(rust_typed_contains("title", "fox")),
        &typed_contains_request("title", "fox"),
    );
    assert_ingress_equivalent(
        &context,
        rust_search(rust_typed_contains("missing", "fox")),
        &typed_contains_request("missing", "fox"),
    );
    assert_ingress_equivalent(
        &context,
        rust_search(rust_typed_comparison()),
        &typed_comparison_request(),
    );

    let mut rust = rust_search(rust_typed_contains("title", "fox"));
    rust.search_fields = Some(vec![SearchFieldInput {
        field: "title".to_owned(),
        weight: None,
    }]);
    assert_ingress_equivalent(
        &context,
        rust,
        &typed_contains_request_with_search_fields("title", "fox", true),
    );
}

#[test]
fn rust_and_portable_ingress_share_ranking_and_query_input_validation() {
    let context = index_context();
    let mut valid = rust_search(QuerySourceInput::Text("fox".to_owned()));
    valid.ranking = Some(RankingInput::Named("configured-profile".to_owned()));
    assert_ingress_equivalent(
        &context,
        valid,
        &text_request_with_profile("configured-profile"),
    );

    let mut invalid = rust_search(QuerySourceInput::Text("fox".to_owned()));
    invalid.ranking = Some(RankingInput::Named("missing".to_owned()));
    assert_ingress_equivalent(&context, invalid, &text_request_with_profile("missing"));

    let mut nearest = rust_search(rust_typed_nearest_neighbor());
    nearest.query_inputs.insert(
        "q".to_owned(),
        QueryInputIr::Literal(RequestValueIr::String("tensor".to_owned())),
    );
    assert_ingress_equivalent(&context, nearest, &typed_nearest_neighbor_request(true));
    assert_ingress_equivalent(
        &context,
        rust_search(rust_typed_nearest_neighbor()),
        &typed_nearest_neighbor_request(false),
    );
}

#[test]
fn rust_and_portable_ingress_share_text_and_match_all_defaults() {
    let context = index_context();
    let mut text = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let source = text_query(&mut text, "fox");
    let request = api::SearchRequest::create(
        &mut text,
        &api::SearchRequestArgs {
            query: Some(source),
            ..Default::default()
        },
    );
    text.finish(request, None);
    assert_ingress_equivalent(
        &context,
        rust_search(QuerySourceInput::Text("fox".to_owned())),
        text.finished_data(),
    );

    let mut browse = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let source = api::MatchAllQuery::create(&mut browse, &api::MatchAllQueryArgs {});
    let source = api::QuerySource::create(
        &mut browse,
        &api::QuerySourceArgs {
            source_type: api::QuerySourceKind::MatchAllQuery,
            source: Some(source.as_union_value()),
        },
    );
    let request = api::SearchRequest::create(
        &mut browse,
        &api::SearchRequestArgs {
            query: Some(source),
            ..Default::default()
        },
    );
    browse.finish(request, None);
    assert_ingress_equivalent(
        &context,
        rust_search(QuerySourceInput::MatchAll),
        browse.finished_data(),
    );

    assert_ingress_equivalent(
        &context,
        rust_search(QuerySourceInput::Yql {
            source: "select * from sources * where true".to_owned(),
            parameters: BTreeMap::new(),
        }),
        &search_request_for_source(api::QuerySourceKind::YqlQuery, false),
    );

    let mut rust_yql = rust_search(QuerySourceInput::Yql {
        source: "select * from sources * where true".to_owned(),
        parameters: BTreeMap::new(),
    });
    rust_yql.search_fields = Some(vec![SearchFieldInput {
        field: "title".to_owned(),
        weight: None,
    }]);
    assert_ingress_equivalent(
        &context,
        rust_yql,
        &search_request_for_source(api::QuerySourceKind::YqlQuery, true),
    );
}

#[test]
fn v2_index_definition_and_text_search_convert_to_ir() {
    let context = index_context();
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let query = text_query(&mut builder, "headphones");
    let limit = api::OptionalUInt32::new(7);
    let offset = api::OptionalUInt32::new(2);
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(query),
            limit: Some(&limit),
            offset: Some(&offset),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let request =
        searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(builder.finished_data())
            .expect("valid search request");
    let converted = context.search_to_ir(request).expect("search converts");
    assert_eq!(converted.query(), &QueryIr::Text("headphones".to_owned()));
    assert_eq!(converted.limit(), 7);
    assert_eq!(converted.offset(), 2);
    assert_eq!(converted.search_fields().len(), 1);
    assert_eq!(context.describe().fields().len(), 2);
}

#[test]
fn v2_structured_filter_sort_and_ranking_convert_to_ir() {
    let context = index_context();
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let query = text_query(&mut builder, "headphones");
    let field_path = builder.create_string("brand");
    let field = api::QueryFieldRef::create(
        &mut builder,
        &api::QueryFieldRefArgs {
            path: Some(field_path),
            element_index: None,
        },
    );
    let value = standalone_value(&mut builder, "headphones");
    let comparison = api::FilterComparison::create(
        &mut builder,
        &api::FilterComparisonArgs {
            field: Some(field),
            op: api::ComparisonOp::Eq,
            value: Some(value),
        },
    );
    let expression = api::FilterExpr::create(
        &mut builder,
        &api::FilterExprArgs {
            expr_type: api::FilterExprKind::FilterComparison,
            expr: Some(comparison.as_union_value()),
        },
    );
    let filters = builder.create_vector(&[expression]);
    let filter_request = api::FilterRequest::create(
        &mut builder,
        &api::FilterRequestArgs {
            structured: Some(filters),
            ..Default::default()
        },
    );
    let field = builder.create_string("brand");
    let field_target = api::FieldSortTarget::create(
        &mut builder,
        &api::FieldSortTargetArgs { field: Some(field) },
    );
    let target = api::SortTarget::create(
        &mut builder,
        &api::SortTargetArgs {
            target_type: api::SortTargetKind::FieldSortTarget,
            target: Some(field_target.as_union_value()),
        },
    );
    let sort = api::SortExpr::create(
        &mut builder,
        &api::SortExprArgs {
            target: Some(target),
            direction: api::SortDirection::Desc,
            ..Default::default()
        },
    );
    let sorts = builder.create_vector(&[sort]);
    let profile = builder.create_string("bm25");
    let ranking = api::RankingRequest::create(
        &mut builder,
        &api::RankingRequestArgs {
            profile: Some(profile),
            ..Default::default()
        },
    );
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(query),
            filters: Some(filter_request),
            sort: Some(sorts),
            ranking: Some(ranking),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let request =
        searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(builder.finished_data())
            .expect("valid search request");
    let converted = context.search_to_ir(request).expect("search converts");
    assert!(converted.filter().is_some());
    assert_eq!(converted.sort().len(), 1);
    assert_eq!(converted.ranking_request().profile, "bm25");
}

#[test]
fn v2_request_controls_and_structured_grouping_are_preserved_in_ir() {
    let context = index_context();
    let mut builder = searchkernel_ir::__flatbuffers::FlatBufferBuilder::new();
    let source = builder.create_string("select * from sources * where true");
    let yql = api::YqlQuery::create(
        &mut builder,
        &api::YqlQueryArgs {
            source: Some(source),
            parameters: None,
        },
    );
    let query = api::QuerySource::create(
        &mut builder,
        &api::QuerySourceArgs {
            source_type: api::QuerySourceKind::YqlQuery,
            source: Some(yql.as_union_value()),
        },
    );
    let root = api::GroupingAllOperation::create(
        &mut builder,
        &api::GroupingAllOperationArgs {
            operations: None,
            label: None,
        },
    );
    let structured = api::StructuredGroupingSource::create(
        &mut builder,
        &api::StructuredGroupingSourceArgs { root: Some(root) },
    );
    let continuation = builder.create_string("next-page");
    let continuations = builder.create_vector(&[continuation]);
    let grouping = api::GroupingRequest::create(
        &mut builder,
        &api::GroupingRequestArgs {
            source_type: api::GroupingSourceKind::StructuredGroupingSource,
            source: Some(structured.as_union_value()),
            continuations: Some(continuations),
        },
    );
    let grouping = builder.create_vector(&[grouping]);
    let rank_key = builder.create_string("rank.foo");
    let rank_value = builder.create_string("bar");
    let rank_property = api::StringEntry::create(
        &mut builder,
        &api::StringEntryArgs {
            key: Some(rank_key),
            value: Some(rank_value),
        },
    );
    let rank_properties = builder.create_vector(&[rank_property]);
    let profile = builder.create_string("default");
    let ranking = api::RankingRequest::create(
        &mut builder,
        &api::RankingRequestArgs {
            profile: Some(profile),
            properties: Some(rank_properties),
        },
    );
    let input_name = builder.create_string("query(vec)");
    let expression = builder.create_string("embed(model,@q)");
    let input = api::ExpressionQueryInput::create(
        &mut builder,
        &api::ExpressionQueryInputArgs {
            expression: Some(expression),
        },
    );
    let input = api::QueryInput::create(
        &mut builder,
        &api::QueryInputArgs {
            name: Some(input_name),
            input_type: api::QueryInputKind::ExpressionQueryInput,
            input: Some(input.as_union_value()),
        },
    );
    let inputs = builder.create_vector(&[input]);
    let language = builder.create_string("en-US");
    let default_index = builder.create_string("title");
    let query_model = api::QueryModel::create(
        &mut builder,
        &api::QueryModelArgs {
            language: Some(language),
            default_index: Some(default_index),
            ..Default::default()
        },
    );
    let summary = builder.create_string("short");
    let presentation = api::PresentationRequest::create(
        &mut builder,
        &api::PresentationRequestArgs {
            summary: Some(summary),
            ..Default::default()
        },
    );
    let property_key = builder.create_string("trace.level");
    let property_value = standalone_value(&mut builder, "3");
    let property = api::ValueEntry::create(
        &mut builder,
        &api::ValueEntryArgs {
            key: Some(property_key),
            value: Some(property_value),
        },
    );
    let properties = builder.create_vector(&[property]);
    let query_profile = builder.create_string("profile-a");
    let search_chain = builder.create_string("chain-a");
    let limit = api::OptionalUInt32::new(3);
    let offset = api::OptionalUInt32::new(4);
    let timeout = api::OptionalUInt64::new(50);
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(query),
            grouping: Some(grouping),
            ranking: Some(ranking),
            inputs: Some(inputs),
            limit: Some(&limit),
            offset: Some(&offset),
            timeout_ms: Some(&timeout),
            query_model: Some(query_model),
            presentation: Some(presentation),
            properties: Some(properties),
            query_profile: Some(query_profile),
            search_chain: Some(search_chain),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let request =
        searchkernel_ir::__flatbuffers::root::<api::SearchRequest>(builder.finished_data())
            .expect("valid search request");

    let converted = context.search_to_ir(request).expect("all controls resolve");
    assert!(matches!(converted.query(), QueryIr::Yql { .. }));
    assert_eq!(converted.grouping().len(), 1);
    assert_eq!(converted.grouping()[0].continuations, ["next-page"]);
    assert_eq!(converted.ranking_request().profile, "default");
    assert!(
        matches!(converted.inputs().get("query(vec)"), Some(QueryInputIr::Expression(value)) if value == "embed(model,@q)")
    );
    assert_eq!(
        converted.query_model().default_index.as_deref(),
        Some("title")
    );
    assert_eq!(converted.presentation().summary.as_deref(), Some("short"));
    assert!(
        matches!(converted.properties().get("trace.level"), Some(RequestValueIr::String(value)) if value == "3")
    );
    assert_eq!(converted.query_profile(), Some("profile-a"));
    assert_eq!(converted.search_chain(), Some("chain-a"));
    assert_eq!(converted.limit(), 3);
    assert_eq!(converted.offset(), 4);
    assert_eq!(
        converted.timeout(),
        Some(std::time::Duration::from_millis(50))
    );
}

#[test]
fn v2_search_result_encoding_uses_result_tree_and_relevance() {
    let hit = Hit {
        id: "doc-1".to_owned(),
        score: Some(f64::INFINITY),
        fields: BTreeMap::from([
            ("title".to_owned(), Value::String("First".to_owned())),
            ("price".to_owned(), Value::Float64(12.5)),
        ]),
        highlights: vec![HighlightResult {
            field: "title".to_owned(),
            fragments: vec![HighlightFragment {
                text: "<em>First</em>".to_owned(),
                matches: vec![HighlightSpan {
                    start_byte: 0,
                    end_byte: 5,
                }],
            }],
        }],
        source: Some("catalog".to_owned()),
        types: vec!["product".to_owned(), "featured".to_owned()],
        match_features: BTreeMap::from([("bm25(title)".to_owned(), Value::Float64(4.25))]),
        summary_features: BTreeMap::from([("summary_score".to_owned(), Value::Int64(7))]),
        rank_features: BTreeMap::from([("rank_score".to_owned(), Value::Float64(9.5))]),
        sort_values: vec![Value::UInt64(10), Value::String("First".to_owned())],
    };
    let nested_hit = Hit {
        id: "doc-2".to_owned(),
        score: Some(f64::NEG_INFINITY),
        ..Default::default()
    };
    let nested_group = ResultGroup {
        id: Some("nested".to_owned()),
        label: Some("Nested group".to_owned()),
        value: Some(Value::String("child".to_owned())),
        fields: BTreeMap::from([("count".to_owned(), Value::UInt64(1))]),
        children: vec![ResultNode::Hit(nested_hit)],
        continuations: BTreeMap::from([("next".to_owned(), "token-2".to_owned())]),
        source: Some("group-source".to_owned()),
        types: vec!["bucket".to_owned()],
        score: Some(2.5),
        limits: Some(ResultLimits {
            from: Some(Value::Int64(1)),
            to: Some(Value::Int64(10)),
        }),
        metadata: BTreeMap::from([("owner".to_owned(), Value::Bool(true))]),
    };
    let result = SearchResult {
        root: ResultNode::Group(ResultGroup {
            id: Some("root".to_owned()),
            label: Some("Root".to_owned()),
            value: Some(Value::Null),
            fields: BTreeMap::from([("total".to_owned(), Value::UInt64(2))]),
            children: vec![ResultNode::Hit(hit), ResultNode::Group(nested_group)],
            continuations: BTreeMap::from([("page".to_owned(), "token-1".to_owned())]),
            source: Some("search".to_owned()),
            types: vec!["root".to_owned()],
            score: Some(3.5),
            limits: Some(ResultLimits {
                from: Some(Value::UInt64(0)),
                to: Some(Value::UInt64(20)),
            }),
            metadata: BTreeMap::from([("trace_id".to_owned(), Value::String("r-1".to_owned()))]),
        }),
        total_hits: TotalHits {
            value: 42,
            relation: TotalHitsRelation::Approximate,
        },
        coverage: Some(Coverage {
            coverage_percent: Some(98.5),
            documents: Some(100),
            full: Some(false),
            nodes: Some(3),
            results: Some(42),
            results_full: Some(40),
            degraded: Some(CoverageDegraded {
                match_phase: Some(true),
                timeout: Some(false),
                adaptive_timeout: Some(true),
                ann_timeout: Some(false),
                non_ideal_state: Some(true),
                metadata: BTreeMap::from([("reason".to_owned(), Value::String("load".to_owned()))]),
            }),
            metadata: BTreeMap::from([("region".to_owned(), Value::String("west".to_owned()))]),
        }),
        timing: Some(SearchTiming {
            query_time_seconds: Some(0.01),
            search_time_seconds: Some(0.02),
            summary_fetch_time_seconds: Some(0.03),
            metadata: BTreeMap::from([("phase".to_owned(), Value::String("done".to_owned()))]),
        }),
        errors: vec![SearchResultError {
            code: 503,
            summary: Some("partial".to_owned()),
            message: Some("one node timed out".to_owned()),
            source: Some("node-2".to_owned()),
            stack_trace: Some("trace".to_owned()),
            transient: Some(true),
            metadata: BTreeMap::from([("node".to_owned(), Value::String("2".to_owned()))]),
        }],
        trace: Some(Value::Object(BTreeMap::from([(
            "level".to_owned(),
            Value::UInt64(3),
        )]))),
        metadata: BTreeMap::from([("request_id".to_owned(), Value::String("req-1".to_owned()))]),
    };
    let bytes = search_result_to_api(&result).expect("valid search result");
    let encoded = searchkernel_ir::__flatbuffers::root::<api::SearchResult>(&bytes)
        .expect("valid search result");
    let root = encoded.root().expect("result root");
    let group = root.node_as_result_group().expect("result group root");
    assert_eq!(group.id(), Some("root"));
    assert_eq!(group.label(), Some("Root"));
    assert_eq!(
        group
            .value()
            .unwrap()
            .kind_as_null_value_value()
            .unwrap()
            .value(),
        api::NullValue::Null
    );
    assert_eq!(group.fields().unwrap().len(), 1);
    assert_eq!(group.fields().unwrap().get(0).key(), "total");
    assert_eq!(
        group
            .fields()
            .unwrap()
            .get(0)
            .value()
            .unwrap()
            .kind_as_uint_64_value()
            .unwrap()
            .value(),
        2
    );
    assert_eq!(group.source(), Some("search"));
    assert_eq!(group.types().unwrap().iter().collect::<Vec<_>>(), ["root"]);
    assert_eq!(group.continuations().unwrap().get(0).key(), "page");
    assert_eq!(
        group.continuations().unwrap().get(0).value(),
        Some("token-1")
    );
    let limits = group.limits().expect("root limits");
    assert_eq!(
        limits
            .from()
            .unwrap()
            .kind_as_uint_64_value()
            .unwrap()
            .value(),
        0
    );
    assert_eq!(
        limits
            .to()
            .unwrap()
            .kind_as_uint_64_value()
            .unwrap()
            .value(),
        20
    );
    assert_eq!(
        group
            .relevance()
            .unwrap()
            .value_as_finite_relevance()
            .unwrap()
            .value(),
        3.5
    );
    assert_eq!(group.metadata().unwrap().len(), 1);
    assert_eq!(group.metadata().unwrap().get(0).key(), "trace_id");
    assert_eq!(
        group
            .metadata()
            .unwrap()
            .get(0)
            .value()
            .unwrap()
            .kind_as_string_value()
            .unwrap()
            .value(),
        Some("r-1")
    );
    let hit = group
        .children()
        .expect("result children")
        .get(0)
        .node_as_hit()
        .expect("hit node");
    assert_eq!(hit.id(), Some("doc-1"));
    assert_eq!(
        hit.relevance().map(|relevance| relevance.value_type()),
        Some(api::RelevanceKind::PositiveInfinityRelevance)
    );
    assert_eq!(hit.source(), Some("catalog"));
    assert_eq!(
        hit.types().unwrap().iter().collect::<Vec<_>>(),
        ["product", "featured"]
    );
    assert_eq!(hit.fields().unwrap().len(), 2);
    assert_eq!(hit.fields().unwrap().get(0).key(), "price");
    assert_eq!(
        hit.fields()
            .unwrap()
            .get(0)
            .value()
            .unwrap()
            .kind_as_float_64_value()
            .unwrap()
            .value(),
        12.5
    );
    assert_eq!(hit.fields().unwrap().get(1).key(), "title");
    assert_eq!(
        hit.fields()
            .unwrap()
            .get(1)
            .value()
            .unwrap()
            .kind_as_string_value()
            .unwrap()
            .value(),
        Some("First")
    );
    let highlight = hit.highlights().unwrap().get(0);
    assert_eq!(highlight.field(), Some("title"));
    assert_eq!(highlight.fragments().unwrap().len(), 1);
    let fragment = highlight.fragments().unwrap().get(0);
    assert_eq!(fragment.text(), Some("<em>First</em>"));
    assert_eq!(fragment.matches().unwrap().len(), 1);
    assert_eq!(fragment.matches().unwrap().get(0).start_byte(), 0);
    assert_eq!(fragment.matches().unwrap().get(0).end_byte(), 5);
    assert_eq!(hit.match_features().unwrap().get(0).key(), "bm25(title)");
    assert_eq!(
        hit.match_features()
            .unwrap()
            .get(0)
            .value()
            .unwrap()
            .kind_as_float_64_value()
            .unwrap()
            .value(),
        4.25
    );
    assert_eq!(
        hit.summary_features().unwrap().get(0).key(),
        "summary_score"
    );
    assert_eq!(
        hit.summary_features()
            .unwrap()
            .get(0)
            .value()
            .unwrap()
            .kind_as_int_64_value()
            .unwrap()
            .value(),
        7
    );
    assert_eq!(hit.rank_features().unwrap().get(0).key(), "rank_score");
    assert_eq!(
        hit.rank_features()
            .unwrap()
            .get(0)
            .value()
            .unwrap()
            .kind_as_float_64_value()
            .unwrap()
            .value(),
        9.5
    );
    assert_eq!(hit.sort_values().unwrap().len(), 2);
    assert_eq!(
        hit.sort_values()
            .unwrap()
            .get(0)
            .kind_as_uint_64_value()
            .unwrap()
            .value(),
        10
    );
    assert_eq!(
        hit.sort_values()
            .unwrap()
            .get(1)
            .kind_as_string_value()
            .unwrap()
            .value(),
        Some("First")
    );
    let nested = group
        .children()
        .unwrap()
        .get(1)
        .node_as_result_group()
        .expect("nested group");
    assert_eq!(
        nested
            .children()
            .unwrap()
            .get(0)
            .node_as_hit()
            .unwrap()
            .id(),
        Some("doc-2")
    );
    assert_eq!(
        nested
            .children()
            .unwrap()
            .get(0)
            .node_as_hit()
            .unwrap()
            .relevance()
            .unwrap()
            .value_type(),
        api::RelevanceKind::NegativeInfinityRelevance
    );
    assert_eq!(nested.id(), Some("nested"));
    assert_eq!(nested.label(), Some("Nested group"));
    assert_eq!(
        nested
            .value()
            .unwrap()
            .kind_as_string_value()
            .unwrap()
            .value(),
        Some("child")
    );
    assert_eq!(nested.fields().unwrap().len(), 1);
    assert_eq!(nested.fields().unwrap().get(0).key(), "count");
    assert_eq!(
        nested
            .fields()
            .unwrap()
            .get(0)
            .value()
            .unwrap()
            .kind_as_uint_64_value()
            .unwrap()
            .value(),
        1
    );
    assert_eq!(nested.continuations().unwrap().len(), 1);
    assert_eq!(nested.continuations().unwrap().get(0).key(), "next");
    assert_eq!(
        nested.continuations().unwrap().get(0).value(),
        Some("token-2")
    );
    assert_eq!(nested.source(), Some("group-source"));
    assert_eq!(
        nested.types().unwrap().iter().collect::<Vec<_>>(),
        ["bucket"]
    );
    assert_eq!(
        nested
            .relevance()
            .unwrap()
            .value_as_finite_relevance()
            .unwrap()
            .value(),
        2.5
    );
    let nested_limits = nested.limits().unwrap();
    assert_eq!(
        nested_limits
            .from()
            .unwrap()
            .kind_as_int_64_value()
            .unwrap()
            .value(),
        1
    );
    assert_eq!(
        nested_limits
            .to()
            .unwrap()
            .kind_as_int_64_value()
            .unwrap()
            .value(),
        10
    );
    assert_eq!(nested.metadata().unwrap().len(), 1);
    assert_eq!(nested.metadata().unwrap().get(0).key(), "owner");
    assert!(nested
        .metadata()
        .unwrap()
        .get(0)
        .value()
        .unwrap()
        .kind_as_bool_value()
        .unwrap()
        .value());
    assert_eq!(encoded.total_hits().unwrap().value(), 42);
    assert_eq!(
        encoded.total_hits().unwrap().relation(),
        api::TotalHitsRelation::Approximate
    );
    assert!(!encoded
        .coverage()
        .unwrap()
        .degraded()
        .unwrap()
        .timeout()
        .unwrap()
        .value());
    assert_eq!(
        encoded
            .coverage()
            .unwrap()
            .coverage_percent()
            .unwrap()
            .value(),
        98.5
    );
    let coverage = encoded.coverage().unwrap();
    assert_eq!(coverage.documents().unwrap().value(), 100);
    assert!(!coverage.full().unwrap().value());
    assert_eq!(coverage.nodes().unwrap().value(), 3);
    assert_eq!(coverage.results().unwrap().value(), 42);
    assert_eq!(coverage.results_full().unwrap().value(), 40);
    let degraded = coverage.degraded().unwrap();
    assert!(degraded.match_phase().unwrap().value());
    assert!(!degraded.timeout().unwrap().value());
    assert!(degraded.adaptive_timeout().unwrap().value());
    assert!(!degraded.ann_timeout().unwrap().value());
    assert!(degraded.non_ideal_state().unwrap().value());
    assert_eq!(degraded.metadata().unwrap().get(0).key(), "reason");
    assert_eq!(
        degraded
            .metadata()
            .unwrap()
            .get(0)
            .value()
            .unwrap()
            .kind_as_string_value()
            .unwrap()
            .value(),
        Some("load")
    );
    assert_eq!(coverage.metadata().unwrap().get(0).key(), "region");
    assert_eq!(
        coverage
            .metadata()
            .unwrap()
            .get(0)
            .value()
            .unwrap()
            .kind_as_string_value()
            .unwrap()
            .value(),
        Some("west")
    );
    assert_eq!(
        encoded
            .timing()
            .unwrap()
            .search_time_seconds()
            .unwrap()
            .value(),
        0.02
    );
    let timing = encoded.timing().unwrap();
    assert_eq!(timing.query_time_seconds().unwrap().value(), 0.01);
    assert_eq!(timing.summary_fetch_time_seconds().unwrap().value(), 0.03);
    assert_eq!(timing.metadata().unwrap().get(0).key(), "phase");
    assert_eq!(
        timing
            .metadata()
            .unwrap()
            .get(0)
            .value()
            .unwrap()
            .kind_as_string_value()
            .unwrap()
            .value(),
        Some("done")
    );
    assert_eq!(encoded.errors().unwrap().get(0).code(), 503);
    let error = encoded.errors().unwrap().get(0);
    assert_eq!(error.summary(), Some("partial"));
    assert_eq!(error.message(), Some("one node timed out"));
    assert_eq!(error.source(), Some("node-2"));
    assert_eq!(error.stack_trace(), Some("trace"));
    assert!(error.transient().unwrap().value());
    assert_eq!(error.metadata().unwrap().get(0).key(), "node");
    assert_eq!(
        error
            .metadata()
            .unwrap()
            .get(0)
            .value()
            .unwrap()
            .kind_as_string_value()
            .unwrap()
            .value(),
        Some("2")
    );
    let trace = encoded.trace().unwrap().kind_as_value_object().unwrap();
    assert_eq!(trace.fields().unwrap().len(), 1);
    assert_eq!(trace.fields().unwrap().get(0).key(), "level");
    assert_eq!(
        trace
            .fields()
            .unwrap()
            .get(0)
            .value()
            .unwrap()
            .kind_as_uint_64_value()
            .unwrap()
            .value(),
        3
    );
    let metadata = encoded.metadata().unwrap();
    assert_eq!(metadata.len(), 1);
    assert_eq!(metadata.get(0).key(), "request_id");
    assert_eq!(
        metadata
            .get(0)
            .value()
            .unwrap()
            .kind_as_string_value()
            .unwrap()
            .value(),
        Some("req-1")
    );
}

#[test]
fn v2_result_direct_hit_and_nonfinite_value_variants_are_encoded() {
    let result = SearchResult {
        root: ResultNode::Hit(Hit {
            id: "direct-hit".to_owned(),
            fields: BTreeMap::from([
                ("nan".to_owned(), Value::Float64(f64::NAN)),
                ("infinity".to_owned(), Value::Float64(f64::INFINITY)),
                (
                    "negative_infinity".to_owned(),
                    Value::Float64(f64::NEG_INFINITY),
                ),
                ("bytes".to_owned(), Value::Bytes(vec![1, 2, 255])),
                (
                    "list".to_owned(),
                    Value::List(vec![Value::Bool(true), Value::String("nested".to_owned())]),
                ),
                (
                    "object".to_owned(),
                    Value::Object(BTreeMap::from([("answer".to_owned(), Value::Int64(42))])),
                ),
                (
                    "tensor".to_owned(),
                    Value::Tensor(RequestTensorIr {
                        type_spec: "tensor(x[2])".to_owned(),
                        payload: RequestTensorPayloadIr::Dense(vec![1.0, 2.0]),
                    }),
                ),
            ]),
            ..Default::default()
        }),
        total_hits: TotalHits {
            value: 0,
            relation: TotalHitsRelation::Unspecified,
        },
        coverage: None,
        timing: None,
        errors: Vec::new(),
        trace: None,
        metadata: BTreeMap::new(),
    };

    let bytes = search_result_to_api(&result).expect("valid direct-hit result");
    let encoded = searchkernel_ir::__flatbuffers::root::<api::SearchResult>(&bytes)
        .expect("valid direct-hit result");
    assert_eq!(
        encoded.total_hits().unwrap().relation(),
        api::TotalHitsRelation::Unspecified
    );
    let hit = encoded
        .root()
        .unwrap()
        .node_as_hit()
        .expect("direct hit root");
    assert_eq!(hit.id(), Some("direct-hit"));
    let fields = hit.fields().expect("hit fields");
    assert_eq!(fields.len(), 7);
    assert_eq!(
        fields
            .get(0)
            .value()
            .unwrap()
            .kind_as_bytes_value()
            .unwrap()
            .value()
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        [1, 2, 255].into_iter().collect::<Vec<_>>()
    );
    assert_eq!(
        fields
            .get(1)
            .value()
            .unwrap()
            .kind_as_non_finite_float_value()
            .unwrap()
            .kind(),
        api::NonFiniteFloatKind::PositiveInfinity
    );
    assert!(fields
        .get(2)
        .value()
        .unwrap()
        .kind_as_value_list()
        .is_some());
    assert_eq!(
        fields
            .get(3)
            .value()
            .unwrap()
            .kind_as_non_finite_float_value()
            .unwrap()
            .kind(),
        api::NonFiniteFloatKind::NaN
    );
    assert_eq!(
        fields
            .get(4)
            .value()
            .unwrap()
            .kind_as_non_finite_float_value()
            .unwrap()
            .kind(),
        api::NonFiniteFloatKind::NegativeInfinity
    );
    assert!(fields
        .get(5)
        .value()
        .unwrap()
        .kind_as_value_object()
        .is_some());
    assert_eq!(
        fields
            .get(6)
            .value()
            .unwrap()
            .kind_as_tensor_value()
            .unwrap()
            .type_spec(),
        Some("tensor(x[2])")
    );
}

#[test]
fn v2_result_validation_rejects_nonportable_doubles_and_tensor_cells_without_panicking() {
    fn assert_error(result: SearchResult, detail_code: &str, path: &str) {
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            search_result_to_api(&result)
        }));
        let error = outcome
            .expect("invalid result encoding must return an error, not panic")
            .expect_err("invalid result must be rejected");
        assert_eq!(error.code, ErrorCode::InvalidArgument);
        assert_eq!(error.details[0].code, detail_code);
        assert_eq!(error.details[0].path.as_deref(), Some(path));
    }

    let mut relevance = SearchResult::empty(TotalHits {
        value: 0,
        relation: TotalHitsRelation::Exact,
    });
    relevance.root = ResultNode::Hit(Hit {
        score: Some(f64::NAN),
        ..Default::default()
    });
    assert_error(relevance, "non_finite_relevance", "root.relevance");

    let mut tensor = SearchResult::empty(TotalHits {
        value: 0,
        relation: TotalHitsRelation::Exact,
    });
    tensor.root = ResultNode::Hit(Hit {
        fields: BTreeMap::from([(
            "embedding".to_owned(),
            Value::Tensor(RequestTensorIr {
                type_spec: "tensor(x[2])".to_owned(),
                payload: RequestTensorPayloadIr::Dense(vec![1.0, f64::NAN]),
            }),
        )]),
        ..Default::default()
    });
    assert_error(
        tensor,
        "non_finite_tensor_cell",
        "root.fields.embedding.payload",
    );

    let mut coverage = SearchResult::empty(TotalHits {
        value: 0,
        relation: TotalHitsRelation::Exact,
    });
    coverage.coverage = Some(Coverage {
        coverage_percent: Some(f64::INFINITY),
        documents: None,
        full: None,
        nodes: None,
        results: None,
        results_full: None,
        degraded: None,
        metadata: BTreeMap::new(),
    });
    assert_error(coverage, "non_finite_number", "coverage.coverage_percent");

    let mut timing = SearchResult::empty(TotalHits {
        value: 0,
        relation: TotalHitsRelation::Exact,
    });
    timing.timing = Some(SearchTiming {
        query_time_seconds: None,
        search_time_seconds: Some(f64::NAN),
        summary_fetch_time_seconds: None,
        metadata: BTreeMap::new(),
    });
    assert_error(timing, "non_finite_number", "timing.search_time_seconds");
}

#[test]
fn v2_error_encoding_remains_portable() {
    let error = PortableError::new(ErrorCode::InvalidArgument, "bad request");
    let bytes = error_to_api(&error);
    let encoded = searchkernel_ir::__flatbuffers::root::<api::Error>(&bytes).expect("valid error");
    assert_eq!(encoded.code(), api::ErrorCode::InvalidArgument);
    assert_eq!(encoded.message(), Some("bad request"));
}
