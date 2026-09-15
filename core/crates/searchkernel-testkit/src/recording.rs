use searchkernel_ir::{Backend, OperationIr, PortableError, Result};
use std::sync::Mutex;

/// A backend that records the exact owned IR requests it receives, in order.
///
/// Recording does not execute a request or invent a result.  Its backend
/// methods return an `Unsupported` portable error after recording the call.
#[derive(Debug, Default)]
pub struct RecordingBackend {
    calls: Mutex<Vec<OperationIr>>,
}

impl RecordingBackend {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a snapshot of calls without changing the recording.
    pub fn calls(&self) -> Vec<OperationIr> {
        self.calls.lock().expect("recording mutex poisoned").clone()
    }

    /// Returns the most recent call, if any, without changing the recording.
    pub fn last_call(&self) -> Option<OperationIr> {
        self.calls
            .lock()
            .expect("recording mutex poisoned")
            .last()
            .cloned()
    }

    pub fn clear(&self) {
        self.calls.lock().expect("recording mutex poisoned").clear();
    }

    pub fn take_calls(&self) -> Vec<OperationIr> {
        std::mem::take(&mut *self.calls.lock().expect("recording mutex poisoned"))
    }

    fn record<T>(&self, operation: OperationIr) -> Result<T> {
        self.calls
            .lock()
            .expect("recording mutex poisoned")
            .push(operation);
        Err(PortableError::unsupported(
            "recording_backend",
            None::<String>,
            "recording backend does not execute operations",
        ))
    }
}

impl Backend for RecordingBackend {
    fn insert(
        &self,
        request: searchkernel_ir::InsertRequestIr,
    ) -> Result<searchkernel_ir::InsertResult> {
        self.record(OperationIr::Insert(request))
    }

    fn upsert(
        &self,
        request: searchkernel_ir::UpsertRequestIr,
    ) -> Result<searchkernel_ir::UpsertResult> {
        self.record(OperationIr::Upsert(request))
    }

    fn patch(
        &self,
        request: searchkernel_ir::PatchRequestIr,
    ) -> Result<searchkernel_ir::PatchResult> {
        self.record(OperationIr::Patch(request))
    }

    fn delete(
        &self,
        request: searchkernel_ir::DeleteRequestIr,
    ) -> Result<searchkernel_ir::DeleteResult> {
        self.record(OperationIr::Delete(request))
    }

    fn get(&self, request: searchkernel_ir::GetRequestIr) -> Result<searchkernel_ir::GetResult> {
        self.record(OperationIr::Get(request))
    }

    fn search(
        &self,
        request: searchkernel_ir::SearchRequestIr,
    ) -> Result<searchkernel_ir::SearchResult> {
        self.record(OperationIr::Search(request))
    }

    fn analyze(
        &self,
        request: searchkernel_ir::AnalyzeRequestIr,
    ) -> Result<searchkernel_ir::AnalyzeResult> {
        self.record(OperationIr::Analyze(request))
    }
}
