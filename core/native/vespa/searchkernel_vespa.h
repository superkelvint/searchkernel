#ifndef SEARCHKERNEL_VESPA_H
#define SEARCHKERNEL_VESPA_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#if defined(_WIN32)
#define SK_API __declspec(dllexport)
#elif defined(__GNUC__) || defined(__clang__)
#define SK_API __attribute__((visibility("default")))
#else
#define SK_API
#endif

#ifdef __cplusplus
#define SK_NOEXCEPT noexcept
#else
#define SK_NOEXCEPT
#endif

#define SK_NATIVE_STATUS_OK 0
#define SK_NATIVE_STATUS_INVALID_ARGUMENT 1
#define SK_NATIVE_STATUS_INTERNAL 2
#define SK_NATIVE_STATUS_OUT_OF_MEMORY 3
#define SK_NATIVE_STATUS_UNSUPPORTED 4
#define SK_NATIVE_STATUS_TYPE_ERROR 6

#define SK_TEXT_QUERY_TERM 1U
#define SK_TEXT_QUERY_PHRASE 2U

#define SK_QUERY_NODE_AND 1U
#define SK_QUERY_NODE_OR 2U
#define SK_QUERY_NODE_NOT 3U
#define SK_QUERY_NODE_TRUE 4U
#define SK_QUERY_TEXT_TERM 5U
#define SK_QUERY_TEXT_PHRASE 6U
#define SK_QUERY_NODE_NEAR 7U
#define SK_QUERY_NODE_ONEAR 8U
#define SK_QUERY_NODE_EQUIV 9U
#define SK_QUERY_NODE_WEAK_AND 10U
#define SK_QUERY_NODE_RANK 11U
#define SK_QUERY_BOUND_UNBOUNDED 0U
#define SK_QUERY_BOUND_INCLUDED 1U
#define SK_QUERY_BOUND_EXCLUDED 2U
#define SK_QUERY_FIELD_TEXT 1U
#define SK_QUERY_FIELD_INTEGER 2U
#define SK_QUERY_FIELD_STRING 3U
#define SK_QUERY_FIELD_FLOAT 4U
#define SK_QUERY_FIELD_BOOLEAN 5U
#define SK_QUERY_FIELD_ATTRIBUTE 6U
#define SK_QUERY_FIELD_TENSOR 7U
#define SK_QUERY_FIELD_ARRAY 8U
/* Kept as an ABI/source compatibility alias for existing clients. */
#define SK_QUERY_FIELD_ARRAY_STRING SK_QUERY_FIELD_ARRAY
#define SK_QUERY_FIELD_POSITION 9U
#define SK_QUERY_FIELD_PREDICATE 10U

#define SK_VALUE_STRING 1U
#define SK_VALUE_INTEGER 2U
#define SK_VALUE_FLOAT 3U
#define SK_VALUE_BOOLEAN 4U
#define SK_VALUE_TENSOR 5U
#define SK_VALUE_STRING_ARRAY 6U
#define SK_VALUE_POSITION 7U
#define SK_VALUE_PREDICATE 8U
#define SK_VALUE_STRUCT_ARRAY 9U

#define SK_SORT_ASCENDING 1U
#define SK_SORT_DESCENDING 2U
#define SK_SORT_MISSING_DEFAULT 0U
#define SK_SORT_MISSING_FIRST 1U
#define SK_SORT_MISSING_LAST 2U
#define SK_SUMMARY_NONE 0U
#define SK_SUMMARY_DEFAULT 1U
#define SK_SUMMARY_FIELDS 2U
#define SK_AGGREGATION_SUM 1U
#define SK_AGGREGATION_MIN 2U
#define SK_AGGREGATION_MAX 3U
#define SK_AGGREGATION_AVERAGE 4U
#define SK_AGGREGATION_STDDEV 5U
#define SK_AGGREGATION_XOR 6U
#define SK_AGGREGATION_QUANTILES 7U
#define SK_AGGREGATION_SUMMARY 8U
#define SK_GROUPING_RESULT_VALUE_STRING 1U
#define SK_GROUPING_RESULT_VALUE_INTEGER 2U
#define SK_GROUPING_RESULT_VALUE_FLOAT 3U
#define SK_GROUPING_RESULT_VALUE_QUANTILES 4U
#define SK_GROUPING_RESULT_VALUE_HIT_LIST 5U
#define SK_GROUPING_RESULT_VALUE_BOOLEAN 6U
#define SK_GROUPING_TREE_OUTPUT_VALUE 1U
#define SK_GROUPING_TREE_OUTPUT_HIT_LIST 2U
#define SK_GROUP_ORDER_NONE 0U
#define SK_GROUP_ORDER_COUNT_ASC 1U
#define SK_GROUP_ORDER_COUNT_DESC 2U
#define SK_GROUP_ORDER_VALUE_ASC 3U
#define SK_GROUP_ORDER_VALUE_DESC 4U

#define SK_GROUPING_BINARY_ADD 1U
#define SK_GROUPING_BINARY_SUBTRACT 2U
#define SK_GROUPING_BINARY_MULTIPLY 3U
#define SK_GROUPING_BINARY_DIVIDE 4U
#define SK_GROUPING_BINARY_MODULO 5U
#define SK_NATIVE_STATUS_DEADLINE_EXCEEDED 5U

#define SK_ARITHMETIC_ADD 0U
#define SK_ARITHMETIC_DIV 1U
#define SK_ARITHMETIC_MUL 2U
#define SK_ARITHMETIC_SUB 3U

#define SK_DEGRADATION_MATCH_PHASE 0x01U
#define SK_DEGRADATION_TIMEOUT 0x02U
#define SK_DEGRADATION_ANN_TIMEOUT 0x04U

/*
 * This is a primitive-only ABI. The semantic objects are opaque handles; the
 * only non-opaque values are fixed-width scalars and borrowed byte ranges.
 * Aggregate structs are deliberately excluded because their C and Rust
 * layouts would be a second ABI contract, while embedded pointers would add
 * another lifetime contract. Text token data therefore crosses through a
 * structure-of-arrays call, with the array index supplying token position.
 */
typedef struct sk_engine sk_engine;
typedef struct sk_document sk_document;
typedef struct sk_struct_array sk_struct_array;
typedef struct sk_document_update sk_document_update;
typedef struct sk_search_request sk_search_request;
typedef struct sk_search_result sk_search_result;
typedef struct sk_text_document sk_text_document;
typedef struct sk_text_update sk_text_update;
typedef struct sk_text_request sk_text_request;
typedef struct sk_text_result sk_text_result;
typedef struct sk_query sk_query;
typedef struct sk_nearest_neighbor_options sk_nearest_neighbor_options;
typedef struct sk_local_search_request sk_local_search_request;
typedef struct sk_local_search_result sk_local_search_result;
typedef struct sk_attribute_document sk_attribute_document;
typedef struct sk_attribute_update sk_attribute_update;
typedef struct sk_local_grouping_request sk_local_grouping_request;
typedef struct sk_local_grouping_result sk_local_grouping_result;
typedef struct sk_generic_search_request sk_generic_search_request;
typedef struct sk_generic_grouping_request sk_generic_grouping_request;
typedef struct sk_local_search_result sk_generic_search_result;
typedef struct sk_local_grouping_result sk_generic_grouping_result;
typedef struct sk_feature_value sk_feature_value;

/* FeatureSet::Value is deliberately narrower than the general Search Kernel
 * value model: Vespa exposes either a scalar double or an opaque data payload.
 * Data bytes returned by a result are borrowed from that result. */
#define SK_FEATURE_VALUE_KIND_INVALID 0U
#define SK_FEATURE_VALUE_DOUBLE 1U
#define SK_FEATURE_VALUE_DATA 2U
typedef struct sk_grouping_plan sk_grouping_plan;
typedef struct sk_grouping_level sk_grouping_level;
typedef struct sk_grouping_expression sk_grouping_expression;
typedef struct sk_grouping_aggregation sk_grouping_aggregation;
typedef struct sk_grouping_result_value sk_grouping_result_value;
typedef struct sk_grouping_tree_output sk_grouping_tree_output;
typedef struct sk_grouping_tree_result sk_grouping_tree_result;
typedef struct sk_grouping_tree_node sk_grouping_tree_node;
typedef struct sk_grouping_filter sk_grouping_filter;

/*
 * Diagnostics are caller-owned bytes rather than a public error struct. The
 * return value is the sole status authority. This avoids exposing struct
 * layout, allocator ownership, initialization rules, and a possible
 * allocation failure while reporting another failure. When capacity is
 * non-zero, implementations write a truncated NUL-terminated diagnostic.
 * Capability errors may use the stable `SK_DETAIL_V1:<code>:<message>` form;
 * callers must consume the code, not classify the human-readable message.
 */
#define SK_ERROR_MESSAGE_CAPACITY 512U

/* ABI revision 16. Diagnostics add setters/accessors around opaque handles;
   existing signatures, layouts and behavior remain compatible. */
#define SK_NATIVE_ABI_VERSION 16U

SK_API uint32_t sk_native_abi_version(void) SK_NOEXCEPT;

SK_API int32_t sk_engine_open(const char *data_dir, const char *config_dir,
                              sk_engine **out_engine, uint8_t *out_error,
                              uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_engine_close(sk_engine *engine, uint8_t *out_error,
                               uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API void sk_engine_destroy(sk_engine *engine) SK_NOEXCEPT;

SK_API int32_t sk_document_create(const char *document_id, const char *field,
                                  int64_t value, sk_document **out_document,
                                  uint8_t *out_error,
                                  uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API void sk_document_destroy(sk_document *document) SK_NOEXCEPT;

/* Generic M-layer document builder. Values are copied during each call and
   are interpreted against the generated schema when the document is put. */
SK_API int32_t sk_document_create_empty(const char *document_id,
                                        sk_document **out_document,
                                        uint8_t *out_error,
                                        uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_document_add_string(sk_document *document, const char *field,
                                      const char *value, uint8_t *out_error,
                                      uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_document_add_integer(sk_document *document, const char *field,
                                       int64_t value, uint8_t *out_error,
                                       uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_document_add_float(sk_document *document, const char *field,
                                     double value, uint8_t *out_error,
                                     uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_document_add_boolean(sk_document *document, const char *field,
                                       uint8_t value, uint8_t *out_error,
                                       uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_document_add_tensor_dense(
    sk_document *document, const char *field, const char *type_spec,
    const double *values, uint32_t value_count, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_document_add_string_array(
    sk_document *document, const char *field, const uint8_t *value_bytes,
    const uint32_t *value_lengths, uint32_t value_count, uint32_t value_bytes_length,
    uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_document_add_position(
    sk_document *document, const char *field, double latitude, double longitude,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_document_add_predicate(
    sk_document *document, const char *field, const uint8_t *feature_key_bytes,
    const uint32_t *feature_key_lengths, uint32_t feature_key_bytes_length,
    const uint8_t *feature_value_bytes, const uint32_t *feature_value_lengths,
    uint32_t feature_value_bytes_length, uint32_t feature_count,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_document_add_predicate_json(
    sk_document *document, const char *field, const char *expression_json,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_struct_array_create(
    sk_struct_array **out_array, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_struct_array_add_element(
    sk_struct_array *array, uint32_t *out_element_index, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_struct_array_set_string(
    sk_struct_array *array, uint32_t element_index, const char *field,
    const char *value, uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_struct_array_set_integer(
    sk_struct_array *array, uint32_t element_index, const char *field,
    int64_t value, uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_struct_array_set_float(
    sk_struct_array *array, uint32_t element_index, const char *field,
    double value, uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_struct_array_set_boolean(
    sk_struct_array *array, uint32_t element_index, const char *field,
    uint8_t value, uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API void sk_struct_array_destroy(sk_struct_array *array) SK_NOEXCEPT;
SK_API int32_t sk_document_add_struct_array(
    sk_document *document, const char *field, const sk_struct_array *array,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_document_annotate_text(
    sk_document *document, const char *field, const uint8_t *term_bytes,
    uint32_t term_bytes_length, const uint32_t *term_lengths,
    const uint32_t *byte_starts, const uint32_t *byte_lengths,
    uint32_t occurrence_count, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_document_annotate_string_array(
    sk_document *document, const char *field, const uint8_t *term_bytes,
    uint32_t term_bytes_length, const uint32_t *term_lengths,
    const uint32_t *byte_starts, const uint32_t *byte_lengths,
    const uint32_t *element_offsets, uint32_t element_count,
    uint32_t occurrence_count, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;

SK_API int32_t sk_document_update_create(const char *document_id, const char *field,
                                         int64_t value,
                                         sk_document_update **out_update,
                                         uint8_t *out_error,
                                         uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API void sk_document_update_destroy(sk_document_update *update) SK_NOEXCEPT;
SK_API int32_t sk_document_update_create_empty(const char *document_id,
                                               sk_document_update **out_update,
                                               uint8_t *out_error,
                                               uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_document_update_set_create_if_nonexistent(
    sk_document_update *update, uint8_t create_if_nonexistent, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_document_update_assign_string(sk_document_update *update,
                                                const char *field, const char *value,
                                                uint8_t *out_error,
                                                uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_document_update_assign_integer(sk_document_update *update,
                                                 const char *field, int64_t value,
                                                 uint8_t *out_error,
                                                 uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_document_update_assign_float(sk_document_update *update,
                                               const char *field, double value,
                                               uint8_t *out_error,
                                               uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_document_update_assign_boolean(sk_document_update *update,
                                                 const char *field, uint8_t value,
                                                 uint8_t *out_error,
                                                 uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_document_update_assign_tensor_dense(
    sk_document_update *update, const char *field, const char *type_spec,
    const double *values, uint32_t value_count, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_document_update_assign_string_array(
    sk_document_update *update, const char *field, const uint8_t *value_bytes,
    const uint32_t *value_lengths, uint32_t value_count, uint32_t value_bytes_length,
    uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
/* Copies the array contents; the caller retains and may destroy the array handle. */
SK_API int32_t sk_document_update_assign_struct_array(
    sk_document_update *update, const char *field, const sk_struct_array *array,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_document_update_assign_position(
    sk_document_update *update, const char *field, double latitude, double longitude,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_document_update_assign_predicate(
    sk_document_update *update, const char *field, const uint8_t *feature_key_bytes,
    const uint32_t *feature_key_lengths, uint32_t feature_key_bytes_length,
    const uint8_t *feature_value_bytes, const uint32_t *feature_value_lengths,
    uint32_t feature_value_bytes_length, uint32_t feature_count,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_document_update_assign_predicate_json(
    sk_document_update *update, const char *field, const char *expression_json,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_document_update_clear(sk_document_update *update, const char *field,
                                        uint8_t *out_error,
                                        uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_document_update_arithmetic(
    sk_document_update *update, const char *field, uint32_t arithmetic_operator,
    double operand, uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_document_update_annotate_text(
    sk_document_update *update, const char *field, const uint8_t *term_bytes,
    uint32_t term_bytes_length, const uint32_t *term_lengths,
    const uint32_t *byte_starts, const uint32_t *byte_lengths,
    uint32_t occurrence_count, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_document_update_annotate_string_array(
    sk_document_update *update, const char *field, const uint8_t *term_bytes,
    uint32_t term_bytes_length, const uint32_t *term_lengths,
    const uint32_t *byte_starts, const uint32_t *byte_lengths,
    const uint32_t *element_offsets, uint32_t element_count,
    uint32_t occurrence_count, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;

SK_API int32_t sk_search_request_create(const char *field, int64_t value,
                                        uint32_t limit,
                                        sk_search_request **out_request,
                                        uint8_t *out_error,
                                        uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API void sk_search_request_destroy(sk_search_request *request) SK_NOEXCEPT;

SK_API int32_t sk_engine_put(sk_engine *engine, const sk_document *document,
                             uint8_t *out_error,
                             uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_engine_update(sk_engine *engine,
                                const sk_document_update *update,
                                uint8_t *out_error,
                                uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_engine_get(const sk_engine *engine, const char *document_id,
                             sk_document **out_document, uint8_t *out_error,
                             uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_document_field_count(const sk_document *document,
                                       uint32_t *out_count, uint8_t *out_error,
                                       uint32_t out_error_capacity) SK_NOEXCEPT;
/* For SK_VALUE_STRING_ARRAY, SK_VALUE_STRUCT_ARRAY, SK_VALUE_POSITION, and
 * SK_VALUE_PREDICATE, out_string is a borrowed UTF-8 JSON payload owned by
 * the document. Scalar fields continue to use the corresponding primitive
 * output below. */
SK_API int32_t sk_document_field(const sk_document *document, uint32_t index,
                                 const char **out_name, uint32_t *out_name_length,
                                 uint32_t *out_kind, const char **out_string,
                                 uint32_t *out_string_length, int64_t *out_integer,
                                 double *out_float, uint8_t *out_boolean,
                                 uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
/* Returns borrowed dense tensor metadata owned by document. The pointers
 * remain valid until document destruction. This accessor is valid only for a
 * SK_VALUE_TENSOR field and exposes Search Kernel-owned primitive values. */
SK_API int32_t sk_document_field_tensor_dense(
    const sk_document *document, uint32_t index,
    const char **out_type_spec, uint32_t *out_type_spec_length,
    const double **out_cells, uint32_t *out_cell_count,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_engine_remove(sk_engine *engine, const char *document_id,
                                int32_t *out_was_found, uint8_t *out_error,
                                uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_engine_search(const sk_engine *engine,
                                const sk_search_request *request,
                                sk_search_result **out_result,
                                uint8_t *out_error,
                                uint32_t out_error_capacity) SK_NOEXCEPT;

/* Engine operations and sk_engine_close may be called concurrently; the
   adapter serializes them. sk_engine_destroy requires exclusive ownership of
   the engine and must not race with any other operation. */

SK_API uint32_t sk_search_result_size(const sk_search_result *result) SK_NOEXCEPT;
/* The returned ID is borrowed from result and remains valid until result is
   destroyed. */
SK_API int32_t sk_search_result_id(const sk_search_result *result, uint32_t index,
                                   const char **out_id, uint32_t *out_length,
                                   uint8_t *out_error,
                                   uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API void sk_search_result_destroy(sk_search_result *result) SK_NOEXCEPT;

/*
 * Text is created first and populated with one structure-of-arrays transfer.
 * `term_bytes` is the concatenation of `term_lengths`; the three parallel
 * arrays have `occurrence_count` entries. Position is the entry index and is
 * not transmitted because accepting a redundant position field would create
 * an invariant the native side must validate forever.
 */
SK_API int32_t sk_text_document_create(const char *document_id, const char *field,
                                       const uint8_t *text, uint32_t text_length,
                                       sk_text_document **out_document,
                                       uint8_t *out_error,
                                       uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_text_document_add_occurrences(
    sk_text_document *document, const uint8_t *term_bytes,
    uint32_t term_bytes_length, const uint32_t *term_lengths,
    const uint32_t *byte_starts, const uint32_t *byte_lengths,
    uint32_t occurrence_count, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API void sk_text_document_destroy(sk_text_document *document) SK_NOEXCEPT;

SK_API int32_t sk_text_update_create(const char *document_id, const char *field,
                                     const uint8_t *text, uint32_t text_length,
                                     sk_text_update **out_update,
                                     uint8_t *out_error,
                                     uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_text_update_add_occurrences(
    sk_text_update *update, const uint8_t *term_bytes,
    uint32_t term_bytes_length, const uint32_t *term_lengths,
    const uint32_t *byte_starts, const uint32_t *byte_lengths,
    uint32_t occurrence_count, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API void sk_text_update_destroy(sk_text_update *update) SK_NOEXCEPT;

SK_API int32_t sk_text_request_create(uint32_t kind, const char *field,
                                      const char *rank_profile,
                                      sk_text_request **out_request,
                                      uint8_t *out_error,
                                      uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_text_request_add_terms(
    sk_text_request *request, const uint8_t *term_bytes,
    uint32_t term_bytes_length, const uint32_t *term_lengths,
    uint32_t term_count, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API void sk_text_request_destroy(sk_text_request *request) SK_NOEXCEPT;

SK_API int32_t sk_engine_put_text(sk_engine *engine,
                                  const sk_text_document *document,
                                  uint8_t *out_error,
                                  uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_engine_update_text(sk_engine *engine,
                                     const sk_text_update *update,
                                     uint8_t *out_error,
                                     uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_engine_search_text(const sk_engine *engine,
                                     const sk_text_request *request,
                                     sk_text_result **out_result,
                                     uint8_t *out_error,
                                     uint32_t out_error_capacity) SK_NOEXCEPT;

SK_API uint32_t sk_text_result_size(const sk_text_result *result) SK_NOEXCEPT;
SK_API int32_t sk_text_result_hit(const sk_text_result *result, uint32_t index,
                                  const char **out_id, uint32_t *out_length,
                                  double *out_score, uint8_t *out_error,
                                  uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API void sk_text_result_destroy(sk_text_result *result) SK_NOEXCEPT;

/*
 * Query construction is prefix ordered and uses only opaque handles,
 * fixed-width tags/scalars, and borrowed primitive byte arrays. The native
 * adapter owns the Vespa query tree and copies all supplied values. Query
 * handles also snapshot the field metadata needed for validation, so their
 * lifetime is independent of the engine used to create them.
 *
 * A query handle may be passed to sk_engine_search_query() at most once.
 * That call consumes the completed query tree while building the native
 * execution tree; after it returns, regardless of status, the handle must not
 * be searched again or extended with more query nodes. Destroy it with
 * sk_query_destroy().
 */
SK_API int32_t sk_query_create(const sk_engine *engine, sk_query **out_query,
                               uint8_t *out_error,
                               uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_operator(sk_query *query, uint32_t kind,
                                      uint32_t child_count, uint8_t *out_error,
                                      uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_validate_field(const sk_query *query, uint32_t kind,
                                       const char *field, uint8_t *out_error,
                                       uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_true(sk_query *query, uint8_t *out_error,
                                 uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_text(sk_query *query, uint32_t kind,
                                 const char *field, const uint8_t *term_bytes,
                                 uint32_t term_bytes_length,
                                 const uint32_t *term_lengths,
                                 uint32_t term_count, uint8_t *out_error,
                                 uint32_t out_error_capacity) SK_NOEXCEPT;
/* Add Vespa's native InTerm node. Values are copied by the adapter. */
SK_API int32_t sk_query_add_in_strings(
    sk_query *query, const char *field, const uint8_t *value_bytes,
    uint32_t value_bytes_length, const uint32_t *value_lengths,
    uint32_t value_count, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_in_integers(
    sk_query *query, const char *field, const int64_t *values,
    uint32_t value_count, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_proximity(
    sk_query *query, uint8_t ordered, const char *field, uint32_t child_count,
    uint32_t distance, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_equiv(
    sk_query *query, uint32_t child_count, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_weak_and(
    sk_query *query, const char *field, uint32_t child_count,
    uint32_t target_num_hits, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_rank(
    sk_query *query, uint32_t child_count, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_fuzzy(
    sk_query *query, const char *field, const char *term,
    uint32_t max_edit_distance, uint32_t prefix_lock_length,
    uint8_t prefix_match, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_alternatives(
    sk_query *query, const char *field, const uint8_t *term_bytes,
    uint32_t term_bytes_length, const uint32_t *term_lengths,
    const int64_t *exactness_weights, uint32_t term_count,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_matches(
    sk_query *query, const char *field, const char *pattern,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_labeled(
    sk_query *query, int32_t unique_id, double score,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_weighted_set_strings(
    sk_query *query, const char *field, const uint8_t *key_bytes,
    uint32_t key_bytes_length, const uint32_t *key_lengths,
    const int64_t *weights, uint32_t item_count, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_weighted_set_integers(
    sk_query *query, const char *field, const int64_t *keys,
    const int64_t *weights, uint32_t item_count, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_dot_product_strings(
    sk_query *query, const char *field, const uint8_t *key_bytes,
    uint32_t key_bytes_length, const uint32_t *key_lengths,
    const int64_t *weights, uint32_t item_count, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_dot_product_integers(
    sk_query *query, const char *field, const int64_t *keys,
    const int64_t *weights, uint32_t item_count, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_wand_strings(
    sk_query *query, const char *field, const uint8_t *key_bytes,
    uint32_t key_bytes_length, const uint32_t *key_lengths,
    const int64_t *weights, uint32_t item_count, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_wand_integers(
    sk_query *query, const char *field, const int64_t *keys,
    const int64_t *weights, uint32_t item_count, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_wand_strings_with_options(
    sk_query *query, const char *field, const uint8_t *key_bytes,
    uint32_t key_bytes_length, const uint32_t *key_lengths,
    const int64_t *weights, uint32_t item_count, uint32_t target_num_hits,
    int64_t score_threshold, double threshold_boost_factor,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_wand_integers_with_options(
    sk_query *query, const char *field, const int64_t *keys,
    const int64_t *weights, uint32_t item_count, uint32_t target_num_hits,
    int64_t score_threshold, double threshold_boost_factor,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_equals(sk_query *query, const char *field,
                                   int64_t value, uint8_t *out_error,
                                   uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_range(sk_query *query, const char *field,
                                  uint32_t lower_kind, int64_t lower_value,
                                  uint32_t upper_kind, int64_t upper_value,
                                  uint8_t *out_error,
                                  uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_nearest_neighbor(
    sk_query *query, const char *document_field, const char *query_tensor,
    uint32_t target_num_hits, uint8_t allow_approximate,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_nearest_neighbor_options_create(
    sk_nearest_neighbor_options **out_options, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API void sk_nearest_neighbor_options_destroy(
    sk_nearest_neighbor_options *options) SK_NOEXCEPT;
SK_API int32_t sk_nearest_neighbor_options_set_distance_threshold(
    sk_nearest_neighbor_options *options, double distance_threshold,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_nearest_neighbor_options_set_explore_additional_hits(
    sk_nearest_neighbor_options *options, uint32_t explore_additional_hits,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_nearest_neighbor_with_options_handle(
    sk_query *query, const char *document_field, const char *query_tensor,
    uint32_t target_num_hits, uint8_t allow_approximate,
    const sk_nearest_neighbor_options *options, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_same_element(
    sk_query *query, const char *field, uint32_t child_count,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_same_element_with_filter(
    sk_query *query, const char *field, uint32_t child_count,
    const uint32_t *element_filter, uint32_t element_filter_count,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_geo_location(
    sk_query *query, const char *field, double latitude, double longitude,
    const char *radius, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_geo_bounding_box(
    sk_query *query, const char *field, double south, double west,
    double north, double east, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_predicate(
    sk_query *query, const char *field,
    const uint8_t *feature_key_bytes, uint32_t feature_key_bytes_length,
    const uint32_t *feature_key_lengths, const uint8_t *feature_value_bytes,
    uint32_t feature_value_bytes_length, const uint32_t *feature_value_lengths,
    const uint64_t *feature_masks, uint32_t feature_count,
    const uint8_t *range_key_bytes, uint32_t range_key_bytes_length,
    const uint32_t *range_key_lengths, const uint64_t *range_values,
    const uint64_t *range_masks,
    uint32_t range_count, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API void sk_query_destroy(sk_query *query) SK_NOEXCEPT;
SK_API int32_t sk_engine_search_query(const sk_engine *engine,
                                      const sk_query *query,
                                      const char *rank_profile,
                                      const char *source,
                                      sk_text_result **out_result,
                                      uint8_t *out_error,
                                      uint32_t out_error_capacity) SK_NOEXCEPT;

SK_API int32_t sk_local_search_request_create(
    const char *rank_profile, uint32_t limit, uint32_t offset,
    uint32_t summary_kind, sk_local_search_request **out_request,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_local_search_request_add_sort(
    sk_local_search_request *request, const char *field, uint32_t order,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_local_search_request_add_summary_field(
    sk_local_search_request *request, const char *field,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_local_search_request_add_highlight_field(
    sk_local_search_request *request, const char *field,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_local_search_request_set_summary_class(
    sk_local_search_request *request, const char *name,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_local_search_request_add_query_feature(
    sk_local_search_request *request, const char *name, double value,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_local_search_request_add_rank_property(
    sk_local_search_request *request, const char *name, const char *value,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API void sk_local_search_request_destroy(sk_local_search_request *request) SK_NOEXCEPT;
SK_API int32_t sk_engine_search_local(
    const sk_engine *engine, const sk_query *query,
    const sk_local_search_request *request,
    sk_local_search_result **out_result, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API uint64_t sk_local_search_result_total_hit_count(
    const sk_local_search_result *result) SK_NOEXCEPT;
SK_API uint32_t sk_local_search_result_hit_count(
    const sk_local_search_result *result) SK_NOEXCEPT;
SK_API int32_t sk_local_search_result_hit(
    const sk_local_search_result *result, uint32_t index,
    const char **out_id, uint32_t *out_id_length, double *out_score,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API uint32_t sk_local_search_result_summary_count(
    const sk_local_search_result *result, uint32_t hit_index) SK_NOEXCEPT;
SK_API int32_t sk_local_search_result_summary_name(
    const sk_local_search_result *result, uint32_t hit_index,
    uint32_t summary_index, const char **out_name, uint32_t *out_length,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_local_search_result_summary_value(
    const sk_local_search_result *result, uint32_t hit_index,
    uint32_t summary_index, uint32_t *out_kind, const char **out_string,
    uint32_t *out_string_length, int64_t *out_integer, double *out_float,
    uint8_t *out_boolean, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API uint32_t sk_local_search_result_match_feature_count(
    const sk_local_search_result *result) SK_NOEXCEPT;
SK_API int32_t sk_local_search_result_match_feature_name(
    const sk_local_search_result *result, uint32_t feature_index,
    const char **out_name, uint32_t *out_length, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_local_search_result_match_feature_value(
    const sk_local_search_result *result, uint32_t hit_index,
    uint32_t feature_index, double *out_value, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
/* Returns a borrowed feature value owned by result. */
SK_API int32_t sk_local_search_result_match_feature_value_ref(
    const sk_local_search_result *result, uint32_t hit_index,
    uint32_t feature_index, const sk_feature_value **out_value,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API void sk_local_search_result_destroy(sk_local_search_result *result) SK_NOEXCEPT;

SK_API uint32_t sk_feature_value_kind(const sk_feature_value *value) SK_NOEXCEPT;
SK_API int32_t sk_feature_value_double(const sk_feature_value *value,
                                       double *out_value, uint8_t *out_error,
                                       uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_feature_value_data(const sk_feature_value *value,
                                     const uint8_t **out_data, size_t *out_size,
                                     uint8_t *out_error,
                                     uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API uint32_t sk_local_search_result_highlight_count(
    const sk_local_search_result *result, uint32_t hit_index) SK_NOEXCEPT;
SK_API int32_t sk_local_search_result_highlight_field(
    const sk_local_search_result *result, uint32_t hit_index,
    uint32_t highlight_index, const char **out_field, uint32_t *out_length,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API uint32_t sk_local_search_result_highlight_fragment_count(
    const sk_local_search_result *result, uint32_t hit_index,
    uint32_t highlight_index) SK_NOEXCEPT;
SK_API int32_t sk_local_search_result_highlight_fragment_text(
    const sk_local_search_result *result, uint32_t hit_index,
    uint32_t highlight_index, uint32_t fragment_index, const char **out_text,
    uint32_t *out_length, uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API uint32_t sk_local_search_result_highlight_match_count(
    const sk_local_search_result *result, uint32_t hit_index,
    uint32_t highlight_index, uint32_t fragment_index) SK_NOEXCEPT;
SK_API int32_t sk_local_search_result_highlight_match(
    const sk_local_search_result *result, uint32_t hit_index,
    uint32_t highlight_index, uint32_t fragment_index, uint32_t match_index,
    uint32_t *out_start_byte, uint32_t *out_end_byte,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;

/* Generic M-layer search request/result. The older local request remains a
   frozen milestone surface; new generic consumers use these handles. */
SK_API int32_t sk_generic_search_request_create(
    const char *rank_profile, uint32_t limit, uint32_t offset,
    uint32_t summary_kind, sk_generic_search_request **out_request,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_generic_search_request_set_timeout(
    sk_generic_search_request *request, uint64_t timeout_millis,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
/* All controls are copied. Diagnostics are read-only and opt-in. */
SK_API int32_t sk_generic_search_request_set_diagnostics(
    sk_generic_search_request *request, uint32_t trace_level,
    uint32_t matching_depth, uint32_t first_phase_depth,
    uint32_t second_phase_depth, uint32_t raw_bytes,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_generic_search_request_add_sort(
    sk_generic_search_request *request, const char *field, uint32_t order,
    uint32_t missing, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_generic_search_request_set_sort_spec(
    sk_generic_search_request *request, const char *sort_spec,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_generic_search_request_add_summary_field(
    sk_generic_search_request *request, const char *field,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_generic_search_request_add_highlight_field(
    sk_generic_search_request *request, const char *field,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_generic_search_request_set_summary_class(
    sk_generic_search_request *request, const char *name,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_generic_search_request_add_query_feature(
    sk_generic_search_request *request, const char *name, double value,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_generic_search_request_add_query_tensor_dense(
    sk_generic_search_request *request, const char *name,
    const char *type_spec, const double *values, uint32_t value_count,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_generic_search_request_add_rank_property(
    sk_generic_search_request *request, const char *name, const char *value,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
/* Add one independent grouping expression to the same native search
   execution. The returned search result exposes grouping results in this
   insertion order. */
SK_API int32_t sk_generic_search_request_add_grouping(
    sk_generic_search_request *request, const char *group_field,
    uint32_t max_groups, uint32_t order, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
/* Add a structured grouping plan to this same native match request. The
 * request copies the plan and owns it thereafter. */
SK_API int32_t sk_generic_search_request_add_grouping_plan(
    sk_generic_search_request *request, const sk_grouping_plan *plan,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API void sk_generic_search_request_destroy(sk_generic_search_request *request) SK_NOEXCEPT;
SK_API int32_t sk_engine_search_generic(
    const sk_engine *engine, const sk_query *query,
    const sk_generic_search_request *request,
    sk_generic_search_result **out_result, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API uint64_t sk_generic_search_result_total_hit_count(
    const sk_generic_search_result *result) SK_NOEXCEPT;
SK_API uint32_t sk_generic_search_result_hit_count(
    const sk_generic_search_result *result) SK_NOEXCEPT;
SK_API uint64_t sk_generic_search_result_coverage_active(
    const sk_generic_search_result *result) SK_NOEXCEPT;
SK_API uint64_t sk_generic_search_result_coverage_covered(
    const sk_generic_search_result *result) SK_NOEXCEPT;
SK_API uint64_t sk_generic_search_result_coverage_target_active(
    const sk_generic_search_result *result) SK_NOEXCEPT;
SK_API uint32_t sk_generic_search_result_degradation_flags(
    const sk_generic_search_result *result) SK_NOEXCEPT;
SK_API uint32_t sk_generic_search_result_issue_count(
    const sk_generic_search_result *result) SK_NOEXCEPT;
/* Borrowed bytes remain valid until result destruction. The caller copies
   them before releasing the opaque result. Truncated payloads are prefixes,
   not complete JSON; timings are measured seconds from this execution.
   out_truncated is a mask: 1=trace prefix, 2=omitted oversized DATA columns. */
/* Caller-owned output buffers; independent calls retain no pointers. Decoding
   is above the unchanged raw DOUBLE/DATA feature ABI, using Vespa's codec. */
SK_API int32_t sk_diagnostic_tensor_decode(
    const uint8_t *data, uint32_t length, char *type_spec,
    uint32_t type_capacity, uint32_t *out_type_length, double *cells,
    uint32_t cell_capacity, uint32_t *out_cell_count,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_generic_search_result_diagnostics(
    const sk_generic_search_result *result, const uint8_t **out_trace,
    uint32_t *out_length, uint32_t *out_truncated,
    double *out_matching_seconds, double *out_summary_seconds,
    double *out_query_seconds, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_generic_search_result_issue_message(
    const sk_generic_search_result *result, uint32_t index,
    const char **out_message, uint32_t *out_length, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_generic_search_result_hit(
    const sk_generic_search_result *result, uint32_t index,
    const char **out_id, uint32_t *out_id_length, double *out_score,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API uint32_t sk_generic_search_result_summary_count(
    const sk_generic_search_result *result, uint32_t hit_index) SK_NOEXCEPT;
SK_API int32_t sk_generic_search_result_summary_name(
    const sk_generic_search_result *result, uint32_t hit_index,
    uint32_t summary_index, const char **out_name, uint32_t *out_length,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_generic_search_result_summary_value(
    const sk_generic_search_result *result, uint32_t hit_index,
    uint32_t summary_index, uint32_t *out_kind, const char **out_string,
    uint32_t *out_string_length, int64_t *out_integer, double *out_float,
    uint8_t *out_boolean, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API uint32_t sk_generic_search_result_highlight_count(
    const sk_generic_search_result *result, uint32_t hit_index) SK_NOEXCEPT;
SK_API int32_t sk_generic_search_result_highlight_field(
    const sk_generic_search_result *result, uint32_t hit_index,
    uint32_t highlight_index, const char **out_field, uint32_t *out_length,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API uint32_t sk_generic_search_result_highlight_fragment_count(
    const sk_generic_search_result *result, uint32_t hit_index,
    uint32_t highlight_index) SK_NOEXCEPT;
SK_API int32_t sk_generic_search_result_highlight_fragment_text(
    const sk_generic_search_result *result, uint32_t hit_index,
    uint32_t highlight_index, uint32_t fragment_index, const char **out_text,
    uint32_t *out_length, uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API uint32_t sk_generic_search_result_highlight_match_count(
    const sk_generic_search_result *result, uint32_t hit_index,
    uint32_t highlight_index, uint32_t fragment_index) SK_NOEXCEPT;
SK_API int32_t sk_generic_search_result_highlight_match(
    const sk_generic_search_result *result, uint32_t hit_index,
    uint32_t highlight_index, uint32_t fragment_index, uint32_t match_index,
    uint32_t *out_start_byte, uint32_t *out_end_byte,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API uint32_t sk_generic_search_result_match_feature_count(
    const sk_generic_search_result *result) SK_NOEXCEPT;
SK_API int32_t sk_generic_search_result_match_feature_name(
    const sk_generic_search_result *result, uint32_t feature_index,
    const char **out_name, uint32_t *out_length, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_generic_search_result_match_feature_value(
    const sk_generic_search_result *result, uint32_t hit_index,
    uint32_t feature_index, double *out_value, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
/* Returns a borrowed feature value owned by result. */
SK_API int32_t sk_generic_search_result_match_feature_value_ref(
    const sk_generic_search_result *result, uint32_t hit_index,
    uint32_t feature_index, const sk_feature_value **out_value,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API uint32_t sk_generic_search_result_grouping_count(
    const sk_generic_search_result *result) SK_NOEXCEPT;
SK_API uint32_t sk_generic_search_result_grouping_tree_count(
    const sk_generic_search_result *result) SK_NOEXCEPT;
SK_API int32_t sk_generic_search_result_grouping_tree_root(
    const sk_generic_search_result *result, uint32_t grouping_index,
    const sk_grouping_tree_node **out_root, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API uint32_t sk_generic_search_result_group_count(
    const sk_generic_search_result *result, uint32_t grouping_index) SK_NOEXCEPT;
SK_API int32_t sk_generic_search_result_group(
    const sk_generic_search_result *result, uint32_t grouping_index,
    uint32_t group_index, uint32_t *out_key_kind, const char **out_key_string,
    uint32_t *out_key_string_length, int64_t *out_key_integer,
    uint64_t *out_count, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API void sk_generic_search_result_destroy(sk_generic_search_result *result) SK_NOEXCEPT;

SK_API int32_t sk_query_add_attribute_string_equals(
    sk_query *query, const char *field, const char *value,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_attribute_float_equals(
    sk_query *query, const char *field, double value,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_attribute_boolean_equals(
    sk_query *query, const char *field, uint8_t value,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_attribute_exists(
    sk_query *query, const char *field, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_attribute_integer_range(
    sk_query *query, const char *field, uint32_t lower_kind,
    int64_t lower_value, uint32_t upper_kind, int64_t upper_value,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_attribute_float_range(
    sk_query *query, const char *field, uint32_t lower_kind,
    double lower_value, uint32_t upper_kind, double upper_value,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;

SK_API int32_t sk_attribute_document_create(
    const char *document_id, sk_attribute_document **out_document,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_attribute_document_add_string(
    sk_attribute_document *document, const char *field, const char *value,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_attribute_document_add_integer(
    sk_attribute_document *document, const char *field, int64_t value,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_attribute_document_add_float(
    sk_attribute_document *document, const char *field, double value,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_attribute_document_add_boolean(
    sk_attribute_document *document, const char *field, uint8_t value,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API void sk_attribute_document_destroy(sk_attribute_document *document) SK_NOEXCEPT;
SK_API int32_t sk_attribute_update_create(
    const char *document_id, sk_attribute_update **out_update,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_attribute_update_add_string(
    sk_attribute_update *update, const char *field, const char *value,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_attribute_update_add_integer(
    sk_attribute_update *update, const char *field, int64_t value,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_attribute_update_add_float(
    sk_attribute_update *update, const char *field, double value,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_attribute_update_add_boolean(
    sk_attribute_update *update, const char *field, uint8_t value,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API void sk_attribute_update_destroy(sk_attribute_update *update) SK_NOEXCEPT;
SK_API int32_t sk_engine_put_attributes(
    sk_engine *engine, const sk_attribute_document *document,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_engine_update_attributes(
    sk_engine *engine, const sk_attribute_update *update,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;

/* General grouping uses opaque, domain-specific handles. Classifier and
   aggregation inputs are cloned when attached to a plan/level. A level handle
   is borrowed from its parent plan and remains valid only while that plan is
   alive. */
SK_API int32_t sk_grouping_plan_create(
    sk_grouping_plan **out_plan, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_plan_set_all(
    sk_grouping_plan *plan, uint8_t all, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_expression_attribute(
    const char *field, sk_grouping_expression **out_expression,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_expression_string(
    const char *value, sk_grouping_expression **out_expression,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_expression_integer(
    int64_t value, sk_grouping_expression **out_expression,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_expression_float(
    double value, sk_grouping_expression **out_expression,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_expression_boolean(
    uint8_t value, sk_grouping_expression **out_expression,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_expression_negative(
    const sk_grouping_expression *input, sk_grouping_expression **out_expression,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_expression_binary(
    uint32_t operator_kind, const sk_grouping_expression *left,
    const sk_grouping_expression *right, sk_grouping_expression **out_expression,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_expression_function(
    const char *name, const sk_grouping_expression *const *arguments,
    uint32_t argument_count, sk_grouping_expression **out_expression,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_expression_aggregation_reference(
    uint32_t index, sk_grouping_expression **out_expression,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API void sk_grouping_expression_destroy(
    sk_grouping_expression *expression) SK_NOEXCEPT;
SK_API int32_t sk_grouping_aggregation_count(
    sk_grouping_aggregation **out, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_aggregation_average(
    const sk_grouping_expression *expression,
    sk_grouping_aggregation **out, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_aggregation_sum(
    const sk_grouping_expression *expression, sk_grouping_aggregation **out,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_aggregation_min(
    const sk_grouping_expression *expression, sk_grouping_aggregation **out,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_aggregation_max(
    const sk_grouping_expression *expression, sk_grouping_aggregation **out,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_aggregation_stddev(
    const sk_grouping_expression *expression, sk_grouping_aggregation **out,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_aggregation_xor(
    const sk_grouping_expression *expression, sk_grouping_aggregation **out,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_aggregation_quantiles(
    const sk_grouping_expression *expression, const double *quantiles,
    uint32_t quantile_count, sk_grouping_aggregation **out,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_aggregation_summary(
    const char *summary_class, uint32_t max_hits, sk_grouping_aggregation **out,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API void sk_grouping_aggregation_destroy(
    sk_grouping_aggregation *aggregation) SK_NOEXCEPT;
SK_API int32_t sk_grouping_plan_add_level(
    sk_grouping_plan *plan, const sk_grouping_expression *classifier,
    sk_grouping_level **out_level, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_level_set_max_groups(
    sk_grouping_level *level, int64_t max_groups, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_level_set_precision(
    sk_grouping_level *level, int64_t precision, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_filter_regex(
    const char *pattern, const char *field, sk_grouping_filter **out_filter,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_filter_regex_expression(
    const char *pattern, const sk_grouping_expression *expression,
    sk_grouping_filter **out_filter, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_filter_and(
    const sk_grouping_filter *const *filters, uint32_t filter_count,
    sk_grouping_filter **out_filter, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_filter_or(
    const sk_grouping_filter *const *filters, uint32_t filter_count,
    sk_grouping_filter **out_filter, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_filter_not(
    const sk_grouping_filter *filter, sk_grouping_filter **out_filter,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_filter_range(
    double lower, double upper, const sk_grouping_expression *expression,
    uint8_t include_lower, uint8_t include_upper,
    sk_grouping_filter **out_filter, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_filter_is_true(
    const sk_grouping_expression *expression, sk_grouping_filter **out_filter,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API void sk_grouping_filter_destroy(sk_grouping_filter *filter) SK_NOEXCEPT;
SK_API int32_t sk_grouping_level_set_filter(
    sk_grouping_level *level, const sk_grouping_filter *filter,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_level_add_order_by(
    sk_grouping_level *level, const sk_grouping_expression *expression,
    uint8_t ascending, uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_level_add_count(
    sk_grouping_level *level, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_level_add_aggregation(
    sk_grouping_level *level, uint32_t kind, const char *field,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_level_add_aggregation_handle(
    sk_grouping_level *level, const sk_grouping_aggregation *aggregation,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_plan_add_root_aggregation_handle(
    sk_grouping_plan *plan, const sk_grouping_aggregation *aggregation,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API void sk_grouping_plan_destroy(sk_grouping_plan *plan) SK_NOEXCEPT;
SK_API int32_t sk_engine_group_tree(
    const sk_engine *engine, const sk_query *query,
    const sk_grouping_plan *plan, sk_grouping_tree_result **out_result,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
/* Executes grouping with the typed generic search request context. The
 * request supplies rank profile, query inputs, tensors, rank properties, and
 * timeout; grouping still uses the plan above and returns the same tree. */
SK_API int32_t sk_engine_group_tree_with_request(
    const sk_engine *engine, const sk_query *query,
    const sk_generic_search_request *request,
    const sk_grouping_plan *plan, sk_grouping_tree_result **out_result,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_tree_result_root(
    const sk_grouping_tree_result *result,
    const sk_grouping_tree_node **out_root, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API uint32_t sk_grouping_tree_node_group_count(
    const sk_grouping_tree_node *node) SK_NOEXCEPT;
SK_API int32_t sk_grouping_tree_node_group(
    const sk_grouping_tree_node *node, uint32_t group_index,
    const sk_grouping_tree_node **out_group, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_tree_node_key(
    const sk_grouping_tree_node *node,
    const sk_grouping_result_value **out_value, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API uint32_t sk_grouping_result_value_kind(
    const sk_grouping_result_value *value) SK_NOEXCEPT;
SK_API int32_t sk_grouping_result_value_string(
    const sk_grouping_result_value *value, const char **out_string,
    uint32_t *out_length, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_result_value_integer(
    const sk_grouping_result_value *value, int64_t *out_integer,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_result_value_float(
    const sk_grouping_result_value *value, double *out_float,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_result_value_boolean(
    const sk_grouping_result_value *value, uint8_t *out_boolean,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API uint32_t sk_grouping_result_value_quantile_count(
    const sk_grouping_result_value *value) SK_NOEXCEPT;
SK_API int32_t sk_grouping_result_value_quantile(
    const sk_grouping_result_value *value, uint32_t index,
    double *out_quantile, double *out_value, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API uint32_t sk_grouping_tree_node_output_count(
    const sk_grouping_tree_node *node) SK_NOEXCEPT;
SK_API int32_t sk_grouping_tree_node_output(
    const sk_grouping_tree_node *node, uint32_t output_index,
    const sk_grouping_tree_output **out_output, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API uint32_t sk_grouping_tree_output_kind(
    const sk_grouping_tree_output *output) SK_NOEXCEPT;
SK_API int32_t sk_grouping_tree_output_value(
    const sk_grouping_tree_output *output,
    const sk_grouping_result_value **out_value, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API uint32_t sk_grouping_tree_output_hit_count(
    const sk_grouping_tree_output *output) SK_NOEXCEPT;
SK_API int32_t sk_grouping_tree_output_hit_id(
    const sk_grouping_tree_output *output, uint32_t hit_index,
    const char **out_id, uint32_t *out_id_length, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_tree_output_hit_score(
    const sk_grouping_tree_output *output, uint32_t hit_index, double *out_score,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API uint32_t sk_grouping_tree_output_hit_summary_count(
    const sk_grouping_tree_output *output, uint32_t hit_index) SK_NOEXCEPT;
SK_API int32_t sk_grouping_tree_output_hit_summary_name(
    const sk_grouping_tree_output *output, uint32_t hit_index,
    uint32_t summary_index, const char **out_name, uint32_t *out_length,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_grouping_tree_output_hit_summary_value(
    const sk_grouping_tree_output *output, uint32_t hit_index,
    uint32_t summary_index, uint32_t *out_kind, const char **out_string,
    uint32_t *out_string_length, int64_t *out_integer, double *out_float,
    uint8_t *out_boolean, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API void sk_grouping_tree_result_destroy(
    sk_grouping_tree_result *result) SK_NOEXCEPT;

SK_API int32_t sk_local_grouping_request_create(
    const char *group_field, sk_local_grouping_request **out_request,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_local_grouping_request_add_aggregation(
    sk_local_grouping_request *request, uint32_t kind, const char *field,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API void sk_local_grouping_request_destroy(sk_local_grouping_request *request) SK_NOEXCEPT;
SK_API int32_t sk_engine_group_local(
    const sk_engine *engine, const sk_query *query,
    const sk_local_grouping_request *request,
    sk_local_grouping_result **out_result, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API uint32_t sk_local_grouping_result_group_count(
    const sk_local_grouping_result *result) SK_NOEXCEPT;
SK_API int32_t sk_local_grouping_result_group(
    const sk_local_grouping_result *result, uint32_t group_index,
    uint32_t *out_key_kind, const char **out_key_string,
    uint32_t *out_key_string_length, int64_t *out_key_integer,
    uint64_t *out_count, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API uint32_t sk_local_grouping_result_aggregation_count(
    const sk_local_grouping_result *result, uint32_t group_index) SK_NOEXCEPT;
SK_API int32_t sk_local_grouping_result_aggregation(
    const sk_local_grouping_result *result, uint32_t group_index,
    uint32_t aggregation_index, uint32_t *out_kind, int64_t *out_integer,
    double *out_float, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API void sk_local_grouping_result_destroy(sk_local_grouping_result *result) SK_NOEXCEPT;

SK_API int32_t sk_generic_grouping_request_create(
    const char *group_field, uint32_t max_groups, uint32_t order,
    sk_generic_grouping_request **out_request, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_generic_grouping_request_add_aggregation(
    sk_generic_grouping_request *request, uint32_t kind, const char *field,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API void sk_generic_grouping_request_destroy(sk_generic_grouping_request *request) SK_NOEXCEPT;
SK_API int32_t sk_engine_group_generic(
    const sk_engine *engine, const sk_query *query,
    const sk_generic_grouping_request *request,
    sk_generic_grouping_result **out_result, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API uint32_t sk_generic_grouping_result_group_count(
    const sk_generic_grouping_result *result) SK_NOEXCEPT;
SK_API int32_t sk_generic_grouping_result_group(
    const sk_generic_grouping_result *result, uint32_t group_index,
    uint32_t *out_key_kind, const char **out_key_string,
    uint32_t *out_key_string_length, int64_t *out_key_integer,
    uint64_t *out_count, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API uint32_t sk_generic_grouping_result_aggregation_count(
    const sk_generic_grouping_result *result, uint32_t group_index) SK_NOEXCEPT;
SK_API int32_t sk_generic_grouping_result_aggregation(
    const sk_generic_grouping_result *result, uint32_t group_index,
    uint32_t aggregation_index, uint32_t *out_kind, int64_t *out_integer,
    double *out_float, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API void sk_generic_grouping_result_destroy(sk_generic_grouping_result *result) SK_NOEXCEPT;

#ifdef __cplusplus
}
#endif

#endif
