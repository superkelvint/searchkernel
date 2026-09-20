#![cfg(feature = "native")]

use searchkernel_ir::{
    Backend, DeleteRequestIr, DocumentId, DocumentIr, FieldName, FieldValueIr, GetRequestIr,
    GetResult, InsertRequestIr, PatchOperationIr, PatchRequestIr, ProjectionIr, ScalarValueIr,
    UpsertRequestIr, Value,
};
use searchkernel_ir_adapter::VespaBackend;
use searchkernel_vespa_engine::{ConfigCompiler, Field, FieldType, Schema};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

const SEQUENTIAL_SEEDS: u64 = 10_000;
const CONCURRENT_SEEDS: u64 = 1_000;
const RANDOM_STEPS: usize = 20;
const MODEL_SEED: u64 = 0x534b_4d4f_4445_4c01;
const IDS: [&str; 4] = ["a", "b", "c", "missing"];
const FIELDS: [&str; 5] = ["text", "keyword", "integer", "float", "boolean"];

#[derive(Clone, Debug, PartialEq)]
struct ModelState(BTreeMap<String, ModelDocument>);

#[derive(Clone, Debug, PartialEq)]
struct ModelDocument {
    fields: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq)]
struct DocumentPayload {
    fields: BTreeMap<String, Value>,
}

impl DocumentPayload {
    fn to_ir(&self, id: &str) -> DocumentIr {
        let fields = self
            .fields
            .iter()
            .map(|(name, value)| {
                (
                    FieldName::parse(name.clone()).expect("generated field name is valid"),
                    FieldValueIr::Single(to_scalar_ir(value)),
                )
            })
            .collect();
        DocumentIr::new(
            DocumentId::parse(id.to_owned()).expect("generated document ID is valid"),
            fields,
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
struct PatchStep {
    field: String,
    value: Option<Value>,
}

#[derive(Clone, Debug, PartialEq)]
enum Action {
    Insert {
        id: String,
        document: DocumentPayload,
    },
    Upsert {
        id: String,
        document: DocumentPayload,
    },
    Patch {
        id: String,
        operations: Vec<PatchStep>,
    },
    Delete {
        id: String,
    },
    Get {
        id: String,
    },
    Reopen,
}

#[derive(Clone, Debug, PartialEq)]
enum ModelResult {
    Unit,
    Deleted(bool),
    Document(Option<ModelDocument>),
}

#[derive(Clone, Debug, PartialEq)]
enum Outcome {
    Ok(ModelResult),
    Err(searchkernel_ir::ErrorCode),
}

impl ModelState {
    fn new() -> Self {
        Self(BTreeMap::new())
    }

    fn get(&self, id: &str) -> Option<&ModelDocument> {
        self.0.get(id)
    }

    fn observable_document(fields: &BTreeMap<String, Value>) -> ModelDocument {
        let mut fields = fields.clone();
        // A boolean attribute has a visible false default in Vespa's
        // document summary even when the source document omitted it.
        fields
            .entry("boolean".to_owned())
            .or_insert(Value::Bool(false));
        ModelDocument { fields }
    }

    fn visible_document(&self, id: &str) -> Option<ModelDocument> {
        let mut document = self.get(id)?.clone();
        // Vespa BOOL attributes have no undefined value. A missing source
        // field is therefore observed as false by the native get path.
        document
            .fields
            .entry("boolean".to_owned())
            .or_insert(Value::Bool(false));
        Some(document)
    }

    fn apply(&mut self, action: &Action) -> Outcome {
        match action {
            Action::Insert { id, document } => {
                if self.0.contains_key(id) {
                    return Outcome::Err(searchkernel_ir::ErrorCode::AlreadyExists);
                }
                if document.fields.is_empty() {
                    return Outcome::Err(searchkernel_ir::ErrorCode::InvalidArgument);
                }
                self.0
                    .insert(id.clone(), Self::observable_document(&document.fields));
                Outcome::Ok(ModelResult::Unit)
            }
            Action::Upsert { id, document } => {
                if document.fields.is_empty() {
                    return Outcome::Err(searchkernel_ir::ErrorCode::InvalidArgument);
                }
                self.0
                    .insert(id.clone(), Self::observable_document(&document.fields));
                Outcome::Ok(ModelResult::Unit)
            }
            Action::Patch { id, operations } => {
                if let Some(document) = self.0.get_mut(id) {
                    for operation in operations {
                        match &operation.value {
                            Some(value) => {
                                document
                                    .fields
                                    .insert(operation.field.clone(), value.clone());
                            }
                            None => {
                                if operation.field == "boolean" {
                                    document
                                        .fields
                                        .insert(operation.field.clone(), Value::Bool(false));
                                } else {
                                    document.fields.remove(&operation.field);
                                }
                            }
                        }
                    }
                }
                Outcome::Ok(ModelResult::Unit)
            }
            Action::Delete { id } => Outcome::Ok(ModelResult::Deleted(self.0.remove(id).is_some())),
            Action::Get { id } => Outcome::Ok(ModelResult::Document(self.visible_document(id))),
            Action::Reopen => Outcome::Ok(ModelResult::Unit),
        }
    }
}

fn to_scalar_ir(value: &Value) -> ScalarValueIr {
    match value {
        Value::Bool(value) => ScalarValueIr::Bool(*value),
        Value::Int64(value) => ScalarValueIr::Int64(*value),
        Value::Float64(value) => ScalarValueIr::Float64(*value),
        Value::String(value) => ScalarValueIr::String(value.clone()),
        other => panic!("generator produced non-scalar value: {other:?}"),
    }
}

fn id_for(seed: u64, slot: usize) -> String {
    format!("model-{seed:016x}-{slot}")
}

fn field(name: &str) -> FieldName {
    FieldName::parse(name.to_owned()).expect("generated field name is valid")
}

fn projection() -> ProjectionIr {
    ProjectionIr::new(FIELDS.iter().map(|name| field(name)).collect())
}

fn request_id(id: &str) -> DocumentId {
    DocumentId::parse(id.to_owned()).expect("generated document ID is valid")
}

fn schema() -> Schema {
    Schema::new("model")
        .field(Field::new("text", FieldType::Text).indexed().summary())
        .field(Field::new("keyword", FieldType::Text).indexed().summary())
        .field(
            Field::new("integer", FieldType::Integer)
                .attribute()
                .summary(),
        )
        .field(Field::new("float", FieldType::Float).attribute().summary())
        .field(
            Field::new("boolean", FieldType::Boolean)
                .attribute()
                .summary(),
        )
}

fn unique_root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after the Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "searchkernel-model-mutation-{label}-{}-{nonce}",
        std::process::id()
    ))
}

fn open_backend(root: &Path) -> (PathBuf, PathBuf, VespaBackend) {
    let config = root.join("config");
    let data = root.join("data");
    fs::create_dir_all(&data).expect("create model data directory");
    ConfigCompiler
        .compile_to(&schema(), &config)
        .expect("compile model schema");
    let backend = VespaBackend::open(&data, &config).expect("open model backend");
    (config, data, backend)
}

fn close_backend(backend: VespaBackend) {
    Backend::close(Box::new(backend)).expect("close model backend");
}

fn reopen_backend(backend: VespaBackend, config: &Path, data: &Path) -> VespaBackend {
    close_backend(backend);
    VespaBackend::open(data, config).expect("reopen model backend")
}

fn actual_get(backend: &VespaBackend, id: &str) -> Outcome {
    match backend.get(GetRequestIr::new(request_id(id), projection())) {
        Ok(GetResult { document }) => {
            Outcome::Ok(ModelResult::Document(document.map(|document| {
                assert_eq!(
                    document.id, id,
                    "native get returned the wrong document identity for requested id={id}"
                );
                ModelDocument {
                    fields: document.fields,
                }
            })))
        }
        Err(error) => Outcome::Err(error.code),
    }
}

fn actual_outcome(backend: &VespaBackend, action: &Action) -> Outcome {
    match action {
        Action::Insert { id, document } => backend
            .insert(InsertRequestIr::new(document.to_ir(id)))
            .map(|result| {
                assert_eq!(
                    result.id, *id,
                    "insert returned the wrong document identity"
                );
                Outcome::Ok(ModelResult::Unit)
            })
            .unwrap_or_else(|error| Outcome::Err(error.code)),
        Action::Upsert { id, document } => backend
            .upsert(UpsertRequestIr::new(document.to_ir(id)))
            .map(|result| {
                assert_eq!(
                    result.id, *id,
                    "upsert returned the wrong document identity"
                );
                Outcome::Ok(ModelResult::Unit)
            })
            .unwrap_or_else(|error| Outcome::Err(error.code)),
        Action::Patch { id, operations } => {
            let operations = operations
                .iter()
                .map(|operation| match &operation.value {
                    Some(value) => PatchOperationIr::Set {
                        field: field(&operation.field),
                        value: FieldValueIr::Single(to_scalar_ir(value)),
                    },
                    None => PatchOperationIr::Unset {
                        field: field(&operation.field),
                    },
                })
                .collect();
            backend
                .patch(PatchRequestIr::new(request_id(id), operations))
                .map(|result| {
                    assert_eq!(result.id, *id, "patch returned the wrong document identity");
                    Outcome::Ok(ModelResult::Unit)
                })
                .unwrap_or_else(|error| Outcome::Err(error.code))
        }
        Action::Delete { id } => backend
            .delete(DeleteRequestIr::new(request_id(id)))
            .map(|result| {
                assert_eq!(
                    result.id, *id,
                    "delete returned the wrong document identity"
                );
                Outcome::Ok(ModelResult::Deleted(result.deleted))
            })
            .unwrap_or_else(|error| Outcome::Err(error.code)),
        Action::Get { id } => actual_get(backend, id),
        Action::Reopen => Outcome::Ok(ModelResult::Unit),
    }
}

fn compare_ids(
    backend: &VespaBackend,
    model: &ModelState,
    ids: &[String],
    seed: u64,
    step: usize,
    trace: &[Action],
) {
    for id in ids {
        let expected = Outcome::Ok(ModelResult::Document(model.visible_document(id)));
        let actual = actual_get(backend, id);
        assert_eq!(
            actual, expected,
            "state mismatch seed={seed} step={step} id={id} trace={trace:?}"
        );
    }
}

fn run_sequential_trace(
    backend: &mut Option<VespaBackend>,
    config: &Path,
    data: &Path,
    seed: u64,
    trace: &[Action],
) {
    let ids = trace_ids(trace);
    let mut model = ModelState::new();
    for (step, action) in trace.iter().enumerate() {
        let expected = model.apply(action);
        let actual = if matches!(action, Action::Reopen) {
            let old = backend.take().expect("sequential backend is open");
            *backend = Some(reopen_backend(old, config, data));
            Outcome::Ok(ModelResult::Unit)
        } else {
            actual_outcome(
                backend.as_ref().expect("sequential backend is open"),
                action,
            )
        };
        assert_eq!(
            actual, expected,
            "operation mismatch seed={seed} step={step} action={action:?} trace={trace:?}"
        );
        compare_ids(
            backend.as_ref().expect("sequential backend is open"),
            &model,
            &ids,
            seed,
            step,
            trace,
        );
    }
}

fn trace_ids(trace: &[Action]) -> Vec<String> {
    let mut ids = BTreeSet::new();
    for action in trace {
        match action {
            Action::Insert { id, .. }
            | Action::Upsert { id, .. }
            | Action::Patch { id, .. }
            | Action::Delete { id }
            | Action::Get { id } => {
                ids.insert(id.clone());
            }
            Action::Reopen => {}
        }
    }
    ids.into_iter().collect()
}

fn full_document(seed: u64, version: u64) -> DocumentPayload {
    let mut fields = BTreeMap::new();
    fields.insert(
        "text".to_owned(),
        Value::String(if version.is_multiple_of(2) {
            String::new()
        } else {
            format!("text-{seed}-{version}")
        }),
    );
    fields.insert(
        "keyword".to_owned(),
        Value::String(format!("keyword-{seed}-{version}")),
    );
    fields.insert(
        "integer".to_owned(),
        Value::Int64(match version % 5 {
            0 => i32::MIN as i64,
            1 => -1,
            2 => 0,
            3 => 1,
            _ => i32::MAX as i64,
        }),
    );
    fields.insert(
        "float".to_owned(),
        Value::Float64(match version % 5 {
            0 => -1.5,
            1 => -0.0,
            2 => 0.0,
            3 => 2.5,
            _ => f64::MAX,
        }),
    );
    fields.insert(
        "boolean".to_owned(),
        Value::Bool(!version.is_multiple_of(2)),
    );
    DocumentPayload { fields }
}

fn coverage_prefix(seed: u64, include_reopen: bool) -> Vec<Action> {
    let a = id_for(seed, 0);
    let b = id_for(seed, 1);
    let missing = id_for(seed, 3);
    let mut trace = vec![
        Action::Get {
            id: missing.clone(),
        },
        Action::Patch {
            id: missing.clone(),
            operations: vec![PatchStep {
                field: "text".to_owned(),
                value: Some(Value::String("created-by-patch".to_owned())),
            }],
        },
        Action::Delete { id: missing },
        Action::Insert {
            id: a.clone(),
            document: full_document(seed, 1),
        },
        Action::Patch {
            id: a.clone(),
            operations: vec![
                PatchStep {
                    field: "text".to_owned(),
                    value: Some(Value::String(String::new())),
                },
                PatchStep {
                    field: "keyword".to_owned(),
                    value: None,
                },
            ],
        },
        Action::Get { id: a.clone() },
        Action::Upsert {
            id: a.clone(),
            document: full_document(seed, 2),
        },
        Action::Insert {
            id: a.clone(),
            document: full_document(seed, 3),
        },
        Action::Delete { id: a.clone() },
        Action::Get { id: a.clone() },
        Action::Patch {
            id: a.clone(),
            operations: vec![PatchStep {
                field: "integer".to_owned(),
                value: Some(Value::Int64(7)),
            }],
        },
        Action::Get { id: a },
        Action::Upsert {
            id: b.clone(),
            document: full_document(seed, 1),
        },
        Action::Patch {
            id: b.clone(),
            operations: FIELDS
                .iter()
                .filter(|field| **field != "boolean")
                .map(|field| PatchStep {
                    field: (*field).to_owned(),
                    value: None,
                })
                .collect(),
        },
        Action::Get { id: b.clone() },
        Action::Insert {
            id: b.clone(),
            document: full_document(seed, 4),
        },
        Action::Delete { id: b },
    ];
    if include_reopen {
        trace.push(Action::Reopen);
    }
    trace
}

fn next_random(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *state
}

fn random_value(state: &mut u64, field_index: usize) -> Value {
    match field_index {
        0 | 1 => match next_random(state) % 5 {
            0 => Value::String(String::new()),
            1 => Value::String("a".to_owned()),
            2 => Value::String("alpha beta".to_owned()),
            3 => Value::String(" ".to_owned()),
            _ => Value::String(format!("value-{}", next_random(state) % 32)),
        },
        2 => match next_random(state) % 5 {
            0 => Value::Int64(i32::MIN as i64),
            1 => Value::Int64(-1),
            2 => Value::Int64(0),
            3 => Value::Int64(1),
            _ => Value::Int64(i32::MAX as i64),
        },
        3 => match next_random(state) % 5 {
            0 => Value::Float64(-1.5),
            1 => Value::Float64(-0.0),
            2 => Value::Float64(0.0),
            3 => Value::Float64(2.5),
            _ => Value::Float64(f64::MAX),
        },
        4 => Value::Bool(next_random(state) % 2 == 1),
        _ => unreachable!("field index comes from the schema field list"),
    }
}

fn random_document(state: &mut u64) -> DocumentPayload {
    let mut fields = BTreeMap::new();
    for (index, name) in FIELDS.iter().enumerate() {
        if !next_random(state).is_multiple_of(3) {
            fields.insert((*name).to_owned(), random_value(state, index));
        }
    }
    if next_random(state).is_multiple_of(16) {
        fields.clear();
    }
    DocumentPayload { fields }
}

fn random_patch(state: &mut u64) -> Vec<PatchStep> {
    let mut indices = (0..FIELDS.len()).collect::<Vec<_>>();
    for index in (1..indices.len()).rev() {
        let swap = (next_random(state) as usize) % (index + 1);
        indices.swap(index, swap);
    }
    let count = 1 + (next_random(state) as usize % 3);
    indices
        .into_iter()
        .take(count)
        .map(|index| PatchStep {
            field: FIELDS[index].to_owned(),
            value: (!next_random(state).is_multiple_of(3)).then(|| random_value(state, index)),
        })
        .collect()
}

fn random_trace(seed: u64, steps: usize, include_reopen: bool) -> Vec<Action> {
    let mut state = MODEL_SEED ^ seed;
    let ids = (0..IDS.len())
        .map(|slot| id_for(seed, slot))
        .collect::<Vec<_>>();
    let mut trace = coverage_prefix(seed, include_reopen);
    for _ in 0..steps {
        let id = ids[(next_random(&mut state) as usize) % ids.len()].clone();
        let action = match next_random(&mut state) % 32 {
            0 => Action::Insert {
                id,
                document: random_document(&mut state),
            },
            1 => Action::Upsert {
                id,
                document: random_document(&mut state),
            },
            2 => Action::Patch {
                id,
                operations: random_patch(&mut state),
            },
            3 => Action::Delete { id },
            4..=30 => Action::Get { id },
            _ => Action::Reopen,
        };
        trace.push(action);
    }
    trace
}

fn actual_document_state(backend: &VespaBackend, ids: &[String]) -> ModelState {
    ModelState(
        ids.iter()
            .filter_map(|id| match actual_get(backend, id) {
                Outcome::Ok(ModelResult::Document(Some(document))) => Some((id.clone(), document)),
                Outcome::Ok(ModelResult::Document(None)) => None,
                other => panic!("quiescent get failed for {id}: {other:?}"),
            })
            .collect(),
    )
}

#[derive(Clone, Debug)]
struct Event {
    action: Action,
    invocation: u64,
    completion: u64,
    outcome: Outcome,
}

fn concurrent_actions(seed: u64, worker: usize) -> Vec<Action> {
    let mut state = MODEL_SEED ^ seed ^ ((worker as u64) << 32);
    let id = id_for(seed, 0);
    (0..3)
        .map(|_| match next_random(&mut state) % 5 {
            0 => Action::Insert {
                id: id.clone(),
                document: DocumentPayload {
                    fields: BTreeMap::from([(
                        "integer".to_owned(),
                        Value::Int64((worker as i64) + (next_random(&mut state) % 7) as i64),
                    )]),
                },
            },
            1 => Action::Upsert {
                id: id.clone(),
                document: DocumentPayload {
                    fields: BTreeMap::from([(
                        "integer".to_owned(),
                        Value::Int64((worker as i64) + (next_random(&mut state) % 7) as i64),
                    )]),
                },
            },
            2 => Action::Patch {
                id: id.clone(),
                operations: vec![PatchStep {
                    field: "integer".to_owned(),
                    value: Some(Value::Int64((next_random(&mut state) % 11) as i64)),
                }],
            },
            3 => Action::Delete { id: id.clone() },
            _ => Action::Get { id: id.clone() },
        })
        .collect()
}

fn linearizable(events: &[Event], final_state: &ModelState) -> bool {
    // The allowed outcome set is every sequential model execution that
    // preserves the real-time order of non-overlapping operations.
    fn search(
        events: &[Event],
        predecessors: &[u64],
        used: u64,
        state: &ModelState,
        final_state: &ModelState,
    ) -> bool {
        if used == (1_u64 << events.len()) - 1 {
            return state == final_state;
        }
        for index in 0..events.len() {
            let bit = 1_u64 << index;
            if used & bit != 0 || predecessors[index] & !used != 0 {
                continue;
            }
            let mut next_state = state.clone();
            if next_state.apply(&events[index].action) != events[index].outcome {
                continue;
            }
            if search(events, predecessors, used | bit, &next_state, final_state) {
                return true;
            }
        }
        false
    }

    let mut predecessors = vec![0_u64; events.len()];
    for (left_index, left) in events.iter().enumerate() {
        for (right_index, right) in events.iter().enumerate() {
            if left.completion < right.invocation {
                predecessors[right_index] |= 1_u64 << left_index;
            }
        }
    }
    search(events, &predecessors, 0, &ModelState::new(), final_state)
}

fn run_concurrent_trace(backend: Arc<VespaBackend>, seed: u64) {
    let start = Arc::new(Barrier::new(3));
    let clock = Arc::new(AtomicU64::new(0));
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut workers = Vec::new();
    for worker in 0..2 {
        let backend = Arc::clone(&backend);
        let start = Arc::clone(&start);
        let clock = Arc::clone(&clock);
        let events = Arc::clone(&events);
        let actions = concurrent_actions(seed, worker);
        workers.push(thread::spawn(move || {
            start.wait();
            for action in actions {
                let invocation = clock.fetch_add(1, Ordering::SeqCst);
                let outcome = actual_outcome(&backend, &action);
                let completion = clock.fetch_add(1, Ordering::SeqCst);
                events
                    .lock()
                    .expect("event mutex is not poisoned")
                    .push(Event {
                        action,
                        invocation,
                        completion,
                        outcome,
                    });
            }
        }));
    }
    start.wait();
    for worker in workers {
        worker
            .join()
            .expect("concurrent mutation worker did not panic");
    }

    let events = events.lock().expect("event mutex is not poisoned").clone();
    let ids = vec![id_for(seed, 0)];
    let final_state = actual_document_state(&backend, &ids);
    assert!(
        linearizable(&events, &final_state),
        "non-linearizable concurrent trace seed={seed} events={events:?} final_state={final_state:?}"
    );
}

#[test]
fn model_trace_covers_mutations_reopen_and_live_empty_documents() {
    let root = unique_root("focused");
    let (config, data, backend) = open_backend(&root);
    let mut backend = Some(backend);
    let trace = coverage_prefix(1, true);
    run_sequential_trace(&mut backend, &config, &data, 1, &trace);
    close_backend(backend.take().expect("focused backend is open"));
    fs::remove_dir_all(root).expect("remove focused model test directory");
}

#[test]
fn model_concurrent_same_document_trace_is_mandatory() {
    let root = unique_root("same-document-concurrent");
    let (_config, _data, backend) = open_backend(&root);
    let backend = Arc::new(backend);
    run_concurrent_trace(backend.clone(), 0x5eed);
    let backend = match Arc::try_unwrap(backend) {
        Ok(backend) => backend,
        Err(_) => panic!("same-document backend still has owners"),
    };
    close_backend(backend);
    fs::remove_dir_all(root).expect("remove same-document concurrency directory");
}

fn run_sequential_corpus(root: &Path) {
    let (config, data, backend) = open_backend(root);
    let mut backend = Some(backend);
    let selected_seed = std::env::var("SEARCHKERNEL_MODEL_SEED")
        .ok()
        .and_then(|value| value.parse::<u64>().ok());
    let sequential_seeds = selected_seed
        .map(|seed| vec![seed])
        .unwrap_or_else(|| (0..SEQUENTIAL_SEEDS).collect());
    for seed in sequential_seeds {
        let trace = random_trace(seed, RANDOM_STEPS, seed % 64 == 0);
        run_sequential_trace(&mut backend, &config, &data, seed, &trace);
    }
    close_backend(backend.take().expect("sequential backend is open"));
}

fn run_concurrent_corpus(root: &Path) {
    let (_config, _data, backend) = open_backend(root);
    let backend = Arc::new(backend);
    let selected_concurrent_seed = std::env::var("SEARCHKERNEL_MODEL_CONCURRENT_SEED")
        .ok()
        .and_then(|value| value.parse::<u64>().ok());
    let concurrent_seeds = match selected_concurrent_seed {
        Some(seed) => vec![seed],
        None => (0..CONCURRENT_SEEDS).collect(),
    };
    for seed in concurrent_seeds {
        run_concurrent_trace(Arc::clone(&backend), seed);
    }
    let backend = match Arc::try_unwrap(backend) {
        Ok(backend) => backend,
        Err(_) => panic!("concurrent backend still has owners"),
    };
    close_backend(backend);
}

#[test]
#[ignore = "native randomized stress corpus; run with --ignored"]
fn model_based_mutation_stress_10000_seeds() {
    let root = unique_root("stress");
    run_sequential_corpus(&root);
    fs::remove_dir_all(root).expect("remove sequential model test directory");
}

#[test]
#[ignore = "native randomized stress corpus; run with --ignored"]
fn model_based_concurrent_mutation_stress_1000_seeds() {
    let root = unique_root("concurrent-stress");
    run_concurrent_corpus(&root);
    fs::remove_dir_all(root).expect("remove randomized model test directory");
}
