//! FlatBuffers API to owned Search Kernel IR conversion.

mod document;
mod filter;
mod grouping;
mod query;
mod ranking;
mod resolve;
mod schema;
mod search;
mod value;

#[cfg(test)]
mod tests;

pub(crate) use grouping::grouping_to_input;
pub use resolve::{
    resolve_analyze, resolve_delete, resolve_document, resolve_get, resolve_insert, resolve_patch,
    resolve_projection, resolve_search, resolve_update, resolve_upsert, AnalyzeInput,
    AnalyzeTargetInput, AnalyzerInput, DeleteInput, DocumentInput, FacetInput, FacetSortInput,
    FilterBoundInput, FilterInput, GetInput, GroupingInput, PatchOperationInput, PresentationInput,
    QueryModelInput, QuerySourceInput, RankingInput, SearchFieldInput, SearchInput, SearchIntent,
    SortInput, SortTargetInput, TextualFilterInput, UpdateOperationInput,
};
pub use schema::{
    analyzer_to_ir, field_to_ir, index_definition_to_ir, resolve_rust_index_definition,
    resolve_rust_index_definition_with_profiles, RustAnalyzerInput, RustFieldInput, RustFieldKind,
    RustIndexDefinitionInput, RustRankProfileInput, RustSearchFieldInput,
};
pub(crate) use search::search_fields_to_ir_at;
pub use value::request_value_from_json;
pub(crate) use value::{request_values_to_map, scalar_to_ir, value_from_api};

use crate::error::{PortableError, Result};
use crate::model::IndexDefinitionIr;
use searchkernel_protocol::v2 as api;
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
