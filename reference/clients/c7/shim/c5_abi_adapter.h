#ifndef SEARCHKERNEL_C7_C5_ABI_ADAPTER_H
#define SEARCHKERNEL_C7_C5_ABI_ADAPTER_H

/*
 * C7 verifier-only C5 ABI adapter.
 *
 * IMPORTANT: reconcile this file against the frozen C5 public header before
 * committing the C7 verifier specification. Do not copy these spellings into
 * Python/Go verifier code.
 *
 * If the real C5 header is available and stable, replace this compatibility
 * declaration with a direct include of that header.
 */

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct sk_handle sk_handle;

typedef struct sk_buffer {
    uint8_t *data;
    size_t len;
} sk_buffer;

/* Canonical logical C5 surface expected by C7. */
uint32_t sk_abi_version(void);
int32_t sk_open(const uint8_t *payload, size_t payload_len, sk_handle **out_handle);
int32_t sk_call(sk_handle *handle,
                const uint8_t *request,
                size_t request_len,
                sk_buffer *out_response);
void sk_buffer_free(sk_buffer *buffer);
void sk_close(sk_handle *handle);

#ifdef __cplusplus
}
#endif

#endif
