//! Query model types and constructors.

use super::{validate_query, yql};
use crate::Result;

pub use crate::document::ScalarValue as Value;

#[derive(Debug, Clone, PartialEq, Eq)]
/// A bound of an integer attribute range.
pub enum RangeBound {
    Unbounded,
    Included(i64),
    Excluded(i64),
}

#[derive(Debug, Clone, PartialEq)]
/// A bound of an attribute range.
pub enum AttributeBound {
    Unbounded,
    Included(Value),
    Excluded(Value),
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// A key/value pair used by Vespa weighted-set and WAND query nodes.
pub enum WeightedQueryItem {
    String { key: String, weight: i64 },
    Integer { key: i64, weight: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// A value in Vespa's native InTerm query node.
pub enum InQueryValue {
    String(String),
    Integer(i64),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WandOptions {
    pub target_num_hits: u32,
    pub score_threshold: i64,
    pub threshold_boost_factor: f64,
}

impl Default for WandOptions {
    fn default() -> Self {
        Self {
            target_num_hits: 100,
            score_threshold: 0,
            threshold_boost_factor: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NearestNeighborOptions {
    pub target_num_hits: u32,
    pub approximate: bool,
    pub distance_threshold: Option<f64>,
    pub explore_additional_hits: u32,
}

impl Default for NearestNeighborOptions {
    fn default() -> Self {
        Self {
            target_num_hits: 100,
            approximate: true,
            distance_threshold: None,
            explore_additional_hits: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct QueryAlternative {
    pub text: String,
    pub exactness: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PredicateFeature {
    pub key: String,
    pub value: String,
    pub sub_query_bitmask: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PredicateRangeFeature {
    pub key: String,
    pub value: u64,
    pub sub_query_bitmask: u64,
}

#[derive(Debug, Clone, PartialEq)]
/// A low-level query which is lowered directly to Vespa query nodes.
pub enum Query {
    True,
    /// Plain text with Vespa's default WeakAnd semantics.
    Text {
        field: String,
        text: String,
        target_num_hits: u32,
    },
    Term {
        field: String,
        text: String,
    },
    Phrase {
        field: String,
        text: String,
    },
    And(Vec<Query>),
    Or(Vec<Query>),
    Not(Box<Query>),
    Near {
        field: String,
        children: Vec<Query>,
        distance: u32,
    },
    ONear {
        field: String,
        children: Vec<Query>,
        distance: u32,
    },
    Equiv(Vec<Query>),
    Alternatives {
        field: String,
        alternatives: Vec<QueryAlternative>,
    },
    Matches {
        field: String,
        pattern: String,
    },
    WeakAnd {
        children: Vec<Query>,
        target_num_hits: u32,
    },
    Rank {
        matching: Box<Query>,
        ranking: Vec<Query>,
    },
    Fuzzy {
        field: String,
        text: String,
        max_edit_distance: u32,
        prefix_lock_length: u32,
        prefix_match: bool,
    },
    WeightedSet {
        field: String,
        items: Vec<WeightedQueryItem>,
    },
    DotProduct {
        field: String,
        items: Vec<WeightedQueryItem>,
    },
    Wand {
        field: String,
        items: Vec<WeightedQueryItem>,
    },
    WandWithOptions {
        field: String,
        items: Vec<WeightedQueryItem>,
        options: WandOptions,
    },
    In {
        field: String,
        values: Vec<InQueryValue>,
    },
    Labeled {
        query: Box<Query>,
        unique_id: i32,
        score: f64,
    },
    Equals {
        field: String,
        value: i64,
    },
    Range {
        field: String,
        lower: RangeBound,
        upper: RangeBound,
    },
    Exists {
        field: String,
    },
    AttributeEquals {
        field: String,
        value: Value,
    },
    AttributeRange {
        field: String,
        lower: AttributeBound,
        upper: AttributeBound,
    },
    NearestNeighbor {
        document_field: String,
        query_tensor: String,
        target_num_hits: u32,
        approximate: bool,
    },
    NearestNeighborWithOptions {
        document_field: String,
        query_tensor: String,
        options: NearestNeighborOptions,
    },
    SameElement {
        field: String,
        children: Vec<Query>,
    },
    GeoLocation {
        field: String,
        latitude: f64,
        longitude: f64,
        radius: String,
    },
    GeoBoundingBox {
        field: String,
        south: f64,
        west: f64,
        north: f64,
        east: f64,
    },
    Predicate {
        field: String,
        features: Vec<PredicateFeature>,
        range_features: Vec<PredicateRangeFeature>,
    },
}

impl Query {
    pub fn term(field: impl Into<String>, text: impl Into<String>) -> Self {
        Self::Term {
            field: field.into(),
            text: text.into(),
        }
    }

    pub fn phrase(field: impl Into<String>, text: impl Into<String>) -> Self {
        Self::Phrase {
            field: field.into(),
            text: text.into(),
        }
    }

    pub fn equals(field: impl Into<String>, value: i64) -> Self {
        Self::Equals {
            field: field.into(),
            value,
        }
    }

    pub fn range(field: impl Into<String>, lower: RangeBound, upper: RangeBound) -> Self {
        Self::Range {
            field: field.into(),
            lower,
            upper,
        }
    }

    pub fn exists(field: impl Into<String>) -> Self {
        Self::Exists {
            field: field.into(),
        }
    }

    pub fn nearest_neighbor(
        document_field: impl Into<String>,
        query_tensor: impl Into<String>,
    ) -> Self {
        Self::NearestNeighbor {
            document_field: document_field.into(),
            query_tensor: query_tensor.into(),
            target_num_hits: 100,
            approximate: true,
        }
    }

    /// Parse the supported YQL subset into the same model used by typed callers.
    ///
    /// This intentionally returns only the expression. Use
    /// [`crate::SearchRequest::from_yql`] when the YQL source selector must be
    /// retained for engine-side validation.
    pub fn from_yql(yql: &str) -> Result<Self> {
        let query = parse_yql(yql)?.query;
        validate_query(&query)?;
        Ok(query)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ParsedYql {
    pub source: String,
    pub query: Query,
}

pub(crate) fn parse_yql(yql: &str) -> Result<ParsedYql> {
    yql::parse(yql)
}
