#ifndef SEARCHKERNEL_VESPA_H
#define SEARCHKERNEL_VESPA_H

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

#define SK_TEXT_QUERY_TERM 1U
#define SK_TEXT_QUERY_PHRASE 2U

#define SK_QUERY_NODE_AND 1U
#define SK_QUERY_NODE_OR 2U
#define SK_QUERY_NODE_NOT 3U
#define SK_QUERY_NODE_TRUE 4U
#define SK_QUERY_TEXT_TERM 5U
#define SK_QUERY_TEXT_PHRASE 6U
#define SK_QUERY_BOUND_UNBOUNDED 0U
#define SK_QUERY_BOUND_INCLUDED 1U
#define SK_QUERY_BOUND_EXCLUDED 2U
#define SK_QUERY_FIELD_TEXT 1U
#define SK_QUERY_FIELD_INTEGER 2U
#define SK_QUERY_FIELD_STRING 3U
#define SK_QUERY_FIELD_FLOAT 4U
#define SK_QUERY_FIELD_BOOLEAN 5U
#define SK_QUERY_FIELD_ATTRIBUTE 6U

#define SK_VALUE_STRING 1U
#define SK_VALUE_INTEGER 2U
#define SK_VALUE_FLOAT 3U
#define SK_VALUE_BOOLEAN 4U

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
#define SK_GROUP_ORDER_NONE 0U
#define SK_GROUP_ORDER_COUNT_ASC 1U
#define SK_GROUP_ORDER_COUNT_DESC 2U
#define SK_GROUP_ORDER_VALUE_ASC 3U
#define SK_GROUP_ORDER_VALUE_DESC 4U

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
typedef struct sk_document_update sk_document_update;
typedef struct sk_search_request sk_search_request;
typedef struct sk_search_result sk_search_result;
typedef struct sk_text_document sk_text_document;
typedef struct sk_text_update sk_text_update;
typedef struct sk_text_request sk_text_request;
typedef struct sk_text_result sk_text_result;
typedef struct sk_query sk_query;
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

/*
 * Diagnostics are caller-owned bytes rather than a public error struct. The
 * return value is the sole status authority. This avoids exposing struct
 * layout, allocator ownership, initialization rules, and a possible
 * allocation failure while reporting another failure. When capacity is
 * non-zero, implementations write a truncated NUL-terminated diagnostic.
 */
#define SK_ERROR_MESSAGE_CAPACITY 512U

/* This ABI revision reflects removal of all public aggregate structs. */
#define SK_NATIVE_ABI_VERSION 5U

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
SK_API int32_t sk_document_annotate_text(
    sk_document *document, const char *field, const uint8_t *term_bytes,
    uint32_t term_bytes_length, const uint32_t *term_lengths,
    const uint32_t *byte_starts, const uint32_t *byte_lengths,
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
SK_API int32_t sk_document_update_clear(sk_document_update *update, const char *field,
                                        uint8_t *out_error,
                                        uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_document_update_annotate_text(
    sk_document_update *update, const char *field, const uint8_t *term_bytes,
    uint32_t term_bytes_length, const uint32_t *term_lengths,
    const uint32_t *byte_starts, const uint32_t *byte_lengths,
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
SK_API int32_t sk_document_field(const sk_document *document, uint32_t index,
                                 const char **out_name, uint32_t *out_name_length,
                                 uint32_t *out_kind, const char **out_string,
                                 uint32_t *out_string_length, int64_t *out_integer,
                                 double *out_float, uint8_t *out_boolean,
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
SK_API int32_t sk_query_add_equals(sk_query *query, const char *field,
                                   int64_t value, uint8_t *out_error,
                                   uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_query_add_range(sk_query *query, const char *field,
                                  uint32_t lower_kind, int64_t lower_value,
                                  uint32_t upper_kind, int64_t upper_value,
                                  uint8_t *out_error,
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
SK_API void sk_local_search_result_destroy(sk_local_search_result *result) SK_NOEXCEPT;

/* Generic M-layer search request/result. The older local request remains a
   frozen milestone surface; new generic consumers use these handles. */
SK_API int32_t sk_generic_search_request_create(
    const char *rank_profile, uint32_t limit, uint32_t offset,
    uint32_t summary_kind, sk_generic_search_request **out_request,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_generic_search_request_add_sort(
    sk_generic_search_request *request, const char *field, uint32_t order,
    uint32_t missing, uint8_t *out_error,
    uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_generic_search_request_add_summary_field(
    sk_generic_search_request *request, const char *field,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_generic_search_request_add_query_feature(
    sk_generic_search_request *request, const char *name, double value,
    uint8_t *out_error, uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_generic_search_request_add_rank_property(
    sk_generic_search_request *request, const char *name, const char *value,
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
