use super::*;

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
        let bytes = crate::wire::v1::search_request_ir_to_api(&request(expected.clone()));
        let api_request = crate::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
        let actual = query_context.search_to_ir(api_request).unwrap();
        assert_eq!(
            actual.query(),
            &expected,
            "{name} lost typed query semantics"
        );
    }
}
