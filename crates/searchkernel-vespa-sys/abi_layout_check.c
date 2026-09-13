#include <stddef.h>
#include <stdint.h>

#include "searchkernel_vespa.h"

_Static_assert(sizeof(((sk_error *)0)->code) == sizeof(int32_t),
               "sk_error.code must be int32_t");
_Static_assert(offsetof(sk_error, code) == 0,
               "sk_error.code offset changed");
_Static_assert(offsetof(sk_error, message) == sizeof(int32_t),
               "sk_error.message offset changed");
_Static_assert(sizeof(((sk_error *)0)->message) == 512,
               "sk_error.message capacity changed");
_Static_assert(sizeof(sk_error) == sizeof(int32_t) + 512,
               "sk_error has unexpected padding");

_Static_assert(offsetof(sk_text_occurrence, term) == 0,
               "sk_text_occurrence.term offset changed");
_Static_assert(sizeof(sk_text_occurrence) == 24,
               "sk_text_occurrence size changed");
_Static_assert(_Alignof(sk_text_occurrence) == 8,
               "sk_text_occurrence alignment changed");
_Static_assert(sizeof(((sk_text_occurrence *)0)->term) == 8,
               "sk_text_occurrence.term width changed");
_Static_assert(sizeof(((sk_text_occurrence *)0)->term_length) == sizeof(uint32_t),
               "sk_text_occurrence.term_length width changed");
_Static_assert(sizeof(((sk_text_occurrence *)0)->byte_start) == sizeof(uint32_t),
               "sk_text_occurrence.byte_start width changed");
_Static_assert(sizeof(((sk_text_occurrence *)0)->byte_length) == sizeof(uint32_t),
               "sk_text_occurrence.byte_length width changed");
_Static_assert(sizeof(((sk_text_occurrence *)0)->position) == sizeof(uint32_t),
               "sk_text_occurrence.position width changed");
_Static_assert(offsetof(sk_text_occurrence, term_length) == 8,
               "sk_text_occurrence.term_length offset changed");
_Static_assert(offsetof(sk_text_occurrence, byte_start) == 12,
               "sk_text_occurrence.byte_start offset changed");
_Static_assert(offsetof(sk_text_occurrence, byte_length) == 16,
               "sk_text_occurrence.byte_length offset changed");
_Static_assert(offsetof(sk_text_occurrence, position) == 20,
               "sk_text_occurrence.position offset changed");

_Static_assert(offsetof(sk_text_query_term, term) == 0,
               "sk_text_query_term.term offset changed");
_Static_assert(sizeof(sk_text_query_term) == 16,
               "sk_text_query_term size changed");
_Static_assert(_Alignof(sk_text_query_term) == 8,
               "sk_text_query_term alignment changed");
_Static_assert(sizeof(((sk_text_query_term *)0)->term) == 8,
               "sk_text_query_term.term width changed");
_Static_assert(sizeof(((sk_text_query_term *)0)->term_length) == sizeof(uint32_t),
               "sk_text_query_term.term_length width changed");
_Static_assert(sizeof(((sk_text_query_term *)0)->position) == sizeof(uint32_t),
               "sk_text_query_term.position width changed");
_Static_assert(offsetof(sk_text_query_term, term_length) == 8,
               "sk_text_query_term.term_length offset changed");
_Static_assert(offsetof(sk_text_query_term, position) == 12,
               "sk_text_query_term.position offset changed");

typedef uint32_t (*sk_native_abi_version_signature)(void);
typedef int32_t (*sk_engine_open_signature)(const char *, const char *, sk_engine **, sk_error *);
typedef int32_t (*sk_engine_close_signature)(sk_engine *, sk_error *);
typedef void (*sk_engine_destroy_signature)(sk_engine *);
typedef int32_t (*sk_document_create_signature)(const char *, const char *, int32_t,
                                                sk_document **, sk_error *);
typedef void (*sk_document_destroy_signature)(sk_document *);
typedef int32_t (*sk_document_update_create_signature)(const char *, const char *, int32_t,
                                                       sk_document_update **, sk_error *);
typedef void (*sk_document_update_destroy_signature)(sk_document_update *);
typedef int32_t (*sk_search_request_create_signature)(const char *, int32_t, uint32_t,
                                                      sk_search_request **, sk_error *);
typedef void (*sk_search_request_destroy_signature)(sk_search_request *);
typedef int32_t (*sk_engine_put_signature)(sk_engine *, const sk_document *, sk_error *);
typedef int32_t (*sk_engine_update_signature)(sk_engine *, const sk_document_update *, sk_error *);
typedef int32_t (*sk_engine_remove_signature)(sk_engine *, const char *, int32_t *, sk_error *);
typedef int32_t (*sk_engine_search_signature)(const sk_engine *, const sk_search_request *,
                                              sk_search_result **, sk_error *);
typedef uint32_t (*sk_search_result_size_signature)(const sk_search_result *);
typedef int32_t (*sk_search_result_id_signature)(const sk_search_result *, uint32_t, const char **,
                                                 uint32_t *, sk_error *);
typedef void (*sk_search_result_destroy_signature)(sk_search_result *);
typedef int32_t (*sk_text_document_create_signature)(const char *, const char *, const uint8_t *,
                                                     uint32_t, const sk_text_occurrence *, uint32_t,
                                                     sk_text_document **, sk_error *);
typedef void (*sk_text_document_destroy_signature)(sk_text_document *);
typedef int32_t (*sk_text_update_create_signature)(const char *, const char *, const uint8_t *,
                                                   uint32_t, const sk_text_occurrence *, uint32_t,
                                                   sk_text_update **, sk_error *);
typedef void (*sk_text_update_destroy_signature)(sk_text_update *);
typedef int32_t (*sk_text_request_create_signature)(uint32_t, const char *,
                                                    const sk_text_query_term *, uint32_t,
                                                    const char *, sk_text_request **, sk_error *);
typedef void (*sk_text_request_destroy_signature)(sk_text_request *);
typedef int32_t (*sk_engine_put_text_signature)(sk_engine *, const sk_text_document *, sk_error *);
typedef int32_t (*sk_engine_update_text_signature)(sk_engine *, const sk_text_update *, sk_error *);
typedef int32_t (*sk_engine_search_text_signature)(const sk_engine *, const sk_text_request *,
                                                   sk_text_result **, sk_error *);
typedef uint32_t (*sk_text_result_size_signature)(const sk_text_result *);
typedef int32_t (*sk_text_result_hit_signature)(const sk_text_result *, uint32_t, const char **,
                                                uint32_t *, double *, sk_error *);
typedef void (*sk_text_result_destroy_signature)(sk_text_result *);

#define SK_ASSERT_SIGNATURE(function, signature) \
    _Static_assert(_Generic(&(function), signature: 1, default: 0), #function " signature changed")

SK_ASSERT_SIGNATURE(sk_native_abi_version, sk_native_abi_version_signature);
SK_ASSERT_SIGNATURE(sk_engine_open, sk_engine_open_signature);
SK_ASSERT_SIGNATURE(sk_engine_close, sk_engine_close_signature);
SK_ASSERT_SIGNATURE(sk_engine_destroy, sk_engine_destroy_signature);
SK_ASSERT_SIGNATURE(sk_document_create, sk_document_create_signature);
SK_ASSERT_SIGNATURE(sk_document_destroy, sk_document_destroy_signature);
SK_ASSERT_SIGNATURE(sk_document_update_create, sk_document_update_create_signature);
SK_ASSERT_SIGNATURE(sk_document_update_destroy, sk_document_update_destroy_signature);
SK_ASSERT_SIGNATURE(sk_search_request_create, sk_search_request_create_signature);
SK_ASSERT_SIGNATURE(sk_search_request_destroy, sk_search_request_destroy_signature);
SK_ASSERT_SIGNATURE(sk_engine_put, sk_engine_put_signature);
SK_ASSERT_SIGNATURE(sk_engine_update, sk_engine_update_signature);
SK_ASSERT_SIGNATURE(sk_engine_remove, sk_engine_remove_signature);
SK_ASSERT_SIGNATURE(sk_engine_search, sk_engine_search_signature);
SK_ASSERT_SIGNATURE(sk_search_result_size, sk_search_result_size_signature);
SK_ASSERT_SIGNATURE(sk_search_result_id, sk_search_result_id_signature);
SK_ASSERT_SIGNATURE(sk_search_result_destroy, sk_search_result_destroy_signature);
SK_ASSERT_SIGNATURE(sk_text_document_create, sk_text_document_create_signature);
SK_ASSERT_SIGNATURE(sk_text_document_destroy, sk_text_document_destroy_signature);
SK_ASSERT_SIGNATURE(sk_text_update_create, sk_text_update_create_signature);
SK_ASSERT_SIGNATURE(sk_text_update_destroy, sk_text_update_destroy_signature);
SK_ASSERT_SIGNATURE(sk_text_request_create, sk_text_request_create_signature);
SK_ASSERT_SIGNATURE(sk_text_request_destroy, sk_text_request_destroy_signature);
SK_ASSERT_SIGNATURE(sk_engine_put_text, sk_engine_put_text_signature);
SK_ASSERT_SIGNATURE(sk_engine_update_text, sk_engine_update_text_signature);
SK_ASSERT_SIGNATURE(sk_engine_search_text, sk_engine_search_text_signature);
SK_ASSERT_SIGNATURE(sk_text_result_size, sk_text_result_size_signature);
SK_ASSERT_SIGNATURE(sk_text_result_hit, sk_text_result_hit_signature);
SK_ASSERT_SIGNATURE(sk_text_result_destroy, sk_text_result_destroy_signature);

#undef SK_ASSERT_SIGNATURE
