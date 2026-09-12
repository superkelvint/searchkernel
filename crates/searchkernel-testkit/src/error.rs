use searchkernel_core::OperationIr;
use std::fmt;

/// The operation families in the real backend contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationKind {
    Insert,
    Upsert,
    Patch,
    Delete,
    Get,
    Search,
    Analyze,
}

impl OperationKind {
    pub(crate) fn of(operation: &OperationIr) -> Self {
        match operation {
            OperationIr::Insert(_) => Self::Insert,
            OperationIr::Upsert(_) => Self::Upsert,
            OperationIr::Patch(_) => Self::Patch,
            OperationIr::Delete(_) => Self::Delete,
            OperationIr::Get(_) => Self::Get,
            OperationIr::Search(_) => Self::Search,
            OperationIr::Analyze(_) => Self::Analyze,
        }
    }
}

/// Misuse of a scripted or recording test backend.
#[derive(Clone, Debug, PartialEq)]
pub enum TestkitError {
    /// The call selected a different operation family than the next script entry.
    UnexpectedCall {
        position: usize,
        expected: OperationKind,
        actual: OperationKind,
        expected_operation: Box<OperationIr>,
        actual_operation: Box<OperationIr>,
    },
    /// The operation family was correct, but its typed IR request was not.
    ExpectedDifferentRequest {
        position: usize,
        expected: Box<OperationIr>,
        actual: Box<OperationIr>,
    },
    /// A call was made after all scripted entries had been consumed.
    ScriptExhausted {
        position: usize,
        actual: Box<OperationIr>,
    },
    /// The script was not fully consumed when verification finished.
    UnusedScriptedCalls { position: usize, remaining: usize },
    /// An expected outcome belongs to a different backend operation family.
    InvalidOutcome {
        operation: OperationKind,
        outcome: &'static str,
    },
    /// Another thread poisoned the testkit's small internal mutex.
    Synchronization,
}

/// Error returned by the checked `ScriptedBackend::call_*` helpers.
#[derive(Clone, Debug, PartialEq)]
pub enum ScriptedCallError {
    Misuse(Box<TestkitError>),
    Backend(searchkernel_core::PortableError),
}

impl fmt::Display for ScriptedCallError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Misuse(error) => error.fmt(formatter),
            Self::Backend(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ScriptedCallError {}

impl fmt::Display for TestkitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedCall {
                position,
                expected,
                actual,
                expected_operation,
                actual_operation,
            } => write!(
                formatter,
                "unexpected operation at script position {position}: expected {expected:?} ({expected_operation:?}), got {actual:?} ({actual_operation:?})"
            ),
            Self::ExpectedDifferentRequest {
                position,
                expected,
                actual,
            } => write!(
                formatter,
                "different request at script position {position}: expected {expected:?}, got {actual:?}"
            ),
            Self::ScriptExhausted { position, actual } => write!(
                formatter,
                "script exhausted at position {position}; unexpected call was {actual:?}"
            ),
            Self::UnusedScriptedCalls { position, remaining } => write!(
                formatter,
                "{remaining} scripted call(s) unused after position {position}"
            ),
            Self::InvalidOutcome { operation, outcome } => {
                write!(formatter, "{outcome} outcome cannot be returned for {operation:?}")
            }
            Self::Synchronization => formatter.write_str("testkit synchronization failed"),
        }
    }
}

impl std::error::Error for TestkitError {}
