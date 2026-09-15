//! Product-facing, backend-independent Search Kernel API.

mod document;
mod error;
mod filter;
mod index;
mod patch;
mod result;
mod schema;
mod search;
mod wire;

pub use document::DocumentId;
pub use error::{Error, Result};
pub use filter::{field, FieldRef, Filter, Range};
pub use index::Index;
pub use patch::Patch;
pub use result::{
    FacetResult, FacetValue, HighlightFragment, HighlightResult, HighlightSpan, Hit, SearchResult,
    TotalHits, TotalHitsRelation,
};
pub use schema::{Analyzer, Field, IndexDefinition, SearchField};
pub use search::{Facet, FacetSort, MissingPlacement, Ranking, SearchBuilder, Sort, SortDirection};

/// Internal constructor namespace used by backend-injected tests.
#[doc(hidden)]
pub mod __private {
    pub use crate::index::Index;
}
