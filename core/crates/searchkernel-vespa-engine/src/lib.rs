#![deny(unsafe_op_in_unsafe_fn)]
//! Safe Vespa-specific adapter for the private Search Kernel ABI.

mod document;
mod engine;
mod error;
mod linguistics;
mod native;
mod query;
mod schema;
mod search;
mod tensor;

#[cfg(test)]
mod tests;

pub use document::{
    ArrayValue, Document, DocumentUpdate, FieldUpdate, FieldValue, IntoFieldValue, IntoScalarValue,
    ScalarValue,
};
pub use engine::Engine;
pub use error::{Error, ErrorKind, Result};
pub use linguistics::{
    DocumentAnalysis, DocumentTerm, Language, Linguistics, LinguisticsOptions, QueryAnalysis,
    QueryTerm, StemMode,
};
pub use query::{AttributeBound, Query, RangeBound, Value};
pub use schema::{ConfigCompiler, Field, FieldType, RankProfile, Schema};
pub use search::{
    AggregateValue, Aggregation, FacetRequest, Group, GroupingOrder, GroupingRequest,
    GroupingResult, Hit, MissingPlacement, QueryInput, SearchRequest, SearchResult, SortField,
    SortOrder, Source, SummaryRequest, TotalHitsRelation,
};
pub use tensor::{TensorAddress, TensorBlock, TensorCell, TensorValue};

#[cfg(feature = "hardening")]
pub mod hardening;
