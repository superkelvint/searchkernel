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

#define SK_STATUS_OK 0
#define SK_STATUS_INVALID_ARGUMENT 1
#define SK_STATUS_INTERNAL 2
#define SK_STATUS_OUT_OF_MEMORY 3

typedef struct sk_engine sk_engine;
typedef struct sk_document sk_document;
typedef struct sk_document_update sk_document_update;
typedef struct sk_search_request sk_search_request;
typedef struct sk_search_result sk_search_result;
typedef struct sk_text_document sk_text_document;
typedef struct sk_text_update sk_text_update;
typedef struct sk_text_request sk_text_request;
typedef struct sk_text_result sk_text_result;

typedef struct sk_text_occurrence {
    const uint8_t *term;
    uint32_t term_length;
    uint32_t byte_start;
    uint32_t byte_length;
    uint32_t position;
} sk_text_occurrence;

typedef struct sk_text_query_term {
    const uint8_t *term;
    uint32_t term_length;
    uint32_t position;
} sk_text_query_term;

typedef struct sk_error {
    /* Initialize to {0, NULL} before the first call. The caller owns message
       and must call sk_error_destroy before reusing or discarding the value. */
    int32_t code;
    char *message;
} sk_error;

#define SK_NATIVE_ABI_VERSION 1U

SK_API uint32_t sk_native_abi_version(void);

SK_API int32_t sk_engine_open(const char *data_dir, const char *config_dir,
                              sk_engine **out_engine, sk_error *out_error);
SK_API int32_t sk_engine_close(sk_engine *engine, sk_error *out_error);
SK_API void sk_engine_destroy(sk_engine *engine);
SK_API void sk_error_destroy(sk_error *error);

SK_API int32_t sk_document_create(const char *document_id, const char *field,
                                  int32_t value, sk_document **out_document,
                                  sk_error *out_error);
SK_API void sk_document_destroy(sk_document *document);

SK_API int32_t sk_document_update_create(const char *document_id, const char *field,
                                         int32_t value,
                                         sk_document_update **out_update,
                                         sk_error *out_error);
SK_API void sk_document_update_destroy(sk_document_update *update);

SK_API int32_t sk_search_request_create(const char *field, int32_t value,
                                        uint32_t limit,
                                        sk_search_request **out_request,
                                        sk_error *out_error);
SK_API void sk_search_request_destroy(sk_search_request *request);

SK_API int32_t sk_engine_put(sk_engine *engine, const sk_document *document,
                             sk_error *out_error);
SK_API int32_t sk_engine_update(sk_engine *engine,
                                const sk_document_update *update,
                                sk_error *out_error);
SK_API int32_t sk_engine_remove(sk_engine *engine, const char *document_id,
                                int32_t *out_was_found, sk_error *out_error);
SK_API int32_t sk_engine_search(const sk_engine *engine,
                                const sk_search_request *request,
                                sk_search_result **out_result, sk_error *out_error);

SK_API uint32_t sk_search_result_size(const sk_search_result *result);
SK_API int32_t sk_search_result_id(const sk_search_result *result, uint32_t index,
                                   const char **out_id, uint32_t *out_length,
                                   sk_error *out_error);
SK_API void sk_search_result_destroy(sk_search_result *result);

SK_API int32_t sk_text_document_create(const char *document_id, const char *field,
                                       const uint8_t *text, uint32_t text_length,
                                       const sk_text_occurrence *occurrences,
                                       uint32_t occurrence_count,
                                       sk_text_document **out_document,
                                       sk_error *out_error);
SK_API void sk_text_document_destroy(sk_text_document *document);

SK_API int32_t sk_text_update_create(const char *document_id, const char *field,
                                     const uint8_t *text, uint32_t text_length,
                                     const sk_text_occurrence *occurrences,
                                     uint32_t occurrence_count,
                                     sk_text_update **out_update,
                                     sk_error *out_error);
SK_API void sk_text_update_destroy(sk_text_update *update);

SK_API int32_t sk_text_request_create(uint32_t kind, const char *field,
                                      const sk_text_query_term *terms,
                                      uint32_t term_count, const char *rank_profile,
                                      sk_text_request **out_request,
                                      sk_error *out_error);
SK_API void sk_text_request_destroy(sk_text_request *request);

SK_API int32_t sk_engine_put_text(sk_engine *engine,
                                  const sk_text_document *document,
                                  sk_error *out_error);
SK_API int32_t sk_engine_update_text(sk_engine *engine,
                                     const sk_text_update *update,
                                     sk_error *out_error);
SK_API int32_t sk_engine_search_text(const sk_engine *engine,
                                     const sk_text_request *request,
                                     sk_text_result **out_result,
                                     sk_error *out_error);

SK_API uint32_t sk_text_result_size(const sk_text_result *result);
SK_API int32_t sk_text_result_hit(const sk_text_result *result, uint32_t index,
                                  const char **out_id, uint32_t *out_length,
                                  double *out_score, sk_error *out_error);
SK_API void sk_text_result_destroy(sk_text_result *result);

#ifdef __cplusplus
}
#endif

#endif
