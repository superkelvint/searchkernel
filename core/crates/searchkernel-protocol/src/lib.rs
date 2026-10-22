//! Generated Rust representation of the portable Search Kernel FlatBuffers contract.

#[allow(warnings)]
pub(crate) mod generated {
    include!("generated/mod.rs");
}

/// Generic standard FlatBuffers JSON conversion at the protocol boundary.
#[cfg(feature = "json")]
pub mod json;

/// Current version-neutral Search Kernel payload schema, selected by the V1 codec.
pub mod wire {
    pub mod v1 {
        pub use crate::generated::search_kernel::wire::*;
    }
}
pub mod protocol;

/// Generated-package-compatible spelling for consumers that mirror the
/// protobuf package hierarchy.
pub mod searchkernel {
    pub use super::protocol;
    pub use super::wire;
}
