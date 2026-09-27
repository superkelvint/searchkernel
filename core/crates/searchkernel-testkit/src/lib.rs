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

pub use searchkernel_ir::{
    AnalyzeRequestIr, AnalyzeResult, Backend, DeleteRequestIr, DeleteResult, GetRequestIr,
    GetResult, InsertRequestIr, InsertResult, OperationIr, PatchRequestIr, PatchResult,
    SearchRequestIr, SearchResult, UpdateRequestIr, UpdateResult, UpsertRequestIr, UpsertResult,
};
