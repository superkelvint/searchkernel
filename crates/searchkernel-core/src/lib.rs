//! Backend-independent Search Kernel semantics.

pub mod error;
pub mod model;
pub mod resolve;

pub use error::{ErrorCode, ErrorDetail, PortableError, Result};
pub use model::*;
pub use resolve::{resolve_index_definition, Backend, IndexSemantics};

/// The generated C1 API used by the semantic resolver.
pub use searchkernel_api::v1 as api;
