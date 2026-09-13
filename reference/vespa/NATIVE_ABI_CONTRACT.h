#ifndef SEARCHKERNEL_NATIVE_ABI_CONTRACT_H
#define SEARCHKERNEL_NATIVE_ABI_CONTRACT_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ABI v2 uses only opaque handles and fixed-width primitive arguments. */
typedef struct sk_engine sk_engine;

#define SK_NATIVE_ABI_VERSION 2U

uint32_t sk_native_abi_version(void);

int32_t sk_engine_open(
    const char *data_dir,
    const char *config_dir,
    sk_engine **out_engine,
    uint8_t *out_error,
    uint32_t out_error_capacity);

void sk_engine_destroy(sk_engine *engine);

/* M2 operations. The opaque request/document constructors may remain private
 * to the Rust sys crate, but these operation symbols are required so the
 * verifier can prove the final Rust executable references the real native
 * path. */
typedef struct sk_document sk_document;
typedef struct sk_document_update sk_document_update;
typedef struct sk_search_request sk_search_request;
typedef struct sk_search_result sk_search_result;

int32_t sk_engine_put(
    sk_engine *engine,
    const sk_document *document,
    uint8_t *out_error,
    uint32_t out_error_capacity);

int32_t sk_engine_update(
    sk_engine *engine,
    const sk_document_update *update,
    uint8_t *out_error,
    uint32_t out_error_capacity);

int32_t sk_engine_remove(
    sk_engine *engine,
    const char *document_id,
    int32_t *out_was_found,
    uint8_t *out_error,
    uint32_t out_error_capacity);

int32_t sk_engine_search(
    const sk_engine *engine,
    const sk_search_request *request,
    sk_search_result **out_result,
    uint8_t *out_error,
    uint32_t out_error_capacity);

#ifdef __cplusplus
}
#endif

#endif
