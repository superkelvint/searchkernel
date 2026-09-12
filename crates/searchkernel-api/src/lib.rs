//! Generated Rust representation of the portable Search Kernel C1 contract.

/// Versioned C1 messages and enums.
pub mod v1 {
    include!(concat!(env!("OUT_DIR"), "/searchkernel.v1.rs"));
}

/// Generated-package-compatible spelling for consumers that mirror the
/// protobuf package hierarchy.
pub mod searchkernel {
    pub use super::v1;
}
