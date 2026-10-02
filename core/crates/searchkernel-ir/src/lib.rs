//! Backend-independent Search Kernel semantics.

#[cfg(test)]
extern crate self as searchkernel_ir;

#[cfg(test)]
#[path = "../tests/semantics.rs"]
mod semantics;

#[cfg(test)]
#[path = "../tests/property.rs"]
mod property;

#[cfg(test)]
#[path = "../tests/filter_parameter_expansion.rs"]
mod filter_parameter_expansion;

mod api_to_ir;
pub mod backend;
pub mod error;
pub mod filter_expression;
mod ir_to_api;
pub mod model;
pub mod session;
pub mod snapshot;
pub mod wire;

#[cfg(test)]
pub use wire::v1::schema as api;
#[cfg(test)]
pub use wire::v1::*;

pub use api_to_ir::control_plane::{
    already_exists_error, check_expected_generation, index_in_use_error, lifecycle_failure,
    not_found_error, path_failure, stale_index_handle_error, validate_index_target,
    LifecycleFailure, LifecycleFailureCode, LifecycleFailureDetail, LifecycleOperation,
    PathFailure, TargetValidationError, CREATE_INDEX_DEFINITION_PATH, EXPECTED_GENERATION_PATH,
    INDEX_ALREADY_EXISTS_DETAIL_CODE, INDEX_IN_USE_DETAIL_CODE, INDEX_NOT_FOUND_DETAIL_CODE,
    INVALID_INDEX_GENERATION_DETAIL_CODE, INVALID_INDEX_TARGET_DETAIL_CODE, MAX_INDEX_TARGET_LEN,
    MISSING_INDEX_DEFINITION_DETAIL_CODE, MISSING_INDEX_TARGET_DETAIL_CODE,
    STALE_INDEX_HANDLE_DETAIL_CODE, TARGET_NAME_PATH, TARGET_PATH,
};
pub use api_to_ir::{
    facet_grouping_input, filter_binding_value_from_json, request_value_from_json, resolve_analyze,
    resolve_delete, resolve_document, resolve_facets, resolve_facets_at, resolve_get,
    resolve_index_definition, resolve_insert, resolve_patch, resolve_projection,
    resolve_rust_index_definition, resolve_rust_index_definition_with_profiles,
    resolve_rust_index_definition_with_schema, resolve_search, resolve_update, resolve_upsert,
    AnalyzeInput, AnalyzeTargetInput, AnalyzerInput, DeleteInput, DocumentInput, FacetInput,
    FacetSortInput, FilterBoundInput, FilterInput, GetInput, GroupingInput, IndexContext,
    IndexDefinitionInput, PatchOperationInput, PresentationInput, QueryModelInput,
    QuerySourceInput, RankingInput, RustAnalyzerInput, RustFieldInput, RustFieldKind,
    RustIndexDefinitionInput, RustRankProfileInput, RustSearchFieldInput, SearchFieldInput,
    SearchInput, SearchIntent, SortInput, SortTargetInput, TextualFilterInput,
    UnresolvedFieldInput, UnresolvedRankProfileInput, UpdateOperationInput,
    DEFAULT_FUZZY_MAX_EDIT_DISTANCE, DEFAULT_FUZZY_PREFIX, DEFAULT_FUZZY_PREFIX_LENGTH,
    DEFAULT_NEAREST_APPROXIMATE, DEFAULT_NEAREST_EXPLORE_ADDITIONAL_HITS,
    DEFAULT_PROXIMITY_DISTANCE, DEFAULT_SEARCH_LIMIT, DEFAULT_WAND_SCORE_THRESHOLD,
    DEFAULT_WAND_THRESHOLD_BOOST_FACTOR, DEFAULT_WEAK_AND_TARGET_HITS,
};
pub use backend::Backend;
pub use error::{ErrorCode, ErrorDetail, PortableError, Result};
pub use filter_expression::{
    bind_filter_expression, bind_filter_expression_with_references, BoundFilter, FilterBindings,
    FilterParameters,
};
pub use model::*;
pub use session::{BackendSession, SessionError};

/// Test and contract clients use the same FlatBuffers parser as this crate.
#[doc(hidden)]
pub use flatbuffers as __flatbuffers;
