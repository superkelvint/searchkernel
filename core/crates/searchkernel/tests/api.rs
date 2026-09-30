use searchkernel::{
    field, DocumentId, Error, ErrorCode, Facet, FacetSort, Field, Filter, GroupingAggregation,
    GroupingExpression, GroupingFilter, GroupingOrder, GroupingOutput, GroupingPlan, Index,
    IndexDefinition, Patch, Range, RankProfile, Ranking, Sort,
};
use searchkernel_ir::{
    Backend, FieldName, FilterIr, GetResult, GroupingAggregationIr, GroupingAllIr,
    GroupingExpressionIr, GroupingOperationIr, GroupingOutputIr, GroupingRequestIr, HighlightingIr,
    OperationIr, ProjectionIr, QueryInputIr, QueryIr, RequestValueIr, SearchRequestIr,
    SearchResult, TotalHits, TotalHitsRelation, Value,
};
use searchkernel_testkit::{ExpectedCall, RecordingBackend, ScriptedBackend, ScriptedResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

#[test]
fn public_schema_signatures_use_searchkernel_owned_types() {
    use searchkernel::{
        DataType, FieldSet, IndexConfig, IndexDefinition, RankPhase, RankProfile, SchemaValue,
    };

    let field = Field::tensor("embedding", "tensor<float>(x[3])");
    let _: &DataType = field.data_type();

    let profile = RankProfile::new("semantic");
    let _: Option<&RankPhase> = profile.first_phase.as_ref();

    let definition = IndexDefinition::new().field(field).rank_profile(profile);
    let _: &[FieldSet] = definition.fieldsets();
    let _: Option<IndexConfig> = None;
    let _: SchemaValue = SchemaValue::Null;
}

#[test]
fn query_model_and_yql_types_remain_public_at_the_crate_root() {
    use searchkernel::{QueryModelType, YqlBuilder};

    let _ = QueryModelType::default();
    fn accepts_yql_builder(_: Option<YqlBuilder>) {}
    accepts_yql_builder(None);
}

#[test]
fn grouping_order_variants_remain_source_compatible() {
    fn descending(order: GroupingOrder) -> bool {
        match order {
            GroupingOrder::Field(_, descending) | GroupingOrder::Aggregation(_, descending) => {
                descending
            }
        }
    }

    let field_order = GroupingOrder::Field(GroupingExpression::attribute("brand"), true);
    assert!(descending(field_order));

    let aggregation_order = GroupingOrder::Aggregation(GroupingAggregation::count(), false);
    assert!(!descending(aggregation_order));
}

#[test]
fn canonical_result_tree_types_are_public_to_downstream_crates() {
    use searchkernel::{ResultGroup, ResultHit, ResultLimits, ResultNode, ResultValue};

    fn inspect_hit(hit: &ResultHit) {
        let _: &DocumentId = &hit.id;
        let _: Option<f64> = hit.relevance;
        let _: Option<f64> = hit.score;
        let _: &BTreeMap<String, ResultValue> = &hit.fields;
        let _: &[searchkernel::HighlightResult] = &hit.highlights;
        let _: &Option<String> = &hit.source;
        let _: &[String] = &hit.types;
        let _: &BTreeMap<String, ResultValue> = &hit.match_features;
        let _: &BTreeMap<String, ResultValue> = &hit.summary_features;
        let _: &BTreeMap<String, ResultValue> = &hit.rank_features;
        let _: &[ResultValue] = &hit.sort_values;
    }

    fn inspect_limits(limits: &ResultLimits) {
        let _: &Option<ResultValue> = &limits.from;
        let _: &Option<ResultValue> = &limits.to;
    }

    fn inspect_group(group: &ResultGroup) {
        let _: &Option<String> = &group.id;
        let _: &Option<String> = &group.label;
        let _: &Option<ResultValue> = &group.value;
        let _: &BTreeMap<String, ResultValue> = &group.fields;
        let _: &[ResultNode] = &group.children;
        let _: &BTreeMap<String, String> = &group.continuations;
        let _: &Option<String> = &group.source;
        let _: &[String] = &group.types;
        let _: Option<f64> = group.relevance;
        let _: &Option<ResultLimits> = &group.limits;
        let _: &BTreeMap<String, ResultValue> = &group.metadata;
    }

    fn visit(node: &ResultNode) -> usize {
        match node {
            ResultNode::Hit(hit) => {
                inspect_hit(hit);
                1
            }
            ResultNode::Group(group) => {
                inspect_group(group);
                if let Some(limits) = &group.limits {
                    inspect_limits(limits);
                }
                group.children.iter().map(visit).sum()
            }
            _ => 0,
        }
    }

    let _ = inspect_hit as fn(&ResultHit);
    let _ = inspect_limits as fn(&ResultLimits);
    let _ = inspect_group as fn(&ResultGroup);
    let _ = visit as fn(&ResultNode) -> usize;
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct Product {
    title: String,
    brand: Option<String>,
    price: f64,
    in_stock: bool,
}

fn definition() -> IndexDefinition {
    IndexDefinition::new()
        .name("products")
        .field(Field::text("title"))
        .field(Field::keyword("brand"))
        .field(Field::float64("price"))
        .field(Field::bool("in_stock"))
        .rank_profile(RankProfile::new("product_search"))
        .rank_profile(RankProfile::new("bm25"))
}

fn index_with(backend: Arc<dyn Backend>) -> Index {
    Index::from_backend(definition(), backend).unwrap()
}

fn expected_search() -> SearchRequestIr {
    SearchRequestIr::new(
        QueryIr::MatchAll,
        vec![],
        None,
        vec![],
        20,
        0,
        ProjectionIr::new(vec![
            FieldName::parse("title").unwrap(),
            FieldName::parse("brand").unwrap(),
            FieldName::parse("price").unwrap(),
            FieldName::parse("in_stock").unwrap(),
        ]),
        HighlightingIr::Disabled,
        None,
        searchkernel_ir::PrefixPolicy::Disabled,
        searchkernel_ir::TypoPolicy::Disabled,
    )
}

fn expected_search_with_facet(field: &str) -> SearchRequestIr {
    expected_search().with_grouping(vec![GroupingRequestIr {
        root: GroupingAllIr {
            operations: vec![
                GroupingOperationIr::Group(GroupingExpressionIr::Attribute(field.into())),
                GroupingOperationIr::Each(GroupingAllIr {
                    operations: vec![
                        GroupingOperationIr::Max(Some(10)),
                        GroupingOperationIr::Order(vec![GroupingExpressionIr::Negative(Box::new(
                            GroupingExpressionIr::Aggregation(Box::new(
                                GroupingAggregationIr::Count,
                            )),
                        ))]),
                        GroupingOperationIr::Output(vec![GroupingOutputIr {
                            label: None,
                            expression: GroupingExpressionIr::Aggregation(Box::new(
                                GroupingAggregationIr::Count,
                            )),
                        }]),
                    ],
                    label: None,
                }),
            ],
            label: Some(field.to_owned()),
        },
        continuations: Vec::new(),
    }])
}

#[test]
fn fluent_requests_cross_the_core_boundary() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Index>();

    let backend = Arc::new(RecordingBackend::new());
    let index = index_with(backend.clone());
    let product = Product {
        title: "wireless headphones".into(),
        brand: Some("Acme".into()),
        price: 449.0,
        in_stock: true,
    };

    assert!(index.upsert("sku-42", &product).is_err());
    let patch = Patch::new()
        .set("price", 450.0)
        .set("in_stock", true)
        .unset("brand");
    assert!(index.patch("sku-42", patch).is_err());
    assert!(index
        .search("wireless headphones")
        .filter(field("in_stock").eq(true))
        .filter(field("price").lte(500.0))
        .sort(Sort::desc("price").missing_last())
        .facet(Facet::new("brand").limit(20).sort(FacetSort::CountDesc))
        .ranking(Ranking::bm25())
        .projection(vec!["title".to_owned(), "price".to_owned()])
        .highlight(vec!["title".to_owned()])
        .language("en-US")
        .limit(20)
        .offset(3)
        .execute::<Product>()
        .is_err());

    let calls = backend.calls();
    assert!(matches!(calls[0], OperationIr::Upsert(_)));
    assert!(matches!(calls[1], OperationIr::Patch(_)));
    let OperationIr::Patch(request) = &calls[1] else {
        unreachable!()
    };
    assert_eq!(request.operations().len(), 3);
    let OperationIr::Search(request) = &calls[2] else {
        unreachable!()
    };
    assert_eq!(
        request.query(),
        &QueryIr::Text("wireless headphones".into())
    );
    assert_eq!(request.limit(), 20);
    assert_eq!(request.offset(), 3);
    assert!(matches!(request.filter(), Some(FilterIr::And(_))));
    let [grouping] = request.grouping() else {
        panic!("facet convenience must compile to one grouping request");
    };
    assert!(matches!(
        grouping.root.operations.as_slice(),
        [
            searchkernel_ir::GroupingOperationIr::Group(
                searchkernel_ir::GroupingExpressionIr::Attribute(field)
            ),
            searchkernel_ir::GroupingOperationIr::Each(_)
        ] if field == "brand"
    ));
}

#[test]
fn update_builder_crosses_the_same_canonical_ir_boundary() {
    let backend = Arc::new(RecordingBackend::new());
    let index = index_with(backend.clone());

    assert!(index
        .update_builder("sku-42")
        .assign("title", "Updated headphones")
        .clear("brand")
        .increment("price", 5.0)
        .create(true)
        .execute()
        .is_err());

    let OperationIr::Update(request) = backend.last_call().expect("recorded update") else {
        panic!("expected update operation");
    };
    assert_eq!(request.id().as_str(), "sku-42");
    assert_eq!(request.create(), Some(true));
    assert_eq!(request.condition(), None);
    assert!(matches!(
        request.operations(),
        [
            searchkernel_ir::UpdateOperationIr::Assign { target, .. },
            searchkernel_ir::UpdateOperationIr::Clear { target: clear_target },
            searchkernel_ir::UpdateOperationIr::Arithmetic {
                target: arithmetic_target,
                operator: searchkernel_ir::UpdateArithmeticOperatorIr::Increment,
                operand: 5.0,
            },
        ] if target.as_str() == "title"
            && clear_target.as_str() == "brand"
            && arithmetic_target.as_str() == "price"
    ));

    for (builder, expected) in [
        (
            index.update_builder("sku-42").decrement("price", 2_i64),
            searchkernel_ir::UpdateArithmeticOperatorIr::Decrement,
        ),
        (
            index.update_builder("sku-42").multiply("price", 3_u32),
            searchkernel_ir::UpdateArithmeticOperatorIr::Multiply,
        ),
        (
            index.update_builder("sku-42").divide("price", 4.0_f64),
            searchkernel_ir::UpdateArithmeticOperatorIr::Divide,
        ),
    ] {
        assert!(builder.execute().is_err());
        let OperationIr::Update(request) = backend.last_call().expect("recorded arithmetic update")
        else {
            panic!("expected update operation");
        };
        assert!(matches!(
            request.operations(),
            [searchkernel_ir::UpdateOperationIr::Arithmetic { operator, .. }] if *operator == expected
        ));
    }
}

#[test]
fn update_builder_matches_portable_dispatcher_ir_for_full_executable_union() {
    let direct_backend = Arc::new(RecordingBackend::new());
    let index = index_with(direct_backend.clone());

    let field = |name: &str, kind| searchkernel_ir::RustFieldInput {
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
    };
    let definition =
        searchkernel_ir::resolve_rust_index_definition(searchkernel_ir::RustIndexDefinitionInput {
            name: Some("products".to_owned()),
            fields: vec![
                field("title", searchkernel_ir::RustFieldKind::Text),
                field("brand", searchkernel_ir::RustFieldKind::Keyword),
                field("price", searchkernel_ir::RustFieldKind::Float64),
                field("in_stock", searchkernel_ir::RustFieldKind::Bool),
            ],
            default_search_fields: None,
        })
        .expect("portable parity schema");
    let portable_backend = Arc::new(RecordingBackend::new());
    let dispatcher = searchkernel_dispatch::Dispatcher::from_backend(
        Arc::new(searchkernel_ir::IndexContext::from_definition_ir(
            definition,
        )),
        portable_backend.clone(),
    );

    let cases = [
        (
            "assign/create-absent",
            index.update_builder("sku-42").assign("brand", "Sony"),
            serde_json::json!({
                "target": {"path": "brand"},
                "operation_type": "AssignUpdate",
                "operation": {
                    "value": {"kind_type": "StringValue", "kind": {"value": "Sony"}}
                }
            }),
            None,
        ),
        (
            "clear/create-false",
            index.update_builder("sku-42").clear("brand").create(false),
            serde_json::json!({
                "target": {"path": "brand"},
                "operation_type": "ClearUpdate",
                "operation": {}
            }),
            Some(false),
        ),
        (
            "increment/create-true",
            index
                .update_builder("sku-42")
                .increment("price", 2.5_f64)
                .create(true),
            serde_json::json!({
                "target": {"path": "price"},
                "operation_type": "ArithmeticUpdate",
                "operation": {
                    "operator": "Increment",
                    "operand": {"kind_type": "Float64Value", "kind": {"value": 2.5}}
                }
            }),
            Some(true),
        ),
        (
            "decrement",
            index.update_builder("sku-42").decrement("price", 2.0_f64),
            serde_json::json!({
                "target": {"path": "price"},
                "operation_type": "ArithmeticUpdate",
                "operation": {
                    "operator": "Decrement",
                    "operand": {"kind_type": "Float64Value", "kind": {"value": 2.0}}
                }
            }),
            None,
        ),
        (
            "multiply",
            index.update_builder("sku-42").multiply("price", 3.0_f64),
            serde_json::json!({
                "target": {"path": "price"},
                "operation_type": "ArithmeticUpdate",
                "operation": {
                    "operator": "Multiply",
                    "operand": {"kind_type": "Float64Value", "kind": {"value": 3.0}}
                }
            }),
            None,
        ),
        (
            "divide",
            index.update_builder("sku-42").divide("price", 4.0_f64),
            serde_json::json!({
                "target": {"path": "price"},
                "operation_type": "ArithmeticUpdate",
                "operation": {
                    "operator": "Divide",
                    "operand": {"kind_type": "Float64Value", "kind": {"value": 4.0}}
                }
            }),
            None,
        ),
    ];

    for (name, builder, operation, create) in cases {
        assert!(
            builder.execute().is_err(),
            "{name}: recording backend returns an error"
        );
        let direct = direct_backend
            .last_call()
            .unwrap_or_else(|| panic!("{name}: direct Rust update IR"));

        let mut request_json = serde_json::json!({
            "protocol_version": 2,
            "payload_type": "SearchKernel_V2_UpdateRequest",
            "payload": {
                "id": "sku-42",
                "operations": [operation]
            }
        });
        if let Some(create) = create {
            request_json["payload"]["create"] = serde_json::json!({"value": create});
        }
        let request_json =
            serde_json::to_vec(&request_json).expect("serialize canonical portable update");
        let request = searchkernel_protocol::json::request_json_to_flatbuffer(&request_json)
            .unwrap_or_else(|error| panic!("{name}: canonical portable update request: {error}"));
        dispatcher.dispatch(&request).unwrap_or_else(|error| {
            panic!("{name}: dispatcher must encode backend errors: {error:?}")
        });
        let portable = portable_backend
            .last_call()
            .unwrap_or_else(|| panic!("{name}: portable dispatcher update IR"));

        assert_eq!(
            direct, portable,
            "{name}: direct and portable update IR differ"
        );
    }
}

#[test]
fn reusable_update_entry_point_remains_compatible() {
    let backend = Arc::new(RecordingBackend::new());
    let index = index_with(backend.clone());

    assert!(index
        .update(
            "sku-42",
            searchkernel::Update::new()
                .set("brand", "Sony")
                .increment("price", 2.5),
        )
        .is_err());

    let OperationIr::Update(request) = backend.last_call().expect("recorded reusable update")
    else {
        panic!("expected update operation");
    };
    assert_eq!(request.id().as_str(), "sku-42");
    assert_eq!(request.create(), None);
    assert!(matches!(
        request.operations(),
        [
            searchkernel_ir::UpdateOperationIr::Assign { target, .. },
            searchkernel_ir::UpdateOperationIr::Arithmetic {
                target: arithmetic_target,
                operator: searchkernel_ir::UpdateArithmeticOperatorIr::Increment,
                operand: 2.5,
            },
        ] if target.as_str() == "brand" && arithmetic_target.as_str() == "price"
    ));
}

#[test]
fn update_builder_defers_semantic_validation_to_the_canonical_resolver() {
    let backend = Arc::new(RecordingBackend::new());
    let index = index_with(backend.clone());

    let empty = index
        .update_builder("sku-42")
        .execute()
        .expect_err("empty update");
    assert_eq!(empty.code(), ErrorCode::InvalidArgument);
    assert_eq!(empty.details()[0].code, "empty_update");
    assert!(backend.calls().is_empty());

    let wrong_type = index
        .update_builder("sku-42")
        .increment("brand", 1_i64)
        .execute()
        .expect_err("arithmetic on a keyword field");
    assert_eq!(wrong_type.code(), ErrorCode::InvalidArgument);
    assert_eq!(
        wrong_type.details()[0].code,
        "arithmetic_requires_numeric_field"
    );
    assert!(backend.calls().is_empty());

    let duplicate = index
        .update_builder("sku-42")
        .set("price", 100.0)
        .unset("price")
        .execute()
        .expect_err("multiple operations on one target remain unsupported");
    assert_eq!(duplicate.code(), ErrorCode::Unsupported);
    assert_eq!(
        duplicate.details()[0].code,
        "multiple_updates_per_target_not_supported"
    );
    assert!(backend.calls().is_empty());

    assert!(index
        .update_builder("sku-42")
        .set("price", 100.0)
        .create(false)
        .execute()
        .is_err());
    let OperationIr::Update(request) = backend.last_call().expect("recorded create=false update")
    else {
        panic!("expected update operation");
    };
    assert_eq!(request.create(), Some(false));
}

#[test]
fn ranking_controls_cross_the_core_boundary() {
    let backend = Arc::new(RecordingBackend::new());
    let index = index_with(backend.clone());

    assert!(index
        .search("fox")
        .ranking(Ranking::named("product_search"))
        .query_input("boost", 2.0)
        .rank_property("source", "catalog")
        .execute::<Product>()
        .is_err());

    let OperationIr::Search(request) = backend.last_call().unwrap() else {
        unreachable!()
    };
    assert_eq!(request.ranking_request().profile, "product_search");
    assert_eq!(
        request.inputs().get("boost"),
        Some(&QueryInputIr::Literal(RequestValueIr::Float64(2.0)))
    );
    assert_eq!(
        request.ranking_request().properties.get("source"),
        Some(&"catalog".to_owned())
    );
}

#[test]
fn presence_and_explicit_zero_are_preserved() {
    let index = index_with(Arc::new(RecordingBackend::new()));
    let inspected = index
        .search("probe")
        .search_fields(["title"])
        .projection(std::iter::empty::<&str>())
        .highlight(std::iter::empty::<&str>())
        .limit(0)
        .offset(0)
        .inspect()
        .unwrap();
    assert_eq!(inspected.search_fields.len(), 1);
    assert!(inspected.projection.is_empty());
    assert_eq!(inspected.highlighting, Some(Vec::new()));
    assert_eq!(inspected.limit, 0);
    assert_eq!(inspected.offset, 0);
}

#[test]
fn browse_is_an_explicit_match_all_search() {
    let backend = Arc::new(RecordingBackend::new());
    let index = index_with(backend.clone());

    assert!(index.browse().execute::<Product>().is_err());
    let OperationIr::Search(request) = backend.last_call().unwrap() else {
        unreachable!()
    };
    assert_eq!(request.query(), &QueryIr::MatchAll);
}

#[test]
fn projected_get_reaches_the_portable_projection_request() {
    let backend = Arc::new(RecordingBackend::new());
    let index = index_with(backend.clone());

    assert!(index
        .get_with_projection::<serde_json::Value>("sku-42", ["title", "brand"])
        .is_err());
    let OperationIr::Get(request) = backend.last_call().unwrap() else {
        unreachable!()
    };
    assert_eq!(
        request
            .projection()
            .fields()
            .iter()
            .map(|field| field.as_str())
            .collect::<Vec<_>>(),
        ["title", "brand"]
    );
}

#[test]
fn describe_and_inspect_expose_effective_request_defaults() {
    let backend = Arc::new(RecordingBackend::new());
    let index = index_with(backend);

    let definition = index.describe().unwrap();
    assert_eq!(definition.index_name(), Some("products"));
    assert_eq!(definition.fields().len(), 4);
    assert_eq!(
        definition.configured_default_search_fields().unwrap()[0].field(),
        "title"
    );

    let inspected = index
        .search("wireless")
        .filter(field("in_stock").eq(true))
        .filter(field("price").lte(500.0))
        .limit(7)
        .inspect()
        .unwrap();
    assert_eq!(inspected.query, "wireless");
    assert_eq!(inspected.limit, 7);
    assert_eq!(inspected.search_fields.len(), 1);
    assert_eq!(inspected.search_fields[0].field(), "title");
    assert!(inspected.filter.is_some());
}

#[test]
fn analyze_uses_the_selected_field_and_returns_tokens() {
    let definition = IndexDefinition::new()
        .field(Field::text("title").analyzer(searchkernel::Analyzer::standard_v1()));
    let recording = Arc::new(RecordingBackend::new());
    let recording_index = Index::from_backend(definition.clone(), recording.clone()).unwrap();
    assert!(recording_index
        .analyze("Running with the foxes")
        .field("title")
        .execute()
        .is_err());
    let expected = recording.last_call().unwrap();

    let backend = Arc::new(ScriptedBackend::new([ExpectedCall::success(
        expected,
        ScriptedResult::Analyze(searchkernel_ir::AnalyzeResult {
            tokens: vec![searchkernel_ir::AnalyzedToken {
                surface: "Running".into(),
                term: "run".into(),
                position: 0,
                start_byte: 0,
                end_byte: 7,
            }],
        }),
    )]));
    let index = Index::from_backend(definition, backend.clone()).unwrap();
    let result = index
        .analyze("Running with the foxes")
        .field("title")
        .execute()
        .unwrap();
    assert_eq!(result.tokens[0].term, "run");
    backend.finish().unwrap();
}

#[test]
fn scripted_results_map_to_product_values_and_preserve_totals() {
    let expected = expected_search();
    let result = SearchResult {
        root: searchkernel_ir::ResultNode::Group(searchkernel_ir::ResultGroup::root(vec![
            searchkernel_ir::ResultNode::Hit(searchkernel_ir::Hit {
                id: "sku-42".into(),
                score: Some(2.5),
                fields: BTreeMap::from([(
                    String::from("title"),
                    Value::String("headphones".into()),
                )]),
                highlights: vec![],
                ..Default::default()
            }),
        ])),
        total_hits: TotalHits {
            value: 9,
            relation: TotalHitsRelation::LowerBound,
        },
        coverage: None,
        timing: None,
        errors: vec![],
        trace: None,
        metadata: BTreeMap::new(),
    };
    let backend = Arc::new(ScriptedBackend::new(vec![ExpectedCall::success(
        OperationIr::Search(expected),
        ScriptedResult::Search(Box::new(result)),
    )]));
    let index = index_with(backend.clone());
    let result = index.browse().execute::<serde_json::Value>().unwrap();
    assert_eq!(result.hits[0].id.as_str(), "sku-42");
    assert_eq!(result.hits[0].score, Some(2.5));
    assert_eq!(
        result.hits[0].document.as_ref().unwrap()["title"],
        "headphones"
    );
    assert_eq!(
        result.total_hits.relation,
        searchkernel::TotalHitsRelation::LowerBound
    );
    backend.finish().unwrap();
}

#[test]
fn get_missing_and_portable_errors_are_distinct() {
    let recording = Arc::new(RecordingBackend::new());
    let recording_index = index_with(recording.clone());
    assert!(recording_index.get::<Product>("missing").is_err());
    let request = recording.last_call().unwrap();
    let backend = Arc::new(ScriptedBackend::new(vec![
        ExpectedCall::success(
            request.clone(),
            ScriptedResult::Get(GetResult { document: None }),
        ),
        ExpectedCall::error(
            request,
            searchkernel_ir::PortableError::new(searchkernel_ir::ErrorCode::Unavailable, "down"),
        ),
    ]));
    let index = index_with(backend);
    assert_eq!(index.get::<Product>("missing").unwrap(), None);
    let error = index.get::<Product>("missing").unwrap_err();
    assert!(matches!(
        error,
        Error::Search {
            code: ErrorCode::Unavailable,
            retryable: false,
            message,
            ..
        } if message == "down"
    ));
}

#[test]
fn range_and_facets_compile_without_facade_validation() {
    let backend = Arc::new(RecordingBackend::new());
    let index = index_with(backend.clone());
    assert!(index
        .browse()
        .filter(field("price").range(Range::new().lower(100.0).upper(500.0).include_lower(false)))
        .facets(["brand", "in_stock"])
        .execute::<serde_json::Value>()
        .is_err());
    assert!(backend.last_call().is_some());
}

#[test]
fn scripted_crud_successes_are_consumed_and_mapped() {
    let recording = Arc::new(RecordingBackend::new());
    let recorder = index_with(recording.clone());
    let value = serde_json::json!({"title": "headphones"});
    assert!(recorder.insert("inserted", &value).is_err());
    let insert = recording.last_call().unwrap();
    assert!(recorder.upsert("upserted", &value).is_err());
    let upsert = recording.last_call().unwrap();
    assert!(recorder
        .patch("patched", Patch::new().unset("brand"))
        .is_err());
    let patch = recording.last_call().unwrap();
    assert!(recorder.delete("deleted").is_err());
    let delete = recording.last_call().unwrap();
    assert!(recorder.get::<serde_json::Value>("fetched").is_err());
    let get = recording.last_call().unwrap();

    let backend = Arc::new(ScriptedBackend::new(vec![
        ExpectedCall::success(
            insert,
            ScriptedResult::Insert(searchkernel_ir::InsertResult {
                id: "inserted".into(),
            }),
        ),
        ExpectedCall::success(
            upsert,
            ScriptedResult::Upsert(searchkernel_ir::UpsertResult {
                id: "upserted".into(),
            }),
        ),
        ExpectedCall::success(
            patch,
            ScriptedResult::Patch(searchkernel_ir::PatchResult {
                id: "patched".into(),
            }),
        ),
        ExpectedCall::success(
            delete,
            ScriptedResult::Delete(searchkernel_ir::DeleteResult {
                id: "deleted".into(),
                deleted: true,
            }),
        ),
        ExpectedCall::success(
            get,
            ScriptedResult::Get(GetResult {
                document: Some(searchkernel_ir::DocumentResult {
                    id: "fetched".into(),
                    fields: BTreeMap::from([(
                        String::from("title"),
                        Value::String("headphones".into()),
                    )]),
                }),
            }),
        ),
    ]));
    let index = index_with(backend.clone());
    assert!(index.insert("inserted", &value).is_ok());
    assert!(index.upsert("upserted", &value).is_ok());
    assert!(index.patch("patched", Patch::new().unset("brand")).is_ok());
    assert!(index.delete("deleted").unwrap());
    assert_eq!(
        index.get::<serde_json::Value>("fetched").unwrap().unwrap()["title"],
        "headphones"
    );
    backend.finish().unwrap();
}

#[test]
fn result_variants_map_to_owned_product_values() {
    let expected = expected_search_with_facet("brand");
    let nested = BTreeMap::from([(String::from("source"), Value::String("catalog".into()))]);
    let result = SearchResult {
        root: searchkernel_ir::ResultNode::Group(searchkernel_ir::ResultGroup {
            children: vec![
                searchkernel_ir::ResultNode::Hit(searchkernel_ir::Hit {
                    id: "sku-43".into(),
                    score: None,
                    fields: BTreeMap::from([
                        (String::from("title"), Value::String("headphones".into())),
                        (String::from("nullable"), Value::Null),
                        (String::from("signed"), Value::Int64(-7)),
                        (String::from("unsigned"), Value::UInt64(u64::MAX)),
                        (String::from("ratio"), Value::Float64(1.25)),
                        (String::from("bytes"), Value::Bytes(vec![1, 2, 255])),
                        (
                            String::from("list"),
                            Value::List(vec![Value::Bool(true), Value::String("nested".into())]),
                        ),
                        (String::from("object"), Value::Object(nested)),
                    ]),
                    highlights: vec![searchkernel_ir::HighlightResult {
                        field: "title".into(),
                        fragments: vec![searchkernel_ir::HighlightFragment {
                            text: "<em>headphones</em>".into(),
                            matches: vec![searchkernel_ir::HighlightSpan {
                                start_byte: 0,
                                end_byte: 18,
                            }],
                        }],
                    }],
                    ..Default::default()
                }),
                searchkernel_ir::ResultNode::Group(searchkernel_ir::ResultGroup {
                    id: Some("brand".into()),
                    label: Some("brand".into()),
                    children: vec![
                        searchkernel_ir::ResultNode::Group(searchkernel_ir::ResultGroup {
                            value: Some(Value::String("Acme".into())),
                            fields: BTreeMap::from([("count".into(), Value::UInt64(1))]),
                            ..searchkernel_ir::ResultGroup::root(vec![])
                        }),
                        searchkernel_ir::ResultNode::Group(searchkernel_ir::ResultGroup {
                            value: Some(Value::UInt64(2)),
                            fields: BTreeMap::from([("count".into(), Value::UInt64(2))]),
                            ..searchkernel_ir::ResultGroup::root(vec![])
                        }),
                    ],
                    ..searchkernel_ir::ResultGroup::root(vec![])
                }),
            ],
            ..searchkernel_ir::ResultGroup::root(vec![])
        }),
        total_hits: TotalHits {
            value: 1,
            relation: TotalHitsRelation::Exact,
        },
        coverage: None,
        timing: None,
        errors: vec![],
        trace: None,
        metadata: BTreeMap::new(),
    };
    let backend = Arc::new(ScriptedBackend::new(vec![ExpectedCall::success(
        OperationIr::Search(expected),
        ScriptedResult::Search(Box::new(result)),
    )]));
    let index = index_with(backend.clone());

    let result = index
        .browse()
        .facet(Facet::new("brand"))
        .execute::<serde_json::Value>()
        .unwrap();
    let searchkernel::ResultNode::Group(root) = &result.root else {
        panic!("fixture root must remain a canonical result group");
    };
    assert!(matches!(
        root.children.as_slice(),
        [
            searchkernel::ResultNode::Hit(_),
            searchkernel::ResultNode::Group(_)
        ]
    ));
    assert_eq!(result.hits[0].score, None);
    assert_eq!(result.hits[0].document.as_ref().unwrap()["signed"], -7);
    assert_eq!(
        result.hits[0].document.as_ref().unwrap()["unsigned"],
        u64::MAX
    );
    assert_eq!(
        result.hits[0].document.as_ref().unwrap()["bytes"],
        serde_json::json!([1, 2, 255])
    );
    assert_eq!(
        result.hits[0].document.as_ref().unwrap()["object"]["source"],
        "catalog"
    );
    assert_eq!(result.hits[0].highlights[0].field, "title");
    assert_eq!(
        result.hits[0].highlights[0].fragments[0].matches[0].start_byte,
        0
    );
    assert_eq!(
        result.facets[0].values[1].value,
        searchkernel::ResultValue::UInt64(2)
    );
    assert_eq!(
        result.total_hits.relation,
        searchkernel::TotalHitsRelation::Exact
    );
    backend.finish().unwrap();
}

#[test]
fn canonical_result_tree_preserves_interleaved_child_order_through_public_api() {
    let backend = Arc::new(ScriptedBackend::new(vec![ExpectedCall::success(
        OperationIr::Search(expected_search()),
        ScriptedResult::Search(Box::new(
            searchkernel_testkit::mixed_hit_and_group_children_result(),
        )),
    )]));
    let index = index_with(backend.clone());

    let result = index.browse().execute::<serde_json::Value>().unwrap();
    let searchkernel::ResultNode::Group(root) = &result.root else {
        panic!("mixed fixture root must remain a canonical result group");
    };

    assert_eq!(root.children.len(), 4);
    assert!(matches!(
        root.children.as_slice(),
        [
            searchkernel::ResultNode::Hit(_),
            searchkernel::ResultNode::Group(_),
            searchkernel::ResultNode::Hit(_),
            searchkernel::ResultNode::Group(_)
        ]
    ));
    backend.finish().unwrap();
}

#[test]
fn all_filter_operators_and_search_options_cross_the_boundary() {
    let backend = Arc::new(RecordingBackend::new());
    let definition = IndexDefinition::new()
        .field(Field::text("title"))
        .field(Field::keyword("brand"))
        .field(Field::float64("price"))
        .field(Field::bool("in_stock"))
        .field(Field::keyword("category"));
    let index = Index::from_backend(definition, backend.clone()).unwrap();
    let filters = Filter::any([
        Filter::all([
            field("brand").eq("Acme"),
            field("brand").ne("Other"),
            field("price").lt(500.0),
            field("price").lte(500.0),
            field("price").gt(100.0),
            field("price").gte(100.0),
        ]),
        !field("brand").exists(),
        field("price").range(Range::new().lower(100.0).upper(500.0).include_upper(true)),
    ]);

    assert!(index
        .search("headphones")
        .search_fields([searchkernel::SearchField::new("title").weight(0.0)])
        .filter(filters)
        .sort(Sort::asc("price").missing_first())
        .sort(Sort::desc("brand").missing_last())
        .ranking(Ranking::default_relevance_v1())
        .projection(["title"])
        .highlight(["title"])
        .language("EN-us")
        .execute::<serde_json::Value>()
        .is_err());

    let OperationIr::Search(request) = backend.last_call().unwrap() else {
        unreachable!()
    };
    assert_eq!(request.search_fields()[0].weight().get(), 0.0);
    assert_eq!(request.sort().len(), 2);
    assert!(request.grouping().is_empty());
    assert_eq!(request.ranking_request().profile, "default");
    assert_eq!(request.language().unwrap().as_str(), "en-us");
    let Some(FilterIr::Or(filters)) = request.filter() else {
        panic!("expected OR filter");
    };
    assert_eq!(filters.len(), 3);
    assert!(matches!(filters[0], FilterIr::And(ref clauses) if clauses.len() == 6));
    assert!(matches!(filters[1], FilterIr::Not(_)));
    assert!(
        matches!(filters[2], FilterIr::Range(ref range) if !matches!(range.upper(), searchkernel_ir::FilterRangeBoundIr::Unbounded) && range.include_upper())
    );
}

#[test]
fn simple_search_field_names_cross_the_boundary() {
    let backend = Arc::new(RecordingBackend::new());
    let index = Index::from_backend(
        definition().field(Field::text("description")),
        backend.clone(),
    )
    .unwrap();

    assert!(index
        .search("headphones")
        .search_fields(["title", "description"])
        .execute::<Product>()
        .is_err());

    let OperationIr::Search(request) = backend.last_call().unwrap() else {
        unreachable!()
    };
    assert_eq!(
        request
            .search_fields()
            .iter()
            .map(|field| field.field().as_str())
            .collect::<Vec<_>>(),
        ["title", "description"]
    );
    assert!(request
        .search_fields()
        .iter()
        .all(|field| field.weight().get() == 1.0));
}

#[test]
fn portable_values_and_serialization_errors_are_preserved() {
    let definition = IndexDefinition::new()
        .field(Field::text("title"))
        .field(Field::keyword("tags").multi())
        .field(Field::int64("signed"))
        .field(Field::float64("ratio"))
        .field(Field::bool("enabled"));
    let backend = Arc::new(RecordingBackend::new());
    let index = Index::from_backend(definition, backend).unwrap();
    let document = serde_json::json!({
        "title": "headphones",
        "tags": ["audio", "sale"],
        "signed": -7,
        "ratio": 1.25,
        "enabled": true,
        "nullable": null,
        "unsigned": u64::MAX,
        "nested": {"source": "catalog"}
    });
    let error = index.insert("sku-44", &document).unwrap_err();
    assert!(matches!(error, Error::Search { .. }));

    let non_object = serde_json::json!(["not", "a", "document"]);
    let error = index.upsert("sku-45", &non_object).unwrap_err();
    assert!(matches!(error, Error::Serialization(ref message) if message.contains("object")));
    assert!(error.to_string().contains("document serialization failed"));
}

#[test]
fn result_conversion_reports_bad_documents_and_preserves_non_finite_numbers() {
    let expected = expected_search_with_facet("brand");
    let bad_document = SearchResult {
        root: searchkernel_ir::ResultNode::Group(searchkernel_ir::ResultGroup::root(vec![
            searchkernel_ir::ResultNode::Hit(searchkernel_ir::Hit {
                id: "bad".into(),
                score: Some(1.0),
                fields: BTreeMap::from([(String::from("title"), Value::String("ok".into()))]),
                highlights: vec![],
                ..Default::default()
            }),
        ])),
        total_hits: TotalHits {
            value: 1,
            relation: TotalHitsRelation::Exact,
        },
        coverage: None,
        timing: None,
        errors: vec![],
        trace: None,
        metadata: BTreeMap::new(),
    };
    let backend = Arc::new(ScriptedBackend::new(vec![
        ExpectedCall::success(
            OperationIr::Search(expected.clone()),
            ScriptedResult::Search(Box::new(bad_document)),
        ),
        ExpectedCall::success(
            OperationIr::Search(expected),
            ScriptedResult::Search(Box::new(SearchResult {
                root: searchkernel_ir::ResultNode::Group(searchkernel_ir::ResultGroup::root(vec![
                    searchkernel_ir::ResultNode::Group(searchkernel_ir::ResultGroup {
                        id: Some("brand".into()),
                        label: Some("brand".into()),
                        children: vec![searchkernel_ir::ResultNode::Group(
                            searchkernel_ir::ResultGroup {
                                value: Some(Value::Float64(f64::NAN)),
                                fields: BTreeMap::from([("count".into(), Value::UInt64(1))]),
                                ..searchkernel_ir::ResultGroup::root(vec![])
                            },
                        )],
                        ..searchkernel_ir::ResultGroup::root(vec![])
                    }),
                ])),
                total_hits: TotalHits {
                    value: 0,
                    relation: TotalHitsRelation::Exact,
                },
                coverage: None,
                timing: None,
                errors: vec![],
                trace: None,
                metadata: BTreeMap::new(),
            })),
        ),
    ]));
    let index = index_with(backend.clone());
    let error = index
        .browse()
        .facet(Facet::new("brand"))
        .execute::<String>()
        .unwrap_err();
    assert!(matches!(error, Error::Deserialization(_)));
    let result = index
        .browse()
        .facet(Facet::new("brand"))
        .execute::<serde_json::Value>()
        .unwrap();
    assert_eq!(
        result.facets[0].values[0].value,
        searchkernel::ResultValue::NonFiniteFloat(searchkernel::NonFiniteFloat::NaN)
    );
    backend.finish().unwrap();
}

#[test]
fn backend_errors_and_false_deletes_are_returned_without_reinterpretation() {
    let recording = Arc::new(RecordingBackend::new());
    let recorder = index_with(recording.clone());
    let value = serde_json::json!({"title": "headphones"});
    assert!(recorder.insert("insert", &value).is_err());
    assert!(recorder.upsert("upsert", &value).is_err());
    assert!(recorder
        .patch("patch", Patch::new().unset("brand"))
        .is_err());
    assert!(recorder.delete("delete").is_err());
    assert!(recorder.browse().execute::<serde_json::Value>().is_err());
    let calls = recording.calls();
    let error = searchkernel_ir::PortableError::new(
        searchkernel_ir::ErrorCode::Unavailable,
        "backend unavailable",
    );
    let backend = Arc::new(ScriptedBackend::new(
        calls
            .into_iter()
            .map(|request| ExpectedCall::error(request, error.clone()))
            .collect::<Vec<_>>(),
    ));
    let index = index_with(backend.clone());
    assert!(matches!(
        index.insert("insert", &value),
        Err(Error::Search { .. })
    ));
    assert!(matches!(
        index.upsert("upsert", &value),
        Err(Error::Search { .. })
    ));
    assert!(matches!(
        index.patch("patch", Patch::new().unset("brand")),
        Err(Error::Search { .. })
    ));
    assert!(matches!(index.delete("delete"), Err(Error::Search { .. })));
    assert!(matches!(
        index.browse().execute::<serde_json::Value>(),
        Err(Error::Search { .. })
    ));
    backend.finish().unwrap();
}

#[test]
fn public_helper_types_and_errors_have_stable_behavior() {
    let text = String::from("sku-46");
    let from_string = DocumentId::from(text.clone());
    let from_reference = DocumentId::from(&text);
    let from_id = DocumentId::from(&from_string);
    assert_eq!(from_string.as_str(), "sku-46");
    assert_eq!(from_reference.as_ref(), "sku-46");
    assert_eq!(from_id.to_string(), "sku-46");
    assert_eq!(format!("{from_id}"), "sku-46");

    let facet: Facet = text.into();
    assert_eq!(facet, Facet::new("sku-46"));
    assert_eq!(Range::default(), Range::new());

    let index = index_with(Arc::new(RecordingBackend::new()));
    assert!(format!("{:?}", index.search("headphones")).contains("headphones"));

    let search = Error::search(ErrorCode::Unavailable, "unavailable");
    assert_eq!(search.code(), ErrorCode::Unavailable);
    assert!(!search.retryable());
    assert!(search.details().is_empty());
    assert_eq!(search.to_string(), "unavailable");
    assert!(std::error::Error::source(&search).is_none());
    let serialization = Error::Serialization("bad document".into());
    assert_eq!(serialization.code(), ErrorCode::InvalidArgument);
    assert!(serialization.details().is_empty());
    assert!(serialization.to_string().contains("bad document"));
    assert_eq!(
        Error::Deserialization("bad result".into()).to_string(),
        "document deserialization failed: bad result"
    );
    assert_eq!(Error::Infrastructure("broken".into()).to_string(), "broken");
}

#[test]
fn advanced_grouping_facade_matches_independent_portable_request_ir() {
    let definition = IndexDefinition::new()
        .name("grouping-products")
        .field(Field::keyword("brand").facetable(true))
        .field(Field::float64("price").facetable(true));
    let direct_backend = Arc::new(RecordingBackend::new());
    let index = Index::from_backend(definition, direct_backend.clone()).unwrap();
    let grouping = GroupingPlan::new().level(
        GroupingPlan::by_expression(GroupingExpression::function(
            "lowercase",
            [GroupingExpression::attribute("brand")],
        ))
        .alias("total", GroupingAggregation::sum("price"))
        .max_groups(5)
        .precision(9)
        .order_by([GroupingOrder::descending_alias("total")])
        .filter(GroupingFilter::range(
            "price",
            GroupingExpression::literal(10_i64),
            GroupingExpression::literal(100_i64),
        ))
        .output(GroupingOutput::alias("total").label("revenue"))
        .output(GroupingAggregation::stddev("price")),
    );
    assert!(index
        .browse()
        .grouping(grouping)
        .execute::<serde_json::Value>()
        .is_err());
    let direct = direct_backend.last_call().expect("direct Rust grouping IR");

    let field = |name: &str, kind, facetable| searchkernel_ir::RustFieldInput {
        name: name.to_owned(),
        kind,
        multi: false,
        stored: None,
        searchable: None,
        filterable: None,
        sortable: None,
        facetable,
        analyzer: None,
        distance_metric: None,
        hnsw: None,
    };
    let definition =
        searchkernel_ir::resolve_rust_index_definition(searchkernel_ir::RustIndexDefinitionInput {
            name: Some("grouping-products".to_owned()),
            fields: vec![
                field("brand", searchkernel_ir::RustFieldKind::Keyword, Some(true)),
                field("price", searchkernel_ir::RustFieldKind::Float64, Some(true)),
            ],
            default_search_fields: None,
        })
        .expect("portable parity schema");
    let portable_backend = Arc::new(RecordingBackend::new());
    let dispatcher = searchkernel_dispatch::Dispatcher::from_backend(
        Arc::new(searchkernel_ir::IndexContext::from_definition_ir(
            definition,
        )),
        portable_backend.clone(),
    );
    let request = searchkernel_protocol::json::request_json_to_flatbuffer(
        br#"{
          "protocol_version": 2,
          "payload_type": "SearchKernel_V2_SearchRequest",
          "payload": {
            "query": {"source_type": "MatchAllQuery", "source": {}},
            "grouping": [{
              "source_type": "StructuredGroupingSource",
              "source": {"root": {"operations": [
                {
                  "operation_type": "GroupingGroupOperation",
                  "operation": {"expression": {
                    "expr_type": "GroupingFunctionExpression",
                    "expr": {
                      "name": "lowercase",
                      "arguments": [{
                        "expr_type": "GroupingAttributeExpression",
                        "expr": {"field": "brand"}
                      }]
                    }
                  }}
                },
                {
                  "operation_type": "GroupingEachOperation",
                  "operation": {"operations": [
                    {
                      "operation_type": "GroupingAliasOperation",
                      "operation": {"name": "total", "expression": {
                        "expr_type": "GroupingAggregationExpression",
                        "expr": {
                          "aggregation_type": "SumAggregation",
                          "aggregation": {"expression": {
                            "expr_type": "GroupingAttributeExpression",
                            "expr": {"field": "price"}
                          }}
                        }
                      }}
                    },
                    {
                      "operation_type": "GroupingMaxOperation",
                      "operation": {
                        "value_type": "FiniteGroupingMax",
                        "value": {"value": 5}
                      }
                    },
                    {
                      "operation_type": "GroupingPrecisionOperation",
                      "operation": {"value": 9}
                    },
                    {
                      "operation_type": "GroupingOrderOperation",
                      "operation": {"expressions": [{"expression": {
                        "expr_type": "GroupingUnaryExpression",
                        "expr": {
                          "op": "Negative",
                          "expression": {
                            "expr_type": "GroupingAliasReference",
                            "expr": {"name": "total"}
                          }
                        }
                      }}]}
                    },
                    {
                      "operation_type": "GroupingFilterOperation",
                      "operation": {"filter": {
                        "expr_type": "GroupingFilterRange",
                        "expr": {
                          "expression": {
                            "expr_type": "GroupingAttributeExpression",
                            "expr": {"field": "price"}
                          },
                          "lower": {
                            "expr_type": "GroupingLiteralExpression",
                            "expr": {"value": {
                              "kind_type": "Int64Value",
                              "kind": {"value": "10"}
                            }}
                          },
                          "upper": {
                            "expr_type": "GroupingLiteralExpression",
                            "expr": {"value": {
                              "kind_type": "Int64Value",
                              "kind": {"value": "100"}
                            }}
                          }
                        }
                      }}
                    },
                    {
                      "operation_type": "GroupingOutputOperation",
                      "operation": {"outputs": [
                        {
                          "label": "revenue",
                          "expression": {
                            "expr_type": "GroupingAliasReference",
                            "expr": {"name": "total"}
                          }
                        },
                        {
                          "expression": {
                            "expr_type": "GroupingAggregationExpression",
                            "expr": {
                              "aggregation_type": "StdDevAggregation",
                              "aggregation": {"expression": {
                                "expr_type": "GroupingAttributeExpression",
                                "expr": {"field": "price"}
                              }}
                            }
                          }
                        }
                      ]}
                    }
                  ]}
                }
              ]}}
            }]
          }
        }"#,
    )
    .expect("canonical portable grouping request");
    dispatcher
        .dispatch(&request)
        .expect("recording backend error is encoded as a portable response");
    let portable = portable_backend.last_call().expect("portable grouping IR");
    assert_eq!(direct, portable);
}
