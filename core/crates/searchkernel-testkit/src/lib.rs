//! Backend-free infrastructure for testing the portable API/IR boundary.
//!
//! This crate records and scripts the existing [`searchkernel_ir::Backend`]
//! execution surface.  It deliberately contains no search or ranking logic.

mod error;
mod recording;
mod runner;
mod scripted;

pub use error::{OperationKind, ScriptedCallError, TestkitError};
pub use recording::RecordingBackend;
pub use runner::{
    build_component_transport_case, ConformanceReport, ConformanceRunner, RunnerError,
    TransportCase,
};
pub use scripted::{ExpectedCall, ScriptedBackend, ScriptedOutcome, ScriptedResult};

pub use searchkernel_ir::{
    AnalyzeRequestIr, AnalyzeResult, Backend, DeleteRequestIr, DeleteResult, GetRequestIr,
    GetResult, InsertRequestIr, InsertResult, OperationIr, PatchRequestIr, PatchResult,
    SearchRequestIr, SearchResult, UpsertRequestIr, UpsertResult,
};
