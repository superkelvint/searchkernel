//! Product-facing, backend-independent Search Kernel API.

mod analyze;
mod document;
mod error;
mod filter;
mod grouping;
mod index;
mod patch;
#[cfg(feature = "native")]
mod portable_definition;
mod query;
mod result;
mod schema;
mod search;
mod tensor;
pub use analyze::{AnalyzeBuilder, AnalyzeResult, AnalyzedToken};
pub use document::DocumentId;
pub use error::{Error, ErrorCode, ErrorDetail, Result};
pub use filter::{field, FieldRef, Filter, FilterBindings, Range, SearchFilterInput};
pub use grouping::{GroupingAggregation, GroupingExpression, GroupingLevel, GroupingPlan};
pub use index::{Index, IndexRuntime};
pub use patch::Patch;
pub use query::{
    NearestNeighborOptions, Query, QueryBound, QueryValue, WandOptions, WeightedQueryItem,
};
pub use result::{
    Coverage, CoverageDegraded, FacetResult, FacetValue, FeatureSet, FeatureValue, GroupingResult,
    HighlightFragment, HighlightResult, HighlightSpan, Hit, ResultIssue, SearchResult, TotalHits,
    TotalHitsRelation,
};
pub use schema::{Analyzer, Field, IndexDefinition, RankProfile, SearchField};
pub use search::{
    Facet, FacetSort, InspectedQuery, MissingPlacement, QueryInputValue, Ranking, SearchBuilder,
    Sort, SortDirection,
};
pub use tensor::Tensor;

/// Build clause-local typed values for a textual filter.
#[macro_export]
macro_rules! params {
    ($($name:expr => $value:expr),* $(,)?) => {{
        let mut bindings = $crate::FilterBindings::new();
        $(
            bindings.insert(
                $name.into(),
                $crate::__private::serde_json::to_value(&$value)
                    .expect("filter parameter must be JSON serializable"),
            );
        )*
        bindings
    }};
}

/// Internal constructor namespace used by backend-injected tests.
#[doc(hidden)]
pub mod __private {
    pub use crate::index::Index;
    pub use serde_json;
}
