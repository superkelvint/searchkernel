//! Canonical scripted response fixtures shared by result conformance tests.
//!
//! Keep result fixtures here rather than inside one client or transport test.
//! The portable result contract is defined by `searchkernel_ir::SearchResult`;
//! language-specific tests should consume responses produced from these values
//! rather than re-inventing equivalent trees independently.

use std::collections::BTreeMap;

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
}
