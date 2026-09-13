#ifndef SEARCHKERNEL_FFI_H
#define SEARCHKERNEL_FFI_H

#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

#define SK_ABI_VERSION 1

#define SK_STATUS_OK 0

#define SK_STATUS_INVALID_ARGUMENT 1

#define SK_STATUS_INVALID_HANDLE 2

#define SK_STATUS_INVALID_REQUEST 3

#define SK_STATUS_UNSUPPORTED_PROTOCOL 4

#define SK_STATUS_UNSUPPORTED_OPERATION 5

#define SK_STATUS_INTERNAL 6

#define SK_STATUS_PANIC 7

typedef uint32_t sk_status_t;

typedef uint64_t sk_handle_t;

typedef struct sk_buffer_t {
  /**
   * Pointer to the response bytes, or null for an empty buffer.
   */
  uint8_t *data;
  /**
   * Number of response bytes visible to the caller.
   */
  size_t len;
  /**
   * Search Kernel's non-reused process-lifetime allocation token.
   */
  uint64_t allocation_id;
} sk_buffer_t;

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

uint32_t sk_abi_version(void);

/**
 * # Safety
 *
 * `out_response` must point to writable storage for a `SkBuffer`. It is a
 * write-only out-parameter on entry, so its previous contents are not read.
 * `request` must point to `request_len` readable bytes when `request_len` is
 * non-zero.
 */
sk_status_t sk_call(sk_handle_t handle,
                    const uint8_t *request,
                    size_t request_len,
                    struct sk_buffer_t *out_response);

sk_status_t sk_close(sk_handle_t handle);

/**
 * # Safety
 *
 * `buffer` must be null or point to a valid writable `SkBuffer`. If it is
 * non-empty, it must be a buffer previously returned by `sk_call`.
 */
void sk_buffer_free(struct sk_buffer_t *buffer);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* SEARCHKERNEL_FFI_H */
