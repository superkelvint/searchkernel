//! Product-facing, backend-independent Search Kernel API.

mod analyze;
mod document;
mod error;
mod filter;
mod index;
mod patch;
#[cfg(feature = "native")]
mod portable_definition;
mod result;
mod schema;
mod search;
pub use analyze::{AnalyzeBuilder, AnalyzeResult, AnalyzedToken};
pub use document::DocumentId;
pub use error::{Error, ErrorCode, ErrorDetail, Result};
pub use filter::{field, FieldRef, Filter, Range};
pub use index::{Index, IndexRuntime};
pub use patch::Patch;
pub use result::{
    FacetResult, FacetValue, HighlightFragment, HighlightResult, HighlightSpan, Hit, SearchResult,
    TotalHits, TotalHitsRelation,
};
pub use schema::{Analyzer, Field, IndexDefinition, SearchField};
pub use search::{
    Facet, FacetSort, InspectedQuery, MissingPlacement, QueryInputValue, Ranking, SearchBuilder,
    Sort, SortDirection,
};

/// Internal constructor namespace used by backend-injected tests.
#[doc(hidden)]
pub mod __private {
    pub use crate::index::Index;
}
