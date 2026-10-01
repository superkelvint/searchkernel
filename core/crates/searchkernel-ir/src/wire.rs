//! Versioned wire adapters around the wire-agnostic Searchkernel IR.
//!
//! Generated FlatBuffers types are intentionally reachable only below a wire
//! major. Core model, backend, and resolver APIs remain version-free.

pub mod v1 {
    pub use searchkernel_protocol::wire::v1 as schema;

    pub use crate::api_to_ir::control_plane::{
        resolve_control_plane_request, resolve_index_target, ControlPlaneRequest,
        ControlPlaneResolutionError,
    };
    pub use crate::api_to_ir::{
        analyze_result_from_api, analyzer_to_ir, delete_result_from_api, field_to_ir,
        get_result_from_api, index_definition_to_ir, insert_result_from_api, patch_result_from_api,
        portable_error_from_api, search_result_from_api, update_result_from_api,
        upsert_result_from_api, WriteRequestRef,
    };
    pub use crate::ir_to_api::*;
}
