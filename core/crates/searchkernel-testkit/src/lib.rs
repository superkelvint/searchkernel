//! Backend-free infrastructure for testing the portable API/IR boundary.
//!
//! This crate records and scripts the existing [`searchkernel_ir::Backend`]
//! execution surface.  It deliberately contains no search or ranking logic.

mod error;
mod recording;
mod result_fixture;
mod runner;
mod scripted;

pub use error::{OperationKind, ScriptedCallError, TestkitError};
pub use recording::RecordingBackend;
pub use result_fixture::{
    absent_optional_fields_result, assert_api_search_result_matches, canonical_facet_result,
    canonical_result_fixtures, continuations_limits_and_metadata_result, envelope_metadata_result,
    finite_relevance_result, group_fields_and_aggregations_result, hit_features_result,
    mixed_hit_and_group_children_result, negative_infinity_relevance_result, nested_groups_result,
    non_facet_grouping_result, one_group_containing_hits_result,
    positive_infinity_relevance_result, present_empty_optional_fields_result, relevance_result,
    representative_search_result, result_fixtures, root_hit_result, sibling_groups_result,
    sparse_mixed_tensor_result, ResultFixture,
};
pub use runner::{
    build_component_transport_case, build_index_definition_from_json, ConformanceReport,
    ConformanceRunner, RunnerError, TransportCase,
};
pub use scripted::{ExpectedCall, ScriptedBackend, ScriptedOutcome, ScriptedResult};

/// Convert tagged 64-bit integer snapshot values to decimal strings before
/// crossing a JSON boundary. JavaScript cannot represent the full int64/uint64
/// domains exactly as numbers, while the portable Search Kernel JSON policy
/// uses decimal strings for these integer widths.
pub fn lossless_integer_tags(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(mut map) => {
            if map.len() == 1 {
                for tag in ["int64", "uint64"] {
                    if let Some(number) = map.get(tag).and_then(serde_json::Value::as_number) {
                        map.insert(tag.to_string(), serde_json::Value::String(number.to_string()));
                        return serde_json::Value::Object(map);
                    }
                }
            }
            serde_json::Value::Object(
                map.into_iter()
                    .map(|(key, value)| (key, lossless_integer_tags(value)))
                    .collect(),
            )
        }
        serde_json::Value::Array(values) => serde_json::Value::Array(
            values.into_iter().map(lossless_integer_tags).collect(),
        ),
        other => other,
    }
}

pub use searchkernel_ir::{
    AnalyzeRequestIr, AnalyzeResult, Backend, DeleteRequestIr, DeleteResult, GetRequestIr,
    GetResult, InsertRequestIr, InsertResult, OperationIr, PatchRequestIr, PatchResult,
    SearchRequestIr, SearchResult, UpdateRequestIr, UpdateResult, UpsertRequestIr, UpsertResult,
};

#[cfg(test)]
mod lossless_integer_tags_tests {
    use super::lossless_integer_tags;
    use serde_json::json;

    #[test]
    fn preserves_full_int64_and_uint64_precision_across_json() {
        let value = json!({
            "signed": {"int64": i64::MAX},
            "unsigned": {"uint64": u64::MAX},
            "ordinary": 42,
            "float64": {"float64": 120.0},
        });
        let actual = lossless_integer_tags(value);
        assert_eq!(
            actual,
            json!({
                "signed": {"int64": i64::MAX.to_string()},
                "unsigned": {"uint64": u64::MAX.to_string()},
                "ordinary": 42,
                "float64": {"float64": 120.0},
            })
        );
    }
}
