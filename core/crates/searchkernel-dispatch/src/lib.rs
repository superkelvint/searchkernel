//! Transport-independent dispatch for canonical Search Kernel envelopes.

use searchkernel_ir::{Backend, BackendSession, IndexContext, SessionError};
use std::sync::Arc;

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
}

impl Dispatcher {
    /// Creates a dispatcher for one already-bound execution target.
    pub fn new(context: Arc<IndexContext>, session: Arc<BackendSession>) -> Self {
        Self { context, session }
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
        self.session.close()
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
