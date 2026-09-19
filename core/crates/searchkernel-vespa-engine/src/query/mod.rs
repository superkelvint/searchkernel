//! Query model, validation, parsing, and native lowering.

mod model;
mod native;
mod validate;
mod yql;

#[cfg(test)]
mod tests;

pub(crate) const MAX_QUERY_DEPTH: usize = 256;
pub(crate) const MAX_QUERY_NODES: usize = 65_536;
pub(crate) const MAX_YQL_BYTES: usize = 4 * 1024 * 1024;

pub(crate) use model::{parse_yql, ParsedYql};
pub use model::{
    AttributeBound, PredicateFeature, PredicateRangeFeature, Query, QueryAlternative, RangeBound,
    Value, WeightedQueryItem,
};
pub(crate) use native::{lower_query, validate_query_fields};
pub(crate) use validate::{invalid_query, validate_query};
