#include "searchkernel_vespa.h"

#include <cassert>
#include <cstdint>
#include <cstdlib>
#include <filesystem>
#include <iostream>
#include <string>

namespace {

constexpr uint32_t ERROR_CAPACITY = SK_ERROR_MESSAGE_CAPACITY;

struct ErrorBuffer {
    uint8_t bytes[ERROR_CAPACITY]{};

    uint8_t *data() { return bytes; }
};

void expect(int32_t actual, int32_t expected, const char *operation) {
    if (actual != expected) {
        std::cerr << operation << ": expected status " << expected << ", got " << actual << '\n';
        std::abort();
    }
}

void expect_ok(int32_t status, const char *operation) {
    expect(status, SK_NATIVE_STATUS_OK, operation);
}

void expect_invalid(int32_t status, const char *operation) {
    expect(status, SK_NATIVE_STATUS_INVALID_ARGUMENT, operation);
}

void put_document(sk_engine *engine, const std::string &id, int64_t qty,
                  ErrorBuffer &error) {
    sk_document *document = nullptr;
    expect_ok(sk_document_create_empty(id.c_str(), &document, error.data(), ERROR_CAPACITY),
              "document create");
    expect_ok(sk_document_add_integer(document, "qty", qty, error.data(),
                                      ERROR_CAPACITY),
              "document value");
    expect_ok(sk_engine_put(engine, document, error.data(), ERROR_CAPACITY), "document put");
    sk_document_destroy(document);
}

sk_query *make_query(sk_engine *engine, ErrorBuffer &error) {
    sk_query *query = nullptr;
    expect_ok(sk_query_create(engine, &query, error.data(), ERROR_CAPACITY), "query create");
    expect_ok(sk_query_add_equals(query, "qty", 18, error.data(), ERROR_CAPACITY),
              "query equals");
    return query;
}

sk_generic_search_request *make_request(const char *profile, ErrorBuffer &error) {
    sk_generic_search_request *request = nullptr;
    expect_ok(sk_generic_search_request_create(profile, 10, 0, SK_SUMMARY_NONE, &request,
                                                error.data(), ERROR_CAPACITY),
              "generic request create");
    return request;
}

void assert_readable_result(sk_generic_search_result *result, ErrorBuffer &error) {
    assert(result != nullptr);
    assert(sk_generic_search_result_hit_count(result) > 0);
    const char *id = nullptr;
    uint32_t id_length = 0;
    double score = 0.0;
    expect_ok(sk_generic_search_result_hit(result, 0, &id, &id_length, &score, error.data(),
                                           ERROR_CAPACITY),
              "result hit");
    assert(id != nullptr && id_length != 0);
}

void assert_issue_bounds(sk_generic_search_result *result, ErrorBuffer &error) {
    assert(sk_generic_search_result_issue_count(result) == 0);
    const char *message = reinterpret_cast<const char *>(1);
    uint32_t length = 7;
    expect_invalid(sk_generic_search_result_issue_message(result, 0, &message, &length,
                                                           error.data(), ERROR_CAPACITY),
                   "issue bounds");
    assert(message == nullptr);
    assert(length == 0);
    assert(error.bytes[0] != 0);
}

void emit_result(const char *name, sk_generic_search_result *result) {
    std::cout << '"' << name << R"(":{"usable":true,"total_hit_count":)"
              << sk_generic_search_result_total_hit_count(result)
              << R"(,"hit_count":)" << sk_generic_search_result_hit_count(result)
              << R"(,"coverage":{"active":)"
              << sk_generic_search_result_coverage_active(result)
              << R"(,"covered":)" << sk_generic_search_result_coverage_covered(result)
              << R"(,"target_active":)"
              << sk_generic_search_result_coverage_target_active(result)
              << R"(},"degradation_flags":)"
              << sk_generic_search_result_degradation_flags(result)
              << ",\"issues\":[]}";
}

void verify_normal_result(sk_engine *engine, ErrorBuffer &error) {
    sk_query *query = make_query(engine, error);
    sk_generic_search_request *request = make_request("default", error);
    sk_generic_search_result *result = nullptr;
    expect_ok(sk_engine_search_generic(engine, query, request, &result, error.data(),
                                       ERROR_CAPACITY),
              "normal search");
    assert_readable_result(result, error);
    assert(sk_generic_search_result_coverage_active(result) > 0);
    assert(sk_generic_search_result_coverage_covered(result) > 0);
    assert(sk_generic_search_result_coverage_target_active(result) > 0);
    assert(sk_generic_search_result_coverage_covered(result) ==
           sk_generic_search_result_coverage_active(result));
    assert(sk_generic_search_result_coverage_target_active(result) ==
           sk_generic_search_result_coverage_active(result));
    assert(sk_generic_search_result_degradation_flags(result) == 0);
    assert_issue_bounds(result, error);
    emit_result("normal", result);
    sk_generic_search_result_destroy(result);
    sk_generic_search_request_destroy(request);
    sk_query_destroy(query);
}

void verify_degraded_result(sk_engine *engine, ErrorBuffer &error) {
    for (uint32_t index = 0; index < 200; ++index) {
        put_document(engine, "id:test:typea::degraded-" + std::to_string(index), 18, error);
    }

    sk_query *query = make_query(engine, error);
    sk_generic_search_request *request = make_request("default", error);
    expect_ok(sk_generic_search_request_add_sort(request, "qty", SK_SORT_ASCENDING,
                                                  SK_SORT_MISSING_DEFAULT, error.data(),
                                                  ERROR_CAPACITY),
              "degraded sort");
    expect_ok(sk_generic_search_request_add_rank_property(
                  request, "vespa.matchphase.degradation.maxfiltercoverage", "0.9",
                  error.data(), ERROR_CAPACITY),
              "degraded match-phase property");
    expect_ok(sk_generic_search_request_add_rank_property(
                  request, "vespa.matchphase.degradation.attribute", "qty",
                  error.data(), ERROR_CAPACITY),
              "degraded match-phase attribute");
    expect_ok(sk_generic_search_request_add_rank_property(
                  request, "vespa.matchphase.degradation.maxhits", "10",
                  error.data(), ERROR_CAPACITY),
              "degraded match-phase max hits");
    sk_generic_search_result *result = nullptr;
    expect_ok(sk_engine_search_generic(engine, query, request, &result, error.data(),
                                       ERROR_CAPACITY),
              "degraded search");
    assert_readable_result(result, error);
    const auto active = sk_generic_search_result_coverage_active(result);
    const auto covered = sk_generic_search_result_coverage_covered(result);
    const auto target = sk_generic_search_result_coverage_target_active(result);
    const auto flags = sk_generic_search_result_degradation_flags(result);
    assert(active > 0);
    assert(covered > 0);
    assert(target > 0);
    assert((flags & SK_DEGRADATION_MATCH_PHASE) != 0);
    assert_issue_bounds(result, error);
    emit_result("degraded", result);
    sk_generic_search_result_destroy(result);
    sk_generic_search_request_destroy(request);
    sk_query_destroy(query);
}

void verify_hard_failure(sk_engine *engine, ErrorBuffer &error) {
    sk_query *query = make_query(engine, error);
    sk_generic_search_request *request = make_request("default", error);
    expect_ok(sk_generic_search_request_add_sort(request, "title", SK_SORT_ASCENDING,
                                                  SK_SORT_MISSING_DEFAULT, error.data(),
                                                  ERROR_CAPACITY),
              "hard failure sort");
    sk_generic_search_result *result = reinterpret_cast<sk_generic_search_result *>(1);
    expect_invalid(sk_engine_search_generic(engine, query, request, &result, error.data(),
                                             ERROR_CAPACITY),
                   "invalid native search request");
    assert(result == nullptr);
    assert(error.bytes[0] != 0);
    sk_generic_search_request_destroy(request);
    sk_query_destroy(query);
}

} // namespace

int main(int argc, char **argv) {
    if (argc != 3) {
        std::cerr << "usage: searchreply_probe DATA_DIR CONFIG_DIR\n";
        return 2;
    }
    assert(sk_native_abi_version() == SK_NATIVE_ABI_VERSION);
    std::filesystem::create_directories(argv[1]);
    ErrorBuffer error;
    sk_engine *engine = nullptr;
    expect_ok(sk_engine_open(argv[1], argv[2], &engine, error.data(), ERROR_CAPACITY),
              "engine open");
    put_document(engine, "id:test:typea::normal", 18, error);
    std::cout << "{";
    verify_normal_result(engine, error);
    std::cout << ',';
    verify_degraded_result(engine, error);
    std::cout << "}\n";
    verify_hard_failure(engine, error);
    expect_ok(sk_engine_close(engine, error.data(), ERROR_CAPACITY), "engine close");
    sk_engine_destroy(engine);
    std::cerr << "SearchReply semantics verifier passed\n";
}
