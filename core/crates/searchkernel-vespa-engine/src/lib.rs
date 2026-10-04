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

#[cfg(all(test, feature = "native"))]
mod tests;

pub use document::{
    ArithmeticOperator, ArrayValue, AttributeDocument, AttributeUpdate, Document, DocumentUpdate,
    FieldUpdate, FieldValue, IntoFieldValue, IntoScalarValue, PositionValue, PredicateValue,
    ScalarValue,
};
pub use engine::{decode_diagnostic_tensor, Engine};
pub use error::{Error, ErrorKind, Result};
pub use linguistics::{
    DocumentAnalysis, DocumentTerm, Language, Linguistics, LinguisticsOptions, QueryAnalysis,
    QueryTerm, StemMode,
};
pub use query::{
    AttributeBound, InQueryValue, NearestNeighborOptions, PredicateFeature, PredicateRangeFeature,
    Query, QueryAlternative, RangeBound, Value, WandOptions, WeightedQueryItem,
};
pub use schema::{
    ConfigCompiler, DistanceMetric, Field, FieldType, HnswIndexConfig, RankProfile, Schema,
    StructField, StructFieldType,
};
pub use search::{
    FacetGroup, FacetRequest, FacetResult, GroupingAggregation, GroupingBinaryOperator,
    GroupingExpression, GroupingFilter, GroupingLevel, GroupingLiteral, GroupingOrder,
    GroupingOrderBy, GroupingOutput, GroupingPlan, GroupingResult, GroupingResultValue,
    HighlightFragment, HighlightResult, HighlightSpan, Hit, MatchFeatureValue, MissingPlacement,
    QueryInput, SearchCoverage, SearchDiagnosticControls, SearchDiagnostics, SearchRequest,
    SearchResult, SortField, SortOrder, Source, SummaryRequest, TotalHitsRelation,
};
pub use tensor::{TensorAddress, TensorBlock, TensorCell, TensorValue};

#[cfg(feature = "hardening")]
pub mod hardening;
