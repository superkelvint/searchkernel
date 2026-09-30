use super::*;

pub(super) fn context() -> IndexContext {
    let field = |name, kind| {
        let data_type = match kind {
            FieldKind::Text | FieldKind::Keyword => {
                DataTypeIr::Primitive(PrimitiveDataTypeIr::String)
            }
            FieldKind::Int64 => DataTypeIr::Primitive(PrimitiveDataTypeIr::Int64),
            FieldKind::Float64 => DataTypeIr::Primitive(PrimitiveDataTypeIr::Float64),
            FieldKind::Bool => DataTypeIr::Primitive(PrimitiveDataTypeIr::Bool),
        };
        let mut field = FieldIr::new(
            FieldName::parse(name).unwrap(),
            data_type,
            None,
            FieldCapabilities::new(false, false, true, false, false),
        );
        if kind == FieldKind::Keyword {
            field.matching = Some(MatchingConfigIr {
                mode: MatchingModeIr::Exact,
                options: BTreeMap::new(),
            });
        }
        field
    };
    let definition = IndexDefinitionIr::new(
        None,
        vec![
            field("brand", FieldKind::Keyword),
            field("price", FieldKind::Int64),
            field("rating", FieldKind::Float64),
            field("in_stock", FieldKind::Bool),
        ],
        Vec::new(),
    )
    .unwrap();
    let document_fields = definition
        .fields()
        .iter()
        .enumerate()
        .map(|(index, field)| (field.name().as_str().to_owned(), index))
        .collect();
    let query_fields = definition
        .fields()
        .iter()
        .enumerate()
        .map(|(index, field)| {
            (
                field.name().as_str().to_owned(),
                QueryFieldRef::Document(index),
            )
        })
        .collect();
    IndexContext {
        definition,
        document_fields,
        query_fields,
    }
}

pub(super) fn int_value(
    builder: &mut crate::__flatbuffers::FlatBufferBuilder<'static>,
    value: i64,
) -> crate::__flatbuffers::WIPOffset<api::Value<'static>> {
    let value = api::Int64Value::create(builder, &api::Int64ValueArgs { value });
    api::Value::create(
        builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::Int64Value,
            kind: Some(value.as_union_value()),
        },
    )
}

pub(super) fn bool_value(
    builder: &mut crate::__flatbuffers::FlatBufferBuilder<'static>,
    value: bool,
) -> crate::__flatbuffers::WIPOffset<api::Value<'static>> {
    let value = api::BoolValue::create(builder, &api::BoolValueArgs { value });
    api::Value::create(
        builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::BoolValue,
            kind: Some(value.as_union_value()),
        },
    )
}

pub(super) fn string_value(
    builder: &mut crate::__flatbuffers::FlatBufferBuilder<'static>,
    value: &str,
) -> crate::__flatbuffers::WIPOffset<api::Value<'static>> {
    let value = builder.create_string(value);
    let value = api::StringValue::create(builder, &api::StringValueArgs { value: Some(value) });
    api::Value::create(
        builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::StringValue,
            kind: Some(value.as_union_value()),
        },
    )
}

pub(super) fn request_value(
    builder: &mut crate::__flatbuffers::FlatBufferBuilder<'static>,
    value: &RequestValueIr,
) -> crate::__flatbuffers::WIPOffset<api::Value<'static>> {
    match value {
        RequestValueIr::Bool(value) => bool_value(builder, *value),
        RequestValueIr::Int64(value) => int_value(builder, *value),
        RequestValueIr::String(value) => string_value(builder, value),
        RequestValueIr::List(values) => {
            let values = values
                .iter()
                .map(|value| request_value(builder, value))
                .collect::<Vec<_>>();
            let values = builder.create_vector(&values);
            let value = api::ValueList::create(
                builder,
                &api::ValueListArgs {
                    values: Some(values),
                },
            );
            api::Value::create(
                builder,
                &api::ValueArgs {
                    kind_type: api::ValueKind::ValueList,
                    kind: Some(value.as_union_value()),
                },
            )
        }
        other => panic!("parity fixture does not encode {other:?}"),
    }
}

pub(super) fn value_entry(
    builder: &mut crate::__flatbuffers::FlatBufferBuilder<'static>,
    key: &str,
    value: &RequestValueIr,
) -> crate::__flatbuffers::WIPOffset<api::ValueEntry<'static>> {
    let key = builder.create_string(key);
    let value = request_value(builder, value);
    api::ValueEntry::create(
        builder,
        &api::ValueEntryArgs {
            key: Some(key),
            value: Some(value),
        },
    )
}

pub(super) fn parity_document(
    builder: &mut crate::__flatbuffers::FlatBufferBuilder<'static>,
) -> crate::__flatbuffers::WIPOffset<api::Document<'static>> {
    let fields = [
        ("brand", RequestValueIr::String("acme".to_owned())),
        ("price", RequestValueIr::Int64(42)),
        (
            "tags",
            RequestValueIr::List(vec![
                RequestValueIr::String("featured".to_owned()),
                RequestValueIr::String("sale".to_owned()),
            ]),
        ),
        ("title", RequestValueIr::String("Coffee grinder".to_owned())),
    ];
    let fields = fields
        .iter()
        .map(|(key, value)| value_entry(builder, key, value))
        .collect::<Vec<_>>();
    let fields = builder.create_vector(&fields);
    let id = builder.create_string("doc-7");
    api::Document::create(
        builder,
        &api::DocumentArgs {
            id: Some(id),
            fields: Some(fields),
        },
    )
}

pub(super) fn parity_schema() -> RustIndexDefinitionInput {
    let analyzer = || RustAnalyzerInput {
        name: "standard".to_owned(),
        version: 1,
        options: BTreeMap::new(),
    };
    let field = |name: &str, kind: RustFieldKind| RustFieldInput {
        name: name.to_owned(),
        kind,
        multi: false,
        stored: Some(true),
        searchable: None,
        filterable: None,
        sortable: None,
        facetable: None,
        analyzer: None,
        distance_metric: None,
        hnsw: None,
    };
    RustIndexDefinitionInput {
        name: Some("catalog".to_owned()),
        fields: vec![
            RustFieldInput {
                searchable: Some(true),
                analyzer: Some(analyzer()),
                ..field("title", RustFieldKind::Text)
            },
            RustFieldInput {
                filterable: Some(true),
                sortable: Some(true),
                facetable: Some(true),
                ..field("brand", RustFieldKind::Keyword)
            },
            RustFieldInput {
                filterable: Some(true),
                sortable: Some(true),
                ..field("price", RustFieldKind::Int64)
            },
            RustFieldInput {
                filterable: Some(true),
                sortable: Some(true),
                ..field("rating", RustFieldKind::Float64)
            },
            RustFieldInput {
                filterable: Some(true),
                ..field("in_stock", RustFieldKind::Bool)
            },
            RustFieldInput {
                multi: true,
                filterable: Some(true),
                facetable: Some(true),
                ..field("tags", RustFieldKind::Keyword)
            },
        ],
        default_search_fields: Some(vec![RustSearchFieldInput {
            field: "title".to_owned(),
            weight: Some(2.5),
        }]),
    }
}

pub(super) fn parity_document_input() -> DocumentInput {
    DocumentInput {
        id: "doc-7".to_owned(),
        fields: BTreeMap::from([
            (
                "brand".to_owned(),
                RequestValueIr::String("acme".to_owned()),
            ),
            ("price".to_owned(), RequestValueIr::Int64(42)),
            (
                "tags".to_owned(),
                RequestValueIr::List(vec![
                    RequestValueIr::String("featured".to_owned()),
                    RequestValueIr::String("sale".to_owned()),
                ]),
            ),
            (
                "title".to_owned(),
                RequestValueIr::String("Coffee grinder".to_owned()),
            ),
        ]),
    }
}

pub(super) fn structured_comparison(
    builder: &mut crate::__flatbuffers::FlatBufferBuilder<'static>,
    field: &str,
    op: api::ComparisonOp,
    value: crate::__flatbuffers::WIPOffset<api::Value<'static>>,
) -> crate::__flatbuffers::WIPOffset<api::FilterExpr<'static>> {
    let field = builder.create_string(field);
    let field = api::QueryFieldRef::create(
        builder,
        &api::QueryFieldRefArgs {
            path: Some(field),
            element_index: None,
        },
    );
    let comparison = api::FilterComparison::create(
        builder,
        &api::FilterComparisonArgs {
            field: Some(field),
            op,
            value: Some(value),
        },
    );
    api::FilterExpr::create(
        builder,
        &api::FilterExprArgs {
            expr_type: api::FilterExprKind::FilterComparison,
            expr: Some(comparison.as_union_value()),
        },
    )
}

pub(super) fn textual_clause(
    builder: &mut crate::__flatbuffers::FlatBufferBuilder<'static>,
    source: &str,
) -> crate::__flatbuffers::WIPOffset<api::FilterClause<'static>> {
    let source = builder.create_string(source);
    api::FilterClause::create(
        builder,
        &api::FilterClauseArgs {
            source: Some(source),
            ..Default::default()
        },
    )
}

pub(super) fn match_all_source(
    builder: &mut crate::__flatbuffers::FlatBufferBuilder<'static>,
) -> crate::__flatbuffers::WIPOffset<api::QuerySource<'static>> {
    let match_all = api::MatchAllQuery::create(builder, &api::MatchAllQueryArgs {});
    api::QuerySource::create(
        builder,
        &api::QuerySourceArgs {
            source_type: api::QuerySourceKind::MatchAllQuery,
            source: Some(match_all.as_union_value()),
        },
    )
}

pub(super) fn portable_search_with_textual_filter(source: &str) -> Vec<u8> {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let query_source = match_all_source(&mut builder);
    let clause = textual_clause(&mut builder, source);
    let textual = builder.create_vector(&[clause]);
    let filters = api::FilterRequest::create(
        &mut builder,
        &api::FilterRequestArgs {
            textual: Some(textual),
            ..Default::default()
        },
    );
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(query_source),
            filters: Some(filters),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    builder.finished_data().to_vec()
}

pub(super) fn filter_search_input(
    filters: Vec<FilterInput>,
    textual_filters: Vec<TextualFilterInput>,
) -> SearchInput {
    SearchInput {
        query: QuerySourceInput::MatchAll,
        search_fields: None,
        filters,
        textual_filters,
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

pub(super) fn assert_same_error(left: &crate::PortableError, right: &crate::PortableError) {
    assert_eq!(left.code, right.code);
    assert_eq!(
        left.details.first().map(|detail| &detail.code),
        right.details.first().map(|detail| &detail.code)
    );
}

pub(super) fn assert_flattened_and(filter: FilterIr, expected_len: usize) {
    match filter {
        FilterIr::And(clauses) => {
            assert_eq!(clauses.len(), expected_len);
            assert!(clauses
                .iter()
                .all(|clause| !matches!(clause, FilterIr::And(_))));
        }
        other => panic!("expected flattened AND, got {other:?}"),
    }
}

pub(super) fn portable_container_control_request(
    query_profile: Option<&str>,
    search_chain: Option<&str>,
    property: Option<&str>,
) -> Vec<u8> {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let query = match_all_source(&mut builder);
    let query_profile = query_profile.map(|value| builder.create_string(value));
    let search_chain = search_chain.map(|value| builder.create_string(value));
    let properties = property.map(|key| {
        let entry = value_entry(
            &mut builder,
            key,
            &RequestValueIr::String("generic".to_owned()),
        );
        builder.create_vector(&[entry])
    });
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(query),
            properties,
            query_profile,
            search_chain,
            ..Default::default()
        },
    );
    builder.finish(request, None);
    builder.finished_data().to_vec()
}

pub(super) fn match_all_search(grouping: Vec<GroupingInput>) -> SearchIntent {
    SearchIntent {
        query: QuerySourceInput::MatchAll,
        search_fields: None,
        filters: Vec::new(),
        textual_filters: Vec::new(),
        sorts: Vec::new(),
        grouping,
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

pub(super) fn root_output_grouping(
    label: Option<&str>,
    aggregation: GroupingAggregationIr,
) -> GroupingRequestIr {
    GroupingRequestIr {
        root: GroupingAllIr {
            operations: vec![GroupingOperationIr::Output(vec![GroupingOutputIr {
                label: label.map(ToOwned::to_owned),
                expression: GroupingExpressionIr::Aggregation(Box::new(aggregation)),
            }])],
            label: None,
        },
        continuations: Vec::new(),
    }
}
