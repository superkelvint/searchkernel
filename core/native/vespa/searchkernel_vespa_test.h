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

/* One-shot malformed-result controls used by Rust decoder tests. */
#define SK_TEST_RESULT_FAULT_NULL_RESULT 1U
#define SK_TEST_RESULT_FAULT_NULL_DOCUMENT_FIELD_NAME 2U
#define SK_TEST_RESULT_FAULT_INVALID_DOCUMENT_FIELD_NAME 3U
#define SK_TEST_RESULT_FAULT_NULL_DOCUMENT_STRING 4U
#define SK_TEST_RESULT_FAULT_INVALID_DOCUMENT_STRING 5U
#define SK_TEST_RESULT_FAULT_UNKNOWN_DOCUMENT_KIND 6U
#define SK_TEST_RESULT_FAULT_DOCUMENT_ACCESSOR_ERROR 7U
#define SK_TEST_RESULT_FAULT_NULL_HIT_ID 8U
#define SK_TEST_RESULT_FAULT_INVALID_HIT_ID 9U
#define SK_TEST_RESULT_FAULT_NULL_SUMMARY_NAME 10U
#define SK_TEST_RESULT_FAULT_INVALID_SUMMARY_NAME 11U
#define SK_TEST_RESULT_FAULT_NULL_SUMMARY_STRING 12U
#define SK_TEST_RESULT_FAULT_INVALID_SUMMARY_STRING 13U
#define SK_TEST_RESULT_FAULT_UNKNOWN_SUMMARY_KIND 14U
#define SK_TEST_RESULT_FAULT_NULL_MATCH_FEATURE_NAME 15U
#define SK_TEST_RESULT_FAULT_INVALID_MATCH_FEATURE_NAME 16U
#define SK_TEST_RESULT_FAULT_DUPLICATE_MATCH_FEATURE_NAME 17U
#define SK_TEST_RESULT_FAULT_MATCH_FEATURE_VALUE_ERROR 18U
#define SK_TEST_RESULT_FAULT_UNSUPPORTED_GROUP_KEY 19U
#define SK_TEST_RESULT_FAULT_INVALID_GROUP_KEY 20U
#define SK_TEST_RESULT_FAULT_GROUP_ACCESSOR_ERROR 21U
#define SK_TEST_RESULT_FAULT_UNKNOWN_AGGREGATION_KIND 22U
#define SK_TEST_RESULT_FAULT_AGGREGATION_ACCESSOR_ERROR 23U

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
SK_API int32_t sk_test_result_fault_set(sk_engine *engine, uint32_t fault,
                                        uint8_t *out_error,
                                        uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_test_result_fault_clear(sk_engine *engine, uint8_t *out_error,
                                          uint32_t out_error_capacity) SK_NOEXCEPT;
SK_API int32_t sk_test_result_destroy_count(const sk_engine *engine,
                                            uint64_t *out_count,
                                            uint8_t *out_error,
                                            uint32_t out_error_capacity) SK_NOEXCEPT;

#ifdef __cplusplus
}
#endif

#endif
