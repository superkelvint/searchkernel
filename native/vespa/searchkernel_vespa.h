#ifndef SEARCHKERNEL_VESPA_H
#define SEARCHKERNEL_VESPA_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct sk_engine sk_engine;
typedef struct sk_document sk_document;
typedef struct sk_document_update sk_document_update;
typedef struct sk_search_request sk_search_request;
typedef struct sk_search_result sk_search_result;

typedef struct sk_error {
    /* Initialize to {0, NULL} before the first call. The caller owns message
       and must call sk_error_destroy before reusing or discarding the value. */
    int32_t code;
    char *message;
} sk_error;

uint32_t sk_native_abi_version(void);

int32_t sk_engine_open(const char *data_dir, const char *config_dir,
                       sk_engine **out_engine, sk_error *out_error);
void sk_engine_destroy(sk_engine *engine);
void sk_error_destroy(sk_error *error);

int32_t sk_document_create(const char *document_id, const char *field,
                           int32_t value, sk_document **out_document,
                           sk_error *out_error);
void sk_document_destroy(sk_document *document);

int32_t sk_document_update_create(const char *document_id, const char *field,
                                  int32_t value,
                                  sk_document_update **out_update,
                                  sk_error *out_error);
void sk_document_update_destroy(sk_document_update *update);

int32_t sk_search_request_create(const char *field, int32_t value,
                                 uint32_t limit,
                                 sk_search_request **out_request,
                                 sk_error *out_error);
void sk_search_request_destroy(sk_search_request *request);

int32_t sk_engine_put(sk_engine *engine, const sk_document *document,
                      sk_error *out_error);
int32_t sk_engine_update(sk_engine *engine,
                         const sk_document_update *update,
                         sk_error *out_error);
int32_t sk_engine_remove(sk_engine *engine, const char *document_id,
                         int32_t *out_was_found, sk_error *out_error);
int32_t sk_engine_search(const sk_engine *engine,
                         const sk_search_request *request,
                         sk_search_result **out_result, sk_error *out_error);

uint32_t sk_search_result_size(const sk_search_result *result);
int32_t sk_search_result_id(const sk_search_result *result, uint32_t index,
                            const char **out_id, uint32_t *out_length,
                            sk_error *out_error);
void sk_search_result_destroy(sk_search_result *result);

#ifdef __cplusplus
}
#endif

#endif
