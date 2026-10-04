//! Stable diagnostic semantics, independent of engine trees and wire tables.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticIntent {
    ExplainHit,
    ZeroResults,
    MissingDocument,
    RankingComparison,
    Overmatching,
    QueryInterpretation,
    VectorRetrieval,
    Performance,
    DeepEngine,
    Experiment,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DiagnosticSection {
    Query,
    Retrieval,
    Ranking,
    Timing,
    MatchedTerms,
    Vectors,
    Trace,
    MatchingProfiler,
    FirstPhaseProfiler,
    SecondPhaseProfiler,
    Blueprint,
    IteratorTree,
    RawEngine,
    Counterfactual,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticStatus {
    Available,
    Unsupported,
    TimedOut,
    Truncated,
    UnavailableForQuery,
    Failed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticProvenance {
    Profile,
    Engine,
    Calculated,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticExecutionKind {
    Original,
    DiagnosticRerun,
    Experiment,
    Counterfactual,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticObservation {
    Unavailable,
    ObservedYes,
    ObservedNo,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticLimits {
    pub timeout_ms: u64,
    pub variant_count: u32,
    pub feature_hit_count: u32,
    pub tensor_cells: u32,
    pub raw_bytes: u32,
    pub trace_depth: u32,
    pub profiler_depth: u32,
    pub exact_nn_hits: u32,
}

impl Default for DiagnosticLimits {
    fn default() -> Self {
        Self {
            timeout_ms: 5_000,
            variant_count: 8,
            feature_hit_count: 20,
            tensor_cells: 256,
            raw_bytes: 65_536,
            trace_depth: 9,
            profiler_depth: 4,
            exact_nn_hits: 100,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct QueryDiagnosticsRequestIr {
    pub request: SearchRequestIr,
    pub intent: DiagnosticIntent,
    pub requested_evidence: Vec<DiagnosticSection>,
    pub source_run_id: Option<String>,
    pub selected_hits: Vec<String>,
    pub expected_document_id: Option<String>,
    pub experiment: Option<SearchRequestIr>,
    pub compare_exact: bool,
    pub limits: DiagnosticLimits,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticExecution {
    pub id: String,
    pub request_hash: String,
    pub target: Option<String>,
    pub generation: Option<String>,
    pub kind: DiagnosticExecutionKind,
    pub complete: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticFinding {
    pub code: String,
    pub message: String,
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DiagnosticEvidence {
    pub id: String,
    pub section: DiagnosticSection,
    pub status: DiagnosticStatus,
    pub provenance: DiagnosticProvenance,
    pub execution_id: String,
    pub message: String,
    pub values: std::collections::BTreeMap<String, Value>,
    pub canonical_request: Option<SearchRequestIr>,
    pub result: Option<SearchResult>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DiagnosticSignal {
    pub name: String,
    pub value: Value,
    pub position: Option<u32>,
    pub ties: Option<u32>,
    pub comparison_count: u32,
    pub comparison_set: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DiagnosticHit {
    pub id: String,
    pub execution_id: String,
    pub returned_position: u32,
    pub relevance: Option<f64>,
    pub signals: Vec<DiagnosticSignal>,
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticDocumentStage {
    pub name: String,
    pub observation: DiagnosticObservation,
    pub evidence_ids: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticExpectedDocument {
    pub id: String,
    pub stages: Vec<DiagnosticDocumentStage>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticRankMovement {
    pub id: String,
    pub baseline_position: Option<u32>,
    pub rerun_position: Option<u32>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticComparison {
    pub baseline_execution_id: String,
    pub rerun_execution_id: String,
    pub results_changed: Option<bool>,
    pub overlap_count: u32,
    pub comparison_count: u32,
    pub movements: Vec<DiagnosticRankMovement>,
    pub entering: Vec<String>,
    pub leaving: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawEngineDiagnostics {
    pub engine: String,
    pub version: String,
    pub execution_id: String,
    pub section: DiagnosticSection,
    pub status: DiagnosticStatus,
    pub media_type: String,
    pub payload: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct QueryDiagnosticsResult {
    pub execution: DiagnosticExecution,
    pub source_run: Option<DiagnosticExecution>,
    pub findings: Vec<DiagnosticFinding>,
    pub evidence: Vec<DiagnosticEvidence>,
    pub hits: Vec<DiagnosticHit>,
    pub expected_document: Option<DiagnosticExpectedDocument>,
    pub comparison: Option<DiagnosticComparison>,
    pub raw_engine: Vec<RawEngineDiagnostics>,
    pub warnings: Vec<String>,
}

/// Engine acquisition only. Portable findings are computed above this boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct DiagnosticSearchResult {
    pub result: SearchResult,
    pub raw_engine: Vec<RawEngineDiagnostics>,
    /// Sections actually observed by the engine adapter, backed by its raw
    /// execution payload. Raw field names remain private to that adapter.
    pub observed_sections: Vec<DiagnosticSection>,
}

/// A bounded, server-held snapshot; clients cannot manufacture original evidence.
#[derive(Clone, Debug)]
pub struct DiagnosticSourceRun {
    pub execution: DiagnosticExecution,
    pub request: SearchRequestIr,
    pub result: SearchResult,
}
