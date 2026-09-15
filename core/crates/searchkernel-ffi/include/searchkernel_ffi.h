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

#define SK_STATUS_ALREADY_EXISTS 8

#define SK_STATUS_NOT_FOUND 9

#define SK_STATUS_CORRUPT_INDEX 10

#define SK_STATUS_INCOMPATIBLE_INDEX 11

typedef uint32_t sk_status_t;

typedef uint64_t sk_handle_t;

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

uint32_t sk_abi_version(void);

/**
 * # Safety
 *
 * `out_data`, `out_len`, and `out_allocation_id` must point to writable
 * storage. They are write-only out-parameters on entry, so their previous
 * contents are not read.
 * The caller must release any response previously held in that storage before
 * reusing it, and must not access the storage concurrently with this call.
 * `request` must point to `request_len` readable, unmodified bytes for the
 * entire duration of this call when `request_len` is non-zero.
 */
sk_status_t sk_call(sk_handle_t handle,
                    const uint8_t *request,
                    size_t request_len,
                    uint8_t **out_data,
                    size_t *out_len,
                    uint64_t *out_allocation_id);

sk_status_t sk_close(sk_handle_t handle);

/**
 * Create an on-disk index from its canonical FlatBuffers definition.
 *
 * # Safety
 *
 * `path` must be a valid NUL-terminated C string. `definition` must point to
 * `definition_len` readable bytes, and `out_handle` must point to writable
 * storage for one handle. The optional error buffer must be writable for its
 * declared capacity.
 */
sk_status_t sk_create(const char *path,
                      const uint8_t *definition,
                      size_t definition_len,
                      sk_handle_t *out_handle,
                      uint8_t *error,
                      size_t error_capacity);

/**
 * Reopen an existing index using its durable manifest.
 *
 * # Safety
 *
 * `path` must be a valid NUL-terminated C string and `out_handle` must point
 * to writable storage for one handle. The optional error buffer must be
 * writable for its declared capacity.
 */
sk_status_t sk_open(const char *path,
                    sk_handle_t *out_handle,
                    uint8_t *error,
                    size_t error_capacity);

/**
 * # Safety
 *
 * `data` must be null or a pointer previously returned through `sk_call`,
 * and `allocation_id` must be the matching token. Passing a stale or already
 * released pair is harmless. The caller must not access the response bytes
 * after releasing them.
 */
void sk_buffer_free(uint8_t *data, uint64_t allocation_id);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* SEARCHKERNEL_FFI_H */
