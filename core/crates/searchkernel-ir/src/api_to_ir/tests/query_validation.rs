use super::*;

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
