use searchkernel_core::api;
use searchkernel_core::*;
use std::collections::HashMap;

fn text_field(name: &str) -> api::FieldDefinition {
    api::FieldDefinition {
        name: name.to_owned(),
        kind: api::FieldKind::Text as i32,
        cardinality: 0,
        stored: None,
        searchable: None,
        filterable: None,
        sortable: None,
        facetable: None,
        analyzer: None,
    }
}

fn field(name: &str, kind: api::FieldKind) -> api::FieldDefinition {
    api::FieldDefinition {
        name: name.to_owned(),
        kind: kind as i32,
        cardinality: 0,
        stored: None,
        searchable: None,
        filterable: None,
        sortable: None,
        facetable: None,
        analyzer: None,
    }
}

fn schema() -> api::IndexDefinition {
    api::IndexDefinition {
        name: Some("products".to_owned()),
        fields: vec![
            text_field("title"),
            field("brand", api::FieldKind::Keyword),
            field("price", api::FieldKind::Int64),
            api::FieldDefinition {
                cardinality: api::FieldCardinality::Multi as i32,
                ..field("tags", api::FieldKind::Keyword)
            },
            field("available", api::FieldKind::Bool),
        ],
        default_search_fields: None,
    }
}

fn string_value(value: &str) -> api::Value {
    api::Value {
        kind: Some(api::value::Kind::StringValue(value.to_owned())),
    }
}

fn int_value(value: i64) -> api::Value {
    api::Value {
        kind: Some(api::value::Kind::Int64Value(value)),
    }
}

fn search_selection(fields: &[(&str, Option<f64>)]) -> api::SearchFieldSelection {
    api::SearchFieldSelection {
        fields: fields
            .iter()
            .map(|(field, weight)| api::SearchField {
                field: (*field).to_owned(),
                weight: *weight,
            })
            .collect(),
    }
}

#[test]
fn schema_defaults_and_presence_are_resolved() {
    let semantics = IndexSemantics::new(&schema()).unwrap();
    let definition = semantics.definition();

    assert_eq!(
        definition.fields[0].analyzer,
        Some(AnalyzerIdentity::StandardV1)
    );
    assert_eq!(definition.fields[0].cardinality, FieldCardinality::Single);
    assert!(definition.fields[0].capabilities.stored);
    assert!(definition.fields[0].capabilities.searchable);
    assert!(definition.fields[1].capabilities.facetable);
    assert!(definition.fields[2].capabilities.sortable);
    assert_eq!(definition.fields[3].cardinality, FieldCardinality::Multi);
    assert!(!definition.fields[3].capabilities.sortable);
    assert_eq!(definition.default_search_fields.len(), 1);
    assert_eq!(definition.default_search_fields[0].field.as_str(), "title");
}

#[test]
fn explicit_empty_and_explicit_weight_are_preserved() {
    let mut definition = schema();
    definition.default_search_fields = Some(search_selection(&[]));
    let semantics = IndexSemantics::new(&definition).unwrap();
    assert!(semantics.definition().default_search_fields.is_empty());

    definition.default_search_fields = Some(search_selection(&[("title", Some(0.0))]));
    let semantics = IndexSemantics::new(&definition).unwrap();
    assert_eq!(
        semantics.definition().default_search_fields[0].weight.get(),
        0.0
    );
}

#[test]
fn documents_are_strictly_typed_and_canonicalized() {
    let semantics = IndexSemantics::new(&schema()).unwrap();
    let mut fields = HashMap::new();
    fields.insert("price".to_owned(), int_value(42));
    fields.insert("title".to_owned(), string_value("headphones"));
    fields.insert(
        "tags".to_owned(),
        api::Value {
            kind: Some(api::value::Kind::ListValue(api::ValueList {
                values: vec![string_value("audio"), string_value("sale")],
            })),
        },
    );
    let resolved = semantics
        .resolve_insert(&api::InsertRequest {
            document: Some(api::Document {
                id: "sku-1".to_owned(),
                fields,
            }),
        })
        .unwrap();
    let names: Vec<_> = resolved
        .document
        .fields
        .keys()
        .map(FieldName::as_str)
        .collect();
    assert_eq!(names, vec!["price", "tags", "title"]);

    let error = semantics
        .resolve_insert(&api::InsertRequest {
            document: Some(api::Document {
                id: "sku-2".to_owned(),
                fields: HashMap::from([("price".to_owned(), string_value("42"))]),
            }),
        })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidArgument);
    assert_eq!(error.details[0].code, "type_mismatch");
    assert_eq!(
        error.details[0].path.as_deref(),
        Some("document.fields.price")
    );
}

#[test]
fn filters_flatten_and_validate_ranges() {
    let semantics = IndexSemantics::new(&schema()).unwrap();
    let comparison = api::FilterExpr {
        expr: Some(api::filter_expr::Expr::Comparison(api::FilterComparison {
            field: "price".to_owned(),
            op: api::ComparisonOp::Gte as i32,
            value: Some(int_value(10)),
        })),
    };
    let nested = api::FilterExpr {
        expr: Some(api::filter_expr::Expr::All(api::FilterAnd {
            clauses: vec![
                comparison.clone(),
                api::FilterExpr {
                    expr: Some(api::filter_expr::Expr::All(api::FilterAnd {
                        clauses: vec![comparison.clone()],
                    })),
                },
            ],
        })),
    };
    let request = api::SearchRequest {
        query: "".to_owned(),
        filter: Some(nested),
        ..Default::default()
    };
    let resolved = semantics.resolve_search(&request).unwrap();
    assert!(
        matches!(resolved.filter, Some(ResolvedFilterExpr::And(ref clauses)) if clauses.len() == 2)
    );

    let bad_range = api::FilterExpr {
        expr: Some(api::filter_expr::Expr::Range(api::FilterRange {
            field: "price".to_owned(),
            lower: Some(int_value(20)),
            upper: Some(int_value(10)),
            include_lower: None,
            include_upper: None,
        })),
    };
    let error = semantics
        .resolve_search(&api::SearchRequest {
            filter: Some(bad_range),
            ..Default::default()
        })
        .unwrap_err();
    assert_eq!(error.details[0].code, "invalid_range");
}

#[test]
fn inspect_query_materializes_defaults_but_preserves_highlight_presence() {
    let semantics = IndexSemantics::new(&schema()).unwrap();
    let empty_highlight = semantics
        .inspect_query(&api::InspectQueryRequest {
            request: Some(api::SearchRequest {
                query: "  ".to_owned(),
                highlighting: Some(api::HighlightRequest { fields: vec![] }),
                ..Default::default()
            }),
        })
        .unwrap();
    let request = empty_highlight.resolved_request.unwrap();
    assert_eq!(request.query, "");
    assert_eq!(request.limit, Some(20));
    assert_eq!(request.offset, Some(0));
    assert!(request.search_fields.is_some());
    assert!(request.projection.is_some());
    assert_eq!(request.highlighting.unwrap().fields, Vec::<String>::new());

    let disabled = semantics
        .inspect_query(&api::InspectQueryRequest {
            request: Some(api::SearchRequest::default()),
        })
        .unwrap()
        .resolved_request
        .unwrap();
    assert!(disabled.highlighting.is_none());
}

#[test]
fn backend_boundary_is_backend_independent() {
    struct BackendStub;
    impl Backend for BackendStub {
        fn insert(&self, _: ResolvedInsertRequest) -> Result<api::InsertResult> {
            unimplemented!()
        }
        fn upsert(&self, _: ResolvedUpsertRequest) -> Result<api::UpsertResult> {
            unimplemented!()
        }
        fn patch(&self, _: ResolvedPatchRequest) -> Result<api::PatchResult> {
            unimplemented!()
        }
        fn delete(&self, _: ResolvedDeleteRequest) -> Result<api::DeleteResult> {
            unimplemented!()
        }
        fn get(&self, _: ResolvedGetRequest) -> Result<api::GetResult> {
            unimplemented!()
        }
        fn search(&self, _: ResolvedSearchRequest) -> Result<api::SearchResult> {
            unimplemented!()
        }
        fn analyze(&self, _: ResolvedAnalyzeRequest) -> Result<api::AnalyzeResult> {
            unimplemented!()
        }
    }

    fn assert_backend<T: Backend>() {}
    assert_backend::<BackendStub>();
}

#[test]
fn structured_query_expr_is_typed_and_round_trips_through_inspection() {
    let semantics = IndexSemantics::new(&schema()).unwrap();
    let query_expr = api::QueryExpr {
        expr: Some(api::query_expr::Expr::Term(api::TermQuery {
            field: "title".to_owned(),
            value: Some(string_value("headphones")),
        })),
    };
    let resolved = semantics
        .resolve_search(&api::SearchRequest {
            query_expr: Some(query_expr),
            ..Default::default()
        })
        .unwrap();
    assert!(matches!(resolved.query, ResolvedQuery::Term { .. }));

    let inspected = semantics
        .inspect_query(&api::InspectQueryRequest {
            request: Some(api::SearchRequest {
                query_expr: Some(api::QueryExpr {
                    expr: Some(api::query_expr::Expr::Term(api::TermQuery {
                        field: "title".to_owned(),
                        value: Some(string_value("headphones")),
                    })),
                }),
                ..Default::default()
            }),
        })
        .unwrap()
        .resolved_request
        .unwrap();
    assert!(inspected.query_expr.is_some());
    assert_eq!(inspected.query, "");
}

#[test]
fn invalid_schema_and_request_values_have_stable_errors() {
    let mut invalid_name = schema();
    invalid_name.fields[0].name = "_score".to_owned();
    let error = IndexSemantics::new(&invalid_name).unwrap_err();
    assert_eq!(error.details[0].code, "invalid_field_name");
    assert_eq!(error.details[0].path.as_deref(), Some("fields[0].name"));

    let mut invalid_capability = schema();
    invalid_capability.fields[0].filterable = Some(true);
    let error = IndexSemantics::new(&invalid_capability).unwrap_err();
    assert_eq!(error.details[0].code, "invalid_capability");
    assert_eq!(
        error.details[0].path.as_deref(),
        Some("fields[0].filterable")
    );

    let semantics = IndexSemantics::new(&schema()).unwrap();
    let error = semantics
        .resolve_search(&api::SearchRequest {
            language: Some("x".to_owned()),
            ..Default::default()
        })
        .unwrap_err();
    assert_eq!(error.details[0].code, "invalid_language");
    assert_eq!(error.details[0].path.as_deref(), Some("language"));
}

#[test]
fn patches_sort_facets_and_projection_resolve_defaults() {
    let semantics = IndexSemantics::new(&schema()).unwrap();
    let patch = semantics
        .resolve_patch(&api::PatchRequest {
            id: "sku-1".to_owned(),
            operations: vec![api::PatchOperation {
                operation: Some(api::patch_operation::Operation::Set(api::SetField {
                    field: "price".to_owned(),
                    value: Some(int_value(99)),
                })),
            }],
        })
        .unwrap();
    assert!(matches!(
        patch.operations[0],
        ResolvedPatchOperation::Set { .. }
    ));

    let resolved = semantics
        .resolve_search(&api::SearchRequest {
            sort: vec![api::SortExpr {
                field: "price".to_owned(),
                direction: 0,
                missing: None,
            }],
            facets: vec![api::FacetRequest {
                field: "brand".to_owned(),
                limit: None,
                sort: None,
            }],
            projection: Some(api::FieldProjection {
                fields: vec!["title".to_owned(), "price".to_owned()],
            }),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(resolved.sort[0].direction, SortDirection::Asc);
    assert_eq!(resolved.sort[0].missing, MissingPlacement::Last);
    assert_eq!(resolved.facets[0].limit, 10);
    assert_eq!(resolved.facets[0].sort, FacetSort::CountDesc);
    assert_eq!(
        resolved
            .projection
            .fields
            .iter()
            .map(FieldName::as_str)
            .collect::<Vec<_>>(),
        vec!["title", "price"]
    );
}
