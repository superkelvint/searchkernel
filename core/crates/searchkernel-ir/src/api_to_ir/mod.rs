//! FlatBuffers API to owned Search Kernel IR conversion.

pub mod control_plane;
mod document;
mod diagnostics;
pub use diagnostics::query_diagnostics_to_ir;
mod filter;
mod grouping;
mod query;
mod ranking;
mod resolve;
mod result;
mod schema;
mod search;
mod value;

pub use query::{
    DEFAULT_FUZZY_MAX_EDIT_DISTANCE, DEFAULT_FUZZY_PREFIX, DEFAULT_FUZZY_PREFIX_LENGTH,
    DEFAULT_NEAREST_APPROXIMATE, DEFAULT_NEAREST_EXPLORE_ADDITIONAL_HITS,
    DEFAULT_PROXIMITY_DISTANCE, DEFAULT_WAND_SCORE_THRESHOLD, DEFAULT_WAND_THRESHOLD_BOOST_FACTOR,
    DEFAULT_WEAK_AND_TARGET_HITS,
};
pub use result::{
    analyze_result_from_api, delete_result_from_api, get_result_from_api, insert_result_from_api,
    patch_result_from_api, portable_error_from_api, search_result_from_api, update_result_from_api,
    upsert_result_from_api,
};

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;

pub(crate) use grouping::grouping_to_input;
pub use resolve::{
    facet_grouping_input, resolve_analyze, resolve_delete, resolve_document, resolve_facets,
    resolve_facets_at, resolve_get, resolve_insert, resolve_patch, resolve_projection,
    resolve_search, resolve_update, resolve_upsert, AnalyzeInput, AnalyzeTargetInput,
    AnalyzerInput, DeleteInput, DocumentInput, FacetInput, FacetSortInput, FilterBoundInput,
    FilterInput, GetInput, GroupingInput, PatchOperationInput, PresentationInput, QueryModelInput,
    QuerySourceInput, RankingInput, SearchFieldInput, SearchInput, SearchIntent, SortInput,
    SortTargetInput, TextualFilterInput, UpdateOperationInput, DEFAULT_SEARCH_LIMIT,
};
pub use schema::{
    analyzer_to_ir, field_to_ir, index_definition_to_ir, resolve_index_definition,
    resolve_rust_index_definition, resolve_rust_index_definition_with_profiles,
    resolve_rust_index_definition_with_schema, IndexDefinitionInput, RustAnalyzerInput,
    RustFieldInput, RustFieldKind, RustIndexDefinitionInput, RustRankProfileInput,
    RustSearchFieldInput, UnresolvedFieldInput, UnresolvedRankProfileInput,
};
pub use value::{filter_binding_value_from_json, request_value_from_json};
pub(crate) use value::{request_values_to_map, value_from_api};

use crate::error::{PortableError, Result};
use crate::model::IndexDefinitionIr;
use searchkernel_protocol::wire::v1 as api;
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
pub enum WriteRequestRef<'a> {
    Insert(api::InsertRequest<'a>),
    Upsert(api::UpsertRequest<'a>),
    Patch(api::PatchRequest<'a>),
    Update(api::UpdateRequest<'a>),
    Delete(api::DeleteRequest<'a>),
}

pub struct IndexContext {
    definition: IndexDefinitionIr,
    document_fields: BTreeMap<String, usize>,
    query_fields: BTreeMap<String, QueryFieldRef>,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum QueryFieldRef {
    Document(usize),
    Schema(usize),
}

impl std::fmt::Debug for IndexContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IndexContext")
            .field("definition", &self.definition)
            .finish_non_exhaustive()
    }
}

pub(crate) fn invalid(
    code: impl Into<String>,
    path: Option<impl Into<String>>,
    message: impl Into<String>,
) -> PortableError {
    PortableError::invalid(code, path, message)
}

pub(crate) fn normalized_optional_string(value: Option<&str>) -> Option<String> {
    value
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}
