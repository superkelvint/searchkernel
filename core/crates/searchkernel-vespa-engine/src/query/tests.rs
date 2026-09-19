use super::{
    parse_yql, validate_query, Query, RangeBound, Value, MAX_QUERY_DEPTH, MAX_QUERY_NODES,
    MAX_YQL_BYTES,
};

fn nested_not(depth: usize) -> Query {
    let mut query = Query::Term {
        field: "title".to_owned(),
        text: "fox".to_owned(),
    };
    for _ in 0..depth {
        query = Query::Not(Box::new(query));
    }
    query
}

#[test]
fn query_depth_is_bounded_before_lowering() {
    assert!(validate_query(&nested_not(MAX_QUERY_DEPTH + 1)).is_err());
    assert!(validate_query(&nested_not(MAX_QUERY_DEPTH)).is_ok());
}

#[test]
fn query_node_count_is_bounded() {
    let query = Query::And(
        (0..MAX_QUERY_NODES)
            .map(|_| Query::Term {
                field: "title".to_owned(),
                text: "fox".to_owned(),
            })
            .collect(),
    );
    assert!(validate_query(&query).is_err());
}

#[test]
fn yql_input_size_is_bounded() {
    let yql = format!(
        "select * from sources * where title contains \"{}\"",
        "x".repeat(MAX_YQL_BYTES)
    );
    assert!(Query::from_yql(&yql).is_err());
}

#[test]
fn yql_respects_boolean_precedence() {
    let query = Query::from_yql(
            "select * from sources * where title contains \"brown\" or title contains \"red\" and title contains \"fox\"",
        )
        .unwrap();
    assert_eq!(
        query,
        Query::Or(vec![
            Query::Term {
                field: "title".into(),
                text: "brown".into()
            },
            Query::And(vec![
                Query::Term {
                    field: "title".into(),
                    text: "red".into()
                },
                Query::Term {
                    field: "title".into(),
                    text: "fox".into()
                },
            ]),
        ])
    );
}

#[test]
fn yql_maps_phrase_and_open_bounds() {
    let phrase = Query::from_yql(
        "select * from sources * where title contains phrase(\"running\", \"foxes\")",
    )
    .unwrap();
    assert_eq!(
        phrase,
        Query::Phrase {
            field: "title".into(),
            text: "running foxes".into()
        }
    );
    let range =
        Query::from_yql("select * from sources * where ({bounds:\"rightOpen\"}range(qty, 10, 20))")
            .unwrap();
    assert_eq!(
        range,
        Query::Range {
            field: "qty".into(),
            lower: RangeBound::Included(10),
            upper: RangeBound::Excluded(20),
        }
    );
}

#[test]
fn yql_maps_full_range_to_exists() {
    let query =
        Query::from_yql("select * from typea where range(qty, -Infinity, Infinity)").unwrap();
    assert_eq!(
        query,
        Query::Exists {
            field: "qty".into()
        }
    );
}

#[test]
fn yql_normalizes_only_unannotated_reversed_ranges() {
    assert_eq!(
        Query::from_yql("select * from sources * where range(qty, 20, 10)").unwrap(),
        Query::Range {
            field: "qty".into(),
            lower: RangeBound::Included(10),
            upper: RangeBound::Included(20),
        }
    );
    assert_eq!(
        Query::from_yql("select * from sources * where ({bounds:\"rightOpen\"}range(qty, 20, 10))")
            .unwrap(),
        Query::Range {
            field: "qty".into(),
            lower: RangeBound::Included(20),
            upper: RangeBound::Excluded(10),
        }
    );
    assert_eq!(
        Query::from_yql("select * from sources * where ({bounds:\"open\"}range(qty, 10, 10))")
            .unwrap(),
        Query::Range {
            field: "qty".into(),
            lower: RangeBound::Excluded(10),
            upper: RangeBound::Excluded(10),
        }
    );
}

#[test]
fn explicit_closed_bounds_are_unsupported_but_text_is_preserved_raw() {
    let closed =
        Query::from_yql("select * from sources * where ({bounds:\"closed\"}range(qty, 10, 20))")
            .unwrap_err()
            .to_string();
    assert!(closed.starts_with("unsupported_yql:"));

    let term =
        Query::from_yql("select * from sources * where title contains \"brown fox\"").unwrap();
    assert_eq!(
        term,
        Query::Term {
            field: "title".into(),
            text: "brown fox".into(),
        }
    );

    let phrase = Query::from_yql(
        "select * from sources * where title contains phrase(\"new york\", \"hotel\")",
    )
    .unwrap();
    assert_eq!(
        phrase,
        Query::Phrase {
            field: "title".into(),
            text: "new york hotel".into(),
        }
    );
}

#[test]
fn yql_does_not_analyze_text_before_field_validation() {
    let query =
        Query::from_yql("select * from sources * where missing contains \"brown fox\"").unwrap();
    assert_eq!(
        query,
        Query::Term {
            field: "missing".into(),
            text: "brown fox".into(),
        }
    );
}

#[test]
fn range_is_available_as_a_literal_field_name() {
    assert_eq!(
        Query::from_yql("select * from sources * where range contains \"foo\"").unwrap(),
        Query::Term {
            field: "range".into(),
            text: "foo".into(),
        }
    );
}

#[test]
fn malformed_and_unsupported_yql_are_classified() {
    let malformed = Query::from_yql("select * from sources * where (title contains \"fox\"")
        .unwrap_err()
        .to_string();
    assert!(malformed.starts_with("invalid_yql:"));
    let unsupported =
        Query::from_yql("select * from sources * where title contains \"fox\" order by title")
            .unwrap_err()
            .to_string();
    assert!(unsupported.starts_with("unsupported_yql:"));
}

#[test]
fn typed_boolean_and_ranges_are_validated() {
    assert!(validate_query(&Query::And(Vec::new())).is_err());
    assert!(validate_query(&Query::Or(Vec::new())).is_err());
    assert!(validate_query(&Query::Equiv(vec![Query::Term {
        field: "title".into(),
        text: "fox".into(),
    }]))
    .is_err());
    assert!(validate_query(&Query::Range {
        field: "qty".into(),
        lower: RangeBound::Excluded(10),
        upper: RangeBound::Included(10),
    })
    .is_ok());
}

#[test]
fn specialized_query_nodes_validate_their_required_shape() {
    assert!(validate_query(&Query::SameElement {
        field: "tags".into(),
        children: vec![Query::term("tags", "one")],
    })
    .is_err());
    assert!(validate_query(&Query::GeoLocation {
        field: "location".into(),
        latitude: f64::NAN,
        longitude: 0.0,
        radius: "1km".into(),
    })
    .is_err());
    assert!(validate_query(&Query::Predicate {
        field: "access".into(),
        features: Vec::new(),
        range_features: Vec::new(),
    })
    .is_err());
    assert!(validate_query(&Query::NearestNeighbor {
        document_field: "embedding".into(),
        query_tensor: "query_embedding".into(),
        target_num_hits: 100,
        approximate: true,
    })
    .is_ok());
}

#[test]
fn parser_accepts_named_source_without_changing_query() {
    let parsed = Query::from_yql("select * from typea where title contains \"fox\"").unwrap();
    assert_eq!(
        parsed,
        Query::Term {
            field: "title".into(),
            text: "fox".into()
        }
    );
}

#[test]
fn internal_parser_preserves_named_query_source() {
    let parsed = parse_yql("select * from typea where title contains \"fox\"").unwrap();
    assert_eq!(parsed.source, "typea");
    assert_eq!(
        parsed.query,
        Query::Term {
            field: "title".into(),
            text: "fox".into(),
        }
    );

    let wildcard = parse_yql("select * from sources * where title contains \"fox\"").unwrap();
    assert_eq!(wildcard.source, "*");
}

#[test]
fn yql_normalizes_and_negatives_after_positive_children() {
    let parsed = Query::from_yql(
            "select * from sources * where !(title contains \"scarlet\") and title contains \"quartz\" and !(title contains \"quiet\")",
        )
        .unwrap();
    assert_eq!(
        parsed,
        Query::And(vec![
            Query::Term {
                field: "title".into(),
                text: "quartz".into(),
            },
            Query::Not(Box::new(Query::Term {
                field: "title".into(),
                text: "scarlet".into(),
            })),
            Query::Not(Box::new(Query::Term {
                field: "title".into(),
                text: "quiet".into(),
            })),
        ])
    );
}

#[test]
fn yql_accepts_negative_i64_values() {
    assert_eq!(
        Query::from_yql("select * from sources * where qty = -10").unwrap(),
        Query::Equals {
            field: "qty".into(),
            value: -10
        }
    );
    assert_eq!(
        Query::from_yql("select * from sources * where range(qty, -9223372036854775808, -1)")
            .unwrap(),
        Query::Range {
            field: "qty".into(),
            lower: RangeBound::Included(i64::MIN),
            upper: RangeBound::Included(-1)
        }
    );
}

#[test]
fn typed_query_validation_rejects_empty_fields_and_nonfinite_values() {
    let invalid_queries = [
        Query::Term {
            field: String::new(),
            text: "fox".into(),
        },
        Query::Phrase {
            field: "title".into(),
            text: String::new(),
        },
        Query::Equals {
            field: String::new(),
            value: 1,
        },
        Query::Exists {
            field: String::new(),
        },
        Query::Range {
            field: String::new(),
            lower: RangeBound::Unbounded,
            upper: RangeBound::Unbounded,
        },
        Query::AttributeEquals {
            field: String::new(),
            value: Value::String("fox".into()),
        },
        Query::AttributeEquals {
            field: "price".into(),
            value: Value::Float(f64::NAN),
        },
        Query::AttributeRange {
            field: String::new(),
            lower: super::AttributeBound::Unbounded,
            upper: super::AttributeBound::Unbounded,
        },
    ];
    for query in invalid_queries {
        assert!(
            validate_query(&query).is_err(),
            "accepted invalid query: {query:?}"
        );
    }

    assert!(validate_query(&Query::AttributeEquals {
        field: "price".into(),
        value: Value::Float(1.5),
    })
    .is_ok());
}
