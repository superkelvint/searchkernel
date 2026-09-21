use super::*;
use crate::model::*;
use crate::ErrorCode;
use searchkernel_protocol::v2 as api;
use std::collections::BTreeMap;
use std::time::Duration;

use super::filter::MAX_FILTER_BINDINGS;
use super::query::normalize_typed_query;

fn context() -> IndexContext {
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
        RankingPolicyIr::DefaultRelevanceV1,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
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

fn int_value(
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

fn bool_value(
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

#[test]
fn update_resolution_preserves_operations_create_and_condition() {
    let request = resolve_update(
        &context(),
        "doc:1",
        &[
            UpdateOperationInput::Assign {
                target: "price".to_owned(),
                value: RequestValueIr::Int64(12),
            },
            UpdateOperationInput::Arithmetic {
                target: "rating".to_owned(),
                operator: UpdateArithmeticOperatorIr::Increment,
                operand: RequestValueIr::Float64(3.0),
            },
        ],
        Some("price == 9"),
        Some(true),
    )
    .expect("valid update resolves");

    assert_eq!(request.id().as_str(), "doc:1");
    assert_eq!(request.operations().len(), 2);
    assert_eq!(request.condition(), Some("price == 9"));
    assert_eq!(request.create(), Some(true));
}

#[test]
fn update_resolution_enforces_target_value_and_arithmetic_policy() {
    fn detail_code(id: &str, operations: &[UpdateOperationInput]) -> String {
        resolve_update(&context(), id, operations, None, None)
            .expect_err("update should be rejected")
            .details[0]
            .code
            .clone()
    }

    assert_eq!(detail_code("doc:1", &[]), "empty_update");
    assert_eq!(
        detail_code(
            "",
            &[UpdateOperationInput::Clear {
                target: "price".to_owned(),
            }],
        ),
        "invalid_document_id"
    );
    assert_eq!(
        detail_code(
            "doc:1",
            &[UpdateOperationInput::Assign {
                target: String::new(),
                value: RequestValueIr::Int64(1),
            }],
        ),
        "invalid_update_target"
    );
    assert_eq!(
        detail_code(
            "doc:1",
            &[UpdateOperationInput::Assign {
                target: "price.value".to_owned(),
                value: RequestValueIr::Int64(1),
            }],
        ),
        "update_field_path_not_supported"
    );
    assert_eq!(
        detail_code(
            "doc:1",
            &[UpdateOperationInput::Assign {
                target: "missing".to_owned(),
                value: RequestValueIr::Int64(1),
            }],
        ),
        "unknown_field"
    );
    assert_eq!(
        detail_code(
            "doc:1",
            &[UpdateOperationInput::Assign {
                target: "price".to_owned(),
                value: RequestValueIr::String("not an integer".to_owned()),
            }],
        ),
        "type_mismatch"
    );
    assert_eq!(
        detail_code(
            "doc:1",
            &[UpdateOperationInput::Arithmetic {
                target: "brand".to_owned(),
                operator: UpdateArithmeticOperatorIr::Increment,
                operand: RequestValueIr::Int64(1),
            }],
        ),
        "arithmetic_requires_numeric_field"
    );
    assert_eq!(
        detail_code(
            "doc:1",
            &[UpdateOperationInput::Arithmetic {
                target: "rating".to_owned(),
                operator: UpdateArithmeticOperatorIr::Divide,
                operand: RequestValueIr::Float64(f64::NAN),
            }],
        ),
        "arithmetic_operand_type"
    );
    let duplicate = resolve_update(
        &context(),
        "doc:1",
        &[
            UpdateOperationInput::Assign {
                target: "price".to_owned(),
                value: RequestValueIr::Int64(1),
            },
            UpdateOperationInput::Clear {
                target: "price".to_owned(),
            },
        ],
        None,
        None,
    )
    .expect_err("multiple updates for one target are unsupported");
    assert_eq!(duplicate.code, ErrorCode::Unsupported);
    assert_eq!(
        duplicate.details[0].code,
        "multiple_updates_per_target_not_supported"
    );

    for operator in [
        UpdateArithmeticOperatorIr::Increment,
        UpdateArithmeticOperatorIr::Decrement,
        UpdateArithmeticOperatorIr::Multiply,
        UpdateArithmeticOperatorIr::Divide,
    ] {
        let request = resolve_update(
            &context(),
            "doc:1",
            &[UpdateOperationInput::Arithmetic {
                target: "rating".to_owned(),
                operator,
                operand: RequestValueIr::Float64(2.0),
            }],
            None,
            None,
        )
        .expect("finite arithmetic update resolves");
        assert_eq!(request.operations().len(), 1);
        assert!(matches!(
            request.operations()[0],
            UpdateOperationIr::Arithmetic { operator: actual, operand, .. }
                if actual == operator && operand == 2.0
        ));
    }
}

fn string_value(
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

fn request_value(
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

fn value_entry(
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

fn parity_document(
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

fn parity_schema() -> RustIndexDefinitionInput {
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
fn rust_rank_profiles_preserve_phase_presence_and_second_phase_expression() {
    let definition = resolve_rust_index_definition_with_profiles(
        RustIndexDefinitionInput {
            name: Some("ranked".to_owned()),
            fields: vec![RustFieldInput {
                name: "title".to_owned(),
                kind: RustFieldKind::Text,
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
        },
        vec![RustRankProfileInput {
            name: "custom".to_owned(),
            first_phase: None,
            second_phase: Some("attribute(score)".to_owned()),
            rerank_count: Some(7),
            match_features: Vec::new(),
            query_inputs: Vec::new(),
        }],
    )
    .expect("rank profile should resolve");

    let profile = &definition.rank_profiles()[0];
    assert_eq!(profile.first_phase, None);
    assert_eq!(
        profile
            .second_phase
            .as_ref()
            .and_then(|phase| phase.expression.as_deref()),
        Some("attribute(score)")
    );
    assert_eq!(
        profile
            .second_phase
            .as_ref()
            .and_then(|phase| phase.rerank_count),
        Some(7)
    );
}

fn parity_document_input() -> DocumentInput {
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

fn structured_comparison(
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

fn textual_clause(
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

fn assert_flattened_and(filter: FilterIr, expected_len: usize) {
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
    let schema_bytes = crate::index_definition_ir_to_api(&rust_definition);
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
    let search_bytes = crate::search_request_ir_to_api(&rust_search);
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
fn unknown_ranking_profile_is_rejected_before_backend_lowering() {
    let error = resolve_search(
        &context(),
        SearchIntent {
            query: QuerySourceInput::MatchAll,
            search_fields: None,
            filters: Vec::new(),
            textual_filters: Vec::new(),
            sorts: Vec::new(),
            grouping: Vec::new(),
            ranking: Some(RankingInput::Named("missing".to_owned())),
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
        },
    )
    .expect_err("unknown ranking profile must not reach Vespa fallback");

    assert_eq!(error.code, crate::ErrorCode::InvalidArgument);
    assert_eq!(error.details[0].code, "invalid_rank_profile");
}

#[test]
fn component_ranking_resolution_does_not_bypass_search_semantics() {
    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let profile = builder.create_string("");
    let ranking = api::RankingRequest::create(
        &mut builder,
        &api::RankingRequestArgs {
            profile: Some(profile),
            ..Default::default()
        },
    );
    builder.finish(ranking, None);
    let ranking_bytes = builder.finished_data().to_vec();
    let ranking = crate::__flatbuffers::root::<api::RankingRequest>(&ranking_bytes).unwrap();
    let direct = context()
        .ranking_request_to_ir(Some(ranking))
        .expect_err("empty named profiles must be rejected by component conversion");
    assert_eq!(direct.details[0].code, "invalid_ranking");

    let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
    let match_all = api::MatchAllQuery::create(&mut builder, &api::MatchAllQueryArgs {});
    let source = api::QuerySource::create(
        &mut builder,
        &api::QuerySourceArgs {
            source_type: api::QuerySourceKind::MatchAllQuery,
            source: Some(match_all.as_union_value()),
        },
    );
    let profile = builder.create_string("");
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
            query: Some(source),
            ranking: Some(ranking),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    let request = crate::__flatbuffers::root::<api::SearchRequest>(builder.finished_data())
        .expect("valid search request");
    let error = context()
        .search_to_ir(request)
        .expect_err("full search resolution must reject an empty named profile");
    assert_eq!(error.details[0].code, "invalid_ranking");
}

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

fn match_all_search(grouping: Vec<GroupingInput>) -> SearchIntent {
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

#[test]
fn search_resolver_validates_facet_fields_and_duplicates_against_schema() {
    let mut context = context();
    context.definition.fields[0].capabilities.groupable = true;

    for (grouping, detail_code) in [
        (
            vec![GroupingInput::Facet(FacetInput {
                field: "missing".to_owned(),
                limit: None,
                sort: None,
            })],
            "unknown_field",
        ),
        (
            vec![GroupingInput::Facet(FacetInput {
                field: "price".to_owned(),
                limit: None,
                sort: None,
            })],
            "field_not_facetable",
        ),
        (
            vec![
                GroupingInput::Facet(FacetInput {
                    field: "brand".to_owned(),
                    limit: None,
                    sort: None,
                }),
                GroupingInput::Facet(FacetInput {
                    field: "brand".to_owned(),
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
fn search_resolver_rejects_non_numeric_grouping_aggregations() {
    let context = context();
    let grouping = GroupingRequestIr {
        root: GroupingAllIr {
            operations: vec![GroupingOperationIr::Output(vec![GroupingOutputIr {
                label: None,
                expression: GroupingExpressionIr::Aggregation(Box::new(
                    GroupingAggregationIr::Avg(GroupingExpressionIr::Attribute("brand".to_owned())),
                )),
            }])],
            label: None,
        },
        continuations: Vec::new(),
    };

    let error = resolve_search(
        &context,
        match_all_search(vec![GroupingInput::Structured(grouping)]),
    )
    .expect_err("string aggregation expressions must be rejected in the canonical resolver");
    assert_eq!(error.details[0].code, "field_not_numeric");
}

#[test]
fn search_resolver_owns_portable_grouping_function_and_quantile_validation() {
    let context = context();
    let invalid_groupings = [
        (
            GroupingExpressionIr::Function {
                name: "not_supported".to_owned(),
                arguments: Vec::new(),
            },
            "unsupported_grouping_function",
        ),
        (
            GroupingExpressionIr::Aggregation(Box::new(GroupingAggregationIr::Quantiles {
                quantiles: vec![-12.0, f64::NAN, 500.0],
                expression: GroupingExpressionIr::Attribute("price".to_owned()),
            })),
            "invalid_quantile",
        ),
    ];

    for (expression, detail_code) in invalid_groupings {
        let grouping = GroupingRequestIr {
            root: GroupingAllIr {
                operations: vec![GroupingOperationIr::Group(expression)],
                label: None,
            },
            continuations: Vec::new(),
        };
        let error = resolve_search(
            &context,
            match_all_search(vec![GroupingInput::Structured(grouping)]),
        )
        .expect_err("portable grouping invariants must be enforced centrally");
        assert_eq!(error.details[0].code, detail_code);
    }
}

#[test]
fn tensor_patch_resolution_preserves_tensor_values_and_rejects_scalars() {
    let definition = IndexDefinitionIr::new(
        None,
        vec![FieldIr::new(
            FieldName::parse("embedding").unwrap(),
            DataTypeIr::Tensor {
                type_spec: "tensor<float>(x[2])".to_owned(),
            },
            None,
            FieldCapabilities::new(false, false, true, false, false),
        )],
        Vec::new(),
        RankingPolicyIr::DefaultRelevanceV1,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    )
    .unwrap();
    let context = IndexContext::from_definition_ir(definition);
    let tensor = RequestValueIr::Tensor(RequestTensorIr {
        type_spec: "tensor<float>(x[2])".to_owned(),
        payload: RequestTensorPayloadIr::Dense(vec![1.0, 0.0]),
    });
    let patch = resolve_patch(
        &context,
        "doc-1",
        &[PatchOperationInput::Set {
            field: "embedding".to_owned(),
            value: tensor.clone(),
        }],
    )
    .expect("dense tensor patch resolves");
    assert_eq!(
        patch.operations(),
        &[PatchOperationIr::Set {
            field: FieldName::parse("embedding").unwrap(),
            value: FieldValueIr::Tensor(match tensor {
                RequestValueIr::Tensor(value) => value,
                _ => unreachable!(),
            }),
        }]
    );

    let error = resolve_patch(
        &context,
        "doc-1",
        &[PatchOperationInput::Set {
            field: "embedding".to_owned(),
            value: RequestValueIr::Float64(1.0),
        }],
    )
    .expect_err("scalar tensor patch unexpectedly resolved");
    assert_eq!(error.details[0].code, "type_mismatch");
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
        RankingPolicyIr::DefaultRelevanceV1,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    )
    .unwrap();
    definition.set_schema_fields(vec![FieldIr::new(
        FieldName::parse("derived").unwrap(),
        DataTypeIr::Primitive(PrimitiveDataTypeIr::Int64),
        None,
        FieldCapabilities::new(true, true, true, true, false),
    )]);
    let bytes = crate::index_definition_ir_to_api(&definition);
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
        RankingPolicyIr::DefaultRelevanceV1,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    )
    .unwrap();
    definition.set_imported_fields(vec![ImportedFieldDefinitionIr {
        reference_field: FieldName::parse("parent").unwrap(),
        target_field: "name".to_owned(),
        alias: FieldName::parse("parent_name").unwrap(),
        properties: BTreeMap::new(),
    }]);
    let bytes = crate::index_definition_ir_to_api(&definition);
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
        RankingPolicyIr::DefaultRelevanceV1,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
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
    let bytes = crate::index_definition_ir_to_api(&invalid);
    let error = crate::index_definition_to_ir(
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
fn v2_query_variant_malformed_payloads_are_rejected_explicitly() {
    fn bare_expression(kind: api::QueryExprKind) -> Vec<u8> {
        let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
        macro_rules! empty_payload {
            ($table:ident, $args:ident) => {
                api::$table::create(&mut builder, &api::$args::default()).as_union_value()
            };
        }
        let payload = match kind {
            api::QueryExprKind::TermQuery => empty_payload!(TermQuery, TermQueryArgs),
            api::QueryExprKind::QueryContains => {
                empty_payload!(QueryContains, QueryContainsArgs)
            }
            api::QueryExprKind::QueryAnd => empty_payload!(QueryAnd, QueryAndArgs),
            api::QueryExprKind::QueryOr => empty_payload!(QueryOr, QueryOrArgs),
            api::QueryExprKind::QueryNot => empty_payload!(QueryNot, QueryNotArgs),
            api::QueryExprKind::QueryTrue => empty_payload!(QueryTrue, QueryTrueArgs),
            api::QueryExprKind::QueryFalse => empty_payload!(QueryFalse, QueryFalseArgs),
            api::QueryExprKind::QueryComparison => {
                empty_payload!(QueryComparison, QueryComparisonArgs)
            }
            api::QueryExprKind::QueryRange => empty_payload!(QueryRange, QueryRangeArgs),
            api::QueryExprKind::QueryIn => empty_payload!(QueryIn, QueryInArgs),
            api::QueryExprKind::QueryPhrase => empty_payload!(QueryPhrase, QueryPhraseArgs),
            api::QueryExprKind::QueryNear => empty_payload!(QueryNear, QueryNearArgs),
            api::QueryExprKind::QueryONear => empty_payload!(QueryONear, QueryONearArgs),
            api::QueryExprKind::QuerySameElement => {
                empty_payload!(QuerySameElement, QuerySameElementArgs)
            }
            api::QueryExprKind::QueryEquiv => empty_payload!(QueryEquiv, QueryEquivArgs),
            api::QueryExprKind::QueryAlternatives => {
                empty_payload!(QueryAlternatives, QueryAlternativesArgs)
            }
            api::QueryExprKind::QueryFuzzy => empty_payload!(QueryFuzzy, QueryFuzzyArgs),
            api::QueryExprKind::QueryMatches => empty_payload!(QueryMatches, QueryMatchesArgs),
            api::QueryExprKind::QueryText => empty_payload!(QueryText, QueryTextArgs),
            api::QueryExprKind::QueryUserInput => {
                empty_payload!(QueryUserInput, QueryUserInputArgs)
            }
            api::QueryExprKind::QueryUserQuery => {
                empty_payload!(QueryUserQuery, QueryUserQueryArgs)
            }
            api::QueryExprKind::QueryDotProduct => {
                empty_payload!(QueryDotProduct, QueryDotProductArgs)
            }
            api::QueryExprKind::QueryWeightedSet => {
                empty_payload!(QueryWeightedSet, QueryWeightedSetArgs)
            }
            api::QueryExprKind::QueryWand => empty_payload!(QueryWand, QueryWandArgs),
            api::QueryExprKind::QueryWeakAnd => empty_payload!(QueryWeakAnd, QueryWeakAndArgs),
            api::QueryExprKind::QueryRank => empty_payload!(QueryRank, QueryRankArgs),
            api::QueryExprKind::QueryLabeled => empty_payload!(QueryLabeled, QueryLabeledArgs),
            api::QueryExprKind::QueryNonEmpty => {
                empty_payload!(QueryNonEmpty, QueryNonEmptyArgs)
            }
            api::QueryExprKind::QueryGeoLocation => {
                empty_payload!(QueryGeoLocation, QueryGeoLocationArgs)
            }
            api::QueryExprKind::QueryGeoBoundingBox => {
                empty_payload!(QueryGeoBoundingBox, QueryGeoBoundingBoxArgs)
            }
            api::QueryExprKind::QueryNearestNeighbor => {
                empty_payload!(QueryNearestNeighbor, QueryNearestNeighborArgs)
            }
            api::QueryExprKind::QueryPredicate => {
                empty_payload!(QueryPredicate, QueryPredicateArgs)
            }
            _ => unreachable!("unknown QueryExprKind in test fixture"),
        };
        let expression = api::QueryExpr::create(
            &mut builder,
            &api::QueryExprArgs {
                expr_type: kind,
                expr: Some(payload),
                annotations: None,
            },
        );
        builder.finish(expression, None);
        builder.finished_data().to_vec()
    }

    fn contains_expression() -> Vec<u8> {
        let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
        let field_name = builder.create_string("brand");
        let field = api::QueryFieldRef::create(
            &mut builder,
            &api::QueryFieldRefArgs {
                path: Some(field_name),
                element_index: None,
            },
        );
        let term_value = builder.create_string("Acme");
        let term_value = api::StringValue::create(
            &mut builder,
            &api::StringValueArgs {
                value: Some(term_value),
            },
        );
        let term_value = api::Value::create(
            &mut builder,
            &api::ValueArgs {
                kind_type: api::ValueKind::StringValue,
                kind: Some(term_value.as_union_value()),
            },
        );
        let term = api::TermQuery::create(
            &mut builder,
            &api::TermQueryArgs {
                value: Some(term_value),
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
        builder.finished_data().to_vec()
    }

    fn boolean_expression(kind: api::QueryExprKind) -> Vec<u8> {
        let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
        let child = {
            let field_name = builder.create_string("brand");
            let field = api::QueryFieldRef::create(
                &mut builder,
                &api::QueryFieldRefArgs {
                    path: Some(field_name),
                    element_index: None,
                },
            );
            let value = builder.create_string("Acme");
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
            let term =
                api::TermQuery::create(&mut builder, &api::TermQueryArgs { value: Some(value) });
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
            api::QueryExpr::create(
                &mut builder,
                &api::QueryExprArgs {
                    expr_type: api::QueryExprKind::QueryContains,
                    expr: Some(contains.as_union_value()),
                    annotations: None,
                },
            )
        };
        let clauses = builder.create_vector(&[child]);
        let expression = match kind {
            api::QueryExprKind::QueryAnd => {
                let value = api::QueryAnd::create(
                    &mut builder,
                    &api::QueryAndArgs {
                        clauses: Some(clauses),
                    },
                );
                api::QueryExpr::create(
                    &mut builder,
                    &api::QueryExprArgs {
                        expr_type: kind,
                        expr: Some(value.as_union_value()),
                        annotations: None,
                    },
                )
            }
            api::QueryExprKind::QueryOr => {
                let value = api::QueryOr::create(
                    &mut builder,
                    &api::QueryOrArgs {
                        clauses: Some(clauses),
                    },
                );
                api::QueryExpr::create(
                    &mut builder,
                    &api::QueryExprArgs {
                        expr_type: kind,
                        expr: Some(value.as_union_value()),
                        annotations: None,
                    },
                )
            }
            api::QueryExprKind::QueryNot => {
                let value = api::QueryNot::create(
                    &mut builder,
                    &api::QueryNotArgs {
                        clause: Some(child),
                    },
                );
                api::QueryExpr::create(
                    &mut builder,
                    &api::QueryExprArgs {
                        expr_type: kind,
                        expr: Some(value.as_union_value()),
                        annotations: None,
                    },
                )
            }
            _ => unreachable!(),
        };
        builder.finish(expression, None);
        builder.finished_data().to_vec()
    }

    let mut query_context = context();
    query_context.definition.fields[0].capabilities.searchable = true;
    let supported = [
        ("QueryContains", contains_expression()),
        ("QueryAnd", boolean_expression(api::QueryExprKind::QueryAnd)),
        ("QueryOr", boolean_expression(api::QueryExprKind::QueryOr)),
        ("QueryNot", boolean_expression(api::QueryExprKind::QueryNot)),
    ];
    for (name, bytes) in &supported {
        let expression = crate::__flatbuffers::root::<api::QueryExpr>(bytes).unwrap();
        assert!(
            query_context
                .typed_query_to_ir(expression, "query", None)
                .is_ok(),
            "{name} should be in the characterized supported subset"
        );
    }

    let unsupported = [
        ("TermQuery", api::QueryExprKind::TermQuery),
        ("QueryTrue", api::QueryExprKind::QueryTrue),
        ("QueryFalse", api::QueryExprKind::QueryFalse),
        ("QueryComparison", api::QueryExprKind::QueryComparison),
        ("QueryRange", api::QueryExprKind::QueryRange),
        ("QueryIn", api::QueryExprKind::QueryIn),
        ("QueryPhrase", api::QueryExprKind::QueryPhrase),
        ("QueryNear", api::QueryExprKind::QueryNear),
        ("QueryONear", api::QueryExprKind::QueryONear),
        ("QuerySameElement", api::QueryExprKind::QuerySameElement),
        ("QueryEquiv", api::QueryExprKind::QueryEquiv),
        ("QueryAlternatives", api::QueryExprKind::QueryAlternatives),
        ("QueryFuzzy", api::QueryExprKind::QueryFuzzy),
        ("QueryMatches", api::QueryExprKind::QueryMatches),
        ("QueryText", api::QueryExprKind::QueryText),
        ("QueryUserInput", api::QueryExprKind::QueryUserInput),
        ("QueryUserQuery", api::QueryExprKind::QueryUserQuery),
        ("QueryDotProduct", api::QueryExprKind::QueryDotProduct),
        ("QueryWeightedSet", api::QueryExprKind::QueryWeightedSet),
        ("QueryWand", api::QueryExprKind::QueryWand),
        ("QueryWeakAnd", api::QueryExprKind::QueryWeakAnd),
        ("QueryRank", api::QueryExprKind::QueryRank),
        ("QueryLabeled", api::QueryExprKind::QueryLabeled),
        ("QueryNonEmpty", api::QueryExprKind::QueryNonEmpty),
        ("QueryGeoLocation", api::QueryExprKind::QueryGeoLocation),
        (
            "QueryGeoBoundingBox",
            api::QueryExprKind::QueryGeoBoundingBox,
        ),
        (
            "QueryNearestNeighbor",
            api::QueryExprKind::QueryNearestNeighbor,
        ),
        ("QueryPredicate", api::QueryExprKind::QueryPredicate),
    ];
    assert_eq!(supported.len() + unsupported.len(), 32);
    for (name, kind) in unsupported {
        let bytes = bare_expression(kind);
        let expression = crate::__flatbuffers::root::<api::QueryExpr>(&bytes).unwrap();
        if let Err(error) = query_context.typed_query_to_ir(expression, "query", None) {
            assert!(
                error
                    .details
                    .iter()
                    .all(|detail| detail.code != "unsupported_query_expression"),
                "{name} must be rejected by validation, not an unsupported-variant fallback"
            );
        }
    }
}

#[test]
fn every_v2_query_variant_round_trips_through_typed_ir() {
    fn field(name: &str) -> QueryFieldRefIr {
        QueryFieldRefIr {
            path: FieldName::parse(name).unwrap(),
            element_index: Some(2),
        }
    }
    fn leaf() -> QueryExprIr {
        QueryExprIr {
            kind: QueryExprKindIr::True,
            annotations: BTreeMap::new(),
        }
    }
    fn term() -> QueryExprIr {
        QueryExprIr {
            kind: QueryExprKindIr::Term {
                value: RequestValueIr::String("Acme".to_owned()),
            },
            annotations: BTreeMap::new(),
        }
    }
    fn annotated(kind: QueryExprKindIr) -> QueryIr {
        let annotations = match &kind {
            QueryExprKindIr::Wand { .. } => BTreeMap::from([
                ("targetNumHits".to_owned(), RequestValueIr::UInt64(100)),
                ("scoreThreshold".to_owned(), RequestValueIr::Int64(0)),
                (
                    "thresholdBoostFactor".to_owned(),
                    RequestValueIr::Float64(1.0),
                ),
            ]),
            QueryExprKindIr::NearestNeighbor { .. } => BTreeMap::from([
                ("targetNumHits".to_owned(), RequestValueIr::UInt64(100)),
                ("approximate".to_owned(), RequestValueIr::Bool(true)),
                (
                    "exploreAdditionalHits".to_owned(),
                    RequestValueIr::UInt64(0),
                ),
            ]),
            QueryExprKindIr::SameElement(_) => BTreeMap::new(),
            _ => BTreeMap::new(),
        };
        QueryIr::Typed(Box::new(QueryExprIr { kind, annotations }))
    }
    fn request(query: QueryIr) -> SearchRequestIr {
        let mut request = SearchRequestIr::new(
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
        );
        request.inputs.insert(
            "q".to_owned(),
            QueryInputIr::Literal(RequestValueIr::Tensor(RequestTensorIr {
                type_spec: "tensor<float>(x[2])".to_owned(),
                payload: RequestTensorPayloadIr::Dense(vec![1.0, 0.0]),
            })),
        );
        request
    }

    let cases = vec![
        (
            "TermQuery",
            QueryExprKindIr::Term {
                value: RequestValueIr::String("x".to_owned()),
            },
        ),
        (
            "QueryContains",
            QueryExprKindIr::Contains {
                field: field("brand"),
                item: Box::new(term()),
            },
        ),
        ("QueryAnd", QueryExprKindIr::And(vec![leaf()])),
        ("QueryOr", QueryExprKindIr::Or(vec![leaf()])),
        ("QueryNot", QueryExprKindIr::Not(Box::new(leaf()))),
        ("QueryTrue", QueryExprKindIr::True),
        ("QueryFalse", QueryExprKindIr::False),
        (
            "QueryComparison",
            QueryExprKindIr::Comparison {
                field: field("price"),
                op: ComparisonOp::Gt,
                value: RequestValueIr::Int64(3),
            },
        ),
        (
            "QueryRange",
            QueryExprKindIr::Range {
                field: field("price"),
                lower: QueryRangeBoundIr::Finite(RequestValueIr::Int64(1)),
                upper: QueryRangeBoundIr::PositiveInfinity,
                include_lower: Some(true),
                include_upper: Some(false),
            },
        ),
        (
            "QueryIn",
            QueryExprKindIr::In {
                field: field("brand"),
                values: vec![RequestValueIr::String("Acme".to_owned())],
            },
        ),
        ("QueryPhrase", QueryExprKindIr::Phrase(vec![term()])),
        (
            "QueryNear",
            QueryExprKindIr::Near {
                children: vec![term()],
                distance: 2,
            },
        ),
        (
            "QueryONear",
            QueryExprKindIr::ONear {
                children: vec![term()],
                distance: 2,
            },
        ),
        (
            "QuerySameElement",
            QueryExprKindIr::SameElement(vec![term(), term()]),
        ),
        ("QueryEquiv", QueryExprKindIr::Equiv(vec![term(), term()])),
        (
            "QueryAlternatives",
            QueryExprKindIr::Alternatives(vec![QueryAlternativeIr {
                query: term(),
                exactness: Some(0.75),
            }]),
        ),
        (
            "QueryFuzzy",
            QueryExprKindIr::Fuzzy {
                value: RequestValueIr::String("acm".to_owned()),
                max_edit_distance: 2,
                prefix_length: 0,
                prefix: false,
            },
        ),
        (
            "QueryMatches",
            QueryExprKindIr::Matches {
                field: field("brand"),
                pattern: "A.*".to_owned(),
            },
        ),
        (
            "QueryText",
            QueryExprKindIr::Text {
                text: "Acme".to_owned(),
            },
        ),
        (
            "QueryUserInput",
            QueryExprKindIr::UserInput {
                value: "query".to_owned(),
                default_index: None,
                target_hits: 100,
            },
        ),
        (
            "QueryUserQuery",
            QueryExprKindIr::UserQuery {
                default_index: None,
                target_hits: 100,
            },
        ),
        (
            "QueryDotProduct",
            QueryExprKindIr::DotProduct {
                field: field("price"),
                items: vec![WeightedQueryItemIr {
                    key: RequestValueIr::Int64(7),
                    weight: 2,
                }],
            },
        ),
        (
            "QueryWeightedSet",
            QueryExprKindIr::WeightedSet {
                field: field("brand"),
                items: vec![WeightedQueryItemIr {
                    key: RequestValueIr::String("x".to_owned()),
                    weight: 2,
                }],
            },
        ),
        (
            "QueryWand",
            QueryExprKindIr::Wand {
                field: field("brand"),
                items: vec![WeightedQueryItemIr {
                    key: RequestValueIr::String("x".to_owned()),
                    weight: 2,
                }],
            },
        ),
        (
            "QueryWeakAnd",
            QueryExprKindIr::WeakAnd {
                children: vec![term()],
                target_hits: 100,
            },
        ),
        (
            "QueryRank",
            QueryExprKindIr::Rank {
                matching: Box::new(term()),
                ranking: vec![leaf()],
            },
        ),
        (
            "QueryLabeled",
            QueryExprKindIr::Labeled {
                query: Box::new(term()),
                label: "my_label".to_owned(),
                score: 1.25,
            },
        ),
        ("QueryNonEmpty", QueryExprKindIr::NonEmpty(Box::new(term()))),
        (
            "QueryGeoLocation",
            QueryExprKindIr::GeoLocation {
                field: field("position"),
                latitude: 59.9,
                longitude: 10.7,
                radius: "10 km".to_owned(),
            },
        ),
        (
            "QueryGeoBoundingBox",
            QueryExprKindIr::GeoBoundingBox {
                field: field("position"),
                south: 59.0,
                west: 10.0,
                north: 60.0,
                east: 11.0,
            },
        ),
        (
            "QueryNearestNeighbor",
            QueryExprKindIr::NearestNeighbor {
                document_field: field("embedding"),
                query_tensor: "q".to_owned(),
            },
        ),
        (
            "QueryPredicate",
            QueryExprKindIr::Predicate {
                field: field("access"),
                features: BTreeMap::from([(
                    String::from("hobby"),
                    RequestValueIr::String("ski".to_owned()),
                )]),
                range_features: BTreeMap::new(),
            },
        ),
    ];
    assert_eq!(cases.len(), 32);

    let mut definition = context().definition.clone();
    definition.fields[0].capabilities.searchable = true;
    definition.fields.extend([
        FieldIr::new(
            FieldName::parse("position").unwrap(),
            DataTypeIr::Position,
            None,
            FieldCapabilities::new(false, true, true, false, false),
        ),
        FieldIr::new(
            FieldName::parse("embedding").unwrap(),
            DataTypeIr::Tensor {
                type_spec: "tensor<float>(x[2])".to_owned(),
            },
            None,
            FieldCapabilities::new(false, true, true, false, false),
        ),
        FieldIr::new(
            FieldName::parse("access").unwrap(),
            DataTypeIr::Predicate,
            None,
            FieldCapabilities::new(false, true, true, false, false),
        ),
    ]);
    let query_context = IndexContext::from_definition_ir(definition);
    for (name, kind) in cases {
        let expected = annotated(kind);
        let expected = if matches!(
            name,
            "TermQuery"
                | "QueryPhrase"
                | "QueryNear"
                | "QueryONear"
                | "QuerySameElement"
                | "QueryEquiv"
                | "QueryAlternatives"
                | "QueryFuzzy"
                | "QueryText"
                | "QueryUserInput"
                | "QueryWeakAnd"
                | "QueryRank"
                | "QueryLabeled"
                | "QueryNonEmpty"
        ) {
            let QueryIr::Typed(inner) = expected else {
                unreachable!()
            };
            QueryIr::Typed(Box::new(QueryExprIr {
                kind: QueryExprKindIr::Contains {
                    field: field("brand"),
                    item: inner,
                },
                annotations: BTreeMap::new(),
            }))
        } else {
            expected
        };
        let bytes = crate::search_request_ir_to_api(&request(expected.clone()));
        let api_request = crate::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
        let actual = query_context.search_to_ir(api_request).unwrap();
        assert_eq!(
            actual.query(),
            &expected,
            "{name} lost typed query semantics"
        );
    }
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
        let bytes = crate::search_request_ir_to_api(&request(expected.clone()));
        let api_request = crate::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
        let actual = query_context.search_to_ir(api_request).unwrap();
        assert_eq!(actual.query(), &expected, "{name} was not resolved in IR");
    }
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
    let bytes = crate::search_request_ir_to_api(&request);
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

#[test]
fn typed_query_special_annotations_are_resolved_and_validated_in_ir() {
    fn expression(
        kind: QueryExprKindIr,
        annotations: BTreeMap<String, RequestValueIr>,
    ) -> QueryExprIr {
        QueryExprIr { kind, annotations }
    }

    let wand = QueryExprKindIr::Wand {
        field: QueryFieldRefIr {
            path: FieldName::parse("brand").unwrap(),
            element_index: None,
        },
        items: vec![WeightedQueryItemIr {
            key: RequestValueIr::String("acme".to_owned()),
            weight: 1,
        }],
    };
    let mut normalized = expression(wand.clone(), BTreeMap::new());
    normalize_typed_query(&mut normalized, "query").expect("WAND defaults are portable");
    assert_eq!(
        normalized.annotations.get("targetNumHits"),
        Some(&RequestValueIr::UInt64(100))
    );
    assert_eq!(
        normalized.annotations.get("scoreThreshold"),
        Some(&RequestValueIr::Int64(0))
    );
    assert_eq!(
        normalized.annotations.get("thresholdBoostFactor"),
        Some(&RequestValueIr::Float64(1.0))
    );

    let mut conflicting_wand = expression(
        wand.clone(),
        BTreeMap::from([
            ("targetNumHits".to_owned(), RequestValueIr::UInt64(10)),
            ("totalTargetHits".to_owned(), RequestValueIr::UInt64(10)),
        ]),
    );
    assert_eq!(
        normalize_typed_query(&mut conflicting_wand, "query")
            .unwrap_err()
            .details[0]
            .code,
        "invalid_typed_query_annotation"
    );

    for annotations in [
        BTreeMap::from([("targetNumHits".to_owned(), RequestValueIr::UInt64(0))]),
        BTreeMap::from([("unknownWandOption".to_owned(), RequestValueIr::Bool(true))]),
        BTreeMap::from([(
            "thresholdBoostFactor".to_owned(),
            RequestValueIr::Float64(0.0),
        )]),
        BTreeMap::from([(
            "thresholdBoostFactor".to_owned(),
            RequestValueIr::Float64(-1.0),
        )]),
    ] {
        let mut query = expression(wand.clone(), annotations);
        assert!(normalize_typed_query(&mut query, "query").is_err());
    }

    let mut fuzzy_with_locked_prefix = expression(
        QueryExprKindIr::Fuzzy {
            value: RequestValueIr::String("Acme".to_owned()),
            max_edit_distance: 0,
            prefix_length: 0,
            prefix: false,
        },
        BTreeMap::from([
            ("maxEditDistance".to_owned(), RequestValueIr::UInt64(0)),
            ("prefixLength".to_owned(), RequestValueIr::UInt64(2)),
        ]),
    );
    let error = normalize_typed_query(&mut fuzzy_with_locked_prefix, "query")
        .expect_err("prefix locking with zero edit distance must be rejected in IR");
    assert_eq!(error.details[0].code, "invalid_typed_query_annotation");
    assert_eq!(
        error.details[0].path.as_deref(),
        Some("query.annotations.prefixLength")
    );

    for kind in [
        QueryExprKindIr::Term {
            value: RequestValueIr::String("term".to_owned()),
        },
        QueryExprKindIr::Fuzzy {
            value: RequestValueIr::String("term".to_owned()),
            max_edit_distance: 2,
            prefix_length: 0,
            prefix: false,
        },
        QueryExprKindIr::Near {
            children: Vec::new(),
            distance: 2,
        },
    ] {
        let mut query = expression(
            kind,
            BTreeMap::from([("unknown".to_owned(), RequestValueIr::Bool(true))]),
        );
        let error = normalize_typed_query(&mut query, "query")
            .expect_err("unknown annotations must be rejected by the canonical resolver");
        assert_eq!(
            error.details[0].code,
            "typed_query_annotations_not_supported"
        );
    }

    let nearest_neighbor = || QueryExprKindIr::NearestNeighbor {
        document_field: QueryFieldRefIr {
            path: FieldName::parse("embedding").unwrap(),
            element_index: None,
        },
        query_tensor: "q".to_owned(),
    };
    let mut normalized = expression(nearest_neighbor(), BTreeMap::new());
    normalize_typed_query(&mut normalized, "query").expect("NN defaults are portable");
    assert_eq!(
        normalized.annotations.get("targetNumHits"),
        Some(&RequestValueIr::UInt64(100))
    );
    assert_eq!(
        normalized.annotations.get("approximate"),
        Some(&RequestValueIr::Bool(true))
    );
    assert_eq!(
        normalized.annotations.get("exploreAdditionalHits"),
        Some(&RequestValueIr::UInt64(0))
    );

    for annotations in [
        BTreeMap::from([("targetNumHits".to_owned(), RequestValueIr::UInt64(0))]),
        BTreeMap::from([
            ("targetNumHits".to_owned(), RequestValueIr::UInt64(10)),
            ("totalTargetHits".to_owned(), RequestValueIr::UInt64(10)),
        ]),
        BTreeMap::from([(
            "distanceThreshold".to_owned(),
            RequestValueIr::Float64(-1.0),
        )]),
        BTreeMap::from([(
            "distanceThreshold".to_owned(),
            RequestValueIr::NonFiniteFloat(RequestNonFiniteFloatIr::NaN),
        )]),
        BTreeMap::from([("unknownNnOption".to_owned(), RequestValueIr::Bool(true))]),
    ] {
        let mut query = expression(nearest_neighbor(), annotations);
        assert!(normalize_typed_query(&mut query, "query").is_err());
    }
    let mut valid = expression(
        nearest_neighbor(),
        BTreeMap::from([
            (
                "distanceThreshold".to_owned(),
                RequestValueIr::Float64(0.25),
            ),
            (
                "exploreAdditionalHits".to_owned(),
                RequestValueIr::UInt64(50),
            ),
        ]),
    );
    normalize_typed_query(&mut valid, "query").expect("valid NN options");
    assert_eq!(
        valid.annotations.get("distanceThreshold"),
        Some(&RequestValueIr::Float64(0.25))
    );
    assert_eq!(
        valid.annotations.get("exploreAdditionalHits"),
        Some(&RequestValueIr::UInt64(50))
    );

    let mut empty_query_tensor = expression(
        QueryExprKindIr::NearestNeighbor {
            document_field: QueryFieldRefIr {
                path: FieldName::parse("embedding").unwrap(),
                element_index: None,
            },
            query_tensor: String::new(),
        },
        BTreeMap::new(),
    );
    normalize_typed_query(&mut empty_query_tensor, "query")
        .expect("empty query tensor names are validated with the schema");
    let error = context()
        .validate_typed_query(&empty_query_tensor, None, None, "query")
        .expect_err("empty nearest-neighbor tensor names must fail in IR");
    assert_eq!(error.details[0].code, "missing_value");

    let mut empty_weighted_key = expression(
        QueryExprKindIr::WeightedSet {
            field: QueryFieldRefIr {
                path: FieldName::parse("brand").unwrap(),
                element_index: None,
            },
            items: vec![WeightedQueryItemIr {
                key: RequestValueIr::String(String::new()),
                weight: 1,
            }],
        },
        BTreeMap::new(),
    );
    normalize_typed_query(&mut empty_weighted_key, "query")
        .expect("weighted key legality is schema validation");
    let error = context()
        .validate_typed_query(&empty_weighted_key, None, None, "query")
        .expect_err("empty weighted keys must fail in IR");
    assert_eq!(error.details[0].code, "invalid_typed_query");

    let same_element = QueryExprKindIr::SameElement(Vec::new());
    for annotations in [
        BTreeMap::from([("elementFilter".to_owned(), RequestValueIr::Bool(true))]),
        BTreeMap::from([(
            "unknownSameElementOption".to_owned(),
            RequestValueIr::Bool(true),
        )]),
    ] {
        let mut query = expression(same_element.clone(), annotations);
        assert!(normalize_typed_query(&mut query, "query").is_err());
    }
}

#[test]
fn typed_query_geo_and_predicate_validity_is_rejected_in_ir() {
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

    let mut definition = context().definition.clone();
    definition.fields.extend([
        FieldIr::new(
            FieldName::parse("position").unwrap(),
            DataTypeIr::Position,
            None,
            FieldCapabilities::new(false, true, true, false, false),
        ),
        FieldIr::new(
            FieldName::parse("access").unwrap(),
            DataTypeIr::Predicate,
            None,
            FieldCapabilities::new(false, true, true, false, false),
        ),
    ]);
    let context = IndexContext::from_definition_ir(definition);
    for (latitude, longitude) in [(91.0, 0.0), (-91.0, 0.0), (0.0, 181.0), (f64::NAN, 0.0)] {
        assert_eq!(
            error_code(
                &context,
                expression(QueryExprKindIr::GeoLocation {
                    field: field("position"),
                    latitude,
                    longitude,
                    radius: "1 km".to_owned(),
                }),
            ),
            if latitude.is_nan() {
                "non_finite_number"
            } else {
                "invalid_geo_coordinate"
            }
        );
    }
    for radius in [
        "garbage",
        "-1km",
        "0km",
        "NaNkm",
        "999999999999999999999999999999km",
    ] {
        assert_eq!(
            error_code(
                &context,
                expression(QueryExprKindIr::GeoLocation {
                    field: field("position"),
                    latitude: 0.0,
                    longitude: 0.0,
                    radius: radius.to_owned(),
                }),
            ),
            "invalid_geo_radius"
        );
    }
    for (south, west, north, east) in [(2.0, 0.0, 1.0, 1.0), (0.0, 2.0, 1.0, 1.0)] {
        assert_eq!(
            error_code(
                &context,
                expression(QueryExprKindIr::GeoBoundingBox {
                    field: field("position"),
                    south,
                    west,
                    north,
                    east,
                }),
            ),
            "invalid_geo_bounding_box"
        );
    }
    assert_eq!(
        error_code(
            &context,
            expression(QueryExprKindIr::Predicate {
                field: field("access"),
                features: BTreeMap::new(),
                range_features: BTreeMap::new(),
            }),
        ),
        "empty_typed_query"
    );
    for (features, range_features) in [
        (
            BTreeMap::from([("hobby".to_owned(), RequestValueIr::List(Vec::new()))]),
            BTreeMap::new(),
        ),
        (
            BTreeMap::from([("1".to_owned(), RequestValueIr::Object(BTreeMap::new()))]),
            BTreeMap::new(),
        ),
        (
            BTreeMap::new(),
            BTreeMap::from([("age".to_owned(), RequestValueIr::List(Vec::new()))]),
        ),
        (
            BTreeMap::new(),
            BTreeMap::from([("1".to_owned(), RequestValueIr::Object(BTreeMap::new()))]),
        ),
    ] {
        assert_eq!(
            error_code(
                &context,
                expression(QueryExprKindIr::Predicate {
                    field: field("access"),
                    features,
                    range_features,
                }),
            ),
            "empty_typed_query"
        );
    }
}
