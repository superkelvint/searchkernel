use searchkernel_core::{Backend, OperationIr, PortableError, Result};
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
        request: searchkernel_core::InsertRequestIr,
    ) -> Result<searchkernel_core::InsertResult> {
        self.record(OperationIr::Insert(request))
    }

    fn upsert(
        &self,
        request: searchkernel_core::UpsertRequestIr,
    ) -> Result<searchkernel_core::UpsertResult> {
        self.record(OperationIr::Upsert(request))
    }

    fn patch(
        &self,
        request: searchkernel_core::PatchRequestIr,
    ) -> Result<searchkernel_core::PatchResult> {
        self.record(OperationIr::Patch(request))
    }

    fn delete(
        &self,
        request: searchkernel_core::DeleteRequestIr,
    ) -> Result<searchkernel_core::DeleteResult> {
        self.record(OperationIr::Delete(request))
    }

    fn get(
        &self,
        request: searchkernel_core::GetRequestIr,
    ) -> Result<searchkernel_core::GetResult> {
        self.record(OperationIr::Get(request))
    }

    fn search(
        &self,
        request: searchkernel_core::SearchRequestIr,
    ) -> Result<searchkernel_core::SearchResult> {
        self.record(OperationIr::Search(request))
    }

    fn analyze(
        &self,
        request: searchkernel_core::AnalyzeRequestIr,
    ) -> Result<searchkernel_core::AnalyzeResult> {
        self.record(OperationIr::Analyze(request))
    }
}
