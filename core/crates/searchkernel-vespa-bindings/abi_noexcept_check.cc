#include <searchkernel_vespa.h>

static_assert(noexcept(sk_native_abi_version()));
static_assert(noexcept(sk_engine_open(nullptr, nullptr, nullptr, nullptr, 0)));
static_assert(noexcept(sk_engine_close(nullptr, nullptr, 0)));
static_assert(noexcept(sk_engine_destroy(nullptr)));
static_assert(noexcept(sk_document_create(nullptr, nullptr, 0, nullptr, nullptr, 0)));
static_assert(noexcept(sk_document_destroy(nullptr)));
static_assert(noexcept(sk_document_update_create(nullptr, nullptr, 0, nullptr, nullptr, 0)));
static_assert(noexcept(sk_document_update_destroy(nullptr)));
static_assert(noexcept(sk_search_request_create(nullptr, 0, 0, nullptr, nullptr, 0)));
static_assert(noexcept(sk_search_request_destroy(nullptr)));
static_assert(noexcept(sk_engine_put(nullptr, nullptr, nullptr, 0)));
static_assert(noexcept(sk_engine_update(nullptr, nullptr, nullptr, 0)));
static_assert(noexcept(sk_engine_remove(nullptr, nullptr, nullptr, nullptr, 0)));
static_assert(noexcept(sk_engine_search(nullptr, nullptr, nullptr, nullptr, 0)));
static_assert(noexcept(sk_search_result_size(nullptr)));
static_assert(noexcept(sk_search_result_id(nullptr, 0, nullptr, nullptr, nullptr, 0)));
static_assert(noexcept(sk_search_result_destroy(nullptr)));
static_assert(noexcept(sk_text_document_create(nullptr, nullptr, nullptr, 0, nullptr, nullptr, 0)));
static_assert(noexcept(sk_text_document_add_occurrences(nullptr, nullptr, 0, nullptr, nullptr, nullptr,
                                                         0, nullptr, 0)));
static_assert(noexcept(sk_text_document_destroy(nullptr)));
static_assert(noexcept(sk_text_update_create(nullptr, nullptr, nullptr, 0, nullptr, nullptr, 0)));
static_assert(noexcept(sk_text_update_add_occurrences(nullptr, nullptr, 0, nullptr, nullptr, nullptr,
                                                      0, nullptr, 0)));
static_assert(noexcept(sk_text_update_destroy(nullptr)));
static_assert(noexcept(sk_text_request_create(0, nullptr, nullptr, nullptr, nullptr, 0)));
static_assert(noexcept(sk_text_request_add_terms(nullptr, nullptr, 0, nullptr, 0, nullptr, 0)));
static_assert(noexcept(sk_text_request_destroy(nullptr)));
static_assert(noexcept(sk_engine_put_text(nullptr, nullptr, nullptr, 0)));
static_assert(noexcept(sk_engine_update_text(nullptr, nullptr, nullptr, 0)));
static_assert(noexcept(sk_engine_search_text(nullptr, nullptr, nullptr, nullptr, 0)));
static_assert(noexcept(sk_text_result_size(nullptr)));
static_assert(noexcept(sk_text_result_hit(nullptr, 0, nullptr, nullptr, nullptr, nullptr, 0)));
static_assert(noexcept(sk_text_result_destroy(nullptr)));
static_assert(noexcept(sk_local_search_request_add_query_feature(nullptr, nullptr, 0.0, nullptr, 0)));
static_assert(noexcept(sk_local_search_request_add_rank_property(nullptr, nullptr, nullptr, nullptr, 0)));
static_assert(noexcept(sk_local_search_result_match_feature_count(nullptr)));
static_assert(noexcept(sk_local_search_result_match_feature_name(nullptr, 0, nullptr, nullptr, nullptr, 0)));
static_assert(noexcept(sk_local_search_result_match_feature_value(nullptr, 0, 0, nullptr, nullptr, 0)));
static_assert(noexcept(sk_generic_search_request_add_query_feature(nullptr, nullptr, 0.0, nullptr, 0)));
static_assert(noexcept(sk_generic_search_request_add_rank_property(nullptr, nullptr, nullptr, nullptr, 0)));
static_assert(noexcept(sk_generic_search_result_match_feature_count(nullptr)));
static_assert(noexcept(sk_generic_search_result_match_feature_name(nullptr, 0, nullptr, nullptr, nullptr, 0)));
static_assert(noexcept(sk_generic_search_result_match_feature_value(nullptr, 0, 0, nullptr, nullptr, 0)));
