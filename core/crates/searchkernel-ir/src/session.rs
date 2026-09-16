use crate::{Backend, PortableError};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

struct SharedBackend(Arc<dyn Backend>);

impl Backend for SharedBackend {
    fn insert(&self, request: crate::InsertRequestIr) -> crate::Result<crate::InsertResult> {
        self.0.insert(request)
    }

    fn upsert(&self, request: crate::UpsertRequestIr) -> crate::Result<crate::UpsertResult> {
        self.0.upsert(request)
    }

    fn patch(&self, request: crate::PatchRequestIr) -> crate::Result<crate::PatchResult> {
        self.0.patch(request)
    }

    fn delete(&self, request: crate::DeleteRequestIr) -> crate::Result<crate::DeleteResult> {
        self.0.delete(request)
    }

    fn get(&self, request: crate::GetRequestIr) -> crate::Result<crate::GetResult> {
        self.0.get(request)
    }

    fn search(&self, request: crate::SearchRequestIr) -> crate::Result<crate::SearchResult> {
        self.0.search(request)
    }

    fn analyze(&self, request: crate::AnalyzeRequestIr) -> crate::Result<crate::AnalyzeResult> {
        self.0.analyze(request)
    }
}

struct BackendHolder {
    backend: Box<dyn Backend>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Open,
    Closing,
    Closed,
}

struct State {
    phase: Phase,
    active: usize,
    backend: Option<Arc<BackendHolder>>,
}

/// Shared backend ownership and operation-draining state.
///
/// This is the lifecycle primitive used by the Rust product handle and native
/// transport registry. It is intentionally not part of the portable wire API.
pub struct BackendSession {
    state: Mutex<State>,
    drained: Condvar,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionError {
    Closed,
}

impl BackendSession {
    pub fn new(backend: Box<dyn Backend>) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State {
                phase: Phase::Open,
                active: 0,
                backend: Some(Arc::new(BackendHolder { backend })),
            }),
            drained: Condvar::new(),
        })
    }

    pub fn from_backend(backend: Arc<dyn Backend>) -> Arc<Self> {
        Self::new(Box::new(SharedBackend(backend)))
    }

    pub fn execute<T, E>(
        &self,
        callback: impl FnOnce(&dyn Backend) -> std::result::Result<T, E>,
    ) -> std::result::Result<std::result::Result<T, E>, SessionError> {
        let backend = self.begin()?;
        Ok(callback(backend.backend()))
    }

    pub fn close(&self) -> crate::Result<()> {
        let backend = {
            let mut state = self.lock_state();
            match state.phase {
                Phase::Closed => return Ok(()),
                Phase::Open => state.phase = Phase::Closing,
                Phase::Closing => {}
            }
            while state.active != 0 {
                state = self
                    .drained
                    .wait(state)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }
            state.phase = Phase::Closed;
            state.backend.take()
        };

        let Some(backend) = backend else {
            return Ok(());
        };
        match Arc::try_unwrap(backend) {
            Ok(holder) => holder.backend.close(),
            Err(backend) => {
                drop(backend);
                Err(PortableError::new(
                    crate::ErrorCode::Internal,
                    "backend still has an active owner after shutdown",
                ))
            }
        }
    }

    pub fn best_effort_close(&self) {
        let _ = self.close();
    }

    fn begin(&self) -> std::result::Result<Operation<'_>, SessionError> {
        let mut state = self.lock_state();
        if state.phase != Phase::Open {
            return Err(SessionError::Closed);
        }
        let backend = state
            .backend
            .as_ref()
            .expect("open sessions always contain a backend")
            .clone();
        state.active += 1;
        drop(state);
        Ok(Operation {
            session: self,
            backend,
        })
    }

    fn lock_state(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Drop for BackendSession {
    fn drop(&mut self) {
        self.best_effort_close();
    }
}

struct Operation<'a> {
    session: &'a BackendSession,
    backend: Arc<BackendHolder>,
}

impl Operation<'_> {
    fn backend(&self) -> &dyn Backend {
        self.backend.backend.as_ref()
    }
}

impl Drop for Operation<'_> {
    fn drop(&mut self) {
        let mut state = self.session.lock_state();
        state.active = state
            .active
            .checked_sub(1)
            .expect("operation lease count cannot underflow");
        if state.active == 0 {
            self.session.drained.notify_all();
        }
    }
}
