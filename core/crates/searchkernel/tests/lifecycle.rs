use searchkernel::{ErrorCode, Field, IndexDefinition, IndexRuntime};
use searchkernel_ir::{
    Backend, DeleteRequestIr, DeleteResult, GetRequestIr, GetResult, InsertRequestIr, InsertResult,
    PatchRequestIr, PatchResult, SearchRequestIr, SearchResult, UpdateRequestIr, UpdateResult,
    UpsertRequestIr, UpsertResult,
};
use searchkernel_testkit::RecordingBackend;
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

fn definition() -> IndexDefinition {
    IndexDefinition::new().field(Field::text("title"))
}

#[test]
fn cloned_handles_share_runtime_and_close_invalidates_all_handles() {
    let runtime = IndexRuntime::from_backend(definition(), Arc::new(RecordingBackend::new()))
        .expect("create test runtime");
    let index = runtime.index();
    let worker = index.clone();

    assert_eq!(index.describe().unwrap(), worker.describe().unwrap());
    runtime.close().expect("close test runtime");

    for handle in [index, worker] {
        let error = handle
            .search("headphones")
            .inspect()
            .expect_err("closed handle must reject operations");
        assert_eq!(error.code(), ErrorCode::IndexClosed);
    }
}

#[test]
fn close_waits_for_in_flight_operation_and_then_closes_handles() {
    let started = Arc::new(std::sync::Barrier::new(2));
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    let backend = Arc::new(BlockingBackend {
        started: Arc::clone(&started),
        release: Arc::clone(&release),
    });
    let runtime = IndexRuntime::from_backend(definition(), backend).expect("create test runtime");
    let index = runtime.index();
    let worker = index.clone();

    let operation = thread::spawn(move || worker.browse().execute::<serde_json::Value>());
    started.wait();

    let (close_sender, close_receiver) = mpsc::channel();
    thread::spawn(move || {
        close_sender
            .send(runtime.close())
            .expect("send close result")
    });
    assert!(close_receiver
        .recv_timeout(Duration::from_millis(50))
        .is_err());

    {
        let (released, condition) = &*release;
        *released.lock().expect("release mutex") = true;
        condition.notify_all();
    }

    operation
        .join()
        .expect("operation thread")
        .expect("blocking search");
    close_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("close after operation drain")
        .expect("close runtime");

    let error = index
        .describe()
        .expect_err("handle must be closed after runtime shutdown");
    assert_eq!(error.code(), ErrorCode::IndexClosed);
}

#[derive(Debug)]
struct BlockingBackend {
    started: Arc<std::sync::Barrier>,
    release: Arc<(Mutex<bool>, Condvar)>,
}

impl Backend for BlockingBackend {
    fn insert(&self, _request: InsertRequestIr) -> searchkernel_ir::Result<InsertResult> {
        unsupported()
    }

    fn upsert(&self, _request: UpsertRequestIr) -> searchkernel_ir::Result<UpsertResult> {
        unsupported()
    }

    fn patch(&self, _request: PatchRequestIr) -> searchkernel_ir::Result<PatchResult> {
        unsupported()
    }

    fn update(&self, _request: UpdateRequestIr) -> searchkernel_ir::Result<UpdateResult> {
        unsupported()
    }

    fn delete(&self, _request: DeleteRequestIr) -> searchkernel_ir::Result<DeleteResult> {
        unsupported()
    }

    fn get(&self, _request: GetRequestIr) -> searchkernel_ir::Result<GetResult> {
        unsupported()
    }

    fn search(&self, _request: SearchRequestIr) -> searchkernel_ir::Result<SearchResult> {
        self.started.wait();
        let (released, condition) = &*self.release;
        let mut released = released.lock().expect("release mutex");
        while !*released {
            released = condition.wait(released).expect("release condition");
        }
        Ok(SearchResult::empty(searchkernel_ir::TotalHits {
            value: 0,
            relation: searchkernel_ir::TotalHitsRelation::Exact,
        }))
    }

    fn analyze(
        &self,
        _request: searchkernel_ir::AnalyzeRequestIr,
    ) -> searchkernel_ir::Result<searchkernel_ir::AnalyzeResult> {
        unsupported()
    }
}

fn unsupported<T>() -> searchkernel_ir::Result<T> {
    Err(searchkernel_ir::PortableError::unsupported(
        "test_backend",
        None::<String>,
        "operation is not part of this test",
    ))
}
