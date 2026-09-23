use super::*;
use crate::model::*;
use proptest::prelude::*;
use std::collections::BTreeMap;

const PROPERTY_CASES: u32 = 128;

fn config() -> ProptestConfig {
    ProptestConfig {
        cases: PROPERTY_CASES,
        max_shrink_iters: 4096,
        ..ProptestConfig::default()
    }
}

fn field_defaults(kind: &RustFieldKind) -> (bool, bool, bool, bool, bool) {
    match kind {
        RustFieldKind::Text => (true, true, false, false, false),
        RustFieldKind::Keyword => (true, false, true, true, true),
        RustFieldKind::Int64 | RustFieldKind::Float64 => (true, false, true, true, false),
        RustFieldKind::Bool => (true, false, true, true, true),
        RustFieldKind::Position | RustFieldKind::Tensor(_) => unreachable!("property schema uses scalar fields"),
    }
}

fn scalar_kind(selector: u8) -> RustFieldKind {
    match selector % 5 {
        0 => RustFieldKind::Text,
        1 => RustFieldKind::Keyword,
        2 => RustFieldKind::Int64,
        3 => RustFieldKind::Float64,
        _ => RustFieldKind::Bool,
    }
}

fn rust_field(name: String, kind: RustFieldKind, explicit_defaults: bool) -> RustFieldInput {
    let (stored, searchable, filterable, sortable, facetable) = field_defaults(&kind);
    RustFieldInput {
        name,
        kind,
        multi: false,
        stored: explicit_defaults.then_some(stored),
        searchable: explicit_defaults.then_some(searchable),
        filterable: explicit_defaults.then_some(filterable),
        sortable: explicit_defaults.then_some(sortable),
        facetable: explicit_defaults.then_some(facetable),
        analyzer: None,
        distance_metric: None,
        hnsw: None,
    }
}

fn schema_input(selectors: &[u8], explicit_defaults: bool) -> RustIndexDefinitionInput {
    RustIndexDefinitionInput {
        name: Some("property_index".to_owned()),
        fields: selectors
            .iter()
            .enumerate()
            .map(|(index, selector)| {
                rust_field(
                    format!("f{index}"),
                    scalar_kind(*selector),
                    explicit_defaults,
                )
            })
            .collect(),
        default_search_fields: None,
    }
}

fn context() -> IndexContext {
    let fields = vec![
        rust_field("title".to_owned(), RustFieldKind::Text, true),
        rust_field("brand".to_owned(), RustFieldKind::Keyword, true),
        rust_field("price".to_owned(), RustFieldKind::Int64, true),
        rust_field("rating".to_owned(), RustFieldKind::Float64, true),
        rust_field("in_stock".to_owned(), RustFieldKind::Bool, true),
    ];
    let definition = resolve_rust_index_definition(RustIndexDefinitionInput {
        name: Some("property_context".to_owned()),
        fields,
        default_search_fields: None,
    })
    .expect("property context schema");
    IndexContext::from_definition_ir(definition)
}

fn base_search(query: QuerySourceInput) -> SearchInput {
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

fn canonical_request(query: QueryIr) -> SearchRequestIr {
    SearchRequestIr::new(
        query,
        Vec::new(),
        None,
        Vec::new(),
        20,
        0,
        ProjectionIr::new(Vec::new()),
        HighlightingIr::Disabled,
        None,
        PrefixPolicy::Disabled,
        TypoPolicy::Disabled,
    )
}

fn roundtrip_search(context: &IndexContext, request: &SearchRequestIr) -> SearchRequestIr {
    let bytes = crate::search_request_ir_to_api(request);
    let portable = crate::__flatbuffers::root::<crate::api::SearchRequest>(&bytes)
        .expect("encoded search request must decode");
    context
        .search_to_ir(portable)
        .expect("encoded supported request must resolve")
}

fn finite_f64() -> impl Strategy<Value = f64> {
    any::<f64>().prop_filter("finite f64", |value| value.is_finite())
}

fn request_value_strategy() -> BoxedStrategy<RequestValueIr> {
    let scalar = prop_oneof![
        Just(RequestValueIr::Null),
        any::<bool>().prop_map(RequestValueIr::Bool),
        any::<i64>().prop_map(RequestValueIr::Int64),
        any::<u64>().prop_map(RequestValueIr::UInt64),
        finite_f64().prop_map(RequestValueIr::Float64),
        (0u8..3).prop_map(|kind| RequestValueIr::NonFiniteFloat(match kind {
            0 => RequestNonFiniteFloatIr::NaN,
            1 => RequestNonFiniteFloatIr::PositiveInfinity,
            _ => RequestNonFiniteFloatIr::NegativeInfinity,
        })),
        prop::collection::vec(any::<u8>(), 0..16).prop_map(RequestValueIr::Bytes),
        (0u64..1_000_000).prop_map(|value| RequestValueIr::String(format!("s{value}"))),
        (finite_f64(), finite_f64(), finite_f64()).prop_map(|(a, b, c)| {
            RequestValueIr::Tensor(RequestTensorIr {
                type_spec: "tensor<float>(x[3])".to_owned(),
                payload: RequestTensorPayloadIr::Dense(vec![a, b, c]),
            })
        }),
    ];

    scalar
        .prop_recursive(3, 32, 4, |inner| {
            prop_oneof![
                prop::collection::vec(inner.clone(), 0..4).prop_map(RequestValueIr::List),
                prop::collection::vec((0u8..8, inner), 0..4).prop_map(|entries| {
                    let mut values = BTreeMap::new();
                    for (key, value) in entries {
                        values.insert(format!("k{key}"), value);
                    }
                    RequestValueIr::Object(values)
                }),
            ]
        })
        .boxed()
}

fn bool_query(kind: QueryExprKindIr) -> QueryExprIr {
    QueryExprIr {
        kind,
        annotations: BTreeMap::new(),
    }
}

fn boolean_query_strategy() -> BoxedStrategy<QueryExprIr> {
    prop_oneof![
        Just(bool_query(QueryExprKindIr::True)),
        Just(bool_query(QueryExprKindIr::False)),
    ]
    .prop_recursive(4, 64, 4, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 1..4)
                .prop_map(|children| bool_query(QueryExprKindIr::And(children))),
            prop::collection::vec(inner.clone(), 1..4)
                .prop_map(|children| bool_query(QueryExprKindIr::Or(children))),
            inner.prop_map(|child| bool_query(QueryExprKindIr::Not(Box::new(child)))),
        ]
    })
    .boxed()
}

fn eval_boolean_query(query: &QueryExprIr) -> bool {
    match &query.kind {
        QueryExprKindIr::True => true,
        QueryExprKindIr::False => false,
        QueryExprKindIr::And(children) => children.iter().all(eval_boolean_query),
        QueryExprKindIr::Or(children) => children.iter().any(eval_boolean_query),
        QueryExprKindIr::Not(child) => !eval_boolean_query(child),
        other => panic!("unexpected generated boolean query node: {other:?}"),
    }
}

fn boolean_filter_strategy() -> BoxedStrategy<FilterInput> {
    any::<bool>()
        .prop_map(|value| FilterInput::Comparison {
            field: "in_stock".to_owned(),
            op: ComparisonOp::Eq,
            value: RequestValueIr::Bool(value),
        })
        .prop_recursive(4, 64, 4, |inner| {
            prop_oneof![
                prop::collection::vec(inner.clone(), 1..4).prop_map(FilterInput::All),
                prop::collection::vec(inner.clone(), 1..4).prop_map(FilterInput::Any),
                inner.prop_map(|child| FilterInput::Not(Box::new(child))),
            ]
        })
        .boxed()
}

fn eval_filter_input(filter: &FilterInput, in_stock: bool) -> bool {
    match filter {
        FilterInput::All(children) => children.iter().all(|child| eval_filter_input(child, in_stock)),
        FilterInput::Any(children) => children.iter().any(|child| eval_filter_input(child, in_stock)),
        FilterInput::Not(child) => !eval_filter_input(child, in_stock),
        FilterInput::Comparison { op, value, .. } => {
            let RequestValueIr::Bool(expected) = value else {
                panic!("generated boolean filter carried a non-boolean value");
            };
            match op {
                ComparisonOp::Eq => in_stock == *expected,
                ComparisonOp::Ne => in_stock != *expected,
                other => panic!("unexpected generated boolean comparison: {other:?}"),
            }
        }
        other => panic!("unexpected generated boolean filter node: {other:?}"),
    }
}

fn eval_filter_ir(filter: &FilterIr, in_stock: bool) -> bool {
    match filter {
        FilterIr::And(children) => children.iter().all(|child| eval_filter_ir(child, in_stock)),
        FilterIr::Or(children) => children.iter().any(|child| eval_filter_ir(child, in_stock)),
        FilterIr::Not(child) => !eval_filter_ir(child, in_stock),
        FilterIr::Comparison(comparison) => {
            let ScalarValueIr::Bool(expected) = comparison.value() else {
                panic!("resolved boolean filter carried a non-boolean value");
            };
            match comparison.op() {
                ComparisonOp::Eq => in_stock == *expected,
                ComparisonOp::Ne => in_stock != *expected,
                other => panic!("unexpected resolved boolean comparison: {other:?}"),
            }
        }
        other => panic!("unexpected resolved boolean filter node: {other:?}"),
    }
}

fn exact_u64_in_f64(value: u64) -> bool {
    if value == 0 {
        return true;
    }
    let bits = u64::BITS - value.leading_zeros();
    if bits <= 53 {
        true
    } else {
        value.trailing_zeros() >= bits - 53
    }
}

fn exact_i64_in_f64(value: i64) -> bool {
    exact_u64_in_f64(value.unsigned_abs())
}

fn bind_parameter(field: &str, value: Value) -> crate::Result<ScalarValueIr> {
    let schema = context();
    let bindings = BTreeMap::from([("v".to_owned(), value)]);
    let bound = crate::bind_filter_expression(
        &format!("{field} = :v"),
        schema.definition(),
        &bindings,
    )?;
    let FilterIr::Comparison(comparison) = bound else {
        panic!("generated scalar parameter should bind to one comparison");
    };
    Ok(comparison.value().clone())
}

fn expected_facet(input: &FacetInput) -> GroupingRequestIr {
    let field = GroupingExpressionIr::Attribute(input.field.clone());
    let count = GroupingExpressionIr::Aggregation(Box::new(GroupingAggregationIr::Count));
    let order = match input.sort.unwrap_or(FacetSortInput::CountDesc) {
        FacetSortInput::CountDesc => GroupingExpressionIr::Negative(Box::new(count.clone())),
        FacetSortInput::CountAsc => count.clone(),
        FacetSortInput::ValueAsc => field.clone(),
        FacetSortInput::ValueDesc => GroupingExpressionIr::Negative(Box::new(field.clone())),
    };
    GroupingRequestIr {
        root: GroupingAllIr {
            operations: vec![
                GroupingOperationIr::Group(field),
                GroupingOperationIr::Each(GroupingAllIr {
                    operations: vec![
                        GroupingOperationIr::Max(Some(input.limit.unwrap_or(10))),
                        GroupingOperationIr::Order(vec![order]),
                        GroupingOperationIr::Output(vec![GroupingOutputIr {
                            label: None,
                            expression: count,
                        }]),
                    ],
                    label: None,
                }),
            ],
            label: Some(
                input
                    .name
                    .as_deref()
                    .filter(|name| !name.is_empty())
                    .unwrap_or(&input.field)
                    .to_owned(),
            ),
        },
        continuations: Vec::new(),
    }
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn api_ir_api_ir_is_stable_for_generated_supported_schemas(
        selectors in prop::collection::vec(0u8..5, 1..7)
    ) {
        let initial = resolve_rust_index_definition(schema_input(&selectors, false))
            .expect("generated schema should resolve");
        let first_bytes = crate::index_definition_ir_to_api(&initial);
        let first_api = crate::__flatbuffers::root::<crate::api::IndexDefinition>(&first_bytes)
            .expect("canonical schema bytes");
        let first_ir = crate::index_definition_to_ir(first_api)
            .expect("generated API should resolve");
        let second_bytes = crate::index_definition_ir_to_api(&first_ir);
        let second_api = crate::__flatbuffers::root::<crate::api::IndexDefinition>(&second_bytes)
            .expect("second canonical schema bytes");
        let second_ir = crate::index_definition_to_ir(second_api)
            .expect("second generated API should resolve");

        prop_assert_eq!(&first_ir, &second_ir);
        prop_assert_eq!(
            crate::snapshot::index_definition(&first_ir),
            crate::snapshot::index_definition(&second_ir)
        );
    }

    #[test]
    fn schema_snapshot_generation_is_deterministic(
        selectors in prop::collection::vec(0u8..5, 1..7)
    ) {
        let schema = resolve_rust_index_definition(schema_input(&selectors, false))
            .expect("generated schema should resolve");
        let first = crate::snapshot::index_definition(&schema);
        let second = crate::snapshot::index_definition(&schema);
        prop_assert_eq!(&first, &second);
        prop_assert_eq!(
            serde_json::to_string(&first).unwrap(),
            serde_json::to_string(&second).unwrap()
        );
    }

    #[test]
    fn equivalent_defaulted_schemas_have_stable_snapshots(
        selector in 0u8..5,
        suffix in 0u32..100_000
    ) {
        let kind = scalar_kind(selector);
        let field_name = format!("f{suffix}");
        let implicit = resolve_rust_index_definition(RustIndexDefinitionInput {
            name: Some("normalized".to_owned()),
            fields: vec![rust_field(field_name.clone(), kind.clone(), false)],
            default_search_fields: None,
        }).expect("implicit defaults should resolve");
        let explicit = resolve_rust_index_definition(RustIndexDefinitionInput {
            name: Some("normalized".to_owned()),
            fields: vec![rust_field(field_name, kind, true)],
            default_search_fields: None,
        }).expect("explicit defaults should resolve");

        prop_assert_eq!(
            crate::snapshot::index_definition(&implicit),
            crate::snapshot::index_definition(&explicit)
        );
    }

    #[test]
    fn portable_value_encode_decode_preserves_supported_values(
        value in request_value_strategy()
    ) {
        let bytes = crate::ir_to_api::request_value_ir_to_api_for_test(&value);
        let portable = crate::__flatbuffers::root::<crate::api::Value>(&bytes)
            .expect("encoded portable value must decode");
        let decoded = super::value::RequestValueIr::from_api(portable, "value".to_owned())
            .expect("generated supported portable value must resolve");
        prop_assert_eq!(decoded, value);
    }

    #[test]
    fn absent_and_present_empty_projection_remain_distinct(
        stored_count in 1usize..7
    ) {
        let fields = (0..stored_count)
            .map(|index| rust_field(format!("f{index}"), RustFieldKind::Keyword, true))
            .collect();
        let schema = resolve_rust_index_definition(RustIndexDefinitionInput {
            name: Some("projection_presence".to_owned()),
            fields,
            default_search_fields: None,
        }).unwrap();
        let context = IndexContext::from_definition_ir(schema);

        let absent = resolve_search(&context, base_search(QuerySourceInput::MatchAll))
            .expect("absent projection resolves");
        let mut empty_input = base_search(QuerySourceInput::MatchAll);
        empty_input.projection = Some(Vec::new());
        let empty = resolve_search(&context, empty_input)
            .expect("explicit empty projection resolves");

        prop_assert_eq!(absent.projection().fields().len(), stored_count);
        prop_assert!(empty.projection().fields().is_empty());
        prop_assert_ne!(absent.projection(), empty.projection());
    }

    #[test]
    fn numeric_coercion_accepts_exact_conversions_only(
        signed in any::<i64>(),
        unsigned in any::<u64>(),
        float in finite_f64()
    ) {
        let signed_to_float = bind_parameter("rating", Value::Int64(signed));
        prop_assert_eq!(signed_to_float.is_ok(), exact_i64_in_f64(signed));
        if let Ok(ScalarValueIr::Float64(actual)) = signed_to_float {
            prop_assert_eq!(actual, signed as f64);
        }

        let unsigned_to_float = bind_parameter("rating", Value::UInt64(unsigned));
        prop_assert_eq!(unsigned_to_float.is_ok(), exact_u64_in_f64(unsigned));

        let unsigned_to_int = bind_parameter("price", Value::UInt64(unsigned));
        prop_assert_eq!(unsigned_to_int.is_ok(), unsigned <= i64::MAX as u64);

        let float_to_int = bind_parameter("price", Value::Float64(float));
        let legal_float_to_int =
            float.fract() == 0.0
            && float >= i64::MIN as f64
            && float < 2_f64.powi(63)
            && (float as i64) as f64 == float;
        prop_assert_eq!(float_to_int.is_ok(), legal_float_to_int);
    }

    #[test]
    fn integer_coercion_rejects_fractional_values(
        whole in -1_000_000i32..1_000_000,
        quarter in 1u8..4
    ) {
        let value = whole as f64 + f64::from(quarter) / 4.0;
        let error = bind_parameter("price", Value::Float64(value))
            .expect_err("fractional Float64 must not coerce to Int64");
        prop_assert_eq!(error.details[0].code.as_str(), "type_mismatch");
    }

    #[test]
    fn forbidden_non_finite_filter_parameters_are_always_rejected(
        value in prop_oneof![Just(f64::NAN), Just(f64::INFINITY), Just(f64::NEG_INFINITY)]
    ) {
        let error = bind_parameter("rating", Value::Float64(value))
            .expect_err("non-finite filter parameter must fail");
        prop_assert_eq!(error.details[0].code.as_str(), "non_finite_number");
    }

    #[test]
    fn textual_filter_bindings_never_leak_between_clauses(
        first in any::<i64>(),
        second in any::<i64>()
    ) {
        prop_assume!(first != second);
        let mut input = base_search(QuerySourceInput::MatchAll);
        input.textual_filters = vec![
            TextualFilterInput {
                source: "price = :v".to_owned(),
                bindings: BTreeMap::from([("v".to_owned(), Value::Int64(first))]),
            },
            TextualFilterInput {
                source: "price = :v".to_owned(),
                bindings: BTreeMap::from([("v".to_owned(), Value::Int64(second))]),
            },
        ];
        let resolved = resolve_search(&context(), input)
            .expect("independent textual clauses should resolve");
        let Some(FilterIr::And(children)) = resolved.filter() else {
            panic!("two textual clauses should normalize to And");
        };
        let values = children
            .iter()
            .map(|child| match child {
                FilterIr::Comparison(comparison) => comparison.value().clone(),
                other => panic!("unexpected textual clause result: {other:?}"),
            })
            .collect::<Vec<_>>();
        prop_assert_eq!(
            values,
            vec![ScalarValueIr::Int64(first), ScalarValueIr::Int64(second)]
        );
    }

    #[test]
    fn direct_pagination_normalization_is_total_and_deterministic(
        limit in prop::option::of(any::<u32>()),
        offset in prop::option::of(any::<u32>())
    ) {
        let first = super::resolve::resolve_pagination(limit, offset);
        let second = super::resolve::resolve_pagination(limit, offset);
        prop_assert_eq!(first, second);
        prop_assert_eq!(first.0, limit.unwrap_or(20));
        prop_assert_eq!(first.1, offset.unwrap_or(0));
    }

    #[test]
    fn boolean_query_and_filter_normalization_preserve_meaning(
        query in boolean_query_strategy(),
        filter in boolean_filter_strategy(),
        in_stock in any::<bool>()
    ) {
        let expected_query = eval_boolean_query(&query);
        let mut query_input = base_search(QuerySourceInput::Typed(query));
        query_input.filters.push(filter.clone());
        let resolved = resolve_search(&context(), query_input)
            .expect("generated boolean query/filter should resolve");

        let QueryIr::Typed(query) = resolved.query() else {
            panic!("typed query must remain typed");
        };
        prop_assert_eq!(eval_boolean_query(query), expected_query);

        let resolved_filter = resolved.filter().expect("generated filter should remain present");
        prop_assert_eq!(
            eval_filter_ir(resolved_filter, in_stock),
            eval_filter_input(&filter, in_stock)
        );
    }

    #[test]
    fn grouping_encode_decode_preserves_child_order(
        values in prop::collection::vec(any::<u32>(), 1..9)
    ) {
        let grouping = GroupingRequestIr {
            root: GroupingAllIr {
                operations: values
                    .iter()
                    .copied()
                    .map(GroupingOperationIr::Precision)
                    .collect(),
                label: None,
            },
            continuations: values
                .iter()
                .enumerate()
                .map(|(index, value)| format!("c{index}_{value}"))
                .collect(),
        };
        let request = canonical_request(QueryIr::MatchAll)
            .with_grouping(vec![grouping.clone()]);
        let roundtripped = roundtrip_search(&context(), &request);
        prop_assert_eq!(roundtripped.grouping(), &[grouping]);
    }

    #[test]
    fn facet_convenience_always_lowers_to_documented_grouping_shape(
        sort_selector in 0u8..5,
        limit in any::<u32>(),
        use_name in any::<bool>(),
        suffix in 0u16..10_000
    ) {
        let sort = match sort_selector {
            0 => None,
            1 => Some(FacetSortInput::CountDesc),
            2 => Some(FacetSortInput::CountAsc),
            3 => Some(FacetSortInput::ValueAsc),
            _ => Some(FacetSortInput::ValueDesc),
        };
        let input = FacetInput {
            field: "brand".to_owned(),
            name: use_name.then(|| format!("facet_{suffix}")),
            limit: Some(limit),
            sort,
        };
        prop_assert_eq!(facet_grouping_input(&input), expected_facet(&input));
    }

    #[test]
    fn duplicate_aliases_fields_and_update_targets_are_rejected_deterministically(
        suffix in 0u32..100_000,
        first in any::<i64>(),
        second in any::<i64>()
    ) {
        let name = format!("f{suffix}");
        let schema_error = resolve_rust_index_definition(RustIndexDefinitionInput {
            name: Some("duplicates".to_owned()),
            fields: vec![
                rust_field(name.clone(), RustFieldKind::Int64, true),
                rust_field(name, RustFieldKind::Bool, true),
            ],
            default_search_fields: None,
        }).expect_err("duplicate schema field must fail");
        prop_assert_eq!(schema_error.details[0].code.as_str(), "duplicate_field");

        let facet_error = resolve_facets(
            &context(),
            vec![
                FacetInput {
                    field: "brand".to_owned(),
                    name: Some(format!("a{suffix}")),
                    limit: Some(10),
                    sort: None,
                },
                FacetInput {
                    field: "brand".to_owned(),
                    name: Some(format!("b{suffix}")),
                    limit: Some(20),
                    sort: Some(FacetSortInput::ValueAsc),
                },
            ],
        ).expect_err("duplicate facet field must fail");
        prop_assert_eq!(facet_error.details[0].code.as_str(), "duplicate_facet_field");

        let alias = format!("alias_{suffix}");
        let grouping = GroupingRequestIr {
            root: GroupingAllIr {
                operations: vec![
                    GroupingOperationIr::Alias {
                        name: alias.clone(),
                        expression: GroupingExpressionIr::Literal(RequestValueIr::Int64(first)),
                    },
                    GroupingOperationIr::Alias {
                        name: alias,
                        expression: GroupingExpressionIr::Literal(RequestValueIr::Int64(second)),
                    },
                ],
                label: None,
            },
            continuations: Vec::new(),
        };
        let mut grouping_input = base_search(QuerySourceInput::MatchAll);
        grouping_input.grouping.push(GroupingInput::Structured(grouping));
        let alias_error = resolve_search(&context(), grouping_input)
            .expect_err("duplicate grouping aliases must fail");
        prop_assert_eq!(alias_error.details[0].code.as_str(), "duplicate_grouping_alias");

        let update_error = resolve_update(
            &context(),
            "doc:property",
            &[
                UpdateOperationInput::Assign {
                    target: "price".to_owned(),
                    value: RequestValueIr::Int64(first),
                },
                UpdateOperationInput::Clear {
                    target: "price".to_owned(),
                },
            ],
            None,
            None,
        ).expect_err("multiple operations for one update target must fail");
        prop_assert_eq!(
            update_error.details[0].code.as_str(),
            "multiple_updates_per_target_not_supported"
        );
    }
}
