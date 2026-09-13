#ifndef SEARCHKERNEL_NATIVE_ABI_CONTRACT_H
#define SEARCHKERNEL_NATIVE_ABI_CONTRACT_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct sk_engine sk_engine;

typedef struct sk_error {
    int32_t code;
    char message[512];
} sk_error;

typedef struct sk_document sk_document;
typedef struct sk_document_update sk_document_update;
typedef struct sk_search_request sk_search_request;
typedef struct sk_search_result sk_search_result;

uint32_t sk_native_abi_version(void);

int32_t sk_engine_open(
    const char *data_dir,
    const char *config_dir,
    sk_engine **out_engine,
    sk_error *out_error);

void sk_engine_destroy(sk_engine *engine);

/* M2 operations. The opaque request/document constructors may remain private to
 * the Rust sys crate, but these operation symbols are required so the verifier
 * can prove the final Rust executable references the real native path. */
int32_t sk_engine_put(
    sk_engine *engine,
    const sk_document *document,
    sk_error *out_error);

int32_t sk_engine_update(
    sk_engine *engine,
    const sk_document_update *update,
    sk_error *out_error);

int32_t sk_engine_remove(
    sk_engine *engine,
    const char *document_id,
    int32_t *out_was_found,
    sk_error *out_error);

int32_t sk_engine_search(
    const sk_engine *engine,
    const sk_search_request *request,
    sk_search_result **out_result,
    sk_error *out_error);

#ifdef __cplusplus
}
#endif

#endif
