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

#define SK_STATUS_OK 0
#define SK_STATUS_INVALID_ARGUMENT 1
#define SK_STATUS_INTERNAL 2
#define SK_STATUS_OUT_OF_MEMORY 3

#define SK_TEXT_QUERY_TERM 1U
#define SK_TEXT_QUERY_PHRASE 2U

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

/*
 * Diagnostics are caller-owned bytes rather than a public error struct. The
 * return value is the sole status authority. This avoids exposing struct
 * layout, allocator ownership, initialization rules, and a possible
 * allocation failure while reporting another failure. When capacity is
 * non-zero, implementations write a truncated NUL-terminated diagnostic.
 */
#define SK_ERROR_MESSAGE_CAPACITY 512U

/* This ABI revision reflects removal of all public aggregate structs. */
#define SK_NATIVE_ABI_VERSION 2U

SK_API uint32_t sk_native_abi_version(void) SK_NOEXCEPT;

SK_API int32_t sk_engine_open(const char *data_dir, const char *config_dir,
                              sk_engine **out_engine, uint8_t *out_error,
                              uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_engine_close(sk_engine *engine, uint8_t *out_error,
                               uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API void sk_engine_destroy(sk_engine *engine) SK_NOEXCEPT;

SK_API int32_t sk_document_create(const char *document_id, const char *field,
                                  int32_t value, sk_document **out_document,
                                  uint8_t *out_error,
                                  uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API void sk_document_destroy(sk_document *document) SK_NOEXCEPT;

SK_API int32_t sk_document_update_create(const char *document_id, const char *field,
                                         int32_t value,
                                         sk_document_update **out_update,
                                         uint8_t *out_error,
                                         uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API void sk_document_update_destroy(sk_document_update *update) SK_NOEXCEPT;

SK_API int32_t sk_search_request_create(const char *field, int32_t value,
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

#ifdef __cplusplus
}
#endif

#endif
