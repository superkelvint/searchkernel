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

/// Canonical response fixture with sparse and mixed tensor values in the
/// positions consumed by public result decoders.
pub fn sparse_mixed_tensor_result() -> searchkernel_ir::SearchResult {
    use searchkernel_ir::{
        RequestTensorBlockIr, RequestTensorCellIr, RequestTensorIr, RequestTensorPayloadIr,
        ResultGroup, ResultNode, Value,
    };

    let sparse = Value::Tensor(RequestTensorIr {
        type_spec: "tensor<float>(label{},region{})".to_owned(),
        payload: RequestTensorPayloadIr::Sparse(vec![
            RequestTensorCellIr {
                address: BTreeMap::from([
                    ("label".to_owned(), "a".to_owned()),
                    ("region".to_owned(), "east".to_owned()),
                ]),
                value: 0.5,
            },
            RequestTensorCellIr {
                address: BTreeMap::from([
                    ("label".to_owned(), "b".to_owned()),
                    ("region".to_owned(), "west".to_owned()),
                ]),
                value: -0.25,
            },
        ]),
    });
    let mixed = Value::Tensor(RequestTensorIr {
        type_spec: "tensor<float>(label{},region{},x[2])".to_owned(),
        payload: RequestTensorPayloadIr::Mixed(vec![
            RequestTensorBlockIr {
                address: BTreeMap::from([
                    ("label".to_owned(), "b".to_owned()),
                    ("region".to_owned(), "west".to_owned()),
                ]),
                values: vec![0.25, 0.75],
            },
            RequestTensorBlockIr {
                address: BTreeMap::from([
                    ("label".to_owned(), "c".to_owned()),
                    ("region".to_owned(), "north".to_owned()),
                ]),
                values: vec![1.5, -2.0],
            },
        ]),
    });
    let dense = Value::Tensor(RequestTensorIr {
        type_spec: "tensor<float>(x[2])".to_owned(),
        payload: RequestTensorPayloadIr::Dense(vec![1.25, 2.5]),
    });

    searchkernel_ir::SearchResult {
        root: ResultNode::Group(ResultGroup {
            value: Some(mixed),
            fields: BTreeMap::from([("dense".into(), dense)]),
            children: vec![ResultNode::Hit(searchkernel_ir::Hit {
                id: "tensor-hit".to_owned(),
                fields: BTreeMap::from([("sparse".to_owned(), sparse)]),
                ..Default::default()
            })],
            ..ResultGroup::root(vec![])
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

/// Canonical result values that are otherwise easy for a client projection to
/// coerce or omit. Non-finite scalar values are intentionally absent here:
/// the response schema has a union for them, but the owned result IR currently
/// has no corresponding `Value` variant.
fn value_variants_result() -> searchkernel_ir::SearchResult {
    use searchkernel_ir::{ResultGroup, ResultNode, Value};

    let nested = Value::List(vec![
        Value::Bool(true),
        Value::List(vec![Value::String("nested".into()), Value::Int64(i64::MIN)]),
        Value::Object(BTreeMap::from([(
            "uint64_max".into(),
            Value::UInt64(u64::MAX),
        )])),
    ]);
    let fields = BTreeMap::from([
        ("bool".into(), Value::Bool(false)),
        ("empty_bytes".into(), Value::Bytes(Vec::new())),
        ("bytes".into(), Value::Bytes(vec![0, 1, 127, 255])),
        ("float64".into(), Value::Float64(1.25)),
        ("float64_negative_fraction".into(), Value::Float64(-0.5)),
        ("float64_zero".into(), Value::Float64(0.0)),
        ("int64_max".into(), Value::Int64(i64::MAX)),
        ("int64_min".into(), Value::Int64(i64::MIN)),
        ("list".into(), nested),
        ("empty_string".into(), Value::String(String::new())),
        ("empty_object".into(), Value::Object(BTreeMap::new())),
        (
            "object".into(),
            Value::Object(BTreeMap::from([
                ("null".into(), Value::Null),
                ("text".into(), Value::String("value".into())),
            ])),
        ),
        ("uint64_max".into(), Value::UInt64(u64::MAX)),
    ]);
    searchkernel_ir::SearchResult {
        root: ResultNode::Group(ResultGroup {
            fields,
            children: vec![ResultNode::Hit(searchkernel_ir::Hit {
                id: "value-variants".into(),
                ..Default::default()
            })],
            ..ResultGroup::root(vec![])
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

fn total_hit_relation_result(
    relation: searchkernel_ir::TotalHitsRelation,
) -> searchkernel_ir::SearchResult {
    let mut result =
        searchkernel_ir::SearchResult::empty(searchkernel_ir::TotalHits { value: 9, relation });
    result.root = searchkernel_ir::ResultNode::Group(searchkernel_ir::ResultGroup::root(vec![]));
    result
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

fn shared_root_hit_result() -> searchkernel_ir::SearchResult {
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
            result: shared_root_hit_result(),
        },
        ResultFixture {
            name: "sparse-mixed-tensors",
            result: sparse_mixed_tensor_result(),
        },
        ResultFixture {
            name: "value-variants",
            result: value_variants_result(),
        },
        ResultFixture {
            name: "total-hits-unspecified",
            result: total_hit_relation_result(searchkernel_ir::TotalHitsRelation::Unspecified),
        },
        ResultFixture {
            name: "total-hits-approximate",
            result: total_hit_relation_result(searchkernel_ir::TotalHitsRelation::Approximate),
        },
        ResultFixture {
            name: "total-hits-unknown",
            result: total_hit_relation_result(searchkernel_ir::TotalHitsRelation::Unknown),
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
    // Surgical recursive/result-state cases are reused from the same canonical
    // IR fixture builders as the encoder round-trip tests. Keeping these in the
    // shared byte corpus lets each language cover the structural partitions
    // without a Cartesian product of otherwise independent fields.
    let extended = canonical_result_fixtures();
    for (wire_name, fixture_name) in [
        ("nested-groups", "nested_groups"),
        ("mixed-hit-group-children", "mixed_hit_and_group_children"),
        ("sibling-groups", "sibling_groups"),
        ("group-fields-aggregations", "group_fields_and_aggregations"),
        (
            "continuations-limits-metadata",
            "continuations_limits_and_metadata",
        ),
        ("absent-optional-fields", "absent_optional_fields"),
        (
            "present-empty-optional-fields",
            "present_empty_optional_fields",
        ),
        ("limits-from-only", "limits_from_only"),
        ("limits-to-only", "limits_to_only"),
    ] {
        let result = extended
            .iter()
            .find(|(name, _)| *name == fixture_name)
            .unwrap_or_else(|| panic!("canonical result fixture missing: {fixture_name}"))
            .1
            .clone();
        fixtures.push(ResultFixture {
            name: wire_name,
            result,
        });
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

mod expanded_canonical_corpus {
    // //! Canonical scripted response fixtures shared by result conformance tests.
    // //!
    // //! Keep result fixtures here rather than inside one client or transport test.
    // //! The portable result contract is defined by `searchkernel_ir::SearchResult`;
    // //! language-specific tests should consume responses produced from these values
    // //! rather than re-inventing equivalent trees independently.

    use std::collections::BTreeMap;

    use searchkernel_ir as ir;
    use searchkernel_protocol::wire::v1 as api;

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
            summary_features: BTreeMap::from([(
                "summary".into(),
                ir::Value::String("ready".into()),
            )]),
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
            ("sparse_mixed_tensors", super::sparse_mixed_tensor_result()),
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
            (
                "limits_from_only",
                result(
                    ir::ResultNode::Group(ir::ResultGroup {
                        limits: Some(ir::ResultLimits {
                            from: Some(ir::Value::UInt64(3)),
                            to: None,
                        }),
                        ..group(Vec::new())
                    }),
                    0,
                ),
            ),
            (
                "limits_to_only",
                result(
                    ir::ResultNode::Group(ir::ResultGroup {
                        limits: Some(ir::ResultLimits {
                            from: None,
                            to: Some(ir::Value::UInt64(9)),
                        }),
                        ..group(Vec::new())
                    }),
                    0,
                ),
            ),
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
            (Some(actual), Some(expected))
                if expected.is_infinite() && expected.is_sign_positive() =>
            {
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
            ir::Value::Tensor(expected) => {
                let actual = actual.kind_as_tensor_value().unwrap();
                assert_eq!(actual.type_spec(), Some(expected.type_spec.as_str()));
                match &expected.payload {
                    ir::RequestTensorPayloadIr::Dense(expected) => {
                        assert_eq!(
                            actual.payload_type(),
                            api::TensorPayloadKind::DenseTensorValues
                        );
                        let values = actual
                            .payload_as_dense_tensor_values()
                            .unwrap()
                            .values()
                            .unwrap();
                        assert_eq!(values.iter().collect::<Vec<_>>(), expected.as_slice());
                    }
                    ir::RequestTensorPayloadIr::Sparse(expected) => {
                        assert_eq!(
                            actual.payload_type(),
                            api::TensorPayloadKind::SparseTensorCells
                        );
                        let cells = actual
                            .payload_as_sparse_tensor_cells()
                            .unwrap()
                            .cells()
                            .unwrap();
                        assert_eq!(cells.len(), expected.len());
                        for (actual, expected) in cells.iter().zip(expected) {
                            assert_tensor_address_matches(actual.address(), &expected.address);
                            assert_eq!(actual.value(), expected.value);
                        }
                    }
                    ir::RequestTensorPayloadIr::Mixed(expected) => {
                        assert_eq!(
                            actual.payload_type(),
                            api::TensorPayloadKind::MixedTensorBlocks
                        );
                        let blocks = actual
                            .payload_as_mixed_tensor_blocks()
                            .unwrap()
                            .blocks()
                            .unwrap();
                        assert_eq!(blocks.len(), expected.len());
                        for (actual, expected) in blocks.iter().zip(expected) {
                            assert_tensor_address_matches(actual.address(), &expected.address);
                            assert_eq!(
                                actual.values().unwrap().iter().collect::<Vec<_>>(),
                                expected.values
                            );
                        }
                    }
                }
            }
        }
    }

    fn assert_tensor_address_matches(
        actual: Option<
            flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::TensorAddressEntry<'_>>>,
        >,
        expected: &BTreeMap<String, String>,
    ) {
        let actual = actual.unwrap();
        assert_eq!(actual.len(), expected.len());
        for (actual, (dimension, label)) in actual.iter().zip(expected) {
            assert_eq!(actual.dimension(), dimension.as_str());
            assert_eq!(actual.label(), Some(label.as_str()));
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
            if let (Some(actual), Some(expected)) = (actual.degraded(), expected.degraded.as_ref())
            {
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
        use searchkernel_protocol::protocol::v1 as wire;

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
                let decoded = searchkernel_ir::wire::v1::search_result_from_api(actual)
                    .unwrap_or_else(|error| panic!("fixture {name} must decode to IR: {error:?}"));
                assert_eq!(decoded, expected, "IR round trip changed fixture {name}");
            }
        }

        #[test]
        fn bounded_recursive_result_trees_and_nested_values_round_trip() {
            fn generated_node(depth: usize, seed: usize) -> ir::ResultNode {
                let fields = BTreeMap::from([
                    (
                        "nested".into(),
                        ir::Value::List(vec![
                            ir::Value::Bool(seed.is_multiple_of(2)),
                            ir::Value::Object(BTreeMap::from([
                                ("empty_list".into(), ir::Value::List(Vec::new())),
                                ("empty_object".into(), ir::Value::Object(BTreeMap::new())),
                            ])),
                        ]),
                    ),
                    ("signed_min".into(), ir::Value::Int64(i64::MIN)),
                    ("unsigned_max".into(), ir::Value::UInt64(u64::MAX)),
                ]);
                if depth == 0 || seed.is_multiple_of(3) {
                    return ir::ResultNode::Hit(ir::Hit {
                        id: format!("hit-{depth}-{seed}"),
                        fields,
                        ..Default::default()
                    });
                }
                let children = (0..(seed % 3 + 1))
                    .map(|index| generated_node(depth - 1, seed + index + 1))
                    .collect();
                ir::ResultNode::Group(ir::ResultGroup {
                    id: Some(format!("group-{depth}-{seed}")),
                    fields,
                    children,
                    ..ir::ResultGroup::root(Vec::new())
                })
            }

            for seed in 0..24 {
                let expected = ir::SearchResult {
                    root: generated_node(seed % 5, seed),
                    total_hits: ir::TotalHits {
                        value: seed as u64,
                        relation: ir::TotalHitsRelation::Exact,
                    },
                    coverage: None,
                    timing: None,
                    errors: Vec::new(),
                    trace: Some(ir::Value::Object(BTreeMap::from([
                        ("empty_map".into(), ir::Value::Object(BTreeMap::new())),
                        ("empty_list".into(), ir::Value::List(Vec::new())),
                    ]))),
                    metadata: BTreeMap::new(),
                };
                let bytes = searchkernel_ir::search_result_to_api(&expected).unwrap();
                let wire =
                    searchkernel_ir::__flatbuffers::root::<api::SearchResult>(&bytes).unwrap();
                let actual = searchkernel_ir::wire::v1::search_result_from_api(wire).unwrap();
                assert_eq!(actual, expected, "generated result tree seed {seed}");
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
            let encoded =
                searchkernel_ir::__flatbuffers::root::<api::SearchResult>(&bytes).unwrap();
            assert!(encoded
                .root()
                .unwrap()
                .node_as_hit()
                .unwrap()
                .relevance()
                .is_none());
        }

        #[test]
        fn result_response_wire_fixtures_reject_malformed_ir_inputs() {
            let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../reference/result-response-conformance/fixtures");
            for (name, class) in [
                ("result-malformed-unknown-result-node-union.skrs", "result"),
                ("result-malformed-unknown-value-union.skrs", "result"),
                ("result-malformed-unknown-relevance-union.skrs", "result"),
                (
                    "result-malformed-unknown-tensor-payload-union.skrs",
                    "result",
                ),
                ("result-malformed-duplicate-result-map-key.skrs", "result"),
                (
                    "result-malformed-unknown-response-payload-union.skrs",
                    "envelope",
                ),
                ("result-malformed-missing-search-result-root.skrs", "result"),
                (
                    "result-malformed-missing-search-result-total-hits.skrs",
                    "result",
                ),
                (
                    "result-malformed-missing-search-result-root-and-total-hits.skrs",
                    "result",
                ),
                (
                    "result-malformed-missing-continuation-key.skrs",
                    "structural",
                ),
                ("result-malformed-missing-continuation-value.skrs", "result"),
                (
                    "result-malformed-missing-sparse-address-dimension.skrs",
                    "structural",
                ),
                (
                    "result-malformed-missing-sparse-address-label.skrs",
                    "result",
                ),
                (
                    "result-malformed-missing-mixed-address-dimension.skrs",
                    "structural",
                ),
                (
                    "result-malformed-missing-mixed-address-label.skrs",
                    "result",
                ),
                ("result-malformed-missing-value-entry-value.skrs", "result"),
                (
                    "result-malformed-non-finite-dense-tensor-cell.skrs",
                    "result",
                ),
                (
                    "result-malformed-non-finite-sparse-tensor-cell.skrs",
                    "result",
                ),
                (
                    "result-malformed-non-finite-mixed-tensor-cell.skrs",
                    "result",
                ),
            ] {
                let bytes = std::fs::read(directory.join(name)).unwrap();
                let decoded =
                    searchkernel_ir::__flatbuffers::root::<wire::ResponseEnvelope>(&bytes);
                if class == "structural" {
                    assert!(
                        decoded.is_err(),
                        "{name} should fail FlatBuffers verification"
                    );
                    continue;
                }
                let envelope = decoded.unwrap_or_else(|error| {
                    panic!("{name} envelope should be structurally valid: {error:?}")
                });
                let Some(result) = envelope.payload_as_search_kernel_wire_search_result() else {
                    assert_eq!(class, "envelope", "{name} must retain SearchResult payload");
                    continue;
                };
                assert_eq!(class, "result", "{name} must reject before result decoding");
                assert!(
                    searchkernel_ir::wire::v1::search_result_from_api(result).is_err(),
                    "malformed result fixture was accepted: {name}"
                );
            }
        }

        #[test]
        fn missing_continuation_key_fixture_preserves_other_map_keys() {
            let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../reference/result-response-conformance/fixtures");
            let bytes =
                std::fs::read(directory.join("result-malformed-missing-continuation-key.skrs"))
                    .unwrap();
            let error =
                searchkernel_ir::__flatbuffers::root::<wire::ResponseEnvelope>(&bytes).unwrap_err();
            assert!(
                format!("{error:?}").contains("field_name: \"continuations\""),
                "only the continuation key should be missing: {error:?}"
            );
        }
    }
}

pub use expanded_canonical_corpus::{
    absent_optional_fields_result, assert_api_search_result_matches, canonical_facet_result,
    canonical_result_fixtures, continuations_limits_and_metadata_result, envelope_metadata_result,
    finite_relevance_result, group_fields_and_aggregations_result, hit_features_result,
    mixed_hit_and_group_children_result, negative_infinity_relevance_result, nested_groups_result,
    non_facet_grouping_result, one_group_containing_hits_result,
    positive_infinity_relevance_result, present_empty_optional_fields_result, relevance_result,
    root_hit_result, sibling_groups_result,
};
