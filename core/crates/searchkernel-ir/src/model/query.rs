//! Query expressions and query-model settings.

use super::*;
#[derive(Clone, Debug, PartialEq)]
pub enum QueryIr {
    MatchAll,
    Text(String),
    /// A YQL source is intentionally kept separate from the portable typed
    /// query tree. Its syntax belongs to the backend, but its request-level
    /// parameters are still portable request semantics.
    Yql {
        source: String,
        parameters: std::collections::BTreeMap<String, RequestValueIr>,
    },
    /// The sole structured-query representation. Validation and normalization
    /// complete before this IR reaches the backend adapter.
    Typed(Box<QueryExprIr>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct QueryExprIr {
    pub kind: QueryExprKindIr,
    pub annotations: std::collections::BTreeMap<String, RequestValueIr>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum QueryExprKindIr {
    Term {
        value: RequestValueIr,
    },
    Contains {
        field: QueryFieldRefIr,
        item: Box<QueryExprIr>,
    },
    And(Vec<QueryExprIr>),
    Or(Vec<QueryExprIr>),
    Not(Box<QueryExprIr>),
    True,
    False,
    Comparison {
        field: QueryFieldRefIr,
        op: ComparisonOp,
        value: RequestValueIr,
    },
    Range {
        field: QueryFieldRefIr,
        lower: QueryRangeBoundIr,
        upper: QueryRangeBoundIr,
        include_lower: Option<bool>,
        include_upper: Option<bool>,
    },
    In {
        field: QueryFieldRefIr,
        values: Vec<RequestValueIr>,
    },
    Phrase(Vec<QueryExprIr>),
    Near {
        children: Vec<QueryExprIr>,
        distance: u32,
    },
    ONear {
        children: Vec<QueryExprIr>,
        distance: u32,
    },
    SameElement(Vec<QueryExprIr>),
    Equiv(Vec<QueryExprIr>),
    Alternatives(Vec<QueryAlternativeIr>),
    Fuzzy {
        value: RequestValueIr,
        max_edit_distance: u32,
        prefix_length: u32,
        prefix: bool,
    },
    Matches {
        field: QueryFieldRefIr,
        pattern: String,
    },
    Text {
        text: String,
    },
    UserInput {
        value: String,
        default_index: Option<String>,
        target_hits: u32,
    },
    UserQuery {
        default_index: Option<String>,
        target_hits: u32,
    },
    DotProduct {
        field: QueryFieldRefIr,
        items: Vec<WeightedQueryItemIr>,
    },
    WeightedSet {
        field: QueryFieldRefIr,
        items: Vec<WeightedQueryItemIr>,
    },
    Wand {
        field: QueryFieldRefIr,
        items: Vec<WeightedQueryItemIr>,
    },
    WeakAnd {
        children: Vec<QueryExprIr>,
        target_hits: u32,
    },
    Rank {
        matching: Box<QueryExprIr>,
        ranking: Vec<QueryExprIr>,
    },
    Labeled {
        query: Box<QueryExprIr>,
        label: String,
        score: f64,
    },
    NonEmpty(Box<QueryExprIr>),
    GeoLocation {
        field: QueryFieldRefIr,
        latitude: f64,
        longitude: f64,
        radius: String,
    },
    GeoBoundingBox {
        field: QueryFieldRefIr,
        south: f64,
        west: f64,
        north: f64,
        east: f64,
    },
    NearestNeighbor {
        document_field: QueryFieldRefIr,
        query_tensor: String,
    },
    Predicate {
        field: QueryFieldRefIr,
        features: std::collections::BTreeMap<String, RequestValueIr>,
        range_features: std::collections::BTreeMap<String, RequestValueIr>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryFieldRefIr {
    pub path: FieldName,
    pub element_index: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum QueryRangeBoundIr {
    Finite(RequestValueIr),
    NegativeInfinity,
    PositiveInfinity,
    Unbounded,
}

#[derive(Clone, Debug, PartialEq)]
pub struct QueryAlternativeIr {
    pub query: QueryExprIr,
    pub exactness: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WeightedQueryItemIr {
    pub key: RequestValueIr,
    pub weight: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct QueryModelIr {
    pub query_string: Option<String>,
    pub default_index: Option<String>,
    pub encoding: Option<String>,
    pub filter: Option<String>,
    pub language: Option<LanguageHint>,
    pub locale: Option<String>,
    pub query_type: Option<QueryModelTypeIr>,
    pub sources: Vec<String>,
    pub restrict: Vec<String>,
    pub search_path: Option<String>,
    pub search_group: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct QueryModelTypeIr {
    pub composite: Option<String>,
    pub tokenization: Option<String>,
    pub syntax: Option<String>,
    pub profile: Option<String>,
    pub is_yql_default: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct PresentationIr {
    pub summary: Option<String>,
    pub format: Option<String>,
    pub bolding: Option<bool>,
    pub timing: Option<bool>,
}
