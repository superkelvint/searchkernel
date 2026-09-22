//! Canonical scripted response fixtures shared by result conformance tests.
//!
//! Keep result fixtures here rather than inside one client or transport test.
//! The portable result contract is defined by `searchkernel_ir::SearchResult`;
//! language-specific tests should consume responses produced from these values
//! rather than re-inventing equivalent trees independently.

use std::collections::BTreeMap;

use searchkernel_ir as ir;
use searchkernel_protocol::v2 as api;

fn result(root: ir::ResultNode, total_hits: u64) -> ir::SearchResult {
    ir::SearchResult {
        root,
        total_hits: ir::TotalHits {
            value: total_hits,
            relation: ir::TotalHitsRelation::Exact,
        },
        coverage: None,
        timing: None,
        errors: Vec::new(),
        trace: None,
        metadata: BTreeMap::new(),
    }
}

fn hit(id: &str, score: Option<f64>) -> ir::Hit {
    ir::Hit {
        id: id.to_owned(),
        score,
        ..Default::default()
    }
}

fn group(children: Vec<ir::ResultNode>) -> ir::ResultGroup {
    ir::ResultGroup::root(children)
}

/// A canonical result whose root is a hit rather than the usual root group.
pub fn root_hit_result() -> ir::SearchResult {
    let mut root = hit("root-hit", Some(1.25));
    root.fields
        .insert("title".into(), ir::Value::String("root".into()));
    result(ir::ResultNode::Hit(root), 1)
}

/// A root group containing two direct hits.
pub fn one_group_containing_hits_result() -> ir::SearchResult {
    result(
        ir::ResultNode::Group(group(vec![
            ir::ResultNode::Hit(hit("group-hit-1", Some(2.0))),
            ir::ResultNode::Hit(hit("group-hit-2", None)),
        ])),
        2,
    )
}

/// A recursively nested group tree ending in a hit.
pub fn nested_groups_result() -> ir::SearchResult {
    let leaf = ir::ResultGroup {
        id: Some("leaf".into()),
        label: Some("Leaf".into()),
        value: Some(ir::Value::String("child".into())),
        children: vec![ir::ResultNode::Hit(hit("nested-hit", Some(3.0)))],
        ..group(Vec::new())
    };
    let middle = ir::ResultGroup {
        id: Some("middle".into()),
        label: Some("Middle".into()),
        children: vec![ir::ResultNode::Group(leaf)],
        ..group(Vec::new())
    };
    let root = ir::ResultGroup {
        id: Some("root".into()),
        children: vec![ir::ResultNode::Group(middle)],
        ..group(Vec::new())
    };
    result(ir::ResultNode::Group(root), 1)
}

/// A mixed root whose child ordering is Hit, Group A, Hit, Group B.
pub fn mixed_hit_and_group_children_result() -> ir::SearchResult {
    let group_a = ir::ResultGroup {
        id: Some("group-a".into()),
        label: Some("A".into()),
        value: Some(ir::Value::String("A".into())),
        children: vec![ir::ResultNode::Hit(hit("nested-a", Some(4.0)))],
        ..group(Vec::new())
    };
    let group_b = ir::ResultGroup {
        id: Some("group-b".into()),
        label: Some("B".into()),
        value: Some(ir::Value::String("B".into())),
        children: vec![ir::ResultNode::Hit(hit("nested-b", Some(5.0)))],
        ..group(Vec::new())
    };
    let root = group(vec![
        ir::ResultNode::Hit(hit("root-a", Some(1.0))),
        ir::ResultNode::Group(group_a),
        ir::ResultNode::Hit(hit("root-b", Some(2.0))),
        ir::ResultNode::Group(group_b),
    ]);
    result(ir::ResultNode::Group(root), 4)
}

/// Sibling groups whose canonical order must remain A, B, C.
pub fn sibling_groups_result() -> ir::SearchResult {
    let groups = ["A", "B", "C"]
        .into_iter()
        .map(|label| {
            ir::ResultNode::Group(ir::ResultGroup {
                id: Some(format!("group-{label}")),
                label: Some(label.to_owned()),
                value: Some(ir::Value::String(label.to_string())),
                children: vec![ir::ResultNode::Hit(hit(&format!("hit-{label}"), Some(1.0)))],
                ..group(Vec::new())
            })
        })
        .collect();
    result(ir::ResultNode::Group(group(groups)), 3)
}

/// A root group with fields that become the legacy aggregation projection.
pub fn group_fields_and_aggregations_result() -> ir::SearchResult {
    let root = ir::ResultGroup {
        fields: BTreeMap::from([
            ("count".into(), ir::Value::UInt64(7)),
            ("max_price".into(), ir::Value::Float64(19.5)),
        ]),
        children: vec![ir::ResultNode::Group(ir::ResultGroup {
            id: Some("brand".into()),
            label: Some("Brand".into()),
            fields: BTreeMap::from([("count".into(), ir::Value::UInt64(3))]),
            ..group(Vec::new())
        })],
        ..group(Vec::new())
    };
    result(ir::ResultNode::Group(root), 7)
}

/// The exact structured shape recognized by the Rust facet compatibility view.
pub fn canonical_facet_result() -> ir::SearchResult {
    let bucket = ir::ResultGroup {
        value: Some(ir::Value::String("acme".into())),
        fields: BTreeMap::from([("count".into(), ir::Value::UInt64(4))]),
        ..group(Vec::new())
    };
    let facet = ir::ResultGroup {
        id: Some("brand".into()),
        label: Some("brands".into()),
        children: vec![ir::ResultNode::Group(bucket)],
        ..group(Vec::new())
    };
    result(
        ir::ResultNode::Group(group(vec![ir::ResultNode::Group(facet)])),
        4,
    )
}

/// A labeled grouping intentionally made non-canonical by meaningful metadata.
pub fn non_facet_grouping_result() -> ir::SearchResult {
    let ordinary = ir::ResultGroup {
        id: Some("brand".into()),
        label: Some("brands".into()),
        metadata: BTreeMap::from([("owner".into(), ir::Value::String("user".into()))]),
        children: vec![ir::ResultNode::Group(ir::ResultGroup {
            value: Some(ir::Value::String("acme".into())),
            fields: BTreeMap::from([("count".into(), ir::Value::UInt64(4))]),
            ..group(Vec::new())
        })],
        ..group(Vec::new())
    };
    result(
        ir::ResultNode::Group(group(vec![ir::ResultNode::Group(ordinary)])),
        4,
    )
}

/// A group carrying every group-level compatibility field.
pub fn continuations_limits_and_metadata_result() -> ir::SearchResult {
    let root = ir::ResultGroup {
        id: Some("limited".into()),
        label: Some("Limited".into()),
        value: Some(ir::Value::Int64(9)),
        children: vec![ir::ResultNode::Hit(hit("limited-hit", Some(6.0)))],
        continuations: BTreeMap::from([
            ("next".into(), "token-2".into()),
            ("prev".into(), "token-0".into()),
        ]),
        source: Some("group-source".into()),
        types: vec!["bucket".into(), "result".into()],
        score: Some(f64::NEG_INFINITY),
        limits: Some(ir::ResultLimits {
            from: Some(ir::Value::UInt64(10)),
            to: Some(ir::Value::UInt64(20)),
        }),
        metadata: BTreeMap::from([("owner".into(), ir::Value::Bool(true))]),
        ..group(Vec::new())
    };
    result(ir::ResultNode::Group(root), 1)
}

/// A hit with fields, highlights, all feature maps, and sort values.
pub fn hit_features_result() -> ir::SearchResult {
    let hit = ir::Hit {
        id: "feature-hit".into(),
        score: Some(4.5),
        fields: BTreeMap::from([
            ("title".into(), ir::Value::String("First".into())),
            ("bytes".into(), ir::Value::Bytes(vec![1, 2, 255])),
            (
                "nested".into(),
                ir::Value::Object(BTreeMap::from([("enabled".into(), ir::Value::Bool(true))])),
            ),
        ]),
        highlights: vec![ir::HighlightResult {
            field: "title".into(),
            fragments: vec![ir::HighlightFragment {
                text: "<em>First</em>".into(),
                matches: vec![ir::HighlightSpan {
                    start_byte: 0,
                    end_byte: 5,
                }],
            }],
        }],
        source: Some("catalog".into()),
        types: vec!["product".into()],
        match_features: BTreeMap::from([("bm25".into(), ir::Value::Float64(2.0))]),
        summary_features: BTreeMap::from([("summary".into(), ir::Value::String("ready".into()))]),
        rank_features: BTreeMap::from([("rank".into(), ir::Value::Float64(8.0))]),
        sort_values: vec![ir::Value::UInt64(1), ir::Value::String("First".into())],
    };
    result(ir::ResultNode::Hit(hit), 1)
}

/// Coverage, timing, structured errors, trace, and top-level metadata.
pub fn envelope_metadata_result() -> ir::SearchResult {
    let mut result = result(ir::ResultNode::Group(group(Vec::new())), 12);
    result.coverage = Some(ir::Coverage {
        coverage_percent: Some(99.0),
        documents: Some(12),
        full: Some(true),
        nodes: Some(2),
        results: Some(12),
        results_full: Some(12),
        degraded: Some(ir::CoverageDegraded {
            match_phase: Some(true),
            timeout: Some(false),
            adaptive_timeout: Some(true),
            ann_timeout: Some(false),
            non_ideal_state: Some(true),
            metadata: BTreeMap::from([("reason".into(), ir::Value::String("load".into()))]),
        }),
        metadata: BTreeMap::from([("region".into(), ir::Value::String("west".into()))]),
    });
    result.timing = Some(ir::SearchTiming {
        query_time_seconds: Some(0.001),
        search_time_seconds: Some(0.002),
        summary_fetch_time_seconds: Some(0.003),
        metadata: BTreeMap::from([("phase".into(), ir::Value::String("done".into()))]),
    });
    result.errors.push(ir::SearchResultError {
        code: 503,
        summary: Some("partial".into()),
        message: Some("one node timed out".into()),
        source: Some("node-2".into()),
        stack_trace: Some("trace".into()),
        transient: Some(true),
        metadata: BTreeMap::from([("node".into(), ir::Value::String("2".into()))]),
    });
    result.trace = Some(ir::Value::Object(BTreeMap::from([(
        "level".into(),
        ir::Value::UInt64(3),
    )])));
    result.metadata = BTreeMap::from([("request_id".into(), ir::Value::String("r1".into()))]);
    result
}

/// A result containing meaningful absent optional fields.
pub fn absent_optional_fields_result() -> ir::SearchResult {
    result(ir::ResultNode::Group(group(Vec::new())), 0)
}

/// The corresponding present-but-empty optional fields.
pub fn present_empty_optional_fields_result() -> ir::SearchResult {
    let root = ir::ResultGroup {
        value: Some(ir::Value::List(Vec::new())),
        source: Some(String::new()),
        limits: Some(ir::ResultLimits {
            from: None,
            to: None,
        }),
        ..group(Vec::new())
    };
    let mut result = result(ir::ResultNode::Group(root), 0);
    result.coverage = Some(ir::Coverage {
        coverage_percent: None,
        documents: None,
        full: None,
        nodes: None,
        results: None,
        results_full: None,
        degraded: None,
        metadata: BTreeMap::new(),
    });
    result.timing = Some(ir::SearchTiming {
        query_time_seconds: None,
        search_time_seconds: None,
        summary_fetch_time_seconds: None,
        metadata: BTreeMap::new(),
    });
    result.trace = Some(ir::Value::Object(BTreeMap::new()));
    result
}

/// Construct a result for each supported relevance union, including absence.
pub fn relevance_result(score: Option<f64>) -> ir::SearchResult {
    result(
        ir::ResultNode::Hit(hit("relevance-hit", score)),
        u64::from(score.is_some()),
    )
}

pub fn finite_relevance_result() -> ir::SearchResult {
    relevance_result(Some(1.25))
}

pub fn positive_infinity_relevance_result() -> ir::SearchResult {
    relevance_result(Some(f64::INFINITY))
}

pub fn negative_infinity_relevance_result() -> ir::SearchResult {
    relevance_result(Some(f64::NEG_INFINITY))
}

/// The original rich fixture remains available for callers that want one case.
pub fn representative_search_result() -> ir::SearchResult {
    let root = ir::ResultGroup {
        id: Some("root".into()),
        label: Some("Root".into()),
        value: Some(ir::Value::String("all".into())),
        fields: BTreeMap::from([("count".into(), ir::Value::UInt64(2))]),
        children: vec![
            ir::ResultNode::Hit(ir::Hit {
                id: "doc-1".into(),
                score: Some(4.5),
                fields: BTreeMap::from([("title".into(), ir::Value::String("First".into()))]),
                highlights: vec![ir::HighlightResult {
                    field: "title".into(),
                    fragments: vec![ir::HighlightFragment {
                        text: "<em>First</em>".into(),
                        matches: vec![ir::HighlightSpan {
                            start_byte: 0,
                            end_byte: 5,
                        }],
                    }],
                }],
                source: Some("catalog".into()),
                types: vec!["product".into()],
                match_features: BTreeMap::from([("bm25".into(), ir::Value::Float64(2.0))]),
                summary_features: BTreeMap::from([(
                    "summary".into(),
                    ir::Value::String("ready".into()),
                )]),
                rank_features: BTreeMap::from([("rank".into(), ir::Value::Float64(8.0))]),
                sort_values: vec![ir::Value::UInt64(1)],
            }),
            ir::ResultNode::Group(ir::ResultGroup {
                id: Some("nested".into()),
                label: Some("Nested".into()),
                value: Some(ir::Value::Int64(7)),
                fields: BTreeMap::from([("count".into(), ir::Value::UInt64(1))]),
                children: vec![ir::ResultNode::Hit(ir::Hit {
                    id: "doc-2".into(),
                    score: Some(1.25),
                    fields: BTreeMap::from([("price".into(), ir::Value::Float64(12.5))]),
                    ..Default::default()
                })],
                continuations: BTreeMap::from([("next".into(), "token".into())]),
                source: Some("group-source".into()),
                types: vec!["bucket".into()],
                score: Some(2.5),
                limits: Some(ir::ResultLimits {
                    from: Some(ir::Value::UInt64(0)),
                    to: Some(ir::Value::UInt64(10)),
                }),
                metadata: BTreeMap::from([("owner".into(), ir::Value::Bool(true))]),
            }),
        ],
        continuations: BTreeMap::from([("page".into(), "token-1".into())]),
        source: Some("search".into()),
        types: vec!["root".into()],
        score: Some(3.5),
        limits: Some(ir::ResultLimits {
            from: Some(ir::Value::UInt64(0)),
            to: Some(ir::Value::UInt64(20)),
        }),
        metadata: BTreeMap::from([("trace_id".into(), ir::Value::String("r-1".into()))]),
    };
    let mut result = result(ir::ResultNode::Group(root), 12);
    result.total_hits.relation = ir::TotalHitsRelation::LowerBound;
    let envelope = envelope_metadata_result();
    result.coverage = envelope.coverage;
    result.timing = envelope.timing;
    result.errors = envelope.errors;
    result.trace = envelope.trace;
    result.metadata = BTreeMap::from([("request_id".into(), ir::Value::String("r1".into()))]);
    result
}

/// Fresh named values for the canonical result corpus.
pub fn canonical_result_fixtures() -> Vec<(&'static str, ir::SearchResult)> {
    vec![
        ("root_hit", root_hit_result()),
        (
            "one_group_containing_hits",
            one_group_containing_hits_result(),
        ),
        ("nested_groups", nested_groups_result()),
        (
            "mixed_hit_and_group_children",
            mixed_hit_and_group_children_result(),
        ),
        ("sibling_groups", sibling_groups_result()),
        (
            "group_fields_and_aggregations",
            group_fields_and_aggregations_result(),
        ),
        ("canonical_facet", canonical_facet_result()),
        ("non_facet_grouping", non_facet_grouping_result()),
        (
            "continuations_limits_and_metadata",
            continuations_limits_and_metadata_result(),
        ),
        ("hit_features", hit_features_result()),
        ("envelope_metadata", envelope_metadata_result()),
        ("absent_optional_fields", absent_optional_fields_result()),
        (
            "present_empty_optional_fields",
            present_empty_optional_fields_result(),
        ),
        ("finite_relevance", finite_relevance_result()),
        (
            "positive_infinity_relevance",
            positive_infinity_relevance_result(),
        ),
        (
            "negative_infinity_relevance",
            negative_infinity_relevance_result(),
        ),
        ("absent_relevance", relevance_result(None)),
        ("representative", representative_search_result()),
    ]
}

/// Assert that a production-encoded FlatBuffers result still contains the
/// expected canonical tree and envelope values.
pub fn assert_api_search_result_matches(
    actual: api::SearchResult<'_>,
    expected: &ir::SearchResult,
) {
    assert_eq!(
        actual.total_hits().unwrap().value(),
        expected.total_hits.value
    );
    assert_eq!(
        actual.total_hits().unwrap().relation(),
        match expected.total_hits.relation {
            ir::TotalHitsRelation::Unspecified => api::TotalHitsRelation::Unspecified,
            ir::TotalHitsRelation::Exact => api::TotalHitsRelation::Exact,
            ir::TotalHitsRelation::LowerBound => api::TotalHitsRelation::LowerBound,
            ir::TotalHitsRelation::Approximate => api::TotalHitsRelation::Approximate,
            ir::TotalHitsRelation::Unknown => api::TotalHitsRelation::Unknown,
        }
    );
    assert_node_matches(actual.root().unwrap(), &expected.root);
    assert_optional_envelope(actual, expected);
}

fn assert_node_matches(actual: api::ResultNode<'_>, expected: &ir::ResultNode) {
    match expected {
        ir::ResultNode::Hit(expected) => {
            let actual = actual.node_as_hit().expect("expected Hit result node");
            assert_eq!(actual.id(), Some(expected.id.as_str()));
            assert_relevance_matches(actual.relevance(), expected.score);
            assert_value_map_matches(actual.fields(), &expected.fields);
            assert_eq!(actual.source(), expected.source.as_deref());
            assert_eq!(
                actual.types().map_or(0, |values| values.len()),
                expected.types.len()
            );
            if let Some(values) = actual.types() {
                assert_eq!(
                    values.iter().collect::<Vec<_>>(),
                    expected
                        .types
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>()
                );
            }
            assert_value_map_matches(actual.match_features(), &expected.match_features);
            assert_value_map_matches(actual.summary_features(), &expected.summary_features);
            assert_value_map_matches(actual.rank_features(), &expected.rank_features);
            let sort_values = actual
                .sort_values()
                .map(|values| values.iter().collect::<Vec<_>>());
            assert_eq!(
                sort_values.as_ref().map_or(0, Vec::len),
                expected.sort_values.len()
            );
            if let Some(values) = sort_values {
                for (actual, expected) in values.into_iter().zip(&expected.sort_values) {
                    assert_value_matches(actual, expected);
                }
            }
            assert_eq!(
                actual.highlights().map_or(0, |values| values.len()),
                expected.highlights.len()
            );
            if let Some(highlights) = actual.highlights() {
                for (actual, expected) in highlights.iter().zip(&expected.highlights) {
                    assert_eq!(actual.field(), Some(expected.field.as_str()));
                    let fragments = actual.fragments().unwrap();
                    assert_eq!(fragments.len(), expected.fragments.len());
                    for (actual, expected) in fragments.iter().zip(&expected.fragments) {
                        assert_eq!(actual.text(), Some(expected.text.as_str()));
                        let matches = actual.matches().unwrap();
                        assert_eq!(matches.len(), expected.matches.len());
                        for (actual, expected) in matches.iter().zip(&expected.matches) {
                            assert_eq!(actual.start_byte(), expected.start_byte);
                            assert_eq!(actual.end_byte(), expected.end_byte);
                        }
                    }
                }
            }
        }
        ir::ResultNode::Group(expected) => {
            let actual = actual
                .node_as_result_group()
                .expect("expected ResultGroup result node");
            assert_eq!(actual.id(), expected.id.as_deref());
            assert_eq!(actual.label(), expected.label.as_deref());
            assert_optional_value_matches(actual.value(), expected.value.as_ref());
            assert_value_map_matches(actual.fields(), &expected.fields);
            assert_eq!(actual.source(), expected.source.as_deref());
            assert_eq!(
                actual.types().map_or(0, |values| values.len()),
                expected.types.len()
            );
            if let Some(values) = actual.types() {
                assert_eq!(
                    values.iter().collect::<Vec<_>>(),
                    expected
                        .types
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>()
                );
            }
            assert_relevance_matches(actual.relevance(), expected.score);
            assert_eq!(
                actual.continuations().map_or(0, |values| values.len()),
                expected.continuations.len()
            );
            if let Some(values) = actual.continuations() {
                for (actual, (key, value)) in values.iter().zip(&expected.continuations) {
                    assert_eq!(actual.key(), key);
                    assert_eq!(actual.value(), Some(value.as_str()));
                }
            }
            assert_eq!(
                actual.children().map_or(0, |values| values.len()),
                expected.children.len()
            );
            if let Some(children) = actual.children() {
                for (actual, expected) in children.iter().zip(&expected.children) {
                    assert_node_matches(actual, expected);
                }
            }
            match (&actual.limits(), &expected.limits) {
                (None, None) => {}
                (Some(actual), Some(expected)) => {
                    assert_optional_value_matches(actual.from(), expected.from.as_ref());
                    assert_optional_value_matches(actual.to(), expected.to.as_ref());
                }
                _ => panic!("result limits presence changed"),
            }
            assert_value_map_matches(actual.metadata(), &expected.metadata);
        }
    }
}

fn assert_relevance_matches(actual: Option<api::Relevance<'_>>, expected: Option<f64>) {
    match (actual, expected) {
        (None, None) => {}
        (Some(actual), Some(expected)) if expected.is_finite() => assert_eq!(
            actual.value_as_finite_relevance().unwrap().value(),
            expected
        ),
        (Some(actual), Some(expected)) if expected.is_infinite() && expected.is_sign_positive() => {
            assert!(actual.value_as_positive_infinity_relevance().is_some())
        }
        (Some(actual), Some(expected)) if expected.is_infinite() => {
            assert!(actual.value_as_negative_infinity_relevance().is_some())
        }
        _ => panic!("relevance presence or union changed"),
    }
}

fn assert_optional_value_matches(actual: Option<api::Value<'_>>, expected: Option<&ir::Value>) {
    match (actual, expected) {
        (None, None) => {}
        (Some(actual), Some(expected)) => assert_value_matches(actual, expected),
        _ => panic!("optional result value presence changed"),
    }
}

fn assert_value_map_matches(
    actual: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::ValueEntry<'_>>>>,
    expected: &BTreeMap<String, ir::Value>,
) {
    assert_eq!(actual.map_or(0, |values| values.len()), expected.len());
    if let Some(values) = actual {
        for (actual, (key, expected)) in values.iter().zip(expected) {
            assert_eq!(actual.key(), key);
            assert_value_matches(actual.value().unwrap(), expected);
        }
    }
}

fn assert_value_matches(actual: api::Value<'_>, expected: &ir::Value) {
    match expected {
        ir::Value::Null => assert!(actual.kind_as_null_value_value().is_some()),
        ir::Value::Bool(expected) => {
            assert_eq!(actual.kind_as_bool_value().unwrap().value(), *expected)
        }
        ir::Value::Int64(expected) => {
            assert_eq!(actual.kind_as_int_64_value().unwrap().value(), *expected)
        }
        ir::Value::UInt64(expected) => {
            assert_eq!(actual.kind_as_uint_64_value().unwrap().value(), *expected)
        }
        ir::Value::Float64(expected) if expected.is_finite() => {
            assert_eq!(actual.kind_as_float_64_value().unwrap().value(), *expected)
        }
        ir::Value::Float64(expected) if expected.is_sign_positive() => assert_eq!(
            actual.kind_as_non_finite_float_value().unwrap().kind(),
            api::NonFiniteFloatKind::PositiveInfinity
        ),
        ir::Value::Float64(_) => assert_eq!(
            actual.kind_as_non_finite_float_value().unwrap().kind(),
            api::NonFiniteFloatKind::NegativeInfinity
        ),
        ir::Value::String(expected) => {
            assert_eq!(
                actual.kind_as_string_value().unwrap().value(),
                Some(expected.as_str())
            )
        }
        ir::Value::Bytes(expected) => assert_eq!(
            actual
                .kind_as_bytes_value()
                .unwrap()
                .value()
                .unwrap()
                .iter()
                .collect::<Vec<_>>(),
            expected.as_slice()
        ),
        ir::Value::List(expected) => {
            let values = actual.kind_as_value_list().unwrap().values().unwrap();
            assert_eq!(values.len(), expected.len());
            for (actual, expected) in values.iter().zip(expected) {
                assert_value_matches(actual, expected);
            }
        }
        ir::Value::Object(expected) => {
            assert_value_map_matches(actual.kind_as_value_object().unwrap().fields(), expected)
        }
        ir::Value::Tensor(_) => panic!("fixture assertion does not cover tensor values"),
    }
}

fn assert_optional_envelope(actual: api::SearchResult<'_>, expected: &ir::SearchResult) {
    assert_eq!(actual.coverage().is_some(), expected.coverage.is_some());
    if let (Some(actual), Some(expected)) = (actual.coverage(), expected.coverage.as_ref()) {
        assert_optional_double(actual.coverage_percent(), expected.coverage_percent);
        assert_optional_u64(actual.documents(), expected.documents);
        assert_optional_bool(actual.full(), expected.full);
        assert_optional_u32(actual.nodes(), expected.nodes);
        assert_optional_u64(actual.results(), expected.results);
        assert_optional_u64(actual.results_full(), expected.results_full);
        assert_value_map_matches(actual.metadata(), &expected.metadata);
        assert_eq!(actual.degraded().is_some(), expected.degraded.is_some());
        if let (Some(actual), Some(expected)) = (actual.degraded(), expected.degraded.as_ref()) {
            assert_optional_bool(actual.match_phase(), expected.match_phase);
            assert_optional_bool(actual.timeout(), expected.timeout);
            assert_optional_bool(actual.adaptive_timeout(), expected.adaptive_timeout);
            assert_optional_bool(actual.ann_timeout(), expected.ann_timeout);
            assert_optional_bool(actual.non_ideal_state(), expected.non_ideal_state);
            assert_value_map_matches(actual.metadata(), &expected.metadata);
        }
    }
    assert_eq!(actual.timing().is_some(), expected.timing.is_some());
    if let (Some(actual), Some(expected)) = (actual.timing(), expected.timing.as_ref()) {
        assert_optional_double(actual.query_time_seconds(), expected.query_time_seconds);
        assert_optional_double(actual.search_time_seconds(), expected.search_time_seconds);
        assert_optional_double(
            actual.summary_fetch_time_seconds(),
            expected.summary_fetch_time_seconds,
        );
        assert_value_map_matches(actual.metadata(), &expected.metadata);
    }
    assert_eq!(
        actual.errors().map_or(0, |values| values.len()),
        expected.errors.len()
    );
    if let Some(errors) = actual.errors() {
        for (actual, expected) in errors.iter().zip(&expected.errors) {
            assert_eq!(actual.code(), expected.code);
            assert_eq!(actual.summary(), expected.summary.as_deref());
            assert_eq!(actual.message(), expected.message.as_deref());
            assert_eq!(actual.source(), expected.source.as_deref());
            assert_eq!(actual.stack_trace(), expected.stack_trace.as_deref());
            assert_optional_bool(actual.transient(), expected.transient);
            assert_value_map_matches(actual.metadata(), &expected.metadata);
        }
    }
    assert_optional_value_matches(actual.trace(), expected.trace.as_ref());
    assert_value_map_matches(actual.metadata(), &expected.metadata);
}

fn assert_optional_bool(actual: Option<&api::OptionalBool>, expected: Option<bool>) {
    assert_eq!(actual.map(|value| value.value()), expected);
}

fn assert_optional_double(actual: Option<&api::OptionalDouble>, expected: Option<f64>) {
    assert_eq!(actual.map(|value| value.value()), expected);
}

fn assert_optional_u32(actual: Option<&api::OptionalUInt32>, expected: Option<u32>) {
    assert_eq!(actual.map(|value| value.value()), expected);
}

fn assert_optional_u64(actual: Option<&api::OptionalUInt64>, expected: Option<u64>) {
    assert_eq!(actual.map(|value| value.value()), expected);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corpus_is_fresh_named_and_covers_the_canonical_cases() {
        let mut first = canonical_result_fixtures();
        let second = canonical_result_fixtures();
        assert_eq!(
            first.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
            second.iter().map(|(name, _)| *name).collect::<Vec<_>>()
        );
        assert!(first.len() >= 17);
        first[0].1.root = ir::ResultNode::Hit(hit("changed", None));
        assert_ne!(first[0].1.root, second[0].1.root);
    }

    #[test]
    fn every_fixture_survives_ir_to_flatbuffers() {
        for (name, expected) in canonical_result_fixtures() {
            let bytes = searchkernel_ir::search_result_to_api(&expected)
                .unwrap_or_else(|error| panic!("fixture {name} must encode: {error:?}"));
            let actual = searchkernel_ir::__flatbuffers::root::<api::SearchResult>(&bytes)
                .unwrap_or_else(|error| panic!("fixture {name} must decode: {error:?}"));
            assert_api_search_result_matches(actual, &expected);
        }
    }

    #[test]
    fn relevance_fixtures_use_the_three_relevance_unions_and_absence() {
        let cases = [
            (
                finite_relevance_result(),
                api::RelevanceKind::FiniteRelevance,
            ),
            (
                positive_infinity_relevance_result(),
                api::RelevanceKind::PositiveInfinityRelevance,
            ),
            (
                negative_infinity_relevance_result(),
                api::RelevanceKind::NegativeInfinityRelevance,
            ),
        ];
        for (result, expected_kind) in cases {
            let bytes = searchkernel_ir::search_result_to_api(&result).unwrap();
            let encoded =
                searchkernel_ir::__flatbuffers::root::<api::SearchResult>(&bytes).unwrap();
            assert_eq!(
                encoded
                    .root()
                    .unwrap()
                    .node_as_hit()
                    .unwrap()
                    .relevance()
                    .unwrap()
                    .value_type(),
                expected_kind
            );
        }
        let bytes = searchkernel_ir::search_result_to_api(&relevance_result(None)).unwrap();
        let encoded = searchkernel_ir::__flatbuffers::root::<api::SearchResult>(&bytes).unwrap();
        assert!(encoded
            .root()
            .unwrap()
            .node_as_hit()
            .unwrap()
            .relevance()
            .is_none());
    }
}
