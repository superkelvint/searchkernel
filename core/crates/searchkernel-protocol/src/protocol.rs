//! Transport-neutral V2 request and response envelopes.

pub mod v2 {
    pub use crate::v2::*;

    #[allow(warnings)]
    mod request_payload_generated {
        use super::*;
        include!("generated/search_kernel/protocol/v2/request_payload_generated.rs");
    }
    pub use request_payload_generated::*;
    #[allow(warnings)]
    mod request_envelope_generated {
        use super::*;
        include!("generated/search_kernel/protocol/v2/request_envelope_generated.rs");
    }
    pub use request_envelope_generated::*;
    #[allow(warnings)]
    mod response_payload_generated {
        use super::*;
        include!("generated/search_kernel/protocol/v2/response_payload_generated.rs");
    }
    pub use response_payload_generated::*;
    #[allow(warnings)]
    mod response_envelope_generated {
        use super::*;
        include!("generated/search_kernel/protocol/v2/response_envelope_generated.rs");
    }
    pub use response_envelope_generated::*;
}
