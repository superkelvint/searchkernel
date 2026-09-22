//! Deterministic model coverage for the shared index/session lifecycle.
//!
//! The non-native suite deliberately keeps the backend stateless.  A recording
//! pass captures the exact IR produced by the public facade, then a scripted
//! pass replays those calls with results computed by the small model below.
//! This keeps the test focused on admission, lifecycle, and result propagation
//! rather than accidentally implementing a second search engine.

#[cfg(not(feature = "native"))]
mod scripted_model {
    use super::super::*;
    use crate::{Error, ErrorCode, Field, IndexDefinition, Patch};
    use searchkernel_ir::{
        Backend, DocumentResult, OperationIr, PortableError, SearchResult, TotalHits,
        TotalHitsRelation, UpdateOperationInput,
    };
    use searchkernel_testkit::{ExpectedCall, RecordingBackend, ScriptedBackend, ScriptedResult};
    use serde_json::{json, Map, Value};
    use std::collections::BTreeMap;
    use std::sync::Arc;

    const GENERATOR_VERSION: &str = "index-state-model-v1";
    const DEFAULT_STEPS: usize = 48;
    const DEFAULT_SEEDS: &[u64] = &[0x51_7a_11, 0x9e_37_79, 0xd1_ce_42];

    type Document = BTreeMap<String, Value>;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Phase {
        Open,
        Closed,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum ClosedOperation {
        Insert,
        Upsert,
        Patch,
        Update,
        Delete,
        Get,
        Search,
    }

    #[derive(Clone, Debug, PartialEq)]
    struct Mutation {
        handle: u32,
        id: String,
        document: Document,
        fail: bool,
    }

    #[derive(Clone, Debug, PartialEq)]
    struct FieldMutation {
        handle: u32,
        id: String,
        field: String,
        value: Option<Value>,
        fail: bool,
    }

    #[derive(Clone, Debug, PartialEq)]
    struct Read {
        handle: u32,
        id: String,
        fail: bool,
    }

    #[derive(Clone, Debug, PartialEq)]
    enum Action {
        Create {
            handle: u32,
        },
        Open {
            handle: u32,
        },
        Reopen {
            handle: u32,
        },
        Clone {
            source: u32,
            target: u32,
        },
        Insert(Mutation),
        Upsert(Mutation),
        Patch(FieldMutation),
        Update(FieldMutation),
        Delete(Read),
        Get(Read),
        Search {
            handle: u32,
            fail: bool,
        },
        Close {
            handle: u32,
        },
        DropHandle {
            handle: u32,
        },
        OperationOnClosed {
            handle: u32,
            operation: ClosedOperation,
        },
    }

    #[derive(Clone, Debug, PartialEq)]
    enum Expected {
        Noop,
        Created,
        Cloned,
        Dropped,
        Success,
        Error(ErrorCode),
        Deleted(bool),
        Got(Option<Document>),
        Searched,
    }

    #[derive(Clone, Debug)]
    struct PlannedAction {
        action: Action,
        before: String,
        after: Model,
        expected: Expected,
        admitted: bool,
    }

    #[derive(Clone, Debug, Default)]
    struct Model {
        next_generation: usize,
        generations: BTreeMap<usize, Phase>,
        handles: BTreeMap<u32, usize>,
        documents: BTreeMap<String, Document>,
    }

    impl Model {
        fn generation_phase(&self, handle: u32) -> Option<Phase> {
            self.handles
                .get(&handle)
                .and_then(|generation| self.generations.get(generation))
                .copied()
        }

        fn new_generation(&mut self, handle: u32) -> Expected {
            if self.handles.contains_key(&handle) {
                return Expected::Noop;
            }
            let generation = self.next_generation;
            self.next_generation += 1;
            self.generations.insert(generation, Phase::Open);
            self.handles.insert(handle, generation);
            Expected::Created
        }

        fn operation_phase(&self, handle: u32) -> Option<Phase> {
            self.generation_phase(handle)
        }

        fn mutation(
            &mut self,
            handle: u32,
            fail: bool,
            operation: impl FnOnce(&mut Self) -> Expected,
        ) -> (Expected, bool) {
            match self.operation_phase(handle) {
                None => (Expected::Noop, false),
                Some(Phase::Closed) => (Expected::Error(ErrorCode::IndexClosed), false),
                Some(Phase::Open) if fail => (Expected::Error(ErrorCode::Unavailable), true),
                Some(Phase::Open) => (operation(self), true),
            }
        }

        fn apply(&mut self, action: &Action) -> (Expected, bool) {
            match action {
                Action::Create { handle } | Action::Open { handle } | Action::Reopen { handle } => {
                    (self.new_generation(*handle), false)
                }
                Action::Clone { source, target } => {
                    if self.handles.contains_key(target) {
                        return (Expected::Noop, false);
                    }
                    match self.handles.get(source).copied() {
                        Some(generation) => {
                            self.handles.insert(*target, generation);
                            (Expected::Cloned, false)
                        }
                        None => (Expected::Noop, false),
                    }
                }
                Action::Insert(mutation) => {
                    self.mutation(mutation.handle, mutation.fail, |model| {
                        if model.documents.contains_key(&mutation.id) {
                            Expected::Error(ErrorCode::AlreadyExists)
                        } else {
                            model
                                .documents
                                .insert(mutation.id.clone(), mutation.document.clone());
                            Expected::Success
                        }
                    })
                }
                Action::Upsert(mutation) => {
                    self.mutation(mutation.handle, mutation.fail, |model| {
                        model
                            .documents
                            .insert(mutation.id.clone(), mutation.document.clone());
                        Expected::Success
                    })
                }
                Action::Patch(mutation) | Action::Update(mutation) => {
                    self.mutation(mutation.handle, mutation.fail, |model| {
                        let Some(document) = model.documents.get_mut(&mutation.id) else {
                            return Expected::Error(ErrorCode::NotFound);
                        };
                        match &mutation.value {
                            Some(value) => {
                                document.insert(mutation.field.clone(), value.clone());
                            }
                            None => {
                                document.remove(&mutation.field);
                            }
                        }
                        Expected::Success
                    })
                }
                Action::Delete(read) => self.mutation(read.handle, read.fail, |model| {
                    Expected::Deleted(model.documents.remove(&read.id).is_some())
                }),
                Action::Get(read) => self.mutation(read.handle, read.fail, |model| {
                    Expected::Got(model.documents.get(&read.id).cloned())
                }),
                Action::Search { handle, fail } => {
                    self.mutation(*handle, *fail, |_| Expected::Searched)
                }
                Action::Close { handle } => match self.handles.get(handle).copied() {
                    None => (Expected::Noop, false),
                    Some(generation) => {
                        self.generations.insert(generation, Phase::Closed);
                        (Expected::Success, false)
                    }
                },
                Action::DropHandle { handle } => match self.handles.remove(handle) {
                    None => (Expected::Noop, false),
                    Some(generation) => {
                        if !self.handles.values().any(|value| *value == generation) {
                            self.generations.insert(generation, Phase::Closed);
                        }
                        (Expected::Dropped, false)
                    }
                },
                Action::OperationOnClosed { handle, operation } => {
                    let expected = match self.operation_phase(*handle) {
                        None => Expected::Noop,
                        Some(Phase::Closed) => Expected::Error(ErrorCode::IndexClosed),
                        Some(Phase::Open) => Expected::Success,
                    };
                    let admitted = matches!(expected, Expected::Success);
                    (
                        match operation {
                            ClosedOperation::Delete if matches!(expected, Expected::Success) => {
                                Expected::Deleted(self.documents.remove("closed-doc").is_some())
                            }
                            ClosedOperation::Get if matches!(expected, Expected::Success) => {
                                Expected::Got(self.documents.get("closed-doc").cloned())
                            }
                            _ => expected,
                        },
                        admitted,
                    )
                }
            }
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            self.0
        }
    }

    fn document(slot: u64) -> Document {
        BTreeMap::from([
            ("title".to_owned(), json!(format!("title-{slot}"))),
            ("brand".to_owned(), json!(format!("brand-{}", slot % 3))),
            ("quantity".to_owned(), json!(slot as i64)),
            ("enabled".to_owned(), json!(slot.is_multiple_of(2))),
        ])
    }

    fn json_document(document: &Document) -> Value {
        Value::Object(
            document
                .iter()
                .map(|(name, value)| (name.clone(), value.clone()))
                .collect::<Map<_, _>>(),
        )
    }

    fn definition() -> IndexDefinition {
        IndexDefinition::new()
            .field(Field::text("title").stored(true))
            .field(Field::keyword("brand").stored(true))
            .field(Field::int64("quantity").stored(true))
            .field(Field::bool("enabled").stored(true))
    }

    fn update_result(index: &Index, mutation: &FieldMutation) -> Result<()> {
        let nested = index.inner.session.execute(|backend| {
            let operation = match &mutation.value {
                Some(value) => UpdateOperationInput::Assign {
                    target: mutation.field.clone(),
                    value: searchkernel_ir::request_value_from_json(
                        &value.clone(),
                        format!("update.{}.value", mutation.field),
                    )
                    .map_err(Error::from_ir)?,
                },
                None => UpdateOperationInput::Clear {
                    target: mutation.field.clone(),
                },
            };
            let request = searchkernel_ir::resolve_update(
                &index.inner.context,
                &mutation.id,
                &[operation],
                None,
                None,
            )
            .map_err(Error::from_ir)?;
            backend.update(request).map_err(Error::from_ir)
        });
        nested.map_err(|_| Error::index_closed())?.map(|_| ())
    }

    fn closed_operation(index: &Index, operation: ClosedOperation) -> Result<()> {
        let document = json_document(&document(99));
        match operation {
            ClosedOperation::Insert => index.insert("closed-doc", &document),
            ClosedOperation::Upsert => index.upsert("closed-doc", &document),
            ClosedOperation::Patch => {
                index.patch("closed-doc", Patch::new().set("title", "closed"))
            }
            ClosedOperation::Update => update_result(
                index,
                &FieldMutation {
                    handle: 0,
                    id: "closed-doc".to_owned(),
                    field: "quantity".to_owned(),
                    value: Some(json!(99_i64)),
                    fail: false,
                },
            ),
            ClosedOperation::Delete => index.delete("closed-doc").map(|_| ()),
            ClosedOperation::Get => index.get::<Value>("closed-doc").map(|_| ()),
            ClosedOperation::Search => index.browse().execute::<Value>().map(|_| ()),
        }
    }

    fn execute_action(
        handles: &mut BTreeMap<u32, Index>,
        action: &Action,
        backend: Arc<dyn Backend>,
    ) -> Expected {
        let error = |result: Result<()>| match result {
            Ok(()) => Expected::Success,
            Err(error) => Expected::Error(error.code()),
        };
        match action {
            Action::Create { handle } | Action::Open { handle } | Action::Reopen { handle } => {
                if handles.contains_key(handle) {
                    Expected::Noop
                } else {
                    let index = Index::from_backend(definition(), Arc::clone(&backend))
                        .expect("test definition must construct");
                    handles.insert(*handle, index);
                    Expected::Created
                }
            }
            Action::Clone { source, target } => {
                match (handles.get(source), handles.contains_key(target)) {
                    (Some(index), false) => {
                        handles.insert(*target, index.clone());
                        Expected::Cloned
                    }
                    _ => Expected::Noop,
                }
            }
            Action::Insert(mutation) => handles
                .get(&mutation.handle)
                .map(|index| error(index.insert(&mutation.id, &json_document(&mutation.document))))
                .unwrap_or(Expected::Noop),
            Action::Upsert(mutation) => handles
                .get(&mutation.handle)
                .map(|index| error(index.upsert(&mutation.id, &json_document(&mutation.document))))
                .unwrap_or(Expected::Noop),
            Action::Patch(mutation) => handles
                .get(&mutation.handle)
                .map(|index| {
                    let patch = match &mutation.value {
                        Some(value) => Patch::new().set(&mutation.field, value),
                        None => Patch::new().unset(&mutation.field),
                    };
                    error(index.patch(&mutation.id, patch))
                })
                .unwrap_or(Expected::Noop),
            Action::Update(mutation) => handles
                .get(&mutation.handle)
                .map(|index| error(update_result(index, mutation)))
                .unwrap_or(Expected::Noop),
            Action::Delete(read) => handles
                .get(&read.handle)
                .map(|index| match index.delete(&read.id) {
                    Ok(deleted) => Expected::Deleted(deleted),
                    Err(error) => Expected::Error(error.code()),
                })
                .unwrap_or(Expected::Noop),
            Action::Get(read) => handles
                .get(&read.handle)
                .map(|index| match index.get::<Value>(&read.id) {
                    Ok(Some(value)) => Expected::Got(value.as_object().map(|object| {
                        object
                            .iter()
                            .map(|(key, value)| (key.clone(), value.clone()))
                            .collect()
                    })),
                    Ok(None) => Expected::Got(None),
                    Err(error) => Expected::Error(error.code()),
                })
                .unwrap_or(Expected::Noop),
            Action::Search { handle, .. } => handles
                .get(handle)
                .map(|index| match index.browse().execute::<Value>() {
                    Ok(result) if result.total_hits.value == 0 && result.hits.is_empty() => {
                        Expected::Searched
                    }
                    Ok(_) => Expected::Error(ErrorCode::Internal),
                    Err(error) => Expected::Error(error.code()),
                })
                .unwrap_or(Expected::Noop),
            Action::Close { handle } => handles
                .get(handle)
                .map(|index| error(index.close()))
                .unwrap_or(Expected::Noop),
            Action::DropHandle { handle } => {
                if handles.remove(handle).is_some() {
                    Expected::Dropped
                } else {
                    Expected::Noop
                }
            }
            Action::OperationOnClosed { handle, operation } => handles
                .get(handle)
                .map(|index| error(closed_operation(index, *operation)))
                .unwrap_or(Expected::Noop),
        }
    }

    fn operation_ir_result(
        operation: &OperationIr,
        expected: &Expected,
    ) -> searchkernel_testkit::ScriptedOutcome {
        if let Expected::Error(code) = expected {
            return searchkernel_testkit::ScriptedOutcome::BackendError(PortableError::new(
                match code {
                    ErrorCode::AlreadyExists => searchkernel_ir::ErrorCode::AlreadyExists,
                    ErrorCode::NotFound => searchkernel_ir::ErrorCode::NotFound,
                    ErrorCode::Unavailable => searchkernel_ir::ErrorCode::Unavailable,
                    _ => searchkernel_ir::ErrorCode::Internal,
                },
                format!("model injected {code:?}"),
            ));
        }
        match operation {
            OperationIr::Insert(request) => searchkernel_testkit::ScriptedOutcome::Success(
                ScriptedResult::Insert(searchkernel_ir::InsertResult {
                    id: request.document().id().to_string(),
                }),
            ),
            OperationIr::Upsert(request) => searchkernel_testkit::ScriptedOutcome::Success(
                ScriptedResult::Upsert(searchkernel_ir::UpsertResult {
                    id: request.document().id().to_string(),
                }),
            ),
            OperationIr::Patch(request) => searchkernel_testkit::ScriptedOutcome::Success(
                ScriptedResult::Patch(searchkernel_ir::PatchResult {
                    id: request.id().to_string(),
                }),
            ),
            OperationIr::Update(request) => searchkernel_testkit::ScriptedOutcome::Success(
                ScriptedResult::Update(searchkernel_ir::UpdateResult {
                    id: request.id().to_string(),
                }),
            ),
            OperationIr::Delete(request) => searchkernel_testkit::ScriptedOutcome::Success(
                ScriptedResult::Delete(searchkernel_ir::DeleteResult {
                    id: request.id().to_string(),
                    deleted: matches!(expected, Expected::Deleted(true)),
                }),
            ),
            OperationIr::Get(request) => {
                let document = match expected {
                    Expected::Got(Some(document)) => Some(DocumentResult {
                        id: request.id().to_string(),
                        fields: document
                            .iter()
                            .map(|(name, value)| (name.clone(), json_to_value(value)))
                            .collect(),
                    }),
                    _ => None,
                };
                searchkernel_testkit::ScriptedOutcome::Success(ScriptedResult::Get(
                    searchkernel_ir::GetResult { document },
                ))
            }
            OperationIr::Search(_) => searchkernel_testkit::ScriptedOutcome::Success(
                ScriptedResult::Search(Box::new(SearchResult::empty(TotalHits {
                    value: 0,
                    relation: TotalHitsRelation::Exact,
                }))),
            ),
            OperationIr::Analyze(_) => unreachable!("model does not generate analyze actions"),
        }
    }

    fn expected_call(request: OperationIr, expected: &Expected) -> ExpectedCall {
        match operation_ir_result(&request, expected) {
            searchkernel_testkit::ScriptedOutcome::BackendError(error) => {
                ExpectedCall::error(request, error)
            }
            searchkernel_testkit::ScriptedOutcome::Success(result) => {
                ExpectedCall::success(request, result)
            }
        }
    }

    fn json_to_value(value: &Value) -> searchkernel_ir::Value {
        match value {
            Value::Null => searchkernel_ir::Value::Null,
            Value::Bool(value) => searchkernel_ir::Value::Bool(*value),
            Value::Number(value) if value.as_i64().is_some() => {
                searchkernel_ir::Value::Int64(value.as_i64().unwrap())
            }
            Value::Number(value) if value.as_u64().is_some() => {
                searchkernel_ir::Value::UInt64(value.as_u64().unwrap())
            }
            Value::Number(value) => searchkernel_ir::Value::Float64(value.as_f64().unwrap()),
            Value::String(value) => searchkernel_ir::Value::String(value.clone()),
            Value::Array(values) => {
                searchkernel_ir::Value::List(values.iter().map(json_to_value).collect())
            }
            Value::Object(values) => searchkernel_ir::Value::Object(
                values
                    .iter()
                    .map(|(key, value)| (key.clone(), json_to_value(value)))
                    .collect(),
            ),
        }
    }

    fn generate_actions(seed: u64, steps: usize) -> Vec<Action> {
        let mut actions = vec![
            Action::Create { handle: 0 },
            Action::Clone {
                source: 0,
                target: 1,
            },
            Action::DropHandle { handle: 1 },
            Action::Clone {
                source: 0,
                target: 1,
            },
            Action::Insert(Mutation {
                handle: 0,
                id: "a".to_owned(),
                document: document(1),
                fail: true,
            }),
            Action::Insert(Mutation {
                handle: 1,
                id: "a".to_owned(),
                document: document(1),
                fail: false,
            }),
            Action::Insert(Mutation {
                handle: 0,
                id: "a".to_owned(),
                document: document(2),
                fail: false,
            }),
            Action::Upsert(Mutation {
                handle: 1,
                id: "a".to_owned(),
                document: document(3),
                fail: true,
            }),
            Action::Upsert(Mutation {
                handle: 1,
                id: "a".to_owned(),
                document: document(4),
                fail: false,
            }),
            Action::Patch(FieldMutation {
                handle: 0,
                id: "a".to_owned(),
                field: "title".to_owned(),
                value: Some(json!("patched")),
                fail: true,
            }),
            Action::Patch(FieldMutation {
                handle: 0,
                id: "a".to_owned(),
                field: "brand".to_owned(),
                value: None,
                fail: false,
            }),
            Action::Update(FieldMutation {
                handle: 1,
                id: "a".to_owned(),
                field: "quantity".to_owned(),
                value: Some(json!(17_i64)),
                fail: true,
            }),
            Action::Update(FieldMutation {
                handle: 1,
                id: "a".to_owned(),
                field: "quantity".to_owned(),
                value: Some(json!(19_i64)),
                fail: false,
            }),
            Action::Delete(Read {
                handle: 0,
                id: "a".to_owned(),
                fail: true,
            }),
            Action::Get(Read {
                handle: 0,
                id: "a".to_owned(),
                fail: true,
            }),
            Action::Search {
                handle: 1,
                fail: true,
            },
            Action::Get(Read {
                handle: 0,
                id: "a".to_owned(),
                fail: false,
            }),
            Action::Search {
                handle: 1,
                fail: false,
            },
            Action::Delete(Read {
                handle: 0,
                id: "a".to_owned(),
                fail: false,
            }),
            Action::Delete(Read {
                handle: 0,
                id: "a".to_owned(),
                fail: false,
            }),
            Action::Close { handle: 0 },
            Action::OperationOnClosed {
                handle: 1,
                operation: ClosedOperation::Search,
            },
            Action::Open { handle: 2 },
            Action::OperationOnClosed {
                handle: 1,
                operation: ClosedOperation::Get,
            },
            Action::Close { handle: 2 },
            Action::Reopen { handle: 3 },
            Action::Clone {
                source: 3,
                target: 4,
            },
        ];
        let mut rng = Rng(seed);
        for step in actions.len()..steps {
            let value = rng.next();
            let handle = ((value >> 8) % 5) as u32;
            let id = format!("doc-{}", (value >> 16) % 4);
            let mutation = || Mutation {
                handle,
                id: id.clone(),
                document: document(value % 20),
                fail: value & 1 == 0,
            };
            let field_mutation = || FieldMutation {
                handle,
                id: id.clone(),
                field: match (value >> 24) % 4 {
                    0 => "title",
                    1 => "brand",
                    2 => "quantity",
                    _ => "enabled",
                }
                .to_owned(),
                value: if value & 2 == 0 {
                    Some(match (value >> 24) % 4 {
                        0 => json!(format!("title-{}", value % 25)),
                        1 => json!(format!("brand-{}", value % 3)),
                        2 => json!((value % 25) as i64),
                        _ => json!(value.is_multiple_of(2)),
                    })
                } else {
                    None
                },
                fail: value & 4 == 0,
            };
            actions.push(match value % 14 {
                0 => Action::Clone {
                    source: handle,
                    target: 1000 + step as u32,
                },
                1 => Action::Insert(mutation()),
                2 => Action::Upsert(mutation()),
                3 => Action::Patch(field_mutation()),
                4 => Action::Update(field_mutation()),
                5 => Action::Delete(Read {
                    handle,
                    id,
                    fail: value & 1 == 0,
                }),
                6 => Action::Get(Read {
                    handle,
                    id,
                    fail: value & 1 == 0,
                }),
                7 => Action::Search {
                    handle,
                    fail: value & 1 == 0,
                },
                8 => Action::Close { handle },
                9 => Action::DropHandle {
                    handle: if value & 0xff == 1 { 0 } else { handle },
                },
                10 => Action::Reopen {
                    handle: 2000 + step as u32,
                },
                11 => Action::OperationOnClosed {
                    handle: 1,
                    operation: match (value >> 32) % 7 {
                        0 => ClosedOperation::Insert,
                        1 => ClosedOperation::Upsert,
                        2 => ClosedOperation::Patch,
                        3 => ClosedOperation::Update,
                        4 => ClosedOperation::Delete,
                        5 => ClosedOperation::Get,
                        _ => ClosedOperation::Search,
                    },
                },
                12 => Action::Create {
                    handle: 3000 + step as u32,
                },
                _ => Action::Open {
                    handle: 4000 + step as u32,
                },
            });
        }
        actions
    }

    fn plan(actions: &[Action]) -> Vec<PlannedAction> {
        let mut model = Model::default();
        actions
            .iter()
            .map(|action| {
                let before = format!("{model:?}");
                let (expected, admitted) = model.apply(action);
                PlannedAction {
                    action: action.clone(),
                    before,
                    after: model.clone(),
                    expected,
                    admitted,
                }
            })
            .collect()
    }

    fn assert_lifecycle(handles: &BTreeMap<u32, Index>, model: &Model, context: &str) {
        for (handle, index) in handles {
            match model.generation_phase(*handle) {
                Some(Phase::Open) => assert!(
                    index.describe().is_ok(),
                    "{context}: handle {handle} should be open"
                ),
                Some(Phase::Closed) => assert_eq!(
                    index
                        .describe()
                        .expect_err("closed handle accepted describe")
                        .code(),
                    ErrorCode::IndexClosed,
                    "{context}: handle {handle}"
                ),
                None => panic!("{context}: actual handle {handle} is absent from model"),
            }
        }
    }

    fn run_seed(seed: u64, steps: usize) {
        let actions = generate_actions(seed, steps.max(28));
        let planned = plan(&actions);

        let recording = Arc::new(RecordingBackend::new());
        let recording_backend: Arc<dyn Backend> = recording.clone();
        let mut recording_handles = BTreeMap::new();
        for planned_action in &planned {
            let _ = execute_action(
                &mut recording_handles,
                &planned_action.action,
                Arc::clone(&recording_backend),
            );
        }
        let recorded_calls = recording.calls();
        let admitted = planned.iter().filter(|action| action.admitted).count();
        assert_eq!(
            recorded_calls.len(),
            admitted,
            "seed={seed} generator={GENERATOR_VERSION}: recording admission mismatch"
        );

        let script = planned
            .iter()
            .filter(|action| action.admitted)
            .zip(recorded_calls)
            .map(|(planned_action, request)| expected_call(request, &planned_action.expected))
            .collect::<Vec<_>>();
        let scripted = Arc::new(ScriptedBackend::new(script));
        let scripted_backend: Arc<dyn Backend> = scripted.clone();
        let mut handles = BTreeMap::new();
        let mut history = Vec::new();
        for (step, planned_action) in planned.iter().enumerate() {
            let actual = execute_action(
                &mut handles,
                &planned_action.action,
                Arc::clone(&scripted_backend),
            );
            if actual != planned_action.expected {
                let recent = history.iter().rev().take(8).collect::<Vec<_>>();
                panic!(
                    "state-machine mismatch; seed={seed}; steps={steps}; generator={GENERATOR_VERSION}; step={step}; action={:?}; model state before={}; expected={:?}; actual={:?}; recent action history={recent:?}",
                    planned_action.action, planned_action.before, planned_action.expected, actual
                );
            }
            assert_lifecycle(
                &handles,
                &planned_action.after,
                &format!("seed={seed} step={step} action={:?}", planned_action.action),
            );
            history.push(planned_action.action.clone());
        }
        scripted
            .finish()
            .expect("every admitted action must consume one scripted call");
        drop(handles);
        drop(recording_handles);
    }

    fn configured_seeds() -> Vec<u64> {
        std::env::var("SEARCHKERNEL_MODEL_SEEDS")
            .ok()
            .map(|value| {
                value
                    .split(',')
                    .map(|seed| {
                        u64::from_str_radix(seed.trim().trim_start_matches("0x"), 16)
                            .expect("valid model seed")
                    })
                    .collect()
            })
            .unwrap_or_else(|| DEFAULT_SEEDS.to_vec())
    }

    #[test]
    fn deterministic_index_model_state_machine() {
        let steps = std::env::var("SEARCHKERNEL_MODEL_STEPS")
            .ok()
            .map(|value| value.parse().expect("valid model step count"))
            .unwrap_or(DEFAULT_STEPS);
        for seed in configured_seeds() {
            run_seed(seed, steps);
        }
    }

    #[test]
    fn recording_backend_rejects_closed_operations_without_recording_them() {
        let backend = Arc::new(RecordingBackend::new());
        let backend_trait: Arc<dyn Backend> = backend.clone();
        let index = Index::from_backend(definition(), backend_trait).expect("test index");
        let document = json_document(&document(1));
        assert_eq!(
            index
                .insert("recorded", &document)
                .map_err(|error| error.code()),
            Err(ErrorCode::Unsupported)
        );
        assert_eq!(backend.calls().len(), 1);
        index.close().expect("close recording index");
        assert_eq!(
            index
                .upsert("closed", &document)
                .map_err(|error| error.code()),
            Err(ErrorCode::IndexClosed)
        );
        assert_eq!(
            backend.calls().len(),
            1,
            "closed operations must not reach RecordingBackend"
        );
    }
}

#[cfg(feature = "native")]
mod native_model {
    use crate::{ErrorCode, Field, Index, IndexDefinition};
    use serde_json::json;

    fn definition() -> IndexDefinition {
        IndexDefinition::new()
            .field(Field::text("title").stored(true))
            .field(Field::keyword("brand").stored(true))
            .field(Field::int64("quantity").stored(true))
            .field(Field::bool("enabled").stored(true))
    }

    fn temporary_path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "searchkernel-model-reopen-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ))
    }

    #[test]
    fn native_model_persists_state_and_keeps_old_generation_closed() {
        let path = temporary_path();
        let definition = definition();
        let generation_one = Index::create(&path, definition).expect("create index");
        let old_handle = generation_one.index();
        generation_one
            .insert(
                "a",
                &json!({"title": "A", "brand": "Acme", "quantity": 1, "enabled": true}),
            )
            .expect("insert A");
        generation_one
            .insert(
                "b",
                &json!({"title": "B", "brand": "Bravo", "quantity": 2, "enabled": true}),
            )
            .expect("insert B");
        generation_one.close().expect("close generation one");

        assert_eq!(
            old_handle
                .get::<serde_json::Value>("a")
                .expect_err("old handle must stay closed")
                .code(),
            ErrorCode::IndexClosed
        );

        let generation_two = Index::open(&path).expect("open generation two");
        assert_eq!(
            generation_two
                .get::<serde_json::Value>("a")
                .expect("get A")
                .expect("A exists")["title"],
            json!("A")
        );
        generation_two
            .upsert(
                "b",
                &json!({"title": "B updated", "brand": "Bravo", "quantity": 20, "enabled": false}),
            )
            .expect("upsert B");
        assert!(generation_two.delete("a").expect("delete A"));
        generation_two.close().expect("close generation two");

        let generation_three = Index::open(&path).expect("open generation three");
        assert!(generation_three
            .get::<serde_json::Value>("a")
            .expect("get deleted A")
            .is_none());
        assert_eq!(
            generation_three
                .get::<serde_json::Value>("b")
                .expect("get updated B")
                .expect("B exists")["quantity"],
            json!(20)
        );
        generation_three.close().expect("close generation three");
        std::fs::remove_dir_all(path).expect("remove native model index");
    }
}
