use super::*;
use crate::schema::{ConstantSource, DataType, PrimitiveDataType};
use crate::{
    field, Analyzer, Filter, GroupingPlan, Query, QueryAlternative, Ranking, SearchBuilder, Sort,
    Tensor, WandOptions,
};
use searchkernel_ir::{BackendSession, IndexContext, OperationIr};
use searchkernel_protocol::wire::v1 as api;
use searchkernel_testkit::{ExpectedCall, RecordingBackend, ScriptedBackend, ScriptedResult};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;

mod fixtures;
mod support;
use fixtures::*;
use support::*;
mod backend;
mod document;
mod grouping;
#[cfg(feature = "native")]
mod lifecycle;
mod schema_base;
mod schema_collections;
mod schema_editing;
mod schema_fields;
mod schema_ranking;
mod schema_validation;
mod search;
