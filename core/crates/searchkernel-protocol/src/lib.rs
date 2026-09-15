//! Generated Rust representation of the portable Search Kernel FlatBuffers contract.

#[allow(warnings)]
mod generated {
    include!("generated/mod.rs");
}

/// Versioned Search Kernel messages and enums.
pub use generated::search_kernel::v1;

/// Generated-package-compatible spelling for consumers that mirror the
/// protobuf package hierarchy.
pub mod searchkernel {
    pub use super::v1;
}
