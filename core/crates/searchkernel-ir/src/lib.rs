//! Backend-independent Search Kernel semantics.

#[cfg(test)]
extern crate self as searchkernel_ir;

#[cfg(test)]
#[path = "../tests/semantics.rs"]
mod semantics;

pub mod api_to_ir;
pub mod backend;
pub mod error;
pub mod filter_expression;
pub mod ir_to_api;
pub mod model;
pub mod session;

pub use api_to_ir::{
    analyzer_to_ir, field_to_ir, index_definition_to_ir, IndexContext, WriteRequestRef,
};
pub use backend::Backend;
pub use error::{ErrorCode, ErrorDetail, PortableError, Result};
pub use filter_expression::{
    bind_filter_expression, bind_filter_expression_with_references, BoundFilter, FilterParameters,
};
pub use ir_to_api::*;
pub use model::*;
pub use session::{BackendSession, SessionError};

/// The generated FlatBuffers API used at the portable API boundary.
pub use searchkernel_protocol::v2 as api;

/// Test and contract clients use the same FlatBuffers parser as this crate.
#[doc(hidden)]
pub use flatbuffers as __flatbuffers;
