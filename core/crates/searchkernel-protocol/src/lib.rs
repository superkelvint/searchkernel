//! Generated Rust representation of the portable Search Kernel FlatBuffers contract.

#[allow(warnings)]
mod generated {
    include!("generated/mod.rs");
}

/// The negotiated version of the canonical portable V2 contract.
pub const PROTOCOL_VERSION: u32 = 2;

/// Generic standard FlatBuffers JSON conversion at the protocol boundary.
pub mod json;

/// Versioned Search Kernel messages and enums.
pub use generated::search_kernel::v2;
pub mod protocol;

/// Generated-package-compatible spelling for consumers that mirror the
/// protobuf package hierarchy.
pub mod searchkernel {
    pub use super::protocol;
    pub use super::v2;
}
