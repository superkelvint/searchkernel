#ifndef SEARCHKERNEL_VESPA_TEST_H
#define SEARCHKERNEL_VESPA_TEST_H

#include "searchkernel_vespa.h"

#ifdef __cplusplus
extern "C" {
#endif

#define SK_TEST_FAILPOINT_ERROR 1U
#define SK_TEST_FAILPOINT_STD_EXCEPTION 2U
#define SK_TEST_FAILPOINT_BAD_ALLOC 3U
#define SK_TEST_FAILPOINT_UNKNOWN_EXCEPTION 4U
#define SK_TEST_FAILPOINT_BLOCK 5U

/* These declarations are linked only by the hardening SDK. */
SK_API int32_t sk_test_failpoint_set(sk_engine *engine, const char *name,
                                     uint32_t hit, uint32_t action,
                                     uint8_t *out_error,
                                     uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_test_failpoint_set_open(const char *name, uint32_t hit,
                                          uint32_t action, uint8_t *out_error,
                                          uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_test_failpoint_clear(sk_engine *engine, const char *name,
                                       uint8_t *out_error,
                                       uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_test_failpoint_clear_open(const char *name,
                                            uint8_t *out_error,
                                            uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_test_failpoint_release(sk_engine *engine, const char *name,
                                         uint8_t *out_error,
                                         uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_test_failpoint_is_blocked(const sk_engine *engine,
                                            const char *name, uint8_t *out_blocked,
                                            uint8_t *out_error,
                                            uint32_t out_error_capacity) SK_NOEXCEPT;

#ifdef __cplusplus
}
#endif

#endif
