#ifndef SEARCHKERNEL_C7_C5_ABI_ADAPTER_H
#define SEARCHKERNEL_C7_C5_ABI_ADAPTER_H

/* Verifier-local spelling of the already-frozen primitive C5 ABI. */

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef uint64_t sk_handle_t;
typedef uint32_t sk_status_t;

#define SK_STATUS_OK 0u
#define SK_STATUS_INVALID_ARGUMENT 1u
#define SK_STATUS_INVALID_HANDLE 2u

uint32_t sk_abi_version(void);
sk_status_t sk_open(const char *path,
                    sk_handle_t *out_handle,
                    uint8_t *error,
                    size_t error_capacity);
sk_status_t sk_call(sk_handle_t handle,
                    const uint8_t *request,
                    size_t request_len,
                    uint8_t **out_data,
                    size_t *out_len,
                    uint64_t *out_allocation_id);
sk_status_t sk_close(sk_handle_t handle, uint8_t *error, size_t error_capacity);
void sk_buffer_free(uint8_t *data, uint64_t allocation_id);

#ifdef __cplusplus
}
#endif

#endif
