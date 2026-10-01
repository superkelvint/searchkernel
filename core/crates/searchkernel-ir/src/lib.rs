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

pub mod api_to_ir;
pub mod backend;
pub mod error;
pub mod filter_expression;
pub mod ir_to_api;
pub mod model;
pub mod session;
pub mod snapshot;

pub use api_to_ir::control_plane;
pub use api_to_ir::control_plane::*;
pub use api_to_ir::{
    analyze_result_from_api, analyzer_to_ir, delete_result_from_api, facet_grouping_input,
    field_to_ir, filter_binding_value_from_json, get_result_from_api, index_definition_to_ir,
    insert_result_from_api, patch_result_from_api, portable_error_from_api,
    request_value_from_json, resolve_analyze, resolve_delete, resolve_document, resolve_facets,
    resolve_facets_at, resolve_get, resolve_index_definition, resolve_insert, resolve_patch,
    resolve_projection, resolve_rust_index_definition, resolve_rust_index_definition_with_profiles,
    resolve_rust_index_definition_with_schema, resolve_search, resolve_update, resolve_upsert,
    search_result_from_api, update_result_from_api, upsert_result_from_api, AnalyzeInput,
    AnalyzeTargetInput, AnalyzerInput, DeleteInput, DocumentInput, FacetInput, FacetSortInput,
    FilterBoundInput, FilterInput, GetInput, GroupingInput, IndexContext, IndexDefinitionInput,
    PatchOperationInput, PresentationInput, QueryModelInput, QuerySourceInput, RankingInput,
    RustAnalyzerInput, RustFieldInput, RustFieldKind, RustIndexDefinitionInput,
    RustRankProfileInput, RustSearchFieldInput, SearchFieldInput, SearchInput, SearchIntent,
    SortInput, SortTargetInput, TextualFilterInput, UnresolvedFieldInput,
    UnresolvedRankProfileInput, UpdateOperationInput, WriteRequestRef, DEFAULT_SEARCH_LIMIT,
};
pub use backend::Backend;
pub use error::{ErrorCode, ErrorDetail, PortableError, Result};
pub use filter_expression::{
    bind_filter_expression, bind_filter_expression_with_references, BoundFilter, FilterBindings,
    FilterParameters,
};
pub use ir_to_api::*;
pub use model::*;
pub use session::{BackendSession, SessionError};

/// The generated FlatBuffers API used at the portable API boundary.
pub use searchkernel_protocol::wire::v1 as api;

/// Test and contract clients use the same FlatBuffers parser as this crate.
#[doc(hidden)]
pub use flatbuffers as __flatbuffers;
