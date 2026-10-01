//! Transport-neutral wire protocol codecs and envelopes.

pub mod v1 {
    /// Current envelope major selected by the V1 wire codec.
    pub const PROTOCOL_VERSION: u32 = 1;

    pub use crate::wire::v1::*;

    pub mod wire {
        pub use crate::wire::v1::*;
    }

    #[allow(warnings)]
    mod request_payload_generated {
        use super::*;
        include!("generated/search_kernel/protocol/request_payload_generated.rs");
    }
    pub use request_payload_generated::*;
    #[allow(warnings)]
    mod request_envelope_generated {
        use super::*;
        include!("generated/search_kernel/protocol/request_envelope_generated.rs");
    }
    pub use request_envelope_generated::*;
    #[allow(warnings)]
    mod response_payload_generated {
        use super::*;
        include!("generated/search_kernel/protocol/response_payload_generated.rs");
    }
    pub use response_payload_generated::*;
    #[allow(warnings)]
    mod response_envelope_generated {
        use super::*;
        include!("generated/search_kernel/protocol/response_envelope_generated.rs");
    }
    pub use response_envelope_generated::*;
}
