//! Canonical scripted response fixtures shared by result conformance tests.
//!
//! Keep result fixtures here rather than inside one client or transport test.
//! The portable result contract is defined by `searchkernel_ir::SearchResult`;
//! language-specific tests should consume responses produced from these values
//! rather than re-inventing equivalent trees independently.

use std::collections::BTreeMap;

/// One named canonical response used by cross-language result conformance
/// tests. The response bytes are generated from these IR values; clients must
/// not reproduce the tree in language-specific object constructors.
#[derive(Clone, Debug)]
pub struct ResultFixture {
    pub name: &'static str,
    pub result: searchkernel_ir::SearchResult,
}

/// Representative canonical result tree used by response-path conformance.
///
/// This intentionally exercises fields that are easy for SDK projections to
/// drop: recursive mixed hit/group structure, group metadata, three feature
/// maps, sort values, coverage/timing, structured errors, trace, and metadata.
pub fn representative_search_result() -> searchkernel_ir::SearchResult {
    searchkernel_ir::SearchResult {
        root: searchkernel_ir::ResultNode::Group(searchkernel_ir::ResultGroup {
            id: Some("root".into()),
            label: Some("Root".into()),
            value: Some(searchkernel_ir::Value::String("all".into())),
            fields: BTreeMap::from([("count".into(), searchkernel_ir::Value::UInt64(2))]),
            children: vec![
                searchkernel_ir::ResultNode::Hit(searchkernel_ir::Hit {
                    id: "doc-1".into(),
                    score: Some(4.5),
                    fields: BTreeMap::from([(
                        "title".into(),
                        searchkernel_ir::Value::String("First".into()),
                    )]),
                    highlights: vec![searchkernel_ir::HighlightResult {
                        field: "title".into(),
                        fragments: vec![searchkernel_ir::HighlightFragment {
                            text: "<em>First</em>".into(),
                            matches: vec![searchkernel_ir::HighlightSpan {
                                start_byte: 0,
                                end_byte: 5,
                            }],
                        }],
                    }],
                    source: Some("catalog".into()),
                    types: vec!["product".into()],
                    match_features: BTreeMap::from([(
                        "bm25".into(),
                        searchkernel_ir::Value::Float64(2.0),
                    )]),
                    summary_features: BTreeMap::from([(
                        "summary".into(),
                        searchkernel_ir::Value::String("ready".into()),
                    )]),
                    rank_features: BTreeMap::from([(
                        "rank".into(),
                        searchkernel_ir::Value::Float64(8.0),
                    )]),
                    sort_values: vec![searchkernel_ir::Value::UInt64(1)],
                }),
                searchkernel_ir::ResultNode::Group(searchkernel_ir::ResultGroup {
                    id: Some("nested".into()),
                    label: Some("Nested".into()),
                    value: Some(searchkernel_ir::Value::Int64(7)),
                    fields: BTreeMap::from([("count".into(), searchkernel_ir::Value::UInt64(1))]),
                    children: vec![searchkernel_ir::ResultNode::Hit(searchkernel_ir::Hit {
                        id: "doc-2".into(),
                        score: Some(1.25),
                        fields: BTreeMap::from([(
                            "price".into(),
                            searchkernel_ir::Value::Float64(12.5),
                        )]),
                        ..Default::default()
                    })],
                    continuations: BTreeMap::from([("next".into(), "token".into())]),
                    source: Some("group-source".into()),
                    types: vec!["bucket".into()],
                    score: Some(2.5),
                    limits: Some(searchkernel_ir::ResultLimits {
                        from: Some(searchkernel_ir::Value::UInt64(0)),
                        to: Some(searchkernel_ir::Value::UInt64(10)),
                    }),
                    metadata: BTreeMap::from([(
                        "owner".into(),
                        searchkernel_ir::Value::Bool(true),
                    )]),
                }),
            ],
            continuations: BTreeMap::from([("page".into(), "token-1".into())]),
            source: Some("search".into()),
            types: vec!["root".into()],
            score: Some(3.5),
            limits: Some(searchkernel_ir::ResultLimits {
                from: Some(searchkernel_ir::Value::UInt64(0)),
                to: Some(searchkernel_ir::Value::UInt64(20)),
            }),
            metadata: BTreeMap::from([(
                "trace_id".into(),
                searchkernel_ir::Value::String("r-1".into()),
            )]),
        }),
        total_hits: searchkernel_ir::TotalHits {
            value: 12,
            relation: searchkernel_ir::TotalHitsRelation::LowerBound,
        },
        coverage: Some(searchkernel_ir::Coverage {
            coverage_percent: Some(99.0),
            documents: Some(12),
            full: Some(true),
            nodes: Some(2),
            results: Some(12),
            results_full: Some(12),
            degraded: Some(searchkernel_ir::CoverageDegraded {
                match_phase: Some(true),
                timeout: Some(false),
                adaptive_timeout: Some(true),
                ann_timeout: Some(false),
                non_ideal_state: Some(true),
                metadata: BTreeMap::from([(
                    "reason".into(),
                    searchkernel_ir::Value::String("load".into()),
                )]),
            }),
            metadata: BTreeMap::from([(
                "region".into(),
                searchkernel_ir::Value::String("west".into()),
            )]),
        }),
        timing: Some(searchkernel_ir::SearchTiming {
            query_time_seconds: Some(0.001),
            search_time_seconds: Some(0.002),
            summary_fetch_time_seconds: Some(0.003),
            metadata: BTreeMap::from([(
                "phase".into(),
                searchkernel_ir::Value::String("done".into()),
            )]),
        }),
        errors: vec![searchkernel_ir::SearchResultError {
            code: 503,
            summary: Some("partial".into()),
            message: Some("one node timed out".into()),
            source: Some("node-2".into()),
            stack_trace: Some("trace".into()),
            transient: Some(true),
            metadata: BTreeMap::from([("node".into(), searchkernel_ir::Value::String("2".into()))]),
        }],
        trace: Some(searchkernel_ir::Value::Object(BTreeMap::from([(
            "level".into(),
            searchkernel_ir::Value::UInt64(3),
        )]))),
        metadata: BTreeMap::from([(
            "request_id".into(),
            searchkernel_ir::Value::String("r1".into()),
        )]),
    }
}

fn facet_result() -> searchkernel_ir::SearchResult {
    searchkernel_ir::SearchResult {
        root: searchkernel_ir::ResultNode::Group(searchkernel_ir::ResultGroup::root(vec![
            searchkernel_ir::ResultNode::Group(searchkernel_ir::ResultGroup {
                id: Some("brand".into()),
                label: Some("brand".into()),
                children: vec![searchkernel_ir::ResultNode::Group(
                    searchkernel_ir::ResultGroup {
                        value: Some(searchkernel_ir::Value::String("Acme".into())),
                        fields: BTreeMap::from([(
                            "count".into(),
                            searchkernel_ir::Value::UInt64(3),
                        )]),
                        ..searchkernel_ir::ResultGroup::root(vec![])
                    },
                )],
                ..searchkernel_ir::ResultGroup::root(vec![])
            }),
        ])),
        total_hits: searchkernel_ir::TotalHits {
            value: 3,
            relation: searchkernel_ir::TotalHitsRelation::Exact,
        },
        coverage: None,
        timing: None,
        errors: Vec::new(),
        trace: None,
        metadata: BTreeMap::new(),
    }
}

fn named_facet_result() -> searchkernel_ir::SearchResult {
    let mut result = facet_result();
    {
        let group = facet_group_mut(&mut result);
        group.id = None;
        group.label = Some("brands".into());
    }
    let bucket = facet_bucket_mut(&mut result);
    bucket.id = Some("grouping-0".into());
    bucket.label = Some("brands".into());
    result
}

fn facet_group_mut(
    result: &mut searchkernel_ir::SearchResult,
) -> &mut searchkernel_ir::ResultGroup {
    let searchkernel_ir::ResultNode::Group(root) = &mut result.root else {
        unreachable!("facet fixture root is a group");
    };
    let searchkernel_ir::ResultNode::Group(group) = &mut root.children[0] else {
        unreachable!("facet fixture child is a group");
    };
    group
}

fn facet_bucket_mut(
    result: &mut searchkernel_ir::SearchResult,
) -> &mut searchkernel_ir::ResultGroup {
    let group = facet_group_mut(result);
    let searchkernel_ir::ResultNode::Group(bucket) = &mut group.children[0] else {
        unreachable!("facet fixture bucket is a group");
    };
    bucket
}

fn invalid_facet_fixture(name: &'static str) -> ResultFixture {
    let mut result = facet_result();
    match name {
        "facet-wrong-id" => facet_group_mut(&mut result).id = Some("wrong".into()),
        "facet-wrong-label" => facet_group_mut(&mut result).label = Some("wrong".into()),
        "facet-fields" => {
            facet_group_mut(&mut result)
                .fields
                .insert("unexpected".into(), searchkernel_ir::Value::Bool(true));
        }
        "facet-metadata" => {
            facet_group_mut(&mut result)
                .metadata
                .insert("unexpected".into(), searchkernel_ir::Value::Bool(true));
        }
        "facet-continuations" => {
            facet_group_mut(&mut result)
                .continuations
                .insert("next".into(), "token".into());
        }
        "facet-source" => facet_group_mut(&mut result).source = Some("unexpected".into()),
        "facet-types" => facet_group_mut(&mut result).types.push("unexpected".into()),
        "facet-relevance" => facet_group_mut(&mut result).score = Some(1.0),
        "facet-limits" => {
            facet_group_mut(&mut result).limits = Some(searchkernel_ir::ResultLimits {
                from: Some(searchkernel_ir::Value::UInt64(0)),
                to: Some(searchkernel_ir::Value::UInt64(1)),
            });
        }
        "facet-mixed-children" => {
            facet_group_mut(&mut result)
                .children
                .push(searchkernel_ir::ResultNode::Hit(
                    searchkernel_ir::Hit::default(),
                ))
        }
        "facet-missing-count" => {
            facet_bucket_mut(&mut result).fields.clear();
        }
        "facet-wrong-count-type" => {
            facet_bucket_mut(&mut result)
                .fields
                .insert("count".into(), searchkernel_ir::Value::Int64(3));
        }
        "facet-bucket-id" => facet_bucket_mut(&mut result).id = Some("wrong".into()),
        "facet-bucket-label" => facet_bucket_mut(&mut result).label = Some("wrong".into()),
        "facet-bucket-fields" => {
            facet_bucket_mut(&mut result)
                .fields
                .insert("extra".into(), searchkernel_ir::Value::Bool(true));
        }
        "facet-bucket-metadata" => {
            facet_bucket_mut(&mut result)
                .metadata
                .insert("unexpected".into(), searchkernel_ir::Value::Bool(true));
        }
        "facet-bucket-continuations" => {
            facet_bucket_mut(&mut result)
                .continuations
                .insert("next".into(), "token".into());
        }
        "facet-bucket-source" => facet_bucket_mut(&mut result).source = Some("unexpected".into()),
        "facet-bucket-types" => facet_bucket_mut(&mut result)
            .types
            .push("unexpected".into()),
        "facet-bucket-relevance" => facet_bucket_mut(&mut result).score = Some(1.0),
        "facet-bucket-limits" => {
            facet_bucket_mut(&mut result).limits = Some(searchkernel_ir::ResultLimits {
                from: Some(searchkernel_ir::Value::UInt64(0)),
                to: Some(searchkernel_ir::Value::UInt64(1)),
            });
        }
        _ => unreachable!("unknown facet fixture {name}"),
    }
    ResultFixture { name, result }
}

fn infinite_relevance_result() -> searchkernel_ir::SearchResult {
    let hits = [
        ("finite", 1.0),
        ("positive-infinity", f64::INFINITY),
        ("negative-infinity", f64::NEG_INFINITY),
    ]
    .into_iter()
    .map(|(id, score)| {
        searchkernel_ir::ResultNode::Hit(searchkernel_ir::Hit {
            id: id.into(),
            score: Some(score),
            ..Default::default()
        })
    })
    .collect();
    searchkernel_ir::SearchResult {
        root: searchkernel_ir::ResultNode::Group(searchkernel_ir::ResultGroup::root(hits)),
        total_hits: searchkernel_ir::TotalHits {
            value: 3,
            relation: searchkernel_ir::TotalHitsRelation::Exact,
        },
        coverage: None,
        timing: None,
        errors: Vec::new(),
        trace: None,
        metadata: BTreeMap::new(),
    }
}

fn root_hit_result() -> searchkernel_ir::SearchResult {
    searchkernel_ir::SearchResult {
        root: searchkernel_ir::ResultNode::Hit(searchkernel_ir::Hit {
            id: "root-hit".into(),
            score: Some(2.0),
            fields: BTreeMap::from([(
                "title".into(),
                searchkernel_ir::Value::String("Root".into()),
            )]),
            ..Default::default()
        }),
        total_hits: searchkernel_ir::TotalHits {
            value: 1,
            relation: searchkernel_ir::TotalHitsRelation::Exact,
        },
        coverage: None,
        timing: None,
        errors: Vec::new(),
        trace: None,
        metadata: BTreeMap::new(),
    }
}

fn root_state_only_result() -> searchkernel_ir::SearchResult {
    searchkernel_ir::SearchResult {
        root: searchkernel_ir::ResultNode::Group(searchkernel_ir::ResultGroup {
            continuations: BTreeMap::from([("page".into(), "token".into())]),
            limits: Some(searchkernel_ir::ResultLimits {
                from: Some(searchkernel_ir::Value::UInt64(1)),
                to: Some(searchkernel_ir::Value::UInt64(2)),
            }),
            metadata: BTreeMap::from([(
                "state".into(),
                searchkernel_ir::Value::String("present".into()),
            )]),
            ..searchkernel_ir::ResultGroup::root(vec![])
        }),
        total_hits: searchkernel_ir::TotalHits {
            value: 0,
            relation: searchkernel_ir::TotalHitsRelation::Exact,
        },
        coverage: None,
        timing: None,
        errors: Vec::new(),
        trace: None,
        metadata: BTreeMap::new(),
    }
}

/// The complete canonical result corpus. Keep all response-shape semantics in
/// these IR values so Rust and non-Rust clients consume identical bytes.
pub fn result_fixtures() -> Vec<ResultFixture> {
    let mut fixtures = vec![
        ResultFixture {
            name: "canonical",
            result: representative_search_result(),
        },
        ResultFixture {
            name: "infinite-relevance",
            result: infinite_relevance_result(),
        },
        ResultFixture {
            name: "empty",
            result: searchkernel_ir::SearchResult::empty(searchkernel_ir::TotalHits {
                value: 0,
                relation: searchkernel_ir::TotalHitsRelation::Exact,
            }),
        },
        ResultFixture {
            name: "root-hit",
            result: root_hit_result(),
        },
        ResultFixture {
            name: "root-state-only",
            result: root_state_only_result(),
        },
        ResultFixture {
            name: "facet-valid",
            result: facet_result(),
        },
        ResultFixture {
            name: "facet-valid-named",
            result: named_facet_result(),
        },
    ];
    for name in [
        "facet-wrong-id",
        "facet-wrong-label",
        "facet-fields",
        "facet-metadata",
        "facet-continuations",
        "facet-source",
        "facet-types",
        "facet-relevance",
        "facet-limits",
        "facet-mixed-children",
        "facet-missing-count",
        "facet-wrong-count-type",
        "facet-bucket-id",
        "facet-bucket-label",
        "facet-bucket-fields",
        "facet-bucket-metadata",
        "facet-bucket-continuations",
        "facet-bucket-source",
        "facet-bucket-types",
        "facet-bucket-relevance",
        "facet-bucket-limits",
    ] {
        fixtures.push(invalid_facet_fixture(name));
    }
    fixtures
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn representative_fixture_keeps_mixed_recursive_result_shape() {
        let result = representative_search_result();
        let searchkernel_ir::ResultNode::Group(root) = result.root else {
            panic!("representative result root must be a group");
        };
        assert_eq!(root.children.len(), 2);
        assert!(matches!(
            root.children[0],
            searchkernel_ir::ResultNode::Hit(_)
        ));
        let searchkernel_ir::ResultNode::Group(nested) = &root.children[1] else {
            panic!("second root child must be a nested group");
        };
        assert!(matches!(
            nested.children.first(),
            Some(searchkernel_ir::ResultNode::Hit(_))
        ));
        assert!(result.timing.is_some());
        assert_eq!(result.errors.len(), 1);
        assert!(result.trace.is_some());
        assert!(!result.metadata.is_empty());
    }

    #[test]
    fn result_corpus_contains_positive_and_negative_facet_shapes() {
        let fixtures = result_fixtures();
        assert!(fixtures.iter().any(|fixture| fixture.name == "facet-valid"));
        assert!(fixtures
            .iter()
            .any(|fixture| fixture.name == "facet-wrong-count-type"));
        assert!(fixtures
            .iter()
            .any(|fixture| fixture.name == "infinite-relevance"));
    }
}
