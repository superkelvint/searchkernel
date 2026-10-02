use super::*;

#[test]
fn rust_resolvers_match_flatbuffer_adapters_at_the_ir_boundary() {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string("doc-7");
    let condition = builder.create_string("tenant = 'acme'");
    let delete = api::DeleteRequest::create(
        &mut builder,
        &api::DeleteRequestArgs {
            id: Some(id),
            condition: Some(condition),
        },
    );
    builder.finish(delete, None);
    let delete = crate::__flatbuffers::root::<api::DeleteRequest>(builder.finished_data())
        .expect("valid delete request");
    assert_eq!(
        resolve_delete("doc-7", Some("tenant = 'acme'")).unwrap(),
        context().delete_to_ir(delete).unwrap()
    );

    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string("doc-7");
    let get = api::GetRequest::create(
        &mut builder,
        &api::GetRequestArgs {
            id: Some(id),
            projection: None,
        },
    );
    builder.finish(get, None);
    let get = crate::__flatbuffers::root::<api::GetRequest>(builder.finished_data())
        .expect("valid get request");
    assert_eq!(
        resolve_get(
            &context(),
            GetInput {
                id: "doc-7".to_owned(),
                projection: None,
            },
        )
        .unwrap(),
        context().get_to_ir(get).unwrap()
    );

    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let name = builder.create_string("standard");
    let analyzer = api::Analyzer::create(
        &mut builder,
        &api::AnalyzerArgs {
            name: Some(name),
            version: 1,
            options: None,
        },
    );
    let text = builder.create_string("The quick brown fox");
    let analyze = api::AnalyzeRequest::create(
        &mut builder,
        &api::AnalyzeRequestArgs {
            target_type: api::AnalyzeTarget::Analyzer,
            target: Some(analyzer.as_union_value()),
            text: Some(text),
            language: None,
        },
    );
    builder.finish(analyze, None);
    let analyze = crate::__flatbuffers::root::<api::AnalyzeRequest>(builder.finished_data())
        .expect("valid analyze request");
    assert_eq!(
        resolve_analyze(
            &context(),
            AnalyzeInput {
                target: Some(AnalyzeTargetInput::Analyzer(AnalyzerInput {
                    name: "standard".to_owned(),
                    version: 1,
                    options: BTreeMap::new(),
                })),
                text: "The quick brown fox".to_owned(),
                language: None,
            },
        )
        .unwrap(),
        context().analyze_to_ir(analyze).unwrap()
    );

    // Schema parity is the foundation for every subsequent resolver: the
    // Rust facade and the portable schema adapter must produce the same
    // capabilities, matching configuration, and default search fields.
    let schema = parity_schema();
    let rust_definition = resolve_rust_index_definition(schema).unwrap();
    let schema_bytes = crate::wire::v1::index_definition_ir_to_api(&rust_definition);
    let schema = crate::__flatbuffers::root::<api::IndexDefinition>(&schema_bytes)
        .expect("valid index definition");
    assert_eq!(
        rust_definition,
        index_definition_to_ir(schema).expect("schema resolves through FlatBuffers")
    );
    let flat_context = IndexContext::from_api(schema).expect("schema context resolves");
    let rust_context = IndexContext::from_definition_ir(rust_definition);

    // Document values are deliberately mixed: scalar values exercise the
    // field-kind checks while the list exercises document cardinality through
    // the portable Value/ValueList path.
    let document_input = parity_document_input();
    let rust_insert = resolve_insert(&rust_context, document_input.clone()).unwrap();
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let document = parity_document(&mut builder);
    let insert = api::InsertRequest::create(
        &mut builder,
        &api::InsertRequestArgs {
            document: Some(document),
        },
    );
    builder.finish(insert, None);
    let insert = crate::__flatbuffers::root::<api::InsertRequest>(builder.finished_data())
        .expect("valid insert request");
    assert_eq!(
        rust_insert,
        flat_context
            .insert_to_ir(insert)
            .expect("insert resolves through FlatBuffers")
    );

    let rust_upsert = resolve_upsert(
        &rust_context,
        document_input.clone(),
        Some("tenant = 'acme'"),
    )
    .unwrap();
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let document = parity_document(&mut builder);
    let condition = builder.create_string("tenant = 'acme'");
    let upsert = api::UpsertRequest::create(
        &mut builder,
        &api::UpsertRequestArgs {
            document: Some(document),
            condition: Some(condition),
        },
    );
    builder.finish(upsert, None);
    let upsert = crate::__flatbuffers::root::<api::UpsertRequest>(builder.finished_data())
        .expect("valid upsert request");
    assert_eq!(
        rust_upsert,
        flat_context
            .upsert_to_ir(upsert)
            .expect("upsert resolves through FlatBuffers")
    );

    let patch_operations = vec![
        PatchOperationInput::Set {
            field: "price".to_owned(),
            value: RequestValueIr::Int64(99),
        },
        PatchOperationInput::Unset {
            field: "tags".to_owned(),
        },
    ];
    let rust_patch = resolve_patch(&rust_context, "doc-7", &patch_operations).unwrap();
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let set_field = builder.create_string("price");
    let set_value = int_value(&mut builder, 99);
    let set_field = api::SetField::create(
        &mut builder,
        &api::SetFieldArgs {
            field: Some(set_field),
            value: Some(set_value),
        },
    );
    let set_operation = api::PatchOperation::create(
        &mut builder,
        &api::PatchOperationArgs {
            operation_type: api::PatchOperationKind::SetField,
            operation: Some(set_field.as_union_value()),
        },
    );
    let unset_field = builder.create_string("tags");
    let unset_field = api::UnsetField::create(
        &mut builder,
        &api::UnsetFieldArgs {
            field: Some(unset_field),
        },
    );
    let unset_operation = api::PatchOperation::create(
        &mut builder,
        &api::PatchOperationArgs {
            operation_type: api::PatchOperationKind::UnsetField,
            operation: Some(unset_field.as_union_value()),
        },
    );
    let operations = builder.create_vector(&[set_operation, unset_operation]);
    let id = builder.create_string("doc-7");
    let patch = api::PatchRequest::create(
        &mut builder,
        &api::PatchRequestArgs {
            id: Some(id),
            operations: Some(operations),
        },
    );
    builder.finish(patch, None);
    let patch = crate::__flatbuffers::root::<api::PatchRequest>(builder.finished_data())
        .expect("valid patch request");
    assert_eq!(
        rust_patch,
        flat_context
            .patch_to_ir(patch)
            .expect("patch resolves through FlatBuffers")
    );

    // Exercise the complete high-level search surface in one request. The
    // canonical IR encoder supplies the equivalent portable request, which is
    // then resolved by the same FlatBuffer adapter used by transport callers.
    let rust_search = resolve_search(
        &rust_context,
        SearchIntent {
            query: QuerySourceInput::Text("coffee".to_owned()),
            search_fields: Some(vec![SearchFieldInput {
                field: "title".to_owned(),
                weight: Some(2.5),
            }]),
            filters: vec![
                FilterInput::Comparison {
                    field: "price".to_owned(),
                    op: ComparisonOp::Gte,
                    value: RequestValueIr::Int64(10),
                },
                FilterInput::Range {
                    field: "rating".to_owned(),
                    lower: FilterBoundInput::Finite(RequestValueIr::Float64(2.0)),
                    upper: FilterBoundInput::Finite(RequestValueIr::Float64(5.0)),
                    include_lower: Some(false),
                    include_upper: Some(true),
                },
            ],
            textual_filters: Vec::new(),
            sorts: vec![SortInput {
                target: SortTargetInput::Field("price".to_owned()),
                direction: SortDirection::Desc,
                missing: Some(MissingPlacement::First),
                annotations: BTreeMap::new(),
            }],
            grouping: vec![GroupingInput::Facet(FacetInput {
                field: "brand".to_owned(),
                name: None,
                limit: Some(7),
                sort: Some(FacetSortInput::ValueAsc),
            })],
            ranking: Some(RankingInput::Bm25),
            query_inputs: BTreeMap::new(),
            rank_properties: BTreeMap::new(),
            query_model: QueryModelInput {
                language: Some("en-US".to_owned()),
                ..Default::default()
            },
            projection: Some(vec![
                "title".to_owned(),
                "brand".to_owned(),
                "price".to_owned(),
            ]),
            highlighting: Some(vec!["title".to_owned()]),
            presentation: PresentationInput::default(),
            properties: BTreeMap::new(),
            query_profile: None,
            search_chain: None,
            timeout: Some(Duration::from_millis(250)),
            limit: Some(7),
            offset: Some(3),
        },
    )
    .unwrap();
    let search_bytes = crate::wire::v1::search_request_ir_to_api(&rust_search);
    let search = crate::__flatbuffers::root::<api::SearchRequest>(&search_bytes)
        .expect("valid search request");
    assert_eq!(
        rust_search,
        flat_context
            .search_to_ir(search)
            .expect("search resolves through FlatBuffers")
    );
}

#[test]
fn search_resolver_rejects_explicit_and_implicit_root_output_name_collisions() {
    let explicit = root_output_grouping(Some("count"), GroupingAggregationIr::Count);
    let implicit = root_output_grouping(None, GroupingAggregationIr::Count);

    let error = resolve_search(
        &context(),
        match_all_search(vec![
            GroupingInput::Structured(explicit),
            GroupingInput::Structured(implicit),
        ]),
    )
    .expect_err("explicit and implicit root names must share one namespace");

    assert_eq!(error.details[0].code, "grouping_output_name_collision");
    assert_eq!(
        error.details[0].path.as_deref(),
        Some("grouping[1].root.operations[0].outputs[0]")
    );
}

#[test]
fn search_resolver_rejects_complex_explicit_and_implicit_canonical_name_collision() {
    let explicit =
        root_output_grouping(Some("sum(Literal(Int64(1)))"), GroupingAggregationIr::Count);
    let implicit = root_output_grouping(
        None,
        GroupingAggregationIr::Sum(GroupingExpressionIr::Literal(RequestValueIr::Int64(1))),
    );

    let error = resolve_search(
        &context(),
        match_all_search(vec![
            GroupingInput::Structured(explicit),
            GroupingInput::Structured(implicit),
        ]),
    )
    .expect_err("canonical complex implicit names must share the root output namespace");

    assert_eq!(error.details[0].code, "grouping_output_name_collision");
    assert_eq!(
        error.details[0].path.as_deref(),
        Some("grouping[1].root.operations[0].outputs[0]")
    );
}

#[test]
fn search_resolver_keeps_native_debug_spelling_distinct_from_canonical_implicit_name() {
    let explicit = root_output_grouping(
        Some("sum(Literal(Integer(1)))"),
        GroupingAggregationIr::Count,
    );
    let implicit = root_output_grouping(
        None,
        GroupingAggregationIr::Sum(GroupingExpressionIr::Literal(RequestValueIr::Int64(1))),
    );

    let request = resolve_search(
        &context(),
        match_all_search(vec![
            GroupingInput::Structured(explicit),
            GroupingInput::Structured(implicit),
        ]),
    )
    .expect("native enum debug spelling is not the canonical implicit output name");

    assert_eq!(request.grouping().len(), 2);
}

#[test]
fn search_resolver_starts_a_new_semantic_scope_for_nested_all() {
    let mut context = context();
    context.definition.fields[0].capabilities.groupable = true;

    let count_output = || {
        GroupingOperationIr::Output(vec![GroupingOutputIr {
            label: None,
            expression: GroupingExpressionIr::Aggregation(Box::new(GroupingAggregationIr::Count)),
        }])
    };
    let grouping = GroupingRequestIr {
        root: GroupingAllIr {
            operations: vec![
                GroupingOperationIr::Group(GroupingExpressionIr::Attribute("brand".to_owned())),
                GroupingOperationIr::Each(GroupingAllIr {
                    operations: vec![
                        count_output(),
                        GroupingOperationIr::All(GroupingAllIr {
                            operations: vec![
                                GroupingOperationIr::Group(GroupingExpressionIr::Attribute(
                                    "brand".to_owned(),
                                )),
                                GroupingOperationIr::Each(GroupingAllIr {
                                    operations: vec![count_output()],
                                    label: None,
                                }),
                            ],
                            label: None,
                        }),
                    ],
                    label: None,
                }),
            ],
            label: None,
        },
        continuations: Vec::new(),
    };

    resolve_search(
        &context,
        match_all_search(vec![GroupingInput::Structured(grouping)]),
    )
    .expect("nested grouping levels may reuse output names");
}

#[test]
fn imported_aliases_are_preserved_but_explicitly_not_query_bound() {
    let mut definition = IndexDefinitionIr::new(
        None,
        vec![FieldIr::new(
            FieldName::parse("parent").unwrap(),
            DataTypeIr::Reference {
                document_type: "parent".to_owned(),
            },
            None,
            FieldCapabilities::new(false, false, false, false, false),
        )],
        Vec::new(),
    )
    .unwrap();
    definition.set_imported_fields(vec![ImportedFieldDefinitionIr {
        reference_field: FieldName::parse("parent").unwrap(),
        target_field: "name".to_owned(),
        alias: FieldName::parse("parent_name").unwrap(),
        properties: BTreeMap::new(),
    }]);
    let bytes = crate::wire::v1::index_definition_ir_to_api(&definition);
    let context =
        IndexContext::from_api(crate::__flatbuffers::root::<api::IndexDefinition>(&bytes).unwrap())
            .unwrap();
    let error = crate::bind_filter_expression(
        "parent_name = 'Acme'",
        context.definition(),
        &BTreeMap::new(),
    )
    .unwrap_err();
    assert_eq!(error.details[0].code, "imported_field_query_not_supported");

    let mut invalid = IndexDefinitionIr::new(
        None,
        vec![FieldIr::new(
            FieldName::parse("title").unwrap(),
            DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
            None,
            FieldCapabilities::new(true, true, false, false, false),
        )],
        Vec::new(),
    )
    .unwrap();
    invalid.set_schema_fields(vec![FieldIr::new(
        FieldName::parse("derived_ref").unwrap(),
        DataTypeIr::Reference {
            document_type: "parent".to_owned(),
        },
        None,
        FieldCapabilities::new(false, false, false, false, false),
    )]);
    invalid.set_imported_fields(vec![ImportedFieldDefinitionIr {
        reference_field: FieldName::parse("derived_ref").unwrap(),
        target_field: "name".to_owned(),
        alias: FieldName::parse("derived_name").unwrap(),
        properties: BTreeMap::new(),
    }]);
    let bytes = crate::wire::v1::index_definition_ir_to_api(&invalid);
    let error = crate::wire::v1::index_definition_to_ir(
        crate::__flatbuffers::root::<api::IndexDefinition>(&bytes).unwrap(),
    )
    .unwrap_err();
    assert_eq!(error.details[0].code, "unknown_field");
}

#[test]
fn repeated_source_errors_identify_clause_and_local_offsets() {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let first = builder.create_string("brand = 'Acme'");
    let second = builder.create_string("price <= ");
    let first = api::FilterClause::create(
        &mut builder,
        &api::FilterClauseArgs {
            source: Some(first),
            ..Default::default()
        },
    );
    let second = api::FilterClause::create(
        &mut builder,
        &api::FilterClauseArgs {
            source: Some(second),
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
    let detail = &error.details[0];
    assert_eq!(detail.code, "syntax_error");
    assert_eq!(detail.metadata.get("clause_index"), Some(&"1".to_owned()));
    assert_eq!(detail.path.as_deref(), Some("filters.textual[1].source"));
    assert!(detail.metadata.contains_key("start_offset"));
    assert!(detail.metadata.contains_key("end_offset"));
    assert!(!detail.metadata.contains_key("span_start"));
    assert!(!detail.metadata.contains_key("span_end"));
}

#[test]
fn query_user_input_accepts_annotation_or_query_model_default_index() {
    fn request_bytes(annotation: bool, query_model: bool) -> Vec<u8> {
        let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
        let input = builder.create_string("free form input");
        let input = api::QueryUserInput::create(
            &mut builder,
            &api::QueryUserInputArgs { value: Some(input) },
        );
        let annotations = if annotation {
            let key = builder.create_string("defaultIndex");
            let value = builder.create_string("title");
            let value = api::StringValue::create(
                &mut builder,
                &api::StringValueArgs { value: Some(value) },
            );
            let value = api::Value::create(
                &mut builder,
                &api::ValueArgs {
                    kind_type: api::ValueKind::StringValue,
                    kind: Some(value.as_union_value()),
                },
            );
            let entry = api::ValueEntry::create(
                &mut builder,
                &api::ValueEntryArgs {
                    key: Some(key),
                    value: Some(value),
                },
            );
            Some(builder.create_vector(&[entry]))
        } else {
            None
        };
        let expression = api::QueryExpr::create(
            &mut builder,
            &api::QueryExprArgs {
                expr_type: api::QueryExprKind::QueryUserInput,
                expr: Some(input.as_union_value()),
                annotations,
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
        let query_model = if query_model {
            let default_index = builder.create_string("title");
            Some(api::QueryModel::create(
                &mut builder,
                &api::QueryModelArgs {
                    default_index: Some(default_index),
                    ..Default::default()
                },
            ))
        } else {
            None
        };
        let request = api::SearchRequest::create(
            &mut builder,
            &api::SearchRequestArgs {
                query: Some(source),
                query_model,
                ..Default::default()
            },
        );
        builder.finish(request, None);
        builder.finished_data().to_vec()
    }

    for (name, bytes) in [
        ("query expression annotation", request_bytes(true, false)),
        ("query model", request_bytes(false, true)),
    ] {
        let request = crate::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
        let converted = context().search_to_ir(request);
        assert!(
            converted.is_ok(),
            "{name} should provide user input context"
        );
    }

    let bytes = request_bytes(false, false);
    let request = crate::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
    let error = context().search_to_ir(request).unwrap_err();
    assert_eq!(error.details[0].code, "missing_field_context");
}

#[test]
fn typed_matching_annotations_are_resolved_into_ir_parameters() {
    fn request(query: QueryIr) -> SearchRequestIr {
        SearchRequestIr::new(
            query,
            Vec::new(),
            None,
            Vec::new(),
            10,
            0,
            ProjectionIr::new(Vec::new()),
            HighlightingIr::Disabled,
            None,
            PrefixPolicy::Disabled,
            TypoPolicy::Disabled,
        )
    }
    fn term(value: &str) -> QueryExprIr {
        QueryExprIr {
            kind: QueryExprKindIr::Term {
                value: RequestValueIr::String(value.to_owned()),
            },
            annotations: BTreeMap::new(),
        }
    }
    fn contains(item: QueryExprKindIr) -> QueryExprKindIr {
        QueryExprKindIr::Contains {
            field: QueryFieldRefIr {
                path: FieldName::parse("brand").unwrap(),
                element_index: None,
            },
            item: Box::new(QueryExprIr {
                kind: item,
                annotations: BTreeMap::new(),
            }),
        }
    }

    let cases = [
        (
            "near",
            contains(QueryExprKindIr::Near {
                children: vec![term("one"), term("two")],
                distance: 7,
            }),
        ),
        (
            "weak_and",
            contains(QueryExprKindIr::WeakAnd {
                children: vec![term("one"), term("two")],
                target_hits: 11,
            }),
        ),
        (
            "fuzzy",
            contains(QueryExprKindIr::Fuzzy {
                value: RequestValueIr::String("fone".to_owned()),
                max_edit_distance: 4,
                prefix_length: 3,
                prefix: true,
            }),
        ),
    ];

    let mut query_context = context();
    query_context.definition.fields[0].capabilities.searchable = true;
    for (name, kind) in cases {
        let expected = QueryIr::Typed(Box::new(QueryExprIr {
            kind,
            annotations: BTreeMap::new(),
        }));
        let bytes = crate::wire::v1::search_request_ir_to_api(&request(expected.clone()));
        let api_request = crate::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
        let actual = query_context.search_to_ir(api_request).unwrap();
        assert_eq!(actual.query(), &expected, "{name} was not resolved in IR");
    }
}
