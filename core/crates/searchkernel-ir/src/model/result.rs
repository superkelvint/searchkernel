//! Backend result values.

use super::*;
#[derive(Clone, Debug, PartialEq)]
pub struct DocumentResult {
    pub id: String,
    pub fields: std::collections::BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InsertResult {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpsertResult {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatchResult {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateResult {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeleteResult {
    pub id: String,
    pub deleted: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GetResult {
    pub document: Option<DocumentResult>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TotalHitsRelation {
    Unspecified,
    Exact,
    LowerBound,
    Approximate,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TotalHits {
    pub value: u64,
    pub relation: TotalHitsRelation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HighlightSpan {
    pub start_byte: u32,
    pub end_byte: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HighlightFragment {
    pub text: String,
    pub matches: Vec<HighlightSpan>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HighlightResult {
    pub field: String,
    pub fragments: Vec<HighlightFragment>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Hit {
    pub id: String,
    pub score: Option<f64>,
    pub fields: std::collections::BTreeMap<String, Value>,
    pub highlights: Vec<HighlightResult>,
    pub source: Option<String>,
    pub types: Vec<String>,
    pub match_features: std::collections::BTreeMap<String, Value>,
    pub summary_features: std::collections::BTreeMap<String, Value>,
    pub rank_features: std::collections::BTreeMap<String, Value>,
    pub sort_values: Vec<Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResultLimits {
    pub from: Option<Value>,
    pub to: Option<Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResultGroup {
    pub id: Option<String>,
    pub label: Option<String>,
    pub value: Option<Value>,
    pub fields: std::collections::BTreeMap<String, Value>,
    pub children: Vec<ResultNode>,
    pub continuations: std::collections::BTreeMap<String, String>,
    pub source: Option<String>,
    pub types: Vec<String>,
    pub score: Option<f64>,
    pub limits: Option<ResultLimits>,
    pub metadata: std::collections::BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ResultNode {
    Hit(Hit),
    Group(ResultGroup),
}

#[derive(Clone, Debug, PartialEq)]
pub struct CoverageDegraded {
    pub match_phase: Option<bool>,
    pub timeout: Option<bool>,
    pub adaptive_timeout: Option<bool>,
    pub ann_timeout: Option<bool>,
    pub non_ideal_state: Option<bool>,
    pub metadata: std::collections::BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Coverage {
    pub coverage_percent: Option<f64>,
    pub documents: Option<u64>,
    pub full: Option<bool>,
    pub nodes: Option<u32>,
    pub results: Option<u64>,
    pub results_full: Option<u64>,
    pub degraded: Option<CoverageDegraded>,
    pub metadata: std::collections::BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchTiming {
    pub query_time_seconds: Option<f64>,
    pub search_time_seconds: Option<f64>,
    pub summary_fetch_time_seconds: Option<f64>,
    pub metadata: std::collections::BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchResultError {
    pub code: i64,
    pub summary: Option<String>,
    pub message: Option<String>,
    pub source: Option<String>,
    pub stack_trace: Option<String>,
    pub transient: Option<bool>,
    pub metadata: std::collections::BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchResult {
    pub root: ResultNode,
    pub total_hits: TotalHits,
    pub coverage: Option<Coverage>,
    pub timing: Option<SearchTiming>,
    pub errors: Vec<SearchResultError>,
    pub trace: Option<Value>,
    pub metadata: std::collections::BTreeMap<String, Value>,
}

impl ResultGroup {
    pub fn root(children: Vec<ResultNode>) -> Self {
        Self {
            id: None,
            label: None,
            value: None,
            fields: std::collections::BTreeMap::new(),
            children,
            continuations: std::collections::BTreeMap::new(),
            source: None,
            types: Vec::new(),
            score: None,
            limits: None,
            metadata: std::collections::BTreeMap::new(),
        }
    }
}

impl SearchResult {
    pub fn empty(total_hits: TotalHits) -> Self {
        Self {
            root: ResultNode::Group(ResultGroup::root(Vec::new())),
            total_hits,
            coverage: None,
            timing: None,
            errors: Vec::new(),
            trace: None,
            metadata: std::collections::BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalyzedToken {
    pub surface: String,
    pub term: String,
    pub position: u32,
    pub start_byte: u32,
    pub end_byte: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalyzeResult {
    pub tokens: Vec<AnalyzedToken>,
}
