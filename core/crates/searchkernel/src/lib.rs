//! Product-facing, backend-independent Search Kernel API.

mod analyze;
mod document;
mod error;
mod filter;
mod grouping;
mod index;
mod patch;
#[cfg(any(feature = "native", test))]
mod portable_definition;
mod query;
#[cfg(feature = "remote")]
mod remote;
mod result;
mod schema;
mod search;
mod tensor;
mod update;
pub use analyze::{AnalyzeBuilder, AnalyzeResult, AnalyzedToken};
pub use document::DocumentId;
pub use error::{Error, ErrorCode, ErrorDetail, Result};
pub use filter::{field, FieldRef, Filter, FilterBindings, Range, SearchFilterInput};
pub use grouping::{
    GroupingAggregation, GroupingExpression, GroupingFilter, GroupingLevel, GroupingOrder,
    GroupingOutput, GroupingPlan,
};
pub use index::{Index, IndexRuntime};
pub use patch::Patch;
pub use query::{
    NearestNeighborOptions, Query, QueryAlternative, QueryBound, QueryValue, WandOptions,
    WeightedQueryItem,
};
#[cfg(feature = "remote")]
pub use remote::RemoteOptions;
pub use result::{
    Coverage, CoverageDegraded, FacetResult, FacetValue, FeatureSet, FeatureValue, GroupingResult,
    HighlightFragment, HighlightResult, HighlightSpan, Hit, NonFiniteFloat, ResultGroup, ResultHit,
    ResultIssue, ResultLimits, ResultNode, ResultTensor, ResultTensorBlock, ResultTensorCell,
    ResultTensorPayload, ResultValue, SearchResult, SearchTiming, TotalHits, TotalHitsRelation,
};
pub use schema::{
    Analyzer, AnalyzerConfig, AttributeConfig, ConstantDefinition, ConstantSource, DataType,
    DistanceMetric, DocumentSummary, DocumentSummaryField, Field, FieldCapabilities, FieldSet,
    HnswConfig, HnswIndexConfig, ImportedField, IndexConfig, IndexDefinition, MatchingConfig,
    MatchingMode, NamedStruct, OnnxModel, PrimitiveDataType, RankFunction, RankPhase, RankProfile,
    RankProfileInput, SchemaNonFiniteFloat, SchemaProperties, SchemaStructField, SchemaTensor,
    SchemaTensorBlock, SchemaTensorCell, SchemaTensorPayload, SchemaValue, SearchField,
    StructFieldConfig, SummaryConfig,
};
pub use search::{
    Facet, FacetSort, InspectedQuery, MissingPlacement, QueryInputValue, QueryModelType, Ranking,
    SearchBuilder, Sort, SortDirection, YqlBuilder,
};
pub use tensor::Tensor;
pub use update::Update;

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
