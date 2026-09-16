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
    close_result: Option<crate::Result<()>>,
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
                close_result: None,
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
                Phase::Closed => return state.close_result.clone().unwrap_or(Ok(())),
                Phase::Open => state.phase = Phase::Closing,
                Phase::Closing => {
                    while state.phase != Phase::Closed {
                        state = self
                            .drained
                            .wait(state)
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                    }
                    return state.close_result.clone().unwrap_or(Ok(()));
                }
            }
            while state.active != 0 {
                state = self
                    .drained
                    .wait(state)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }
            state.backend.take()
        };

        let result = match backend {
            None => Ok(()),
            Some(backend) => match Arc::try_unwrap(backend) {
                Ok(holder) => holder.backend.close(),
                Err(backend) => {
                    drop(backend);
                    Err(PortableError::new(
                        crate::ErrorCode::Internal,
                        "backend still has an active owner after shutdown",
                    ))
                }
            },
        };
        let mut state = self.lock_state();
        state.phase = Phase::Closed;
        state.close_result = Some(result.clone());
        self.drained.notify_all();
        result
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AnalyzeRequestIr, AnalyzeResult, DeleteRequestIr, DeleteResult, GetRequestIr, GetResult,
        InsertRequestIr, InsertResult, PatchRequestIr, PatchResult, SearchRequestIr, SearchResult,
        UpsertRequestIr, UpsertResult,
    };
    use std::panic::AssertUnwindSafe;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;
    use std::thread;

    struct ProbeBackend {
        close_calls: std::sync::Arc<AtomicUsize>,
        search_calls: std::sync::Arc<AtomicUsize>,
        close_error: Option<PortableError>,
    }

    impl ProbeBackend {
        fn new(close_error: Option<PortableError>) -> (Box<Self>, std::sync::Arc<AtomicUsize>) {
            let close_calls = std::sync::Arc::new(AtomicUsize::new(0));
            (
                Box::new(Self {
                    close_calls: std::sync::Arc::clone(&close_calls),
                    search_calls: std::sync::Arc::new(AtomicUsize::new(0)),
                    close_error,
                }),
                close_calls,
            )
        }

        fn with_search_counter(search_calls: std::sync::Arc<AtomicUsize>) -> std::sync::Arc<Self> {
            std::sync::Arc::new(Self {
                close_calls: std::sync::Arc::new(AtomicUsize::new(0)),
                search_calls,
                close_error: None,
            })
        }

        fn unsupported<T>() -> crate::Result<T> {
            Err(PortableError::unsupported(
                "session_test_backend",
                None::<String>,
                "operation is not part of this test",
            ))
        }
    }

    impl Backend for ProbeBackend {
        fn insert(&self, _request: InsertRequestIr) -> crate::Result<InsertResult> {
            Self::unsupported()
        }

        fn upsert(&self, _request: UpsertRequestIr) -> crate::Result<UpsertResult> {
            Self::unsupported()
        }

        fn patch(&self, _request: PatchRequestIr) -> crate::Result<PatchResult> {
            Self::unsupported()
        }

        fn delete(&self, _request: DeleteRequestIr) -> crate::Result<DeleteResult> {
            Self::unsupported()
        }

        fn get(&self, _request: GetRequestIr) -> crate::Result<GetResult> {
            Self::unsupported()
        }

        fn search(&self, _request: SearchRequestIr) -> crate::Result<SearchResult> {
            self.search_calls.fetch_add(1, Ordering::SeqCst);
            Ok(SearchResult {
                hits: Vec::new(),
                total_hits: crate::TotalHits {
                    value: 0,
                    relation: crate::TotalHitsRelation::Exact,
                },
                facets: Vec::new(),
            })
        }

        fn analyze(&self, _request: AnalyzeRequestIr) -> crate::Result<AnalyzeResult> {
            Self::unsupported()
        }

        fn close(self: Box<Self>) -> crate::Result<()> {
            self.close_calls.fetch_add(1, Ordering::SeqCst);
            self.close_error.map_or(Ok(()), Err)
        }
    }

    fn search_request() -> SearchRequestIr {
        SearchRequestIr::new(
            crate::QueryIr::MatchAll,
            Vec::new(),
            None,
            Vec::new(),
            Vec::new(),
            crate::RankingPolicyIr::DefaultRelevanceV1,
            10,
            0,
            crate::ProjectionIr::new(Vec::new()),
            crate::HighlightingIr::Disabled,
            None,
            crate::PrefixPolicy::Disabled,
            crate::TypoPolicy::Disabled,
        )
    }

    fn document() -> crate::DocumentIr {
        crate::DocumentIr {
            id: crate::DocumentId::parse("session-test-document").expect("valid test document ID"),
            fields: std::collections::BTreeMap::new(),
        }
    }

    fn analyze_request() -> AnalyzeRequestIr {
        AnalyzeRequestIr {
            analyzer: crate::AnalyzerIdentity::StandardV1,
            text: "session test".to_owned(),
            language: None,
        }
    }

    fn wait_until_closing(session: &BackendSession) {
        for _ in 0..10_000 {
            if session.lock_state().phase == Phase::Closing {
                return;
            }
            thread::yield_now();
        }
        panic!("session did not enter the closing phase");
    }

    #[test]
    fn from_backend_forwards_operations_and_preserves_results() {
        let search_calls = std::sync::Arc::new(AtomicUsize::new(0));
        let backend = ProbeBackend::with_search_counter(std::sync::Arc::clone(&search_calls));
        let session = BackendSession::from_backend(backend);

        let result = session
            .execute(|backend| {
                assert!(backend
                    .insert(InsertRequestIr {
                        document: document(),
                    })
                    .is_err());
                assert!(backend
                    .upsert(UpsertRequestIr {
                        document: document(),
                    })
                    .is_err());
                assert!(backend
                    .patch(PatchRequestIr {
                        id: crate::DocumentId::parse("session-test-document")
                            .expect("valid test document ID"),
                        operations: Vec::new(),
                    })
                    .is_err());
                assert!(backend
                    .delete(DeleteRequestIr {
                        id: crate::DocumentId::parse("session-test-document")
                            .expect("valid test document ID"),
                    })
                    .is_err());
                assert!(backend
                    .get(GetRequestIr {
                        id: crate::DocumentId::parse("session-test-document")
                            .expect("valid test document ID"),
                        projection: crate::ProjectionIr::new(Vec::new()),
                    })
                    .is_err());
                backend
                    .search(search_request())
                    .map(|result| result.total_hits)
                    .inspect(|_| {
                        assert!(backend.analyze(analyze_request()).is_err());
                    })
            })
            .expect("open session")
            .expect("search result");

        assert_eq!(result.value, 0);
        assert_eq!(search_calls.load(Ordering::SeqCst), 1);
        session.close().expect("close session");
    }

    #[test]
    fn operation_errors_release_their_lease() {
        let (backend, close_calls) = ProbeBackend::new(None);
        let session = BackendSession::new(backend);

        let result = session.execute(|_| Err::<(), _>("operation failed"));
        assert_eq!(result, Ok(Err("operation failed")));
        session
            .close()
            .expect("operation error must not block close");
        assert_eq!(close_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn close_drains_operations_rejects_new_work_and_closes_once() {
        let (backend, close_calls) = ProbeBackend::new(None);
        let session = BackendSession::new(backend);
        let (started_sender, started_receiver) = mpsc::channel();
        let (release_sender, release_receiver) = mpsc::channel();

        let worker_session = std::sync::Arc::clone(&session);
        let worker = thread::spawn(move || {
            worker_session.execute(|_| {
                started_sender.send(()).expect("signal operation start");
                release_receiver.recv().expect("wait for release");
                Ok::<_, ()>(())
            })
        });
        started_receiver
            .recv()
            .expect("operation must enter before close");

        let close_session = std::sync::Arc::clone(&session);
        let closer = thread::spawn(move || close_session.close());
        wait_until_closing(&session);

        assert_eq!(
            session.execute(|_| Ok::<_, ()>(())),
            Err(SessionError::Closed)
        );
        assert_eq!(close_calls.load(Ordering::SeqCst), 0);

        release_sender.send(()).expect("release operation");
        assert_eq!(worker.join().expect("operation thread"), Ok(Ok(())));
        closer.join().expect("close thread").expect("close session");
        assert_eq!(close_calls.load(Ordering::SeqCst), 1);
        assert_eq!(session.close(), Ok(()));
        assert_eq!(close_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn concurrent_closes_do_not_double_shutdown() {
        let (backend, close_calls) = ProbeBackend::new(None);
        let session = BackendSession::new(backend);
        let (started_sender, started_receiver) = mpsc::channel();
        let (release_sender, release_receiver) = mpsc::channel();

        let worker_session = std::sync::Arc::clone(&session);
        let worker = thread::spawn(move || {
            worker_session.execute(|_| {
                started_sender.send(()).expect("signal operation start");
                release_receiver.recv().expect("wait for release");
                Ok::<_, ()>(())
            })
        });
        started_receiver
            .recv()
            .expect("operation must enter before close");

        let first_session = std::sync::Arc::clone(&session);
        let first = thread::spawn(move || first_session.close());
        wait_until_closing(&session);
        let second_session = std::sync::Arc::clone(&session);
        let second = thread::spawn(move || second_session.close());

        release_sender.send(()).expect("release operation");
        assert_eq!(worker.join().expect("operation thread"), Ok(Ok(())));
        assert_eq!(first.join().expect("first close thread"), Ok(()));
        assert_eq!(second.join().expect("second close thread"), Ok(()));
        assert_eq!(close_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn callback_panics_release_their_lease() {
        let (backend, close_calls) = ProbeBackend::new(None);
        let session = BackendSession::new(backend);

        let panic_result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            session.execute::<(), ()>(|_| panic!("intentional session test panic"))
        }));
        assert!(panic_result.is_err());
        session.close().expect("panic must not block close");
        assert_eq!(close_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn close_errors_are_returned_and_best_effort_close_suppresses_them() {
        let expected = PortableError::new(crate::ErrorCode::Unavailable, "shutdown failed");
        let (backend, close_calls) = ProbeBackend::new(Some(expected.clone()));
        let session = BackendSession::new(backend);

        assert_eq!(session.close(), Err(expected));
        assert_eq!(close_calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            session.close(),
            Err(PortableError::new(
                crate::ErrorCode::Unavailable,
                "shutdown failed"
            ))
        );
        assert_eq!(close_calls.load(Ordering::SeqCst), 1);

        let (backend, close_calls) = ProbeBackend::new(Some(PortableError::new(
            crate::ErrorCode::Internal,
            "best effort shutdown failed",
        )));
        let session = BackendSession::new(backend);
        session.best_effort_close();
        assert_eq!(close_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn dropping_the_final_owner_performs_best_effort_shutdown_once() {
        let (backend, close_calls) = ProbeBackend::new(Some(PortableError::new(
            crate::ErrorCode::Internal,
            "drop shutdown failed",
        )));
        {
            let _session = BackendSession::new(backend);
        }
        assert_eq!(close_calls.load(Ordering::SeqCst), 1);
    }
}
