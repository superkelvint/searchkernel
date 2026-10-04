//! Transport-independent dispatch for canonical Search Kernel envelopes.

use searchkernel_ir::{Backend, BackendSession, IndexContext, SessionError};
use std::collections::VecDeque;
use std::sync::{atomic::{AtomicU64, Ordering}, Arc, Mutex};

#[cfg(test)]
mod tests;
mod wire;

/// Failures at the dispatcher/framing boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DispatchError {
    InvalidRequest,
    UnsupportedProtocol(u32),
    UnsupportedOperation(u8),
    Closed,
}

/// An execution target bound to its schema context and lifecycle session.
pub struct Dispatcher {
    context: Arc<IndexContext>,
    session: Arc<BackendSession>,
    source_runs: Mutex<VecDeque<searchkernel_ir::DiagnosticSourceRun>>,
    execution_prefix: String,
    execution_counter: AtomicU64,
}

impl Dispatcher {
    /// Creates a dispatcher for one already-bound execution target.
    pub fn new(context: Arc<IndexContext>, session: Arc<BackendSession>) -> Self {
        static TARGET_COUNTER: AtomicU64 = AtomicU64::new(0);
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default().as_nanos();
        Self { context, session, source_runs: Mutex::new(VecDeque::new()),
            execution_prefix: format!("{time:x}-{}", TARGET_COUNTER.fetch_add(1, Ordering::Relaxed)),
            execution_counter: AtomicU64::new(0) }
    }

    /// Creates an execution target from a backend for tests and embedded callers.
    pub fn from_backend(context: Arc<IndexContext>, backend: Arc<dyn Backend>) -> Self {
        Self::new(context, BackendSession::from_backend(backend))
    }

    /// Runs one canonical request envelope.
    pub fn dispatch(&self, request: &[u8]) -> Result<Vec<u8>, DispatchError> {
        wire::dispatch(self, request)
    }

    /// Closes the underlying execution target after active operations drain.
    pub fn close(&self) -> searchkernel_ir::Result<()> {
        if let Ok(mut sources) = self.source_runs.lock() { sources.clear(); }
        self.session.close()
    }

    fn execution(&self, request: &searchkernel_ir::SearchRequestIr,
        target: Option<&str>, generation: Option<&str>,
        kind: searchkernel_ir::DiagnosticExecutionKind) -> searchkernel_ir::DiagnosticExecution {
        searchkernel_ir::DiagnosticExecution {
            id: format!("{}-{}", self.execution_prefix, self.execution_counter.fetch_add(1, Ordering::Relaxed)),
            request_hash: searchkernel_ir::diagnostics::request_hash(request),
            target: target.map(str::to_owned), generation: generation.map(str::to_owned),
            kind, complete: false,
        }
    }

    fn remember_source(&self, request: &searchkernel_ir::SearchRequestIr,
        result: &mut searchkernel_ir::SearchResult, target: Option<&str>, generation: Option<&str>) {
        use searchkernel_ir::{DiagnosticExecutionKind, DiagnosticSourceRun, Hit, ResultGroup, ResultNode, Value};
        let mut execution = self.execution(request, target, generation, DiagnosticExecutionKind::Original);
        execution.complete = searchkernel_ir::diagnostics::execution_complete(result);
        result.metadata.insert(searchkernel_ir::diagnostics::SOURCE_RUN_METADATA_KEY.into(), Value::String(execution.id.clone()));
        let hits = searchkernel_ir::diagnostics::returned_hits(&result.root);
        // This is lightweight response retention, not engine profiling. Large
        // executions remain valid searches but have explicitly unavailable
        // original evidence. Cache lifetime follows this exact target session.
        if hits.len() > 100 { return; }
        let snapshot_hits = hits.into_iter().map(|hit| ResultNode::Hit(Hit {
            id: hit.id.clone(), score: hit.score, match_features: hit.match_features.clone(),
            summary_features: hit.summary_features.clone(), rank_features: hit.rank_features.clone(),
            ..Hit::default()
        })).collect();
        let snapshot = searchkernel_ir::SearchResult {
            root: ResultNode::Group(ResultGroup::root(snapshot_hits)), total_hits: result.total_hits,
            coverage: result.coverage.clone(), timing: result.timing.clone(), errors: Vec::new(),
            trace: None, metadata: Default::default(),
        };
        // The byte bound also covers feature names/tensor shape overhead.
        if searchkernel_ir::wire::v1::search_result_to_api(&snapshot).map_or(true, |bytes| bytes.len() > 524_288)
            || searchkernel_ir::wire::v1::search_request_ir_to_api(request).len() > 262_144 { return; }
        if let Ok(mut sources) = self.source_runs.lock() {
            sources.push_back(DiagnosticSourceRun { execution, request: request.clone(), result: snapshot });
            while sources.len() > 16 { sources.pop_front(); }
        }
    }

    fn source_run(&self, id: Option<&str>, target: Option<&str>, generation: Option<&str>)
        -> Option<searchkernel_ir::DiagnosticSourceRun> {
        let id = id?;
        self.source_runs.lock().ok()?.iter().find(|source| source.execution.id == id
            && source.execution.target.as_deref() == target
            && source.execution.generation.as_deref() == generation).cloned()
    }

    fn execute_backend<T>(
        &self,
        callback: impl FnOnce(&IndexContext, &dyn Backend) -> searchkernel_ir::Result<T>,
    ) -> Result<searchkernel_ir::Result<T>, DispatchError> {
        Ok(
            match self
                .session
                .execute(|backend| callback(&self.context, backend))
            {
                Ok(result) => result,
                Err(SessionError::Closed) => Err(closed_target_error()),
            },
        )
    }

    fn execute_context<T>(
        &self,
        callback: impl FnOnce(&IndexContext) -> searchkernel_ir::Result<T>,
    ) -> Result<searchkernel_ir::Result<T>, DispatchError> {
        Ok(match self.session.execute(|_| callback(&self.context)) {
            Ok(result) => result,
            Err(SessionError::Closed) => Err(closed_target_error()),
        })
    }
}

fn closed_target_error() -> searchkernel_ir::PortableError {
    searchkernel_ir::PortableError::failed_precondition(
        "target_closed",
        None::<String>,
        "Search Kernel target is closed",
    )
}
