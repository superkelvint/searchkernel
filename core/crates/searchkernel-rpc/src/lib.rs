//! Shared generated gRPC framing for Search Kernel.
//!
//! Search Kernel domain semantics do not live here. The generated service
//! transports opaque canonical FlatBuffer envelopes between clients and
//! `searchkerneld`.

/// Explicit transport bound for canonical FlatBuffer request and response
/// frames. Tonic's default decode limit is 4 MiB, which is too small for
/// legitimate documents and tensor payloads.
pub const MAX_GRPC_MESSAGE_SIZE: usize = 256 * 1024 * 1024;

/// Rust bindings generated from the public Search Kernel gRPC protocol.
#[allow(clippy::result_large_err)]
pub mod rpc {
    include!("generated/rpc.rs");
}
