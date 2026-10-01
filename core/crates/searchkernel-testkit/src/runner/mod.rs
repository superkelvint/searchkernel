//! Corpus-runner API.

use searchkernel_ir::wire::v1::{
    analyzer_to_ir, error_to_api, field_to_ir, index_definition_to_ir, schema as api,
    WriteRequestRef,
};
use searchkernel_ir::{
    resolve_facets, resolve_facets_at, AnalyzeRequestIr, AnalyzerIdentity, ComparisonIr,
    ComparisonOp, DeleteRequestIr, DocumentIr, FacetInput, FacetSortInput, FieldIr, FieldKind,
    FieldValueIr, FilterIr, GetRequestIr, HighlightingIr, IndexContext, IndexDefinitionIr,
    InsertRequestIr, LanguageHint, PatchOperationIr, PatchRequestIr, PortableError, ProjectionIr,
    QueryIr, RangeIr, ScalarValueIr, SearchFieldIr, SearchRequestIr, SortIr,
    UpdateArithmeticOperatorIr, UpdateOperationIr, UpdateRequestIr, UpsertRequestIr,
    WriteRequestIr,
};
use searchkernel_protocol::protocol::v1 as transport;
use serde_json::{json, Map, Value as JsonValue};
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::result::Result as StdResult;

mod component_transport;
mod conformance;
mod error;
mod json;
mod wire;

pub use component_transport::{build_component_transport_case, TransportCase};
pub use conformance::{ConformanceReport, ConformanceRunner};
pub use error::RunnerError;
use json::*;
use wire::WireBuilder;

/// Encode an index fixture JSON object using the same FlatBuffers builder used
/// by the frozen API→IR corpus.
///
/// Client differential tests use this to send their actual packed request
/// objects through the Rust resolver without introducing a language-specific
/// semantic decoder or expected-value model.
pub fn build_index_definition_from_json(input: &JsonValue) -> StdResult<Vec<u8>, String> {
    let mut builder = WireBuilder::new();
    let root = builder.index(input)?;
    builder.finish(root)
}
