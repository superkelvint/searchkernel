use super::*;

#[test]
fn highlighting_resolution_preserves_multi_valued_field_shape_for_the_adapter() {
    let tags = FieldIr::new(
        FieldName::parse("tags").unwrap(),
        DataTypeIr::Array(Box::new(DataTypeIr::Primitive(PrimitiveDataTypeIr::String))),
        None,
        FieldCapabilities::new(true, true, false, false, false),
    );
    let definition = IndexDefinitionIr::new(None, vec![tags], Vec::new()).unwrap();
    let context = IndexContext {
        document_fields: BTreeMap::from([("tags".to_owned(), 0)]),
        query_fields: BTreeMap::from([("tags".to_owned(), QueryFieldRef::Document(0))]),
        definition,
    };
    let request = resolve_search(
        &context,
        SearchIntent {
            query: QuerySourceInput::Text("brown".to_owned()),
            search_fields: Some(vec![SearchFieldInput {
                field: "tags".to_owned(),
                weight: None,
            }]),
            filters: Vec::new(),
            textual_filters: Vec::new(),
            sorts: Vec::new(),
            grouping: Vec::new(),
            ranking: None,
            query_inputs: BTreeMap::new(),
            rank_properties: BTreeMap::new(),
            query_model: QueryModelInput::default(),
            projection: None,
            highlighting: Some(vec!["tags".to_owned()]),
            presentation: PresentationInput::default(),
            properties: BTreeMap::new(),
            query_profile: None,
            search_chain: None,
            timeout: None,
            limit: None,
            offset: None,
        },
    )
    .expect("multi-valued text remains semantically representable");
    let HighlightingIr::Requested {
        fields,
        multi_valued_fields,
    } = request.highlighting()
    else {
        panic!("highlighting should be requested");
    };
    assert_eq!(
        fields.iter().map(FieldName::as_str).collect::<Vec<_>>(),
        ["tags"]
    );
    assert_eq!(
        multi_valued_fields
            .iter()
            .map(FieldName::as_str)
            .collect::<Vec<_>>(),
        ["tags"]
    );

    let request_bytes = crate::wire::v1::search_request_ir_to_api(&request);
    let portable_request = crate::__flatbuffers::root::<api::SearchRequest>(&request_bytes)
        .expect("decode portable highlighting request");
    assert_eq!(
        request,
        context
            .search_to_ir(portable_request)
            .expect("portable highlighting request resolves")
    );
}

#[test]
fn rust_tensor_hnsw_fields_preserve_metric_and_graph_configuration() {
    let definition = resolve_rust_index_definition(RustIndexDefinitionInput {
        name: Some("vectors".to_owned()),
        fields: vec![RustFieldInput {
            name: "embedding".to_owned(),
            kind: RustFieldKind::Tensor("tensor<float>(x[3])".to_owned()),
            multi: false,
            stored: Some(true),
            searchable: None,
            filterable: None,
            sortable: None,
            facetable: None,
            analyzer: None,
            distance_metric: Some(DistanceMetricIr::Euclidean),
            hnsw: Some(HnswIndexConfigIr {
                max_links_per_node: Some(4),
                neighbors_to_explore_at_insert: Some(16),
                options: BTreeMap::new(),
            }),
        }],
        default_search_fields: None,
    })
    .expect("valid tensor HNSW schema");

    let field = &definition.fields()[0];
    assert!(field.capabilities().searchable());
    assert_eq!(
        field.attribute().unwrap().distance_metric,
        DistanceMetricIr::Euclidean
    );
    let hnsw = field.index().unwrap().hnsw.as_ref().unwrap();
    assert_eq!(hnsw.max_links_per_node, Some(4));
    assert_eq!(hnsw.neighbors_to_explore_at_insert, Some(16));
}

#[test]
fn rust_and_portable_field_defaults_match_for_position_and_tensor() {
    for (name, kind, data_type) in [
        (
            "position",
            RustFieldKind::Position,
            api::DataTypeKind::PositionDataType,
        ),
        (
            "embedding",
            RustFieldKind::Tensor("tensor<float>(x[3])".to_owned()),
            api::DataTypeKind::TensorDataType,
        ),
    ] {
        let rust_definition = resolve_rust_index_definition(RustIndexDefinitionInput {
            name: Some("defaults".to_owned()),
            fields: vec![RustFieldInput {
                name: name.to_owned(),
                kind,
                multi: false,
                stored: None,
                searchable: None,
                filterable: None,
                sortable: None,
                facetable: None,
                analyzer: None,
                distance_metric: None,
                hnsw: None,
            }],
            default_search_fields: None,
        })
        .expect("Rust field defaults should resolve");

        let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
        let field_name = builder.create_string(name);
        let kind = match data_type {
            api::DataTypeKind::PositionDataType => {
                api::PositionDataType::create(&mut builder, &api::PositionDataTypeArgs {})
                    .as_union_value()
            }
            api::DataTypeKind::TensorDataType => {
                let type_spec = builder.create_string("tensor<float>(x[3])");
                api::TensorDataType::create(
                    &mut builder,
                    &api::TensorDataTypeArgs {
                        type_spec: Some(type_spec),
                    },
                )
                .as_union_value()
            }
            _ => unreachable!(),
        };
        let data_type = api::DataType::create(
            &mut builder,
            &api::DataTypeArgs {
                kind_type: data_type,
                kind: Some(kind),
            },
        );
        let field = api::FieldDefinition::create(
            &mut builder,
            &api::FieldDefinitionArgs {
                name: Some(field_name),
                data_type: Some(data_type),
                ..Default::default()
            },
        );
        builder.finish(field, None);
        let portable_field =
            crate::__flatbuffers::root::<api::FieldDefinition>(builder.finished_data())
                .expect("portable field should decode");
        let portable = field_to_ir(portable_field).expect("portable field should resolve");

        assert_eq!(
            rust_definition.fields()[0],
            portable,
            "{name} ingress drift"
        );
    }
}

#[test]
fn search_resolver_validates_facet_fields_and_duplicates_against_schema() {
    let mut context = context();
    context.definition.fields[0].capabilities.groupable = true;

    for (grouping, detail_code) in [
        (
            vec![GroupingInput::Facet(FacetInput {
                field: "missing".to_owned(),
                name: None,
                limit: None,
                sort: None,
            })],
            "unknown_field",
        ),
        (
            vec![GroupingInput::Facet(FacetInput {
                field: "price".to_owned(),
                name: None,
                limit: None,
                sort: None,
            })],
            "field_not_facetable",
        ),
        (
            vec![
                GroupingInput::Facet(FacetInput {
                    field: "brand".to_owned(),
                    name: None,
                    limit: None,
                    sort: None,
                }),
                GroupingInput::Facet(FacetInput {
                    field: "brand".to_owned(),
                    name: None,
                    limit: None,
                    sort: None,
                }),
            ],
            "duplicate_facet_field",
        ),
    ] {
        let error = resolve_search(&context, match_all_search(grouping))
            .expect_err("invalid facet semantics must fail in the canonical resolver");
        assert_eq!(error.details[0].code, detail_code);
    }
}

#[test]
fn search_resolver_validates_structured_grouping_fields() {
    let mut context = context();
    context.definition.fields[0].capabilities.groupable = true;

    for (field, detail_code) in [
        ("missing", "unknown_field"),
        ("price", "field_not_facetable"),
    ] {
        let grouping = GroupingRequestIr {
            root: GroupingAllIr {
                operations: vec![GroupingOperationIr::Group(GroupingExpressionIr::Attribute(
                    field.to_owned(),
                ))],
                label: None,
            },
            continuations: Vec::new(),
        };
        let error = resolve_search(
            &context,
            match_all_search(vec![GroupingInput::Structured(grouping)]),
        )
        .expect_err("invalid grouping fields must fail in the canonical resolver");
        assert_eq!(error.details[0].code, detail_code);
    }
}

#[test]
fn search_resolver_allows_numeric_aggregations_on_attribute_fields() {
    let context = context();
    let grouping = GroupingRequestIr {
        root: GroupingAllIr {
            operations: vec![GroupingOperationIr::Output(vec![GroupingOutputIr {
                label: None,
                expression: GroupingExpressionIr::Aggregation(Box::new(
                    GroupingAggregationIr::Avg(GroupingExpressionIr::Attribute("price".to_owned())),
                )),
            }])],
            label: None,
        },
        continuations: Vec::new(),
    };

    resolve_search(
        &context,
        match_all_search(vec![GroupingInput::Structured(grouping)]),
    )
    .expect("numeric aggregation expressions should not require facetable group keys");
}

#[test]
fn schema_only_fields_are_query_visible_but_not_feedable() {
    let mut definition = IndexDefinitionIr::new(
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
    definition.set_schema_fields(vec![FieldIr::new(
        FieldName::parse("derived").unwrap(),
        DataTypeIr::Primitive(PrimitiveDataTypeIr::Int64),
        None,
        FieldCapabilities::new(true, true, true, true, false),
    )]);
    let bytes = crate::wire::v1::index_definition_ir_to_api(&definition);
    let context =
        IndexContext::from_api(crate::__flatbuffers::root::<api::IndexDefinition>(&bytes).unwrap())
            .unwrap();

    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let name = builder.create_string("derived");
    let weight = api::OptionalDouble::new(1.0);
    let search_field = api::SearchField::create(
        &mut builder,
        &api::SearchFieldArgs {
            field: Some(name),
            weight: Some(&weight),
        },
    );
    let fields = builder.create_vector(&[search_field]);
    let selection = api::SearchFieldSelection::create(
        &mut builder,
        &api::SearchFieldSelectionArgs {
            fields: Some(fields),
        },
    );
    builder.finish(selection, None);
    let selection =
        crate::__flatbuffers::root::<api::SearchFieldSelection>(builder.finished_data()).unwrap();
    assert_eq!(context.search_fields_to_ir(selection).unwrap().len(), 1);

    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let value = api::Int64Value::create(&mut builder, &api::Int64ValueArgs { value: 3 });
    let value = api::Value::create(
        &mut builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::Int64Value,
            kind: Some(value.as_union_value()),
        },
    );
    builder.finish(value, None);
    let value = crate::__flatbuffers::root::<api::Value>(builder.finished_data()).unwrap();
    let error = context.value_to_ir("derived", value).unwrap_err();
    assert_eq!(error.details[0].code, "unknown_field");
}

#[test]
fn legacy_global_textual_filter_fields_are_rejected() {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let source = builder.create_string("price <= :value");
    let sources = builder.create_vector(&[source]);
    let request = api::FilterRequest::create(
        &mut builder,
        &api::FilterRequestArgs {
            sources: Some(sources),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let request = crate::__flatbuffers::root::<api::FilterRequest>(builder.finished_data())
        .expect("valid legacy filter request");

    let error = context().filters_to_ir(Some(request)).unwrap_err();
    assert_eq!(error.details[0].code, "invalid_filter");
}

#[test]
fn typed_query_validation_enforces_field_capabilities_and_types() {
    fn field(name: &str) -> QueryFieldRefIr {
        QueryFieldRefIr {
            path: FieldName::parse(name).unwrap(),
            element_index: None,
        }
    }

    fn expression(kind: QueryExprKindIr) -> QueryExprIr {
        QueryExprIr {
            kind,
            annotations: BTreeMap::new(),
        }
    }

    fn error_code(context: &IndexContext, query: QueryExprIr) -> String {
        context
            .validate_typed_query(&query, None, None, "query")
            .unwrap_err()
            .details[0]
            .code
            .clone()
    }

    let phrase = expression(QueryExprKindIr::Phrase(vec![expression(
        QueryExprKindIr::Term {
            value: RequestValueIr::String("Acme".to_owned()),
        },
    )]));
    assert_eq!(
        error_code(
            &context(),
            expression(QueryExprKindIr::Contains {
                field: field("brand"),
                item: Box::new(phrase),
            }),
        ),
        "field_not_searchable",
        "complex text queries must enforce the same searchable-field rule as terms"
    );

    assert_eq!(
        error_code(
            &context(),
            expression(QueryExprKindIr::Comparison {
                field: field("brand"),
                op: ComparisonOp::Gt,
                value: RequestValueIr::String("Acme".to_owned()),
            }),
        ),
        "invalid_filter",
        "ordered comparisons must require numeric fields"
    );
    assert_eq!(
        error_code(
            &context(),
            expression(QueryExprKindIr::Range {
                field: field("brand"),
                lower: QueryRangeBoundIr::Finite(RequestValueIr::String("A".to_owned())),
                upper: QueryRangeBoundIr::Unbounded,
                include_lower: Some(true),
                include_upper: None,
            }),
        ),
        "invalid_filter",
        "ranges must require numeric fields"
    );

    let mut non_filterable = context();
    non_filterable.definition.fields[1].capabilities.filterable = false;
    assert_eq!(
        error_code(
            &non_filterable,
            expression(QueryExprKindIr::Comparison {
                field: field("price"),
                op: ComparisonOp::Eq,
                value: RequestValueIr::Int64(10),
            }),
        ),
        "field_not_filterable"
    );

    assert_eq!(
        error_code(
            &context(),
            expression(QueryExprKindIr::In {
                field: field("brand"),
                values: vec![RequestValueIr::Int64(1)],
            }),
        ),
        "type_mismatch",
        "membership values must match the field type"
    );
    assert_eq!(
        error_code(
            &context(),
            expression(QueryExprKindIr::Comparison {
                field: field("price"),
                op: ComparisonOp::Eq,
                value: RequestValueIr::String("10".to_owned()),
            }),
        ),
        "type_mismatch",
        "comparison values must match the field type"
    );

    let mut searchable = context();
    searchable.definition.fields[0].capabilities.searchable = true;
    let query_field = field("brand");
    assert_eq!(
        searchable
            .validate_typed_query(
                &expression(QueryExprKindIr::Equiv(vec![expression(
                    QueryExprKindIr::Term {
                        value: RequestValueIr::String("Acme".to_owned()),
                    },
                )])),
                Some(&query_field),
                None,
                "query",
            )
            .unwrap_err()
            .details[0]
            .code,
        "invalid_query_children"
    );

    let mut special_definition = context().definition.clone();
    let special_capabilities = FieldCapabilities::new(false, true, true, false, false);
    special_definition.fields.extend([
        FieldIr::new(
            FieldName::parse("position").unwrap(),
            DataTypeIr::Position,
            None,
            special_capabilities,
        ),
        FieldIr::new(
            FieldName::parse("embedding").unwrap(),
            DataTypeIr::Tensor {
                type_spec: "tensor<float>(x[2])".to_owned(),
            },
            None,
            special_capabilities,
        ),
        FieldIr::new(
            FieldName::parse("access").unwrap(),
            DataTypeIr::Predicate,
            None,
            special_capabilities,
        ),
    ]);
    let special = IndexContext::from_definition_ir(special_definition);
    assert_eq!(
        error_code(
            &special,
            expression(QueryExprKindIr::GeoLocation {
                field: field("price"),
                latitude: 1.0,
                longitude: 2.0,
                radius: "1 km".to_owned(),
            }),
        ),
        "type_mismatch"
    );
    assert_eq!(
        error_code(
            &special,
            expression(QueryExprKindIr::NearestNeighbor {
                document_field: field("price"),
                query_tensor: "q".to_owned(),
            }),
        ),
        "type_mismatch"
    );
    assert_eq!(
        error_code(
            &special,
            expression(QueryExprKindIr::WeightedSet {
                field: field("embedding"),
                items: vec![WeightedQueryItemIr {
                    key: RequestValueIr::String("x".to_owned()),
                    weight: 1,
                }],
            }),
        ),
        "type_mismatch"
    );
    assert_eq!(
        error_code(
            &special,
            expression(QueryExprKindIr::Predicate {
                field: field("price"),
                features: BTreeMap::from([(
                    "hobby".to_owned(),
                    RequestValueIr::String("music".to_owned()),
                )]),
                range_features: BTreeMap::new(),
            }),
        ),
        "type_mismatch"
    );
    assert_eq!(
        error_code(
            &special,
            expression(QueryExprKindIr::Predicate {
                field: field("access"),
                features: BTreeMap::from([("hobby".to_owned(), RequestValueIr::Int64(1))]),
                range_features: BTreeMap::new(),
            }),
        ),
        "invalid_predicate_value"
    );
    assert_eq!(
        error_code(
            &special,
            expression(QueryExprKindIr::Predicate {
                field: field("access"),
                features: BTreeMap::new(),
                range_features: BTreeMap::from([("age".to_owned(), RequestValueIr::Int64(-1))]),
            }),
        ),
        "invalid_predicate_value"
    );

    let mut relative_definition = context().definition.clone();
    let searchable_string = || FieldCapabilities::new(true, true, true, false, false);
    relative_definition.fields.extend([
        FieldIr::new(
            FieldName::parse("name").unwrap(),
            DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
            None,
            searchable_string(),
        ),
        FieldIr::new(
            FieldName::parse("foo").unwrap(),
            DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
            None,
            searchable_string(),
        ),
        FieldIr::new(
            FieldName::parse("items").unwrap(),
            DataTypeIr::Array(Box::new(DataTypeIr::Struct {
                fields: vec![
                    StructFieldIr {
                        name: "name".to_owned(),
                        data_type: DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
                    },
                    StructFieldIr {
                        name: "color".to_owned(),
                        data_type: DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
                    },
                ],
            })),
            None,
            FieldCapabilities::new(true, false, true, false, false),
        ),
    ]);
    let relative = IndexContext::from_definition_ir(relative_definition);
    let same_element = |child_field: &str| {
        expression(QueryExprKindIr::Contains {
            field: field("items"),
            item: Box::new(expression(QueryExprKindIr::SameElement(vec![
                expression(QueryExprKindIr::Contains {
                    field: field(child_field),
                    item: Box::new(expression(QueryExprKindIr::Term {
                        value: RequestValueIr::String("value".to_owned()),
                    })),
                }),
                expression(QueryExprKindIr::Contains {
                    field: field("color"),
                    item: Box::new(expression(QueryExprKindIr::Term {
                        value: RequestValueIr::String("blue".to_owned()),
                    })),
                }),
            ]))),
        })
    };
    relative
        .validate_typed_query(&same_element("name"), None, None, "query")
        .expect("sameElement member name resolves relative to items before top-level name");
    let same_element_matches = expression(QueryExprKindIr::Contains {
        field: field("items"),
        item: Box::new(expression(QueryExprKindIr::SameElement(vec![
            expression(QueryExprKindIr::Matches {
                field: field("name"),
                pattern: "bo.*".to_owned(),
            }),
            expression(QueryExprKindIr::Contains {
                field: field("color"),
                item: Box::new(expression(QueryExprKindIr::Term {
                    value: RequestValueIr::String("blue".to_owned()),
                })),
            }),
        ]))),
    });
    relative
        .validate_typed_query(&same_element_matches, None, None, "query")
        .expect("sameElement Matches member resolves relative to items");
    let missing_member = relative
        .validate_typed_query(&same_element("foo"), None, None, "query")
        .expect_err("top-level foo must not mask missing items.foo");
    assert_eq!(missing_member.details[0].code, "unknown_field");

    let resolved = relative
        .resolve_searchable_query_field(
            &field("name"),
            Some(&field("items")),
            "query.same_element.name",
        )
        .expect("relative member resolves");
    assert_eq!(resolved.path.as_str(), "items.name");
}
