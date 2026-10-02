//! Owned Search Kernel IR to portable FlatBuffers encoding.

use crate::error::{ErrorCode, PortableError};
use crate::model as ir;
use searchkernel_protocol::wire::v1 as api;

mod error;
mod filter;
mod grouping;
mod query;
mod ranking;
mod result;
mod schema;
mod search;
mod transport;
mod value;

#[cfg(test)]
mod tests;

use filter::{
    comparison_op_to_api, filter_offset, resolved_sort_offset, sort_offset, value_object_offset,
};
use grouping::{grouping_offsets, resolved_grouping_offsets};
use query::{
    highlighting_offset, projection_offset, query_source_offset, search_field_selection_offset,
};
use ranking::ranking_request_offset;
use schema::{string_entry_offsets, string_vector_offset};
use value::{request_tensor_offset, request_value_entries_offset, request_value_offset};

pub use error::{error_offset, error_to_api};
pub use result::{
    analyze_result_offset, analyze_result_to_api, delete_result_offset, delete_result_to_api,
    get_result_offset, get_result_to_api, insert_result_offset, insert_result_to_api,
    patch_result_offset, patch_result_to_api, search_result_offset, search_result_to_api,
    update_result_offset, update_result_to_api, upsert_result_offset, upsert_result_to_api,
    validate_search_result,
};
pub use schema::{
    describe_result_offset, describe_result_to_api, index_definition_input_to_api,
    index_definition_ir_to_api, index_definition_offset,
};
pub use search::{
    inspect_query_result_offset, inspect_query_result_to_api, search_request_ir_to_api,
};
pub use transport::{
    encode_analyze_request, encode_analyze_request_for_target,
    encode_analyze_request_for_target_generation, encode_create_index_request,
    encode_delete_index_request, encode_delete_request, encode_delete_request_for_target,
    encode_delete_request_for_target_generation, encode_describe_request,
    encode_describe_request_for_target, encode_describe_request_for_target_generation,
    encode_get_request, encode_get_request_for_target, encode_get_request_for_target_generation,
    encode_insert_request, encode_insert_request_for_target,
    encode_insert_request_for_target_generation, encode_list_indexes_request,
    encode_open_index_request, encode_patch_request, encode_patch_request_for_target,
    encode_patch_request_for_target_generation, encode_search_request,
    encode_search_request_for_target, encode_search_request_for_target_generation,
    encode_update_request, encode_update_request_for_target,
    encode_update_request_for_target_generation, encode_upsert_request,
    encode_upsert_request_for_target, encode_upsert_request_for_target_generation,
};
