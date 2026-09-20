use crate::{OperationKind, ScriptedCallError, TestkitError};
use searchkernel_ir::{Backend, OperationIr, PortableError};
use std::sync::Mutex;

/// An owned result that can be returned by exactly one backend operation.
#[derive(Clone, Debug, PartialEq)]
pub enum ScriptedResult {
    Insert(searchkernel_ir::InsertResult),
    Upsert(searchkernel_ir::UpsertResult),
    Patch(searchkernel_ir::PatchResult),
    Update(searchkernel_ir::UpdateResult),
    Delete(searchkernel_ir::DeleteResult),
    Get(searchkernel_ir::GetResult),
    Search(Box<searchkernel_ir::SearchResult>),
    Analyze(searchkernel_ir::AnalyzeResult),
}

/// A predefined success or portable backend failure.
#[derive(Clone, Debug, PartialEq)]
pub enum ScriptedOutcome {
    Success(ScriptedResult),
    BackendError(PortableError),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExpectedCall {
    pub request: OperationIr,
    pub outcome: ScriptedOutcome,
}

impl ExpectedCall {
    pub fn new(request: OperationIr, outcome: ScriptedOutcome) -> Self {
        Self { request, outcome }
    }

    pub fn success(request: OperationIr, result: ScriptedResult) -> Self {
        Self::new(request, ScriptedOutcome::Success(result))
    }

    pub fn error(request: OperationIr, error: PortableError) -> Self {
        Self::new(request, ScriptedOutcome::BackendError(error))
    }
}

#[derive(Debug)]
struct State {
    script: Vec<ExpectedCall>,
    position: usize,
}

/// An ordered, exact-request replay backend.
#[derive(Debug)]
pub struct ScriptedBackend {
    state: Mutex<State>,
}

impl ScriptedBackend {
    pub fn new(script: impl Into<Vec<ExpectedCall>>) -> Self {
        Self {
            state: Mutex::new(State {
                script: script.into(),
                position: 0,
            }),
        }
    }

    pub fn empty() -> Self {
        Self::new(Vec::new())
    }

    pub fn remaining(&self) -> usize {
        let state = self.state.lock().expect("script mutex poisoned");
        state.script.len().saturating_sub(state.position)
    }

    /// Fails if the ordered script still contains entries.
    pub fn finish(&self) -> std::result::Result<(), TestkitError> {
        let state = self
            .state
            .lock()
            .map_err(|_| TestkitError::Synchronization)?;
        let remaining = state.script.len().saturating_sub(state.position);
        if remaining == 0 {
            Ok(())
        } else {
            Err(TestkitError::UnusedScriptedCalls {
                position: state.position,
                remaining,
            })
        }
    }

    pub fn assert_exhausted(&self) -> std::result::Result<(), TestkitError> {
        self.finish()
    }

    fn next(&self, actual: OperationIr) -> std::result::Result<ScriptedOutcome, TestkitError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| TestkitError::Synchronization)?;
        let position = state.position;
        let Some(expected) = state.script.get(position) else {
            return Err(TestkitError::ScriptExhausted {
                position,
                actual: Box::new(actual),
            });
        };
        let expected_kind = OperationKind::of(&expected.request);
        let actual_kind = OperationKind::of(&actual);
        if expected_kind != actual_kind {
            return Err(TestkitError::UnexpectedCall {
                position,
                expected: expected_kind,
                actual: actual_kind,
                expected_operation: Box::new(expected.request.clone()),
                actual_operation: Box::new(actual),
            });
        }
        if expected.request != actual {
            return Err(TestkitError::ExpectedDifferentRequest {
                position,
                expected: Box::new(expected.request.clone()),
                actual: Box::new(actual),
            });
        }
        let outcome = expected.outcome.clone();
        state.position += 1;
        Ok(outcome)
    }

    pub fn call_insert(
        &self,
        request: searchkernel_ir::InsertRequestIr,
    ) -> std::result::Result<searchkernel_ir::InsertResult, ScriptedCallError> {
        self.call_typed(OperationIr::Insert(request), OperationKind::Insert)
    }

    pub fn call_upsert(
        &self,
        request: searchkernel_ir::UpsertRequestIr,
    ) -> std::result::Result<searchkernel_ir::UpsertResult, ScriptedCallError> {
        self.call_typed(OperationIr::Upsert(request), OperationKind::Upsert)
    }
    pub fn call_patch(
        &self,
        request: searchkernel_ir::PatchRequestIr,
    ) -> std::result::Result<searchkernel_ir::PatchResult, ScriptedCallError> {
        self.call_typed(OperationIr::Patch(request), OperationKind::Patch)
    }
    pub fn call_update(
        &self,
        request: searchkernel_ir::UpdateRequestIr,
    ) -> std::result::Result<searchkernel_ir::UpdateResult, ScriptedCallError> {
        self.call_typed(OperationIr::Update(request), OperationKind::Update)
    }
    pub fn call_delete(
        &self,
        request: searchkernel_ir::DeleteRequestIr,
    ) -> std::result::Result<searchkernel_ir::DeleteResult, ScriptedCallError> {
        self.call_typed(OperationIr::Delete(request), OperationKind::Delete)
    }
    pub fn call_get(
        &self,
        request: searchkernel_ir::GetRequestIr,
    ) -> std::result::Result<searchkernel_ir::GetResult, ScriptedCallError> {
        self.call_typed(OperationIr::Get(request), OperationKind::Get)
    }
    pub fn call_search(
        &self,
        request: searchkernel_ir::SearchRequestIr,
    ) -> std::result::Result<searchkernel_ir::SearchResult, ScriptedCallError> {
        self.call_typed(OperationIr::Search(request), OperationKind::Search)
    }
    pub fn call_analyze(
        &self,
        request: searchkernel_ir::AnalyzeRequestIr,
    ) -> std::result::Result<searchkernel_ir::AnalyzeResult, ScriptedCallError> {
        self.call_typed(OperationIr::Analyze(request), OperationKind::Analyze)
    }

    fn call_typed<T>(
        &self,
        operation: OperationIr,
        kind: OperationKind,
    ) -> std::result::Result<T, ScriptedCallError>
    where
        T: FromScriptedResult,
    {
        match self
            .next(operation)
            .map_err(|error| ScriptedCallError::Misuse(Box::new(error)))?
        {
            ScriptedOutcome::BackendError(error) => Err(ScriptedCallError::Backend(error)),
            ScriptedOutcome::Success(result) => T::from_result(result, kind)
                .map_err(|error| ScriptedCallError::Misuse(Box::new(error))),
        }
    }
}

trait FromScriptedResult: Sized {
    fn from_result(
        result: ScriptedResult,
        kind: OperationKind,
    ) -> std::result::Result<Self, TestkitError>;
}

impl FromScriptedResult for searchkernel_ir::InsertResult {
    fn from_result(
        result: ScriptedResult,
        operation: OperationKind,
    ) -> std::result::Result<Self, TestkitError> {
        match result {
            ScriptedResult::Insert(value) => Ok(value),
            _ => Err(TestkitError::InvalidOutcome {
                operation,
                outcome: "insert",
            }),
        }
    }
}

impl FromScriptedResult for searchkernel_ir::UpsertResult {
    fn from_result(
        result: ScriptedResult,
        operation: OperationKind,
    ) -> std::result::Result<Self, TestkitError> {
        match result {
            ScriptedResult::Upsert(value) => Ok(value),
            _ => Err(TestkitError::InvalidOutcome {
                operation,
                outcome: "upsert",
            }),
        }
    }
}

impl FromScriptedResult for searchkernel_ir::PatchResult {
    fn from_result(
        result: ScriptedResult,
        operation: OperationKind,
    ) -> std::result::Result<Self, TestkitError> {
        match result {
            ScriptedResult::Patch(value) => Ok(value),
            _ => Err(TestkitError::InvalidOutcome {
                operation,
                outcome: "patch",
            }),
        }
    }
}

impl FromScriptedResult for searchkernel_ir::UpdateResult {
    fn from_result(
        result: ScriptedResult,
        operation: OperationKind,
    ) -> std::result::Result<Self, TestkitError> {
        match result {
            ScriptedResult::Update(value) => Ok(value),
            _ => Err(TestkitError::InvalidOutcome {
                operation,
                outcome: "update",
            }),
        }
    }
}

impl FromScriptedResult for searchkernel_ir::DeleteResult {
    fn from_result(
        result: ScriptedResult,
        operation: OperationKind,
    ) -> std::result::Result<Self, TestkitError> {
        match result {
            ScriptedResult::Delete(value) => Ok(value),
            _ => Err(TestkitError::InvalidOutcome {
                operation,
                outcome: "delete",
            }),
        }
    }
}

impl FromScriptedResult for searchkernel_ir::GetResult {
    fn from_result(
        result: ScriptedResult,
        operation: OperationKind,
    ) -> std::result::Result<Self, TestkitError> {
        match result {
            ScriptedResult::Get(value) => Ok(value),
            _ => Err(TestkitError::InvalidOutcome {
                operation,
                outcome: "get",
            }),
        }
    }
}

impl FromScriptedResult for searchkernel_ir::SearchResult {
    fn from_result(
        result: ScriptedResult,
        operation: OperationKind,
    ) -> std::result::Result<Self, TestkitError> {
        match result {
            ScriptedResult::Search(value) => Ok(*value),
            _ => Err(TestkitError::InvalidOutcome {
                operation,
                outcome: "search",
            }),
        }
    }
}

impl FromScriptedResult for searchkernel_ir::AnalyzeResult {
    fn from_result(
        result: ScriptedResult,
        operation: OperationKind,
    ) -> std::result::Result<Self, TestkitError> {
        match result {
            ScriptedResult::Analyze(value) => Ok(value),
            _ => Err(TestkitError::InvalidOutcome {
                operation,
                outcome: "analyze",
            }),
        }
    }
}

fn portable_misuse(error: TestkitError) -> PortableError {
    PortableError::new(searchkernel_ir::ErrorCode::Internal, error.to_string()).with_detail(
        "testkit_misuse",
        None::<String>,
        error.to_string(),
    )
}

impl Backend for ScriptedBackend {
    fn insert(
        &self,
        request: searchkernel_ir::InsertRequestIr,
    ) -> searchkernel_ir::Result<searchkernel_ir::InsertResult> {
        match self
            .next(OperationIr::Insert(request))
            .map_err(portable_misuse)?
        {
            ScriptedOutcome::Success(ScriptedResult::Insert(result)) => Ok(result),
            ScriptedOutcome::BackendError(error) => Err(error),
            ScriptedOutcome::Success(_) => Err(portable_misuse(TestkitError::InvalidOutcome {
                operation: OperationKind::Insert,
                outcome: "success",
            })),
        }
    }
    fn upsert(
        &self,
        request: searchkernel_ir::UpsertRequestIr,
    ) -> searchkernel_ir::Result<searchkernel_ir::UpsertResult> {
        self.trait_call(OperationIr::Upsert(request), OperationKind::Upsert)
    }
    fn patch(
        &self,
        request: searchkernel_ir::PatchRequestIr,
    ) -> searchkernel_ir::Result<searchkernel_ir::PatchResult> {
        self.trait_call(OperationIr::Patch(request), OperationKind::Patch)
    }
    fn update(
        &self,
        request: searchkernel_ir::UpdateRequestIr,
    ) -> searchkernel_ir::Result<searchkernel_ir::UpdateResult> {
        self.trait_call(OperationIr::Update(request), OperationKind::Update)
    }
    fn delete(
        &self,
        request: searchkernel_ir::DeleteRequestIr,
    ) -> searchkernel_ir::Result<searchkernel_ir::DeleteResult> {
        self.trait_call(OperationIr::Delete(request), OperationKind::Delete)
    }
    fn get(
        &self,
        request: searchkernel_ir::GetRequestIr,
    ) -> searchkernel_ir::Result<searchkernel_ir::GetResult> {
        self.trait_call(OperationIr::Get(request), OperationKind::Get)
    }
    fn search(
        &self,
        request: searchkernel_ir::SearchRequestIr,
    ) -> searchkernel_ir::Result<searchkernel_ir::SearchResult> {
        self.trait_call(OperationIr::Search(request), OperationKind::Search)
    }
    fn analyze(
        &self,
        request: searchkernel_ir::AnalyzeRequestIr,
    ) -> searchkernel_ir::Result<searchkernel_ir::AnalyzeResult> {
        self.trait_call(OperationIr::Analyze(request), OperationKind::Analyze)
    }
}

impl ScriptedBackend {
    fn trait_call<T: FromScriptedResult>(
        &self,
        operation: OperationIr,
        kind: OperationKind,
    ) -> searchkernel_ir::Result<T> {
        match self.next(operation).map_err(portable_misuse)? {
            ScriptedOutcome::BackendError(error) => Err(error),
            ScriptedOutcome::Success(result) => {
                T::from_result(result, kind).map_err(portable_misuse)
            }
        }
    }
}
