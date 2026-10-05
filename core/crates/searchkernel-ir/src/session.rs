use crate::{Backend, PortableError};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

struct SharedBackend(Arc<dyn Backend>);

impl Backend for SharedBackend {
    fn target_identity(&self) -> Option<(&str, &str)> {
        self.0.target_identity()
    }

    fn search_diagnostic(
        &self,
        request: crate::SearchRequestIr,
        sections: &[crate::DiagnosticSection],
        limits: &crate::DiagnosticLimits,
    ) -> crate::Result<crate::DiagnosticSearchResult> {
        self.0.search_diagnostic(request, sections, limits)
    }
    fn insert(&self, request: crate::InsertRequestIr) -> crate::Result<crate::InsertResult> {
        self.0.insert(request)
    }

    fn upsert(&self, request: crate::UpsertRequestIr) -> crate::Result<crate::UpsertResult> {
        self.0.upsert(request)
    }

    fn patch(&self, request: crate::PatchRequestIr) -> crate::Result<crate::PatchResult> {
        self.0.patch(request)
    }

    fn update(&self, request: crate::UpdateRequestIr) -> crate::Result<crate::UpdateResult> {
        self.0.update(request)
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

    /// Admit one backend operation while the session is open.
    ///
    /// Admission and the in-flight increment are one mutex-protected
    /// transition. The returned operation lease releases that accounting on
    /// every return path, including Rust unwinding, so reads and mutations
    /// have identical close-race semantics.
    pub fn execute<T, E>(
        &self,
        callback: impl FnOnce(&dyn Backend) -> std::result::Result<T, E>,
    ) -> std::result::Result<std::result::Result<T, E>, SessionError> {
        let backend = self.begin()?;
        Ok(callback(backend.backend()))
    }

    /// Idempotently transition the session from OPEN through CLOSING to
    /// CLOSED, draining operations admitted before the transition. Calls
    /// arriving after CLOSING begins return [`SessionError::Closed`]. A
    /// backend shutdown error, including a caught Rust panic, is returned to
    /// every close caller after the session is nevertheless finalized CLOSED.
    pub fn close(&self) -> crate::Result<()> {
        let backend = {
            let mut state = self.lock_state();
            match state.phase {
                Phase::Closed => return state.close_result.clone().unwrap_or(Ok(())),
                Phase::Open => {
                    state.phase = Phase::Closing;
                    // Wake observers as soon as admission closes. The same
                    // condition variable is also used for drain completion;
                    // notifying here makes the transition observable without
                    // polling and does not permit new work.
                    self.drained.notify_all();
                }
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
                Ok(holder) => {
                    // A Rust backend must not be able to strand the lifecycle in
                    // CLOSING. Native adapters catch their own C++ exceptions;
                    // this guard handles a Rust backend panic and lets us
                    // publish CLOSED before returning the shutdown failure.
                    let BackendHolder { backend } = holder;
                    catch_unwind(AssertUnwindSafe(|| backend.close())).unwrap_or_else(|_| {
                        Err(PortableError::new(
                            crate::ErrorCode::Internal,
                            "backend shutdown panicked",
                        ))
                    })
                }
                Err(backend) => {
                    let _ = catch_unwind(AssertUnwindSafe(|| drop(backend)));
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

    /// Best-effort shutdown used by [`Drop`]. It never propagates a backend
    /// error or panic.
    pub fn best_effort_close(&self) {
        // Drop cannot report shutdown failures and must never unwind. `close`
        // already catches backend shutdown panics; this outer guard also keeps
        // best-effort cleanup safe if that implementation gains another
        // fallible/panicking step later.
        let _ = catch_unwind(AssertUnwindSafe(|| self.close()));
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
            backend: Some(backend),
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
    backend: Option<Arc<BackendHolder>>,
}

impl Operation<'_> {
    fn backend(&self) -> &dyn Backend {
        self.backend
            .as_ref()
            .expect("operation backend is present until drop")
            .backend
            .as_ref()
    }
}

impl Drop for Operation<'_> {
    fn drop(&mut self) {
        // Release the operation's backend owner before publishing active == 0.
        // Otherwise a closer can wake, take the session's owner, and race
        // Arc::try_unwrap against this Operation's still-live backend field.
        drop(self.backend.take());

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
        UpdateRequestIr, UpdateResult, UpsertRequestIr, UpsertResult,
    };
    use std::panic::AssertUnwindSafe;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;
    use std::thread;

    struct ProbeBackend {
        close_calls: std::sync::Arc<AtomicUsize>,
        search_calls: std::sync::Arc<AtomicUsize>,
        close_error: Option<PortableError>,
        search_error: Option<PortableError>,
        panic_on_search: bool,
        panic_on_close: bool,
    }

    impl ProbeBackend {
        fn new(close_error: Option<PortableError>) -> (Box<Self>, std::sync::Arc<AtomicUsize>) {
            let close_calls = std::sync::Arc::new(AtomicUsize::new(0));
            (
                Box::new(Self {
                    close_calls: std::sync::Arc::clone(&close_calls),
                    search_calls: std::sync::Arc::new(AtomicUsize::new(0)),
                    close_error,
                    search_error: None,
                    panic_on_search: false,
                    panic_on_close: false,
                }),
                close_calls,
            )
        }

        fn with_search_counter(search_calls: std::sync::Arc<AtomicUsize>) -> std::sync::Arc<Self> {
            std::sync::Arc::new(Self {
                close_calls: std::sync::Arc::new(AtomicUsize::new(0)),
                search_calls,
                close_error: None,
                search_error: None,
                panic_on_search: false,
                panic_on_close: false,
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

        fn update(&self, _request: UpdateRequestIr) -> crate::Result<UpdateResult> {
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
            if self.panic_on_search {
                panic!("intentional session backend panic");
            }
            if let Some(error) = &self.search_error {
                return Err(error.clone());
            }
            Ok(SearchResult::empty(crate::TotalHits {
                value: 0,
                relation: crate::TotalHitsRelation::Exact,
            }))
        }

        fn analyze(&self, _request: AnalyzeRequestIr) -> crate::Result<AnalyzeResult> {
            Self::unsupported()
        }

        fn close(self: Box<Self>) -> crate::Result<()> {
            self.close_calls.fetch_add(1, Ordering::SeqCst);
            if self.panic_on_close {
                panic!("intentional session shutdown panic");
            }
            self.close_error.map_or(Ok(()), Err)
        }
    }

    fn search_request() -> SearchRequestIr {
        SearchRequestIr::new(
            crate::QueryIr::MatchAll,
            Vec::new(),
            None,
            Vec::new(),
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
        let mut state = session.lock_state();
        while state.phase != Phase::Closing {
            state = session
                .drained
                .wait(state)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
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
                        condition: None,
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
                        condition: None,
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
    fn backend_errors_release_their_lease() {
        let close_calls = std::sync::Arc::new(AtomicUsize::new(0));
        let backend_error = PortableError::new(crate::ErrorCode::Unavailable, "search failed");
        let session = BackendSession::new(Box::new(ProbeBackend {
            close_calls: std::sync::Arc::clone(&close_calls),
            search_calls: std::sync::Arc::new(AtomicUsize::new(0)),
            close_error: None,
            search_error: Some(backend_error.clone()),
            panic_on_search: false,
            panic_on_close: false,
        }));

        let result = session
            .execute(|backend| backend.search(search_request()).map(|_| ()))
            .expect("session must admit the operation");
        assert_eq!(result, Err(backend_error));
        session.close().expect("backend error must not block close");
        assert_eq!(close_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn close_drains_operations_rejects_new_work_and_closes_once() {
        let close_calls = std::sync::Arc::new(AtomicUsize::new(0));
        let search_calls = std::sync::Arc::new(AtomicUsize::new(0));
        let backend = Box::new(ProbeBackend {
            close_calls: std::sync::Arc::clone(&close_calls),
            search_calls: std::sync::Arc::clone(&search_calls),
            close_error: None,
            search_error: None,
            panic_on_search: false,
            panic_on_close: false,
        });
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

        let rejected_callback_calls = std::sync::Arc::new(AtomicUsize::new(0));
        let callback_counter = std::sync::Arc::clone(&rejected_callback_calls);
        assert_eq!(
            session.execute(|_| {
                callback_counter.fetch_add(1, Ordering::SeqCst);
                Ok::<_, ()>(())
            }),
            Err(SessionError::Closed)
        );
        assert_eq!(rejected_callback_calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            session.execute(|backend| backend.search(search_request()).map(|_| ())),
            Err(SessionError::Closed)
        );
        assert_eq!(search_calls.load(Ordering::SeqCst), 0);
        assert_eq!(close_calls.load(Ordering::SeqCst), 0);

        release_sender.send(()).expect("release operation");
        assert_eq!(worker.join().expect("operation thread"), Ok(Ok(())));
        closer.join().expect("close thread").expect("close session");
        assert_eq!(close_calls.load(Ordering::SeqCst), 1);
        assert_eq!(session.close(), Ok(()));
        assert_eq!(close_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn close_waits_for_multiple_admitted_operations() {
        const OPERATION_COUNT: usize = 4;
        let (backend, close_calls) = ProbeBackend::new(None);
        let session = BackendSession::new(backend);
        let (started_sender, started_receiver) = mpsc::channel();
        let releases = (0..OPERATION_COUNT)
            .map(|_| std::sync::Arc::new((Mutex::new(false), Condvar::new())))
            .collect::<Vec<_>>();
        let mut workers = Vec::new();

        for release in &releases {
            let worker_session = std::sync::Arc::clone(&session);
            let started_sender = started_sender.clone();
            let release = std::sync::Arc::clone(release);
            workers.push(thread::spawn(move || {
                worker_session.execute(|_| {
                    started_sender.send(()).expect("signal admitted operation");
                    let (released, condition) = &*release;
                    let mut released = released.lock().expect("release mutex");
                    while !*released {
                        released = condition.wait(released).expect("release condition");
                    }
                    Ok::<_, ()>(())
                })
            }));
        }
        drop(started_sender);
        for _ in 0..OPERATION_COUNT {
            started_receiver
                .recv()
                .expect("all operations must be admitted");
        }

        let close_session = std::sync::Arc::clone(&session);
        let (close_sender, close_receiver) = mpsc::channel();
        let closer = thread::spawn(move || close_sender.send(close_session.close()));
        wait_until_closing(&session);
        assert_eq!(close_calls.load(Ordering::SeqCst), 0);

        for (index, release) in releases.iter().enumerate() {
            let (released, condition) = &**release;
            *released.lock().expect("release mutex") = true;
            condition.notify_all();
            assert_eq!(
                workers.remove(0).join().expect("operation thread"),
                Ok(Ok(()))
            );
            if index + 1 < OPERATION_COUNT {
                assert!(close_receiver.try_recv().is_err());
                assert_eq!(close_calls.load(Ordering::SeqCst), 0);
            }
        }

        closer.join().expect("close thread").expect("close signal");
        assert_eq!(close_receiver.recv().expect("close result"), Ok(()));
        assert_eq!(close_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn mutation_admission_has_the_same_deterministic_close_boundary() {
        // The session has no operation-kind special cases: this callback is a
        // mutation for the purpose of the lifecycle contract, so an admitted
        // mutation completes while close drains and a later one is rejected.
        let (backend, close_calls) = ProbeBackend::new(None);
        let session = BackendSession::new(backend);
        let (admitted_sender, admitted_receiver) = mpsc::channel();
        let release = std::sync::Arc::new((Mutex::new(false), Condvar::new()));
        let worker_session = std::sync::Arc::clone(&session);
        let worker_release = std::sync::Arc::clone(&release);
        let worker = thread::spawn(move || {
            worker_session.execute(|_| {
                admitted_sender.send(()).expect("signal admitted mutation");
                let (released, condition) = &*worker_release;
                let mut released = released.lock().expect("release mutex");
                while !*released {
                    released = condition.wait(released).expect("release condition");
                }
                Ok::<_, ()>(())
            })
        });
        admitted_receiver
            .recv()
            .expect("mutation must be admitted before close");

        let close_session = std::sync::Arc::clone(&session);
        let closer = thread::spawn(move || close_session.close());
        wait_until_closing(&session);
        assert_eq!(
            session.execute(|_| Ok::<_, ()>(())),
            Err(SessionError::Closed)
        );

        let (released, condition) = &*release;
        *released.lock().expect("release mutex") = true;
        condition.notify_all();
        assert_eq!(worker.join().expect("mutation thread"), Ok(Ok(())));
        closer.join().expect("close thread").expect("close session");
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
    fn dropping_one_owner_does_not_shutdown_cloned_session() {
        let (backend, close_calls) = ProbeBackend::new(None);
        let session = BackendSession::new(backend);
        let clone = std::sync::Arc::clone(&session);

        drop(session);
        assert_eq!(close_calls.load(Ordering::SeqCst), 0);
        clone.close().expect("remaining owner closes session");
        assert_eq!(close_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn lifecycle_state_sequences_are_deterministic() {
        #[derive(Clone, Copy)]
        enum Step {
            Operation,
            Close,
        }

        let cases = vec![
            ("open-operation-open", vec![Step::Operation]),
            ("open-close-closed", vec![Step::Close]),
            (
                "open-operation-admitted-closing-operation-complete-closed",
                vec![Step::Operation, Step::Close],
            ),
            ("open-close-close-closed", vec![Step::Close, Step::Close]),
            (
                "closed-operation-rejected",
                vec![Step::Close, Step::Operation],
            ),
        ];

        for (name, steps) in cases {
            let (backend, _close_calls) = ProbeBackend::new(None);
            let session = BackendSession::new(backend);
            let mut closed = false;
            for step in steps {
                match step {
                    Step::Operation => {
                        let result = session.execute(|_| Ok::<_, ()>(()));
                        if closed {
                            assert_eq!(result, Err(SessionError::Closed), "{name}");
                        } else {
                            assert_eq!(result, Ok(Ok(())), "{name}");
                        }
                    }
                    Step::Close => {
                        assert_eq!(session.close(), Ok(()), "{name}");
                        closed = true;
                    }
                }
            }
        }

        let (backend, _close_calls) = ProbeBackend::new(None);
        let owner = BackendSession::new(backend);
        let clone = std::sync::Arc::clone(&owner);
        owner.close().expect("owner closes shared lifecycle");
        assert_eq!(
            clone.execute(|_| Ok::<_, ()>(())),
            Err(SessionError::Closed)
        );
    }

    #[test]
    fn repeated_session_open_close_cycles_do_not_accumulate_state() {
        for _ in 0..100 {
            let (backend, close_calls) = ProbeBackend::new(None);
            let session = BackendSession::new(backend);
            assert_eq!(session.execute(|_| Ok::<_, ()>(())), Ok(Ok(())));
            session.close().expect("cycle close");
            assert_eq!(close_calls.load(Ordering::SeqCst), 1);
        }
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
    fn backend_panics_release_their_lease() {
        let close_calls = std::sync::Arc::new(AtomicUsize::new(0));
        let session = BackendSession::new(Box::new(ProbeBackend {
            close_calls: std::sync::Arc::clone(&close_calls),
            search_calls: std::sync::Arc::new(AtomicUsize::new(0)),
            close_error: None,
            search_error: None,
            panic_on_search: true,
            panic_on_close: false,
        }));

        let panic_result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            session
                .execute::<(), crate::PortableError>(|backend| {
                    backend.search(search_request()).map(|_| ())
                })
                .expect("session must admit the operation")
        }));
        assert!(panic_result.is_err());
        session.close().expect("backend panic must not block close");
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

    #[test]
    fn backend_shutdown_panic_is_converted_and_does_not_wedge_close_or_drop() {
        let close_calls = std::sync::Arc::new(AtomicUsize::new(0));
        let session = BackendSession::new(Box::new(ProbeBackend {
            close_calls: std::sync::Arc::clone(&close_calls),
            search_calls: std::sync::Arc::new(AtomicUsize::new(0)),
            close_error: None,
            search_error: None,
            panic_on_search: false,
            panic_on_close: true,
        }));

        let first = session.close();
        assert!(matches!(first, Err(error) if error.code == crate::ErrorCode::Internal));
        assert!(matches!(
            session.close(),
            Err(error) if error.code == crate::ErrorCode::Internal
        ));
        assert_eq!(close_calls.load(Ordering::SeqCst), 1);
        session.best_effort_close();
    }

    #[test]
    fn dropping_backend_shutdown_panic_never_unwinds() {
        let close_calls = std::sync::Arc::new(AtomicUsize::new(0));
        let result = std::panic::catch_unwind(AssertUnwindSafe({
            let close_calls = std::sync::Arc::clone(&close_calls);
            move || {
                let session = BackendSession::new(Box::new(ProbeBackend {
                    close_calls,
                    search_calls: std::sync::Arc::new(AtomicUsize::new(0)),
                    close_error: None,
                    search_error: None,
                    panic_on_search: false,
                    panic_on_close: true,
                }));
                drop(session);
            }
        }));
        assert!(result.is_ok());
        assert_eq!(close_calls.load(Ordering::SeqCst), 1);
    }
}
