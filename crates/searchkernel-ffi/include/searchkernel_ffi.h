#ifndef SEARCHKERNEL_FFI_H
#define SEARCHKERNEL_FFI_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define SK_ABI_VERSION 1u

typedef uint64_t sk_handle_t;
typedef uint32_t sk_status_t;

typedef struct sk_buffer {
    uint8_t *data;
    size_t len;
} sk_buffer_t;

enum {
    SK_STATUS_OK = 0u,
    SK_STATUS_INVALID_ARGUMENT = 1u,
    SK_STATUS_INVALID_HANDLE = 2u,
    SK_STATUS_INVALID_REQUEST = 3u,
    SK_STATUS_UNSUPPORTED_PROTOCOL = 4u,
    SK_STATUS_UNSUPPORTED_OPERATION = 5u,
    SK_STATUS_INTERNAL = 6u,
    SK_STATUS_PANIC = 7u
};

uint32_t sk_abi_version(void);

sk_status_t sk_call(
    sk_handle_t handle,
    const uint8_t *request,
    size_t request_len,
    sk_buffer_t *out_response
);

sk_status_t sk_close(sk_handle_t handle);

void sk_buffer_free(sk_buffer_t *buffer);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* SEARCHKERNEL_FFI_H */
