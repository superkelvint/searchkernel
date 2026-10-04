#include "searchkernel_vespa.h"
#include "highlighting.h"
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
#include "searchkernel_vespa_test.h"
#endif

#include <cassert>
#include <cmath>
#include <cstdint>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <limits>
#include <string>
#include <tuple>
#include <vector>

namespace {

void highlighting_decoder_unit() {
    const auto clipped_annotation = searchkernel::native_highlighting::decode(
        "\x1e" "before \x1fspeaker\x1f \xEF\xBF\xB9Long\xEF\xBF\xBAlong\x1e");
    assert(clipped_annotation && clipped_annotation->size() == 1);
    assert((*clipped_annotation)[0].text == "before speaker Long");
    assert((*clipped_annotation)[0].matches.size() == 1);
    assert((*clipped_annotation)[0].matches[0].start_byte == 7);
    assert((*clipped_annotation)[0].matches[0].end_byte == 14);
    const auto clipped_surface = searchkernel::native_highlighting::decode("before \xEF\xBF\xB9trailing\x1e");
    assert(clipped_surface && clipped_surface->at(0).text == "before trailing");
    const auto clipped_prefix = searchkernel::native_highlighting::decode("\x1e" "an\xEF\xBF\xBB intro \x1f" "headphones\x1f");
    assert(clipped_prefix && clipped_prefix->at(0).text == " intro headphones");
    assert(clipped_prefix->at(0).matches.at(0).start_byte == 7);
    assert(clipped_prefix->at(0).matches.at(0).end_byte == 17);
    for (const auto malformed : {
             "\xEF\xBF\xB9Long\xEF\xBF\xBAlong",
             "\xEF\xBF\xB9Long",
             "\x1f\xEF\xBF\xB9Long\xEF\xBF\xBAlong\x1e"}) {
        bool rejected = false;
        try { (void)searchkernel::native_highlighting::decode(malformed); }
        catch (const std::runtime_error&) { rejected = true; }
        assert(rejected);
    }
    const auto decoded = searchkernel::native_highlighting::decode(
        "prefix\x1f" "Brown" "\x1f " "\x1f" "fox" "\x1f");
    assert(decoded);
    assert(decoded->size() == 1);
    assert(decoded->at(0).text == "prefixBrown fox");
    assert(decoded->at(0).matches.size() == 2);
    assert(decoded->at(0).matches[0].start_byte == 6);
    assert(decoded->at(0).matches[0].end_byte == 11);
    assert(decoded->at(0).matches[1].start_byte == 12);
    assert(decoded->at(0).matches[1].end_byte == 15);

    const auto continued = searchkernel::native_highlighting::decode(
        "\x1e" "before \x1f" "brown" "\x1f" "\x1e" "after \x1f" "fox" "\x1f" "\x1e");
    assert(continued);
    assert(continued->size() == 2);
    assert(continued->at(0).text == "before brown");
    assert(continued->at(0).matches.size() == 1);
    assert(continued->at(0).matches[0].start_byte == 7);
    assert(continued->at(0).matches[0].end_byte == 12);
    assert(continued->at(1).text == "after fox");
    assert(continued->at(1).matches.size() == 1);
    assert(continued->at(1).matches[0].start_byte == 6);
    assert(continued->at(1).matches[0].end_byte == 9);

    const auto literal_markup = searchkernel::native_highlighting::decode(
        "stored <b>text</b>");
    assert(literal_markup);
    assert(literal_markup->size() == 1);
    assert(literal_markup->at(0).text == "stored <b>text</b>");
    assert(literal_markup->at(0).matches.empty());

    const auto annotated = searchkernel::native_highlighting::decode(
        "\xEF\xBF\xB9The\xEF\xBF\xBAthe\xEF\xBF\xBB quick");
    assert(annotated);
    assert(annotated->size() == 1);
    assert(annotated->at(0).text == "The quick");
    assert(annotated->at(0).matches.empty());

    bool rejected = false;
    try {
        (void)searchkernel::native_highlighting::decode("broken\x1f");
    } catch (const std::runtime_error &) {
        rejected = true;
    }
    assert(rejected);
}

constexpr uint32_t ERROR_CAPACITY = SK_ERROR_MESSAGE_CAPACITY;
constexpr const char *TITLE_FIELD = "tit" "le";
constexpr char QUICK_TERM[] = "qui" "ck";
constexpr char QUICK_FOX_TEXT[] = "qui" "ck fox";
constexpr char QUICKFOX_TERM[] = "qui" "ckfox";

struct ErrorBuffer {
    uint8_t bytes[ERROR_CAPACITY]{};

    uint8_t *data() { return bytes; }
};

void expect(int32_t actual, int32_t expected, const char *operation) {
    if (actual != expected) {
        std::cerr << operation << ": expected status " << expected << ", got " << actual
                  << '\n';
        std::abort();
    }
}

void expect_ok(int32_t status, const char *operation) {
    expect(status, SK_NATIVE_STATUS_OK, operation);
}

void expect_invalid(int32_t status, const char *operation) {
    expect(status, SK_NATIVE_STATUS_INVALID_ARGUMENT, operation);
}

void packed_document_buffer_contract() {
    ErrorBuffer error;
    sk_document *document = nullptr;
    sk_document_update *update = nullptr;
    expect_ok(sk_document_create_empty("packed-doc", &document, error.data(), ERROR_CAPACITY),
              "packed buffer document setup");
    expect_ok(sk_document_update_create_empty("packed-update", &update, error.data(),
                                              ERROR_CAPACITY),
              "packed buffer update setup");

    const uint8_t bytes[] = {'x', 'y'};
    const auto zero_byte_sentinel = reinterpret_cast<const uint8_t *>(uintptr_t{1});
    const uint32_t one_length[] = {1};
    const uint32_t empty_length[] = {0};
    const uint32_t multiple_lengths[] = {1, 0, 1};
    expect_ok(sk_document_add_string_array(document, "empty-list", bytes, one_length, 0, 0,
                                           error.data(), ERROR_CAPACITY),
              "empty packed string array");
    expect_ok(sk_document_add_string_array(document, "empty-item", zero_byte_sentinel,
                                           empty_length, 1, 0,
                                           error.data(), ERROR_CAPACITY),
              "zero-length packed string item");
    expect_ok(sk_document_add_string_array(document, "multiple-items", bytes, multiple_lengths,
                                           3, 2, error.data(), ERROR_CAPACITY),
              "multi-item packed string array");
    const uint8_t boundary_bytes[] = {'a', 'b'};
    const uint32_t offset_boundary_lengths[] = {1, 2};
    expect_invalid(sk_document_add_string_array(document, "offset-boundary", boundary_bytes,
                                                 offset_boundary_lengths, 2, 2, error.data(),
                                                 ERROR_CAPACITY),
                   "string array remaining bytes after offset advance");

    expect_invalid(sk_document_add_string_array(document, "short-total", bytes, one_length, 1, 0,
                                                 error.data(), ERROR_CAPACITY),
                   "string array total shorter than lengths");
    expect_invalid(sk_document_add_string_array(document, "long-total", bytes, one_length, 1, 2,
                                                 error.data(), ERROR_CAPACITY),
                   "string array total longer than lengths");
    expect_invalid(sk_document_add_string_array(document, "null-nonzero", nullptr, one_length, 1,
                                                 1, error.data(), ERROR_CAPACITY),
                   "string array null bytes with nonzero total");
    expect_invalid(sk_document_add_string_array(document, "null-empty-item", nullptr,
                                                 empty_length, 1, 0, error.data(), ERROR_CAPACITY),
                   "string array null bytes for zero-length item");
    expect_invalid(sk_document_add_string_array(document, "too-many-items", bytes, one_length,
                                                 65537, 0, error.data(), ERROR_CAPACITY),
                   "string array count limit");
    const uint32_t oversized_total[] = {16 * 1024 * 1024 + 1};
    expect_invalid(sk_document_add_string_array(document, "oversized-total", bytes, one_length,
                                                 1, oversized_total[0], error.data(),
                                                 ERROR_CAPACITY),
                   "string array total size limit");
    expect_ok(sk_document_update_assign_string_array(update, "empty-list", bytes, one_length, 0,
                                                     0, error.data(), ERROR_CAPACITY),
              "empty packed update string array");
    expect_ok(sk_document_update_assign_string_array(update, "empty-item", zero_byte_sentinel,
                                                     empty_length,
                                                     1, 0, error.data(), ERROR_CAPACITY),
              "zero-length packed update string item");
    expect_ok(sk_document_update_assign_string_array(update, "multiple-items", bytes,
                                                     multiple_lengths, 3, 2, error.data(),
                                                     ERROR_CAPACITY),
              "multi-item packed update string array");
    expect_invalid(sk_document_update_assign_string_array(update, "short-total", bytes,
                                                           one_length, 1, 0, error.data(),
                                                           ERROR_CAPACITY),
                   "update string array total shorter than lengths");
    expect_invalid(sk_document_update_assign_string_array(update, "long-total", bytes,
                                                           one_length, 1, 2, error.data(),
                                                           ERROR_CAPACITY),
                   "update string array total longer than lengths");
    expect_invalid(sk_document_update_assign_string_array(update, "null-empty-item", nullptr,
                                                           empty_length, 1, 0, error.data(),
                                                           ERROR_CAPACITY),
                   "update string array null bytes for zero-length item");
    expect_invalid(sk_document_update_assign_string_array(update, "too-many-items", bytes,
                                                           one_length, 65537, 0, error.data(),
                                                           ERROR_CAPACITY),
                   "update string array count limit");
    expect_invalid(sk_document_update_assign_string_array(
                       update, "offset-boundary", boundary_bytes, offset_boundary_lengths, 2, 2,
                       error.data(), ERROR_CAPACITY),
                   "update string array remaining bytes after offset advance");

    const uint32_t max_element_length[] = {1024 * 1024 + 1};
    expect_invalid(sk_document_add_string_array(document, "oversized-element", bytes,
                                                 max_element_length, 1, max_element_length[0],
                                                 error.data(), ERROR_CAPACITY),
                   "oversized string array element");
    expect_invalid(sk_document_update_assign_string_array(update, "oversized-element", bytes,
                                                           max_element_length, 1,
                                                           max_element_length[0], error.data(),
                                                           ERROR_CAPACITY),
                   "oversized update string array element");

    const uint32_t key_length[] = {1};
    const uint32_t value_length[] = {1};
    expect_ok(sk_document_add_predicate(document, "valid-predicate", bytes, key_length, 1,
                                        bytes, value_length, 1, 1, error.data(), ERROR_CAPACITY),
              "valid packed predicate");
    expect_ok(sk_document_add_predicate(document, "empty-predicate-feature", bytes,
                                        empty_length, 0, bytes, empty_length, 0, 1,
                                        error.data(), ERROR_CAPACITY),
              "zero-length packed predicate feature");
    const uint32_t multiple_feature_lengths[] = {1, 0, 1};
    expect_ok(sk_document_add_predicate(document, "multiple-predicate-features", bytes,
                                        multiple_feature_lengths, 2, bytes,
                                        multiple_feature_lengths, 2, 3, error.data(),
                                        ERROR_CAPACITY),
              "multi-feature packed predicate");
    expect_invalid(sk_document_add_predicate(document, "short-key-total", bytes, key_length, 0,
                                             bytes, value_length, 1, 1, error.data(),
                                             ERROR_CAPACITY),
                   "predicate key total shorter than lengths");
    expect_invalid(sk_document_add_predicate(document, "long-key-total", bytes, key_length, 2,
                                             bytes, value_length, 1, 1, error.data(),
                                             ERROR_CAPACITY),
                   "predicate key total longer than lengths");
    expect_invalid(sk_document_add_predicate(document, "short-value-total", bytes, key_length, 1,
                                             bytes, value_length, 0, 1, error.data(),
                                             ERROR_CAPACITY),
                   "predicate value total shorter than lengths");
    expect_invalid(sk_document_add_predicate(document, "long-value-total", bytes, key_length, 1,
                                             bytes, value_length, 2, 1, error.data(),
                                             ERROR_CAPACITY),
                   "predicate value total longer than lengths");
    expect_invalid(sk_document_add_predicate(document, "null-key-nonzero", nullptr, key_length, 1,
                                             bytes, value_length, 1, 1, error.data(),
                                             ERROR_CAPACITY),
                   "predicate null key bytes with nonzero total");
    expect_invalid(sk_document_add_predicate(document, "null-value-nonzero", bytes, key_length, 1,
                                             nullptr, value_length, 1, 1, error.data(),
                                             ERROR_CAPACITY),
                   "predicate null value bytes with nonzero total");
    expect_invalid(sk_document_add_predicate(document, "too-many-features", bytes, key_length, 0,
                                             bytes, value_length, 0, 65537, error.data(),
                                             ERROR_CAPACITY),
                   "predicate feature count limit");
    const uint32_t predicate_offset_lengths[] = {1, 2};
    const uint32_t predicate_valid_lengths[] = {1, 0};
    expect_invalid(sk_document_add_predicate(
                       document, "predicate-key-offset", boundary_bytes,
                       predicate_offset_lengths, 2, boundary_bytes, predicate_valid_lengths, 1, 2,
                       error.data(), ERROR_CAPACITY),
                   "predicate key remaining bytes after offset advance");
    expect_invalid(sk_document_add_predicate(
                       document, "predicate-value-offset", boundary_bytes,
                       predicate_valid_lengths, 1, boundary_bytes, predicate_offset_lengths, 2,
                       2, error.data(), ERROR_CAPACITY),
                   "predicate value remaining bytes after offset advance");
    expect_ok(sk_document_add_predicate(
                  document, "predicate-empty-key", zero_byte_sentinel, empty_length, 0, bytes,
                  one_length, 1, 1, error.data(), ERROR_CAPACITY),
              "zero-length predicate key avoids packed pointer use");
    expect_ok(sk_document_add_predicate(
                  document, "predicate-empty-value", bytes, one_length, 1, zero_byte_sentinel,
                  empty_length, 0, 1, error.data(), ERROR_CAPACITY),
              "zero-length predicate value avoids packed pointer use");
    expect_ok(sk_document_add_predicate(
                  document, "predicate-empty-key-value", zero_byte_sentinel, empty_length, 0,
                  zero_byte_sentinel, empty_length, 0, 1, error.data(), ERROR_CAPACITY),
              "zero-length predicate key and value avoid packed pointer use");
    expect_invalid(sk_document_update_assign_predicate(
                       update, "predicate-key-offset", boundary_bytes, predicate_offset_lengths, 2,
                       boundary_bytes, predicate_valid_lengths, 1, 2, error.data(),
                       ERROR_CAPACITY),
                   "update predicate key remaining bytes after offset advance");
    expect_invalid(sk_document_update_assign_predicate(
                       update, "predicate-value-offset", boundary_bytes, predicate_valid_lengths,
                       1, boundary_bytes, predicate_offset_lengths, 2, 2, error.data(),
                       ERROR_CAPACITY),
                   "update predicate value remaining bytes after offset advance");
    const uint32_t empty_lengths[] = {0};
    expect_invalid(sk_document_add_predicate(document, "null-empty-key", nullptr, empty_lengths, 0,
                                             bytes, value_length, 1, 1, error.data(),
                                             ERROR_CAPACITY),
                   "predicate null key bytes for zero-length feature");
    expect_invalid(sk_document_add_predicate(document, "null-empty-value", bytes, key_length, 1,
                                             nullptr, empty_lengths, 0, 1, error.data(),
                                             ERROR_CAPACITY),
                   "predicate null value bytes for zero-length feature");
    expect_invalid(sk_document_update_assign_predicate(
                       update, "short-key-total", bytes, key_length, 0, bytes, value_length, 1,
                       1, error.data(), ERROR_CAPACITY),
                   "update predicate key total shorter than lengths");
    expect_invalid(sk_document_update_assign_predicate(
                       update, "short-value-total", bytes, key_length, 1, bytes, value_length, 0,
                       1, error.data(), ERROR_CAPACITY),
                   "update predicate value total shorter than lengths");
    expect_invalid(sk_document_update_assign_predicate(
                       update, "null-key-nonzero", nullptr, key_length, 1, bytes, value_length, 1,
                       1, error.data(), ERROR_CAPACITY),
                   "update predicate null key bytes with nonzero total");
    expect_invalid(sk_document_update_assign_predicate(
                       update, "null-value-nonzero", bytes, key_length, 1, nullptr, value_length,
                       1, 1, error.data(), ERROR_CAPACITY),
                   "update predicate null value bytes with nonzero total");
    expect_invalid(sk_document_update_assign_predicate(
                       update, "null-empty-key", nullptr, empty_lengths, 0, bytes, value_length,
                       1, 1, error.data(), ERROR_CAPACITY),
                   "update predicate null key bytes for zero-length feature");
    expect_invalid(sk_document_update_assign_predicate(
                       update, "null-empty-value", bytes, key_length, 1, nullptr, empty_lengths,
                       0, 1, error.data(), ERROR_CAPACITY),
                   "update predicate null value bytes for zero-length feature");

    const uint32_t oversized_length[] = {16 * 1024 * 1024 + 1};
    expect_invalid(sk_document_add_predicate(document, "oversized-key", bytes, oversized_length,
                                             oversized_length[0], bytes, value_length, 1, 1,
                                             error.data(), ERROR_CAPACITY),
                   "oversized predicate key");
    expect_invalid(sk_document_add_predicate(document, "oversized-element", bytes,
                                             oversized_length, oversized_length[0], bytes,
                                             value_length, 1, 1, error.data(), ERROR_CAPACITY),
                   "predicate per-element size boundary");
    expect_invalid(sk_document_update_assign_predicate(
                       update, "oversized-value", bytes, key_length, 1, bytes, oversized_length,
                       oversized_length[0], 1, error.data(), ERROR_CAPACITY),
                   "oversized update predicate value");
    expect_invalid(sk_document_update_assign_predicate(
                       update, "too-many-features", bytes, key_length, 0, bytes, value_length, 0,
                       65537, error.data(), ERROR_CAPACITY),
                   "update predicate feature count limit");

    sk_document_update_destroy(update);
    sk_document_destroy(document);
}

void invalid_argument_surface() {
    ErrorBuffer error;
    sk_engine *engine = nullptr;
    sk_document *document = nullptr;
    sk_document_update *update = nullptr;
    sk_search_request *request = nullptr;
    sk_search_result *result = nullptr;
    sk_text_document *text_document = nullptr;
    sk_text_update *text_update = nullptr;
    sk_text_request *text_request = nullptr;
    sk_text_result *text_result = nullptr;
    sk_query *query = nullptr;
    sk_local_search_request *local_request = nullptr;
    sk_local_search_result *local_result = nullptr;
    sk_generic_search_request *generic_request = nullptr;
    sk_generic_search_result *generic_result = nullptr;
    sk_attribute_document *attribute_document = nullptr;
    sk_attribute_update *attribute_update = nullptr;
    sk_local_grouping_result *local_grouping_result = nullptr;
    sk_generic_grouping_request *generic_grouping = nullptr;
    sk_generic_grouping_result *generic_grouping_result = nullptr;
    uint32_t count = 7;
    uint8_t blocked = 7;
    int32_t found = 7;
    const char *text = reinterpret_cast<const char *>(1);
    uint32_t length = 7;
    uint32_t kind = 7;
    uint32_t string_length = 7;
    int64_t integer = 7;
    double floating = 7;
    uint8_t boolean = 7;
    uint64_t total = 7;

    expect_invalid(sk_engine_open(nullptr, nullptr, &engine, error.data(), ERROR_CAPACITY),
                   "engine open null");
    expect_invalid(sk_engine_open("data", "config", nullptr, error.data(), ERROR_CAPACITY),
                   "engine open output null");
    sk_engine_destroy(nullptr);
    expect_invalid(sk_engine_close(nullptr, error.data(), ERROR_CAPACITY), "engine close null");

    expect_invalid(sk_document_create(nullptr, "qty", 1, &document, error.data(), ERROR_CAPACITY),
                   "document create null id");
    expect_invalid(sk_document_create_empty(nullptr, &document, error.data(), ERROR_CAPACITY),
                   "document empty null id");
    expect_invalid(sk_document_create_empty("id", nullptr, error.data(), ERROR_CAPACITY),
                   "document empty null output");
    expect_invalid(sk_document_add_string(nullptr, "qty", "x", error.data(), ERROR_CAPACITY),
                   "document add string null");
    expect_invalid(sk_document_add_integer(nullptr, "qty", 1, error.data(), ERROR_CAPACITY),
                   "document add integer null");
    expect_invalid(sk_document_add_float(nullptr, "qty", 1, error.data(), ERROR_CAPACITY),
                   "document add float null");
    expect_invalid(sk_document_add_boolean(nullptr, "qty", 1, error.data(), ERROR_CAPACITY),
                   "document add boolean null");
    expect_invalid(sk_document_annotate_text(nullptr, TITLE_FIELD, nullptr, 0, nullptr, nullptr,
                                             nullptr, 0, error.data(), ERROR_CAPACITY),
                   "document annotate null");
    const uint8_t array_value_bytes[] = {'x'};
    const uint32_t array_value_lengths[] = {1};
    const uint32_t array_term_lengths[] = {1};
    const uint32_t array_byte_starts[] = {0};
    const uint32_t array_byte_lengths[] = {1};
    const uint32_t array_element_offsets[] = {0, 1};
    expect_ok(sk_document_create_empty("array-doc", &document, error.data(), ERROR_CAPACITY),
              "document array annotation setup");
    expect_ok(sk_document_add_string_array(document, "tags", array_value_bytes,
                                           array_value_lengths, 1, 1, error.data(), ERROR_CAPACITY),
              "document array annotation value setup");
    const uint32_t zero_length[] = {0};
    const uint32_t two_lengths[] = {1, 1};
    const uint8_t two_bytes[] = {'a', 'b'};
    expect_invalid(sk_document_add_string_array(document, "bad", nullptr, zero_length, 1, 0,
                                                error.data(), ERROR_CAPACITY),
                   "string array null bytes with nonzero count");
    expect_invalid(sk_document_add_string_array(document, "bad", array_value_bytes,
                                                array_value_lengths, 1, 0, error.data(), ERROR_CAPACITY),
                   "string array short packed total");
    expect_invalid(sk_document_add_string_array(document, "bad", array_value_bytes,
                                                zero_length, 1, 1, error.data(), ERROR_CAPACITY),
                   "string array long packed total");
    expect_ok(sk_document_add_string_array(document, "empty", nullptr, nullptr, 0, 0,
                                           error.data(), ERROR_CAPACITY),
              "string array empty descriptor");
    expect_ok(sk_document_add_string_array(document, "multi", two_bytes, two_lengths, 2, 2,
                                           error.data(), ERROR_CAPACITY),
              "string array multi element descriptor");
    expect_invalid(sk_document_add_string_array(document, "too-many", array_value_bytes,
                                                array_value_lengths, 1'000'001, 1,
                                                error.data(), ERROR_CAPACITY),
                   "string array element count limit");
    const uint8_t predicate_key[] = {'k'};
    const uint8_t predicate_value[] = {'v'};
    expect_invalid(sk_document_add_predicate(document, "pred", nullptr, zero_length, 0,
                                             predicate_value, array_value_lengths, 1, 1,
                                             error.data(), ERROR_CAPACITY),
                   "predicate null key bytes with nonzero count");
    expect_invalid(sk_document_add_predicate(document, "pred", predicate_key, array_value_lengths, 0,
                                             predicate_value, array_value_lengths, 1, 1,
                                             error.data(), ERROR_CAPACITY),
                   "predicate short key total");
    expect_invalid(sk_document_add_predicate(document, "pred", predicate_key, array_value_lengths, 1,
                                             predicate_value, zero_length, 1, 1,
                                             error.data(), ERROR_CAPACITY),
                   "predicate long value total");
    expect_invalid(sk_document_add_predicate(document, "pred", predicate_key, array_value_lengths, 1,
                                             nullptr, zero_length, 0, 1, error.data(), ERROR_CAPACITY),
                   "predicate null value bytes with nonzero count");
    expect_invalid(sk_document_annotate_string_array(
                       document, "tags", nullptr, 1, array_term_lengths, array_byte_starts,
                       array_byte_lengths, array_element_offsets, 1, 1, error.data(),
                       ERROR_CAPACITY),
                   "document array annotation null term bytes");
    sk_document_destroy(document);
    document = nullptr;
    sk_document_destroy(nullptr);

    expect_invalid(sk_document_update_create(nullptr, "qty", 1, &update, error.data(), ERROR_CAPACITY),
                   "update create null id");
    expect_invalid(sk_document_update_create_empty(nullptr, &update, error.data(), ERROR_CAPACITY),
                   "empty update null id");
    expect_invalid(sk_document_update_set_create_if_nonexistent(nullptr, 1, error.data(),
                                                                ERROR_CAPACITY),
                   "update create flag null");
    expect_invalid(sk_document_update_assign_string(nullptr, "qty", "x", error.data(), ERROR_CAPACITY),
                   "update string null");
    expect_invalid(sk_document_update_assign_integer(nullptr, "qty", 1, error.data(), ERROR_CAPACITY),
                   "update integer null");
    expect_invalid(sk_document_update_assign_float(nullptr, "qty", 1, error.data(), ERROR_CAPACITY),
                   "update float null");
    expect_invalid(sk_document_update_assign_boolean(nullptr, "qty", 1, error.data(), ERROR_CAPACITY),
                   "update boolean null");
    expect_invalid(sk_document_update_clear(nullptr, "qty", error.data(), ERROR_CAPACITY),
                   "update clear null");
    expect_invalid(sk_document_update_annotate_text(nullptr, TITLE_FIELD, nullptr, 0, nullptr,
                                                     nullptr, nullptr, 0, error.data(), ERROR_CAPACITY),
                   "update annotate null");
    expect_invalid(sk_document_update_annotate_string_array(
                       nullptr, TITLE_FIELD, nullptr, 0, nullptr, nullptr, nullptr, nullptr, 0, 0,
                       error.data(), ERROR_CAPACITY),
                   "update annotate array null");
    expect_ok(sk_document_update_create_empty("array-update", &update, error.data(),
                                              ERROR_CAPACITY),
              "update array annotation setup");
    expect_ok(sk_document_update_assign_string_array(update, "tags", array_value_bytes,
                                                     array_value_lengths, 1, 1, error.data(),
                                                     ERROR_CAPACITY),
              "update array annotation value setup");
    expect_invalid(sk_document_update_annotate_string_array(
                       update, "tags", nullptr, 1, array_term_lengths, array_byte_starts,
                       array_byte_lengths, array_element_offsets, 1, 1, error.data(),
                       ERROR_CAPACITY),
                   "update array annotation null term bytes");
    sk_document_update_destroy(update);
    update = nullptr;
    sk_document_update_destroy(nullptr);

    expect_invalid(sk_search_request_create(nullptr, 1, 1, &request, error.data(), ERROR_CAPACITY),
                   "search request null field");
    sk_search_request_destroy(nullptr);
    expect_invalid(sk_engine_put(nullptr, nullptr, error.data(), ERROR_CAPACITY), "put null");
    expect_invalid(sk_engine_update(nullptr, nullptr, error.data(), ERROR_CAPACITY), "update null");
    expect_invalid(sk_engine_get(nullptr, nullptr, &document, error.data(), ERROR_CAPACITY), "get null");
    expect_invalid(sk_engine_remove(nullptr, nullptr, &found, error.data(), ERROR_CAPACITY), "remove null");
    expect_invalid(sk_engine_search(nullptr, nullptr, &result, error.data(), ERROR_CAPACITY), "search null");
    assert(sk_search_result_size(nullptr) == 0);
    expect_invalid(sk_search_result_id(nullptr, 0, &text, &length, error.data(), ERROR_CAPACITY),
                   "result id null");
    sk_search_result_destroy(nullptr);

    expect_invalid(sk_text_document_create("id", TITLE_FIELD, nullptr, 1, &text_document,
                                           error.data(), ERROR_CAPACITY), "text null bytes");
    expect_invalid(sk_text_document_add_occurrences(nullptr, nullptr, 0, nullptr, nullptr, nullptr,
                                                    0, error.data(), ERROR_CAPACITY), "text document null");
    sk_text_document_destroy(nullptr);
    expect_invalid(sk_text_update_create("id", TITLE_FIELD, nullptr, 1, &text_update,
                                         error.data(), ERROR_CAPACITY), "text update null bytes");
    expect_invalid(sk_text_update_add_occurrences(nullptr, nullptr, 0, nullptr, nullptr, nullptr,
                                                   0, error.data(), ERROR_CAPACITY), "text update null");
    sk_text_update_destroy(nullptr);
    expect_invalid(sk_text_request_create(0, TITLE_FIELD, "default", &text_request,
                                          error.data(), ERROR_CAPACITY), "text request invalid kind");
    expect_invalid(sk_text_request_add_terms(nullptr, nullptr, 0, nullptr, 0,
                                             error.data(), ERROR_CAPACITY), "text request null");
    sk_text_request_destroy(nullptr);
    expect_invalid(sk_engine_put_text(nullptr, nullptr, error.data(), ERROR_CAPACITY), "text put null");
    expect_invalid(sk_engine_update_text(nullptr, nullptr, error.data(), ERROR_CAPACITY), "text update call null");
    expect_invalid(sk_engine_search_text(nullptr, nullptr, &text_result, error.data(), ERROR_CAPACITY),
                   "text search null");
    assert(sk_text_result_size(nullptr) == 0);
    expect_invalid(sk_text_result_hit(nullptr, 0, &text, &length, &floating, error.data(), ERROR_CAPACITY),
                   "text result null");
    sk_text_result_destroy(nullptr);

    expect_invalid(sk_query_create(nullptr, &query, error.data(), ERROR_CAPACITY), "query create null");
    expect_invalid(sk_query_add_operator(nullptr, 0, 0, error.data(), ERROR_CAPACITY), "query operator null");
    expect_invalid(sk_query_validate_field(nullptr, 0, nullptr, error.data(), ERROR_CAPACITY), "query field null");
    expect_invalid(sk_query_add_true(nullptr, error.data(), ERROR_CAPACITY), "query true null");
    expect_invalid(sk_query_add_text(nullptr, 0, nullptr, nullptr, 0, nullptr, 0,
                                     error.data(), ERROR_CAPACITY), "query text null");
    expect_invalid(sk_query_add_equals(nullptr, nullptr, 0, error.data(), ERROR_CAPACITY), "query equals null");
    expect_invalid(sk_query_add_range(nullptr, nullptr, 0, 0, 0, 0, error.data(), ERROR_CAPACITY), "query range null");
    sk_query_destroy(nullptr);
    expect_invalid(sk_engine_search_query(nullptr, nullptr, nullptr, nullptr, &text_result,
                                          error.data(), ERROR_CAPACITY), "query search null");

    expect_invalid(sk_local_search_request_create(nullptr, 1, 0, 0, &local_request,
                                                  error.data(), ERROR_CAPACITY), "local request null");
    expect_invalid(sk_local_search_request_add_sort(nullptr, nullptr, 0, error.data(), ERROR_CAPACITY),
                   "local sort null");
    expect_invalid(sk_local_search_request_add_summary_field(nullptr, nullptr, error.data(), ERROR_CAPACITY),
                   "local summary null");
    expect_invalid(sk_local_search_request_set_summary_class(nullptr, nullptr, error.data(), ERROR_CAPACITY),
                   "local summary class null");
    expect_invalid(sk_local_search_request_add_query_feature(nullptr, nullptr, 0, error.data(), ERROR_CAPACITY),
                   "local feature null");
    expect_invalid(sk_local_search_request_add_rank_property(nullptr, nullptr, nullptr, error.data(), ERROR_CAPACITY),
                   "local property null");
    sk_local_search_request_destroy(nullptr);
    expect_invalid(sk_engine_search_local(nullptr, nullptr, nullptr, &local_result,
                                          error.data(), ERROR_CAPACITY), "local search null");
    assert(sk_local_search_result_total_hit_count(nullptr) == 0);
    assert(sk_local_search_result_hit_count(nullptr) == 0);
    assert(sk_local_search_result_summary_count(nullptr, 0) == 0);
    assert(sk_local_search_result_match_feature_count(nullptr) == 0);
    sk_local_search_result_destroy(nullptr);

    expect_invalid(sk_generic_search_request_create(nullptr, 1, 0, 0, &generic_request,
                                                    error.data(), ERROR_CAPACITY), "generic request null");
    expect_invalid(sk_generic_search_request_set_timeout(nullptr, 1, error.data(), ERROR_CAPACITY),
                   "generic timeout null");
    expect_invalid(sk_generic_search_request_add_sort(nullptr, nullptr, 0, 0, error.data(), ERROR_CAPACITY),
                   "generic sort null");
    expect_invalid(sk_generic_search_request_add_summary_field(nullptr, nullptr, error.data(), ERROR_CAPACITY),
                   "generic summary null");
    expect_invalid(sk_generic_search_request_set_summary_class(nullptr, nullptr, error.data(), ERROR_CAPACITY),
                   "generic summary class null");
    expect_invalid(sk_generic_search_request_add_query_feature(nullptr, nullptr, 0, error.data(), ERROR_CAPACITY),
                   "generic feature null");
    expect_invalid(sk_generic_search_request_add_rank_property(nullptr, nullptr, nullptr,
                                                               error.data(), ERROR_CAPACITY), "generic property null");
    expect_invalid(sk_generic_search_request_add_grouping(nullptr, nullptr, 1, 0,
                                                           error.data(), ERROR_CAPACITY), "generic grouping null");
    sk_generic_search_request_destroy(nullptr);
    expect_invalid(sk_engine_search_generic(nullptr, nullptr, nullptr, &generic_result,
                                            error.data(), ERROR_CAPACITY), "generic search null");
    assert(sk_generic_search_result_total_hit_count(nullptr) == 0);
    assert(sk_generic_search_result_hit_count(nullptr) == 0);
    assert(sk_generic_search_result_coverage_active(nullptr) == 0);
    assert(sk_generic_search_result_coverage_covered(nullptr) == 0);
    assert(sk_generic_search_result_coverage_target_active(nullptr) == 0);
    assert(sk_generic_search_result_degradation_flags(nullptr) == 0);
    assert(sk_generic_search_result_issue_count(nullptr) == 0);
    assert(sk_generic_search_result_summary_count(nullptr, 0) == 0);
    assert(sk_generic_search_result_match_feature_count(nullptr) == 0);
    assert(sk_generic_search_result_grouping_count(nullptr) == 0);
    sk_generic_search_result_destroy(nullptr);

    expect_invalid(sk_attribute_document_create(nullptr, &attribute_document, error.data(), ERROR_CAPACITY),
                   "attribute document null");
    expect_invalid(sk_attribute_document_add_string(nullptr, nullptr, nullptr, error.data(), ERROR_CAPACITY),
                   "attribute string null");
    expect_invalid(sk_attribute_document_add_integer(nullptr, nullptr, 0, error.data(), ERROR_CAPACITY),
                   "attribute integer null");
    expect_invalid(sk_attribute_document_add_float(nullptr, nullptr, 0, error.data(), ERROR_CAPACITY),
                   "attribute float null");
    expect_invalid(sk_attribute_document_add_boolean(nullptr, nullptr, 0, error.data(), ERROR_CAPACITY),
                   "attribute boolean null");
    sk_attribute_document_destroy(nullptr);
    expect_invalid(sk_attribute_update_create(nullptr, &attribute_update, error.data(), ERROR_CAPACITY),
                   "attribute update null");
    expect_invalid(sk_attribute_update_add_string(nullptr, nullptr, nullptr, error.data(), ERROR_CAPACITY),
                   "attribute update string null");
    expect_invalid(sk_attribute_update_add_integer(nullptr, nullptr, 0, error.data(), ERROR_CAPACITY),
                   "attribute update integer null");
    expect_invalid(sk_attribute_update_add_float(nullptr, nullptr, 0, error.data(), ERROR_CAPACITY),
                   "attribute update float null");
    expect_invalid(sk_attribute_update_add_boolean(nullptr, nullptr, 0, error.data(), ERROR_CAPACITY),
                   "attribute update boolean null");
    sk_attribute_update_destroy(nullptr);
    expect_invalid(sk_engine_put_attributes(nullptr, nullptr, error.data(), ERROR_CAPACITY), "attribute put null");
    expect_invalid(sk_engine_update_attributes(nullptr, nullptr, error.data(), ERROR_CAPACITY), "attribute update call null");
    expect_invalid(sk_local_grouping_request_create(nullptr, nullptr, error.data(), ERROR_CAPACITY),
                   "local grouping null");
    expect_invalid(sk_local_grouping_request_add_aggregation(nullptr, 0, nullptr,
                                                              error.data(), ERROR_CAPACITY), "local aggregation null");
    sk_local_grouping_request_destroy(nullptr);
    expect_invalid(sk_engine_group_local(nullptr, nullptr, nullptr, &local_grouping_result,
                                         error.data(), ERROR_CAPACITY), "local grouping call null");
    assert(sk_local_grouping_result_group_count(nullptr) == 0);
    assert(sk_local_grouping_result_aggregation_count(nullptr, 0) == 0);
    sk_local_grouping_result_destroy(nullptr);
    expect_invalid(sk_generic_grouping_request_create(nullptr, 1, 0, &generic_grouping,
                                                      error.data(), ERROR_CAPACITY), "generic grouping null");
    expect_invalid(sk_generic_grouping_request_add_aggregation(nullptr, 0, nullptr,
                                                                error.data(), ERROR_CAPACITY), "generic grouping aggregation null");
    sk_generic_grouping_request_destroy(nullptr);
    expect_invalid(sk_engine_group_generic(nullptr, nullptr, nullptr, &generic_grouping_result,
                                           error.data(), ERROR_CAPACITY), "generic grouping call null");
    assert(sk_generic_grouping_result_group_count(nullptr) == 0);
    assert(sk_generic_grouping_result_aggregation_count(nullptr, 0) == 0);
    sk_generic_grouping_result_destroy(nullptr);

    (void)blocked;
    (void)count;
    (void)kind;
    (void)string_length;
    (void)integer;
    (void)boolean;
    (void)total;
}

void check_document_fields(const sk_document *document) {
    ErrorBuffer error;
    uint32_t count = 0;
    expect_ok(sk_document_field_count(document, &count, error.data(), ERROR_CAPACITY), "field count");
    assert(count >= 2);
    for (uint32_t index = 0; index < count; ++index) {
        const char *name = nullptr;
        const char *string = nullptr;
        uint32_t name_length = 0;
        uint32_t kind = 0;
        uint32_t string_length = 0;
        int64_t integer = 0;
        double floating = 0;
        uint8_t boolean = 0;
        expect_ok(sk_document_field(document, index, &name, &name_length, &kind, &string,
                                     &string_length, &integer, &floating, &boolean,
                                     error.data(), ERROR_CAPACITY), "field");
        assert(name != nullptr && name_length != 0);
        if (kind == SK_VALUE_INTEGER) assert(integer == 17);
        if (kind == SK_VALUE_STRING) assert(string != nullptr && string_length != 0);
    }
    expect_invalid(sk_document_field(document, count, nullptr, nullptr, nullptr, nullptr, nullptr,
                                     nullptr, nullptr, nullptr, error.data(), ERROR_CAPACITY),
                   "field out of bounds");
}

sk_query *make_equals_query(const sk_engine *engine) {
    ErrorBuffer error;
    sk_query *query = nullptr;
    expect_ok(sk_query_create(engine, &query, error.data(), ERROR_CAPACITY), "query create");
    expect_ok(sk_query_add_equals(query, "qty", 18, error.data(), ERROR_CAPACITY), "query equals");
    return query;
}

void predicate_query_buffer_contract(sk_engine *engine) {
    ErrorBuffer error;
    const uint8_t predicate_bytes[] = {'k', 'v'};
    const uint8_t one_byte_allocation[] = {'x'};
    const uint32_t one_length[] = {1};
    const uint32_t two_length[] = {2};
    const uint32_t two_one_lengths[] = {1, 1};
    const uint32_t one_two_lengths[] = {1, 2};
    const uint32_t zero_length[] = {0};
    const uint32_t oversized_term_length[] = {1024 * 1024 + 1};
    const uint32_t overflow_lengths[] = {1, std::numeric_limits<uint32_t>::max()};
    const uint64_t predicate_masks[] = {1, 2};
    const uint64_t predicate_values[] = {7, 8};
    auto predicate_query = [&] {
        sk_query *result = nullptr;
        expect_ok(sk_query_create(engine, &result, error.data(), ERROR_CAPACITY),
                  "predicate bounds query create");
        return result;
    };
    auto expect_bad_predicate = [&](const uint8_t *keys, uint32_t key_total,
                                    const uint32_t *key_lengths, const uint8_t *values,
                                    uint32_t value_total, const uint32_t *value_lengths,
                                    uint32_t feature_count, const uint8_t *range_keys,
                                    uint32_t range_total, const uint32_t *range_lengths,
                                    uint32_t range_count, const char *label) {
        auto *candidate = predicate_query();
        expect_invalid(sk_query_add_predicate(
                           candidate, "access", keys, key_total, key_lengths, values, value_total,
                           value_lengths, predicate_masks, feature_count, range_keys, range_total,
                           range_lengths, predicate_values, predicate_masks, range_count,
                           error.data(), ERROR_CAPACITY),
                       label);
        sk_query_destroy(candidate);
    };

    // These are real one-byte allocations with a two-byte advertised element.
    // If native validation ever moves after the read, ASan must catch the regression.
    expect_bad_predicate(one_byte_allocation, 1, two_length, one_byte_allocation, 1, one_length, 1,
                         nullptr, 0, nullptr, 0, "predicate feature key exceeds packed total");
    expect_bad_predicate(one_byte_allocation, 1, one_length, one_byte_allocation, 1, two_length, 1,
                         nullptr, 0, nullptr, 0, "predicate feature value exceeds packed total");
    expect_bad_predicate(nullptr, 0, nullptr, nullptr, 0, nullptr, 0, one_byte_allocation, 1,
                         two_length, 1, "predicate range key exceeds packed total");

    // Exact totals are mandatory; deliberate trailing bytes are rejected independently.
    expect_bad_predicate(predicate_bytes, 2, one_length, predicate_bytes, 1, one_length, 1,
                         nullptr, 0, nullptr, 0, "predicate feature key trailing bytes rejected");
    expect_bad_predicate(predicate_bytes, 1, one_length, predicate_bytes, 2, one_length, 1,
                         nullptr, 0, nullptr, 0, "predicate feature value trailing bytes rejected");
    expect_bad_predicate(nullptr, 0, nullptr, nullptr, 0, nullptr, 0, predicate_bytes, 2,
                         one_length, 1, "predicate range key trailing bytes rejected");

    // Null/non-zero descriptors are invalid even when that side has zero items.
    expect_bad_predicate(nullptr, 1, nullptr, nullptr, 0, nullptr, 0, predicate_bytes, 1,
                         one_length, 1, "predicate null feature key with nonzero total");
    expect_bad_predicate(predicate_bytes, 0, nullptr, nullptr, 1, nullptr, 0, predicate_bytes, 1,
                         one_length, 1, "predicate null feature value with nonzero total");
    expect_bad_predicate(predicate_bytes, 1, one_length, predicate_bytes, 1, one_length, 1,
                         nullptr, 1, nullptr, 0, "predicate null range key with nonzero total");

    // Predicate query terms deliberately reject empty keys/values.
    expect_bad_predicate(predicate_bytes, 0, zero_length, predicate_bytes, 1, one_length, 1,
                         nullptr, 0, nullptr, 0, "predicate zero-length feature key rejected");
    expect_bad_predicate(predicate_bytes, 1, one_length, predicate_bytes, 0, zero_length, 1,
                         nullptr, 0, nullptr, 0, "predicate zero-length feature value rejected");
    expect_bad_predicate(nullptr, 0, nullptr, nullptr, 0, nullptr, 0, predicate_bytes, 0,
                         zero_length, 1, "predicate zero-length range key rejected");

    // Count, per-element, aggregate-size, and checked-offset boundaries fail before reads.
    expect_bad_predicate(predicate_bytes, 0, one_length, predicate_bytes, 1, one_length, 65537,
                         nullptr, 0, nullptr, 0, "predicate feature count limit");
    expect_bad_predicate(nullptr, 0, nullptr, nullptr, 0, nullptr, 0, predicate_bytes, 0,
                         one_length, 65537, "predicate range count limit");
    expect_bad_predicate(one_byte_allocation, oversized_term_length[0], oversized_term_length,
                         one_byte_allocation, 1, one_length, 1, nullptr, 0, nullptr, 0,
                         "predicate feature key element size limit");
    expect_bad_predicate(one_byte_allocation, 1, one_length, one_byte_allocation,
                         oversized_term_length[0], oversized_term_length, 1, nullptr, 0, nullptr,
                         0, "predicate feature value element size limit");
    expect_bad_predicate(nullptr, 0, nullptr, nullptr, 0, nullptr, 0, one_byte_allocation,
                         oversized_term_length[0], oversized_term_length, 1,
                         "predicate range key element size limit");
    expect_bad_predicate(predicate_bytes, std::numeric_limits<uint32_t>::max(), one_length,
                         predicate_bytes, 1, one_length, 1, nullptr, 0, nullptr, 0,
                         "predicate feature key packed total size limit");
    expect_bad_predicate(predicate_bytes, 1, one_length, predicate_bytes,
                         std::numeric_limits<uint32_t>::max(), one_length, 1, nullptr, 0, nullptr,
                         0, "predicate feature value packed total size limit");
    expect_bad_predicate(nullptr, 0, nullptr, nullptr, 0, nullptr, 0, predicate_bytes,
                         std::numeric_limits<uint32_t>::max(), one_length, 1,
                         "predicate range key packed total size limit");
    // Exercise the remaining-length check after a prior descriptor advances the
    // offset. Each individual length is valid-sized, but the second span would
    // read one byte past the actual two-byte allocation if checked after access.
    expect_bad_predicate(predicate_bytes, 2, one_two_lengths, predicate_bytes, 2,
                         two_one_lengths, 2, nullptr, 0, nullptr, 0,
                         "predicate feature key exceeds remaining packed bytes");
    expect_bad_predicate(predicate_bytes, 2, two_one_lengths, predicate_bytes, 2,
                         one_two_lengths, 2, nullptr, 0, nullptr, 0,
                         "predicate feature value exceeds remaining packed bytes");
    expect_bad_predicate(nullptr, 0, nullptr, nullptr, 0, nullptr, 0, predicate_bytes, 2,
                         one_two_lengths, 2, "predicate range key exceeds remaining packed bytes");

    // Overflow-shaped lengths are rejected before offset addition as an extra
    // defense, even though the per-element limit makes arithmetic wrap unreachable.
    expect_bad_predicate(predicate_bytes, 2, overflow_lengths, predicate_bytes, 2,
                         two_one_lengths, 2, nullptr, 0, nullptr, 0,
                         "predicate feature key overflow-shaped length");
    expect_bad_predicate(predicate_bytes, 2, two_one_lengths, predicate_bytes, 2,
                         overflow_lengths, 2, nullptr, 0, nullptr, 0,
                         "predicate feature value overflow-shaped length");
    expect_bad_predicate(nullptr, 0, nullptr, nullptr, 0, nullptr, 0, predicate_bytes, 2,
                         overflow_lengths, 2, "predicate range key overflow-shaped length");

    // A completely absent zero-count/zero-total side is valid and must not be dereferenced.
    auto *query = predicate_query();
    expect_ok(sk_query_add_predicate(query, "access", predicate_bytes, 1, one_length,
                                     predicate_bytes, 1, one_length, predicate_masks, 1,
                                     nullptr, 0, nullptr, nullptr, nullptr, 0,
                                     error.data(), ERROR_CAPACITY),
              "predicate null zero-count zero-total range side");
    sk_query_destroy(query);
    query = predicate_query();
    expect_ok(sk_query_add_predicate(query, "access", nullptr, 0, nullptr, nullptr, 0, nullptr,
                                     nullptr, 0, predicate_bytes, 1, one_length,
                                     predicate_values, predicate_masks, 1,
                                     error.data(), ERROR_CAPACITY),
              "predicate null zero-count zero-total feature side");
    sk_query_destroy(query);

    // Zero-count/zero-total sides may also carry non-null sentinels because they are never dereferenced.
    query = predicate_query();
    expect_ok(sk_query_add_predicate(query, "access", predicate_bytes, 1, one_length,
                                     predicate_bytes, 1, one_length, predicate_masks, 1,
                                     predicate_bytes, 0, one_length, predicate_values,
                                     predicate_masks, 0, error.data(), ERROR_CAPACITY),
              "predicate non-null zero-count zero-total range side");
    sk_query_destroy(query);
    query = predicate_query();
    expect_ok(sk_query_add_predicate(query, "access", predicate_bytes, 0, one_length,
                                     predicate_bytes, 0, one_length, predicate_masks, 0,
                                     predicate_bytes, 1, one_length, predicate_values,
                                     predicate_masks, 1, error.data(), ERROR_CAPACITY),
              "predicate non-null zero-count zero-total feature side");
    sk_query_destroy(query);

    query = predicate_query();
    expect_ok(sk_query_add_predicate(query, "access", predicate_bytes, 2, two_one_lengths,
                                     predicate_bytes, 2, two_one_lengths, predicate_masks, 2,
                                     predicate_bytes, 2, two_one_lengths, predicate_values,
                                     predicate_masks, 2, error.data(), ERROR_CAPACITY),
              "predicate exact totals and multiple elements");
    sk_query_destroy(query);
}

void predicate_query_buffer_contract_with_engine(const std::filesystem::path &data,
                                                 const std::filesystem::path &config) {
    ErrorBuffer error;
    sk_engine *engine = nullptr;
    expect_ok(sk_engine_open(data.c_str(), config.c_str(), &engine, error.data(), ERROR_CAPACITY),
              "predicate contract engine open");
    assert(engine != nullptr);
    predicate_query_buffer_contract(engine);
    expect_ok(sk_engine_close(engine, error.data(), ERROR_CAPACITY), "predicate contract engine close");
    sk_engine_destroy(engine);
}

void exercise_native_engine(const std::filesystem::path &data,
                            const std::filesystem::path &config) {
    ErrorBuffer error;
    sk_engine *engine = nullptr;
    const auto invalid_config = config / "invalid-open-config";
    std::filesystem::create_directories(invalid_config);
    {
        std::ofstream manifest(invalid_config / "searchkernel-schema.json");
        manifest << "{}";
    }
    expect(sk_engine_open(data.c_str(), invalid_config.c_str(), &engine,
                          error.data(), ERROR_CAPACITY),
           SK_NATIVE_STATUS_INVALID_ARGUMENT, "invalid generated schema on engine open");
    assert(engine == nullptr);
    std::filesystem::remove_all(invalid_config);

#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    for (const auto *stage : {"open_constructor", "open_initialize"}) {
        for (const auto &[action, expected, label] : {
                 std::tuple{SK_TEST_FAILPOINT_ERROR, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                            "open invalid argument"},
                 std::tuple{SK_TEST_FAILPOINT_BAD_ALLOC, SK_NATIVE_STATUS_OUT_OF_MEMORY,
                            "open out of memory"},
                 std::tuple{SK_TEST_FAILPOINT_STD_EXCEPTION, SK_NATIVE_STATUS_INTERNAL,
                            "open internal failure"},
             }) {
            expect_ok(sk_test_failpoint_set_open(stage, 1, action,
                                                 error.data(), ERROR_CAPACITY),
                      "set open failpoint");
            engine = reinterpret_cast<sk_engine *>(1);
            expect(sk_engine_open(data.c_str(), config.c_str(), &engine,
                                  error.data(), ERROR_CAPACITY),
                   expected, label);
            assert(engine == nullptr);
            expect_ok(sk_test_failpoint_clear_open(stage, error.data(),
                                                    ERROR_CAPACITY),
                      "clear open failpoint");
        }
    }
#endif

    sk_local_grouping_request *local_grouping = nullptr;
    sk_local_grouping_result *local_grouping_result = nullptr;
    expect_ok(sk_engine_open(data.c_str(), config.c_str(), &engine, error.data(), ERROR_CAPACITY),
              "engine open");
    assert(engine != nullptr);

    sk_query *query = nullptr;
    expect_ok(sk_query_create(engine, &query, error.data(), ERROR_CAPACITY), "query metadata");
    expect_ok(sk_query_validate_field(query, SK_QUERY_FIELD_TEXT, TITLE_FIELD, error.data(),
                                     ERROR_CAPACITY), "validate text field");
    expect_ok(sk_query_validate_field(query, SK_QUERY_FIELD_INTEGER, "qty", error.data(),
                                     ERROR_CAPACITY), "validate integer field");
    expect_ok(sk_query_add_true(query, error.data(), ERROR_CAPACITY), "query true");
    sk_query_destroy(query);

    const auto *query_text = reinterpret_cast<const uint8_t *>(QUICK_TERM);
    const uint32_t query_text_lengths[] = {5};
    expect_ok(sk_query_create(engine, &query, error.data(), ERROR_CAPACITY), "text query create");
    expect_ok(sk_query_add_text(query, SK_QUERY_TEXT_TERM, TITLE_FIELD, query_text,
                                sizeof(QUICK_TERM) - 1, query_text_lengths, 1,
                                error.data(), ERROR_CAPACITY), "text query");
    sk_query_destroy(query);
    predicate_query_buffer_contract(engine);

    expect_ok(sk_query_create(engine, &query, error.data(), ERROR_CAPACITY), "range query create");
    expect_ok(sk_query_add_range(query, "qty", SK_QUERY_BOUND_INCLUDED, 1,
                                 SK_QUERY_BOUND_EXCLUDED, 100, error.data(), ERROR_CAPACITY),
              "range query");
    sk_query_destroy(query);

    expect_ok(sk_query_create(engine, &query, error.data(), ERROR_CAPACITY), "attribute exists query create");
    expect_ok(sk_query_add_attribute_exists(query, "qty", error.data(), ERROR_CAPACITY),
              "attribute exists query");
    sk_query_destroy(query);
    expect_ok(sk_query_create(engine, &query, error.data(), ERROR_CAPACITY), "attribute integer range create");
    expect_ok(sk_query_add_attribute_integer_range(query, "qty", SK_QUERY_BOUND_UNBOUNDED, 0,
                                                   SK_QUERY_BOUND_INCLUDED, 100, error.data(),
                                                   ERROR_CAPACITY), "attribute integer range");
    sk_query_destroy(query);
    sk_document *document = nullptr;
    expect_ok(sk_document_create_empty("id:test:typea::coverage", &document,
                                       error.data(), ERROR_CAPACITY), "document create");
    expect_ok(sk_document_add_string(document, TITLE_FIELD, QUICK_FOX_TEXT, error.data(), ERROR_CAPACITY),
              "document string");
    expect_ok(sk_document_add_integer(document, "qty", 17, error.data(), ERROR_CAPACITY),
              "document integer");
    check_document_fields(document);
    expect_ok(sk_engine_put(engine, document, error.data(), ERROR_CAPACITY), "document put");

    sk_document *loaded = nullptr;
    expect_ok(sk_engine_get(engine, "id:test:typea::coverage", &loaded, error.data(), ERROR_CAPACITY),
              "document get");
    check_document_fields(loaded);
    sk_document_destroy(loaded);
    sk_document_destroy(document);

    sk_document_update *update = nullptr;
    expect_ok(sk_document_update_create_empty("id:test:typea::coverage", &update,
                                               error.data(), ERROR_CAPACITY), "update create");
    expect_ok(sk_document_update_set_create_if_nonexistent(update, 1, error.data(),
                                                            ERROR_CAPACITY),
              "update create flag");
    expect_ok(sk_document_update_assign_integer(update, "qty", 18, error.data(), ERROR_CAPACITY),
              "update integer");
    expect_ok(sk_engine_update(engine, update, error.data(), ERROR_CAPACITY), "document update");
    sk_document_update_destroy(update);

    int32_t found = 0;
    expect_ok(sk_engine_remove(engine, "id:test:typea::missing", &found, error.data(), ERROR_CAPACITY),
              "remove missing");
    assert(found == 0);

    sk_search_request *old_request = nullptr;
    expect_ok(sk_search_request_create("qty", 18, 10, &old_request,
                                       error.data(), ERROR_CAPACITY), "old request create");
    sk_search_result *old_result = nullptr;
    expect_ok(sk_engine_search(engine, old_request, &old_result, error.data(), ERROR_CAPACITY),
              "old search");
    assert(sk_search_result_size(old_result) == 1);
    const char *id = nullptr;
    uint32_t id_length = 0;
    expect_ok(sk_search_result_id(old_result, 0, &id, &id_length, error.data(), ERROR_CAPACITY),
              "old result id");
    assert(std::string(id, id_length) == "id:test:typea::coverage");
    expect_invalid(sk_search_result_id(old_result, 1, &id, &id_length, error.data(), ERROR_CAPACITY),
                   "old result bounds");
    sk_search_result_destroy(old_result);
    sk_search_request_destroy(old_request);

    query = make_equals_query(engine);
    sk_text_result *query_result = nullptr;
    expect_ok(sk_engine_search_query(engine, query, "default", "*", &query_result,
                                     error.data(), ERROR_CAPACITY), "query search");
    assert(sk_text_result_size(query_result) == 0 || sk_text_result_size(query_result) == 1);
    sk_text_result_destroy(query_result);
    sk_query_destroy(query);

    query = make_equals_query(engine);
    sk_local_search_request *local = nullptr;
    expect_ok(sk_local_search_request_create("default", 10, 0, SK_SUMMARY_NONE, &local,
                                             error.data(), ERROR_CAPACITY), "local request");
    expect_ok(sk_local_search_request_add_sort(local, "qty", SK_SORT_ASCENDING,
                                               error.data(), ERROR_CAPACITY), "local sort");
    expect_ok(sk_local_search_request_add_query_feature(local, "feature", 1.5,
                                                        error.data(), ERROR_CAPACITY), "local feature");
    expect_invalid(sk_local_search_request_add_query_feature(
                       local, "vespa.matching.numthreadspersearch", 2.0, error.data(),
                       ERROR_CAPACITY),
                   "local reserved query feature");
    expect_ok(sk_local_search_request_add_rank_property(local, "custom", "value",
                                                        error.data(), ERROR_CAPACITY), "local property");
    sk_local_search_result *local_result = nullptr;
    expect_ok(sk_engine_search_local(engine, query, local, &local_result,
                                     error.data(), ERROR_CAPACITY), "local search");
    assert(sk_local_search_result_hit_count(local_result) == 1);
    assert(sk_local_search_result_total_hit_count(local_result) >= 1);
    double score = 0;
    expect_ok(sk_local_search_result_hit(local_result, 0, &id, &id_length, &score,
                                         error.data(), ERROR_CAPACITY), "local hit");
    assert(id_length != 0);
    expect_invalid(sk_local_search_result_hit(local_result, 1, &id, &id_length, &score,
                                              error.data(), ERROR_CAPACITY), "local hit bounds");
    sk_local_search_result_destroy(local_result);
    sk_local_search_request_destroy(local);
    sk_query_destroy(query);

    query = make_equals_query(engine);
    expect_ok(sk_local_search_request_create("default", 10, 0, SK_SUMMARY_DEFAULT, &local,
                                             error.data(), ERROR_CAPACITY), "summary request");
    sk_local_search_result *summary_result = nullptr;
    expect_ok(sk_engine_search_local(engine, query, local, &summary_result,
                                     error.data(), ERROR_CAPACITY), "summary search");
    // Summary fields are schema-dependent. The M11 fixture intentionally
    // exercises the valid empty-summary result, while richer native fixtures
    // exercise the accessor loop below.
    for (uint32_t index = 0; index < sk_local_search_result_summary_count(summary_result, 0);
         ++index) {
        const char *summary_name = nullptr;
        uint32_t summary_name_length = 0;
        expect_ok(sk_local_search_result_summary_name(summary_result, 0, index, &summary_name,
                                                      &summary_name_length, error.data(),
                                                      ERROR_CAPACITY), "summary name");
        assert(summary_name != nullptr && summary_name_length != 0);
        uint32_t summary_kind = 0;
        const char *summary_string = nullptr;
        uint32_t summary_string_length = 0;
        int64_t summary_integer = 0;
        double summary_float = 0;
        uint8_t summary_boolean = 0;
        expect_ok(sk_local_search_result_summary_value(
                      summary_result, 0, index, &summary_kind, &summary_string,
                      &summary_string_length, &summary_integer, &summary_float,
                      &summary_boolean, error.data(), ERROR_CAPACITY),
                  "summary value");
        assert(summary_kind == SK_VALUE_STRING || summary_kind == SK_VALUE_INTEGER ||
               summary_kind == SK_VALUE_FLOAT || summary_kind == SK_VALUE_BOOLEAN);
    }
    expect_invalid(sk_local_search_result_summary_name(summary_result, 1, 0, nullptr, nullptr,
                                                       error.data(), ERROR_CAPACITY),
                   "summary name bounds");
    expect_invalid(sk_local_search_result_summary_value(summary_result, 1, 0, nullptr, nullptr,
                                                        nullptr, nullptr, nullptr, nullptr,
                                                        error.data(), ERROR_CAPACITY),
                   "summary value bounds");
    sk_local_search_result_destroy(summary_result);
    sk_local_search_request_destroy(local);
    sk_query_destroy(query);

    query = make_equals_query(engine);
    expect_ok(sk_local_search_request_create("default", 10, 0, SK_SUMMARY_NONE, &local,
                                             error.data(), ERROR_CAPACITY), "feature request");
    sk_local_search_result *feature_result = nullptr;
    expect_ok(sk_engine_search_local(engine, query, local, &feature_result,
                                     error.data(), ERROR_CAPACITY), "feature search");
    const uint32_t feature_count = sk_local_search_result_match_feature_count(feature_result);
    if (feature_count != 0) {
        const char *feature_name = nullptr;
        uint32_t feature_name_length = 0;
        expect_ok(sk_local_search_result_match_feature_name(feature_result, 0, &feature_name,
                                                            &feature_name_length, error.data(),
                                                            ERROR_CAPACITY), "feature name");
        double feature_value = 0;
        expect_ok(sk_local_search_result_match_feature_value(feature_result, 0, 0,
                                                             &feature_value, error.data(),
                                                             ERROR_CAPACITY), "feature value");
    }
    expect_invalid(sk_local_search_result_match_feature_name(feature_result, feature_count,
                                                              nullptr, nullptr, error.data(),
                                                              ERROR_CAPACITY), "feature bounds");
    sk_local_search_result_destroy(feature_result);
    sk_local_search_request_destroy(local);
    sk_query_destroy(query);

    query = make_equals_query(engine);
    expect_ok(sk_local_grouping_request_create("qty", &local_grouping, error.data(), ERROR_CAPACITY),
              "local grouping request");
    for (uint32_t aggregation = SK_AGGREGATION_SUM;
         aggregation <= SK_AGGREGATION_AVERAGE; ++aggregation) {
        expect_ok(sk_local_grouping_request_add_aggregation(local_grouping, aggregation, "qty",
                                                            error.data(), ERROR_CAPACITY),
                  "local grouping aggregation");
    }
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    expect_ok(sk_test_failpoint_set(engine, "group_before_match", 1,
                                    SK_TEST_FAILPOINT_STD_EXCEPTION, error.data(), ERROR_CAPACITY),
              "group before failpoint");
    expect(sk_engine_group_local(engine, query, local_grouping, &local_grouping_result,
                                 error.data(), ERROR_CAPACITY),
           SK_NATIVE_STATUS_INTERNAL, "group before failure");
    expect_ok(sk_test_failpoint_set(engine, "group_after_match", 1,
                                    SK_TEST_FAILPOINT_UNKNOWN_EXCEPTION, error.data(), ERROR_CAPACITY),
              "group after failpoint");
    expect(sk_engine_group_local(engine, query, local_grouping, &local_grouping_result,
                                 error.data(), ERROR_CAPACITY),
           SK_NATIVE_STATUS_INTERNAL, "group after failure");
#endif
    expect_ok(sk_engine_group_local(engine, query, local_grouping, &local_grouping_result,
                                    error.data(), ERROR_CAPACITY), "local grouping");
    assert(sk_local_grouping_result_group_count(local_grouping_result) != 0);
    uint32_t local_key_kind = 0;
    const char *local_key_string = nullptr;
    uint32_t local_key_string_length = 0;
    int64_t local_key_integer = 0;
    uint64_t local_count = 0;
    expect_ok(sk_local_grouping_result_group(local_grouping_result, 0, &local_key_kind,
                                             &local_key_string, &local_key_string_length,
                                             &local_key_integer, &local_count, error.data(),
                                             ERROR_CAPACITY), "local group");
    assert(local_key_kind == SK_VALUE_INTEGER && local_count != 0);
    for (uint32_t aggregation = 0; aggregation < 4; ++aggregation) {
        uint32_t result_kind = 0;
        int64_t result_integer = 0;
        double result_float = 0;
        expect_ok(sk_local_grouping_result_aggregation(local_grouping_result, 0, aggregation,
                                                       &result_kind, &result_integer,
                                                       &result_float, error.data(), ERROR_CAPACITY),
                  "local aggregation result");
        assert(result_kind == SK_VALUE_INTEGER || result_kind == SK_VALUE_FLOAT);
    }
    expect_invalid(sk_local_grouping_result_group(local_grouping_result, 1, &local_key_kind,
                                                  &local_key_string, &local_key_string_length,
                                                  &local_key_integer, &local_count, error.data(),
                                                  ERROR_CAPACITY), "local group bounds");
    sk_local_grouping_result_destroy(local_grouping_result);
    sk_local_grouping_request_destroy(local_grouping);
    sk_query_destroy(query);

    query = make_equals_query(engine);
    sk_generic_search_request *generic = nullptr;
    expect_ok(sk_generic_search_request_create("default", 10, 0, SK_SUMMARY_NONE, &generic,
                                               error.data(), ERROR_CAPACITY), "generic request");
    expect_ok(sk_generic_search_request_set_timeout(generic, 1000, error.data(), ERROR_CAPACITY),
              "generic timeout");
    expect_ok(sk_generic_search_request_add_sort(generic, "qty", SK_SORT_DESCENDING,
                                                 SK_SORT_MISSING_DEFAULT, error.data(), ERROR_CAPACITY),
              "generic sort");
    expect_ok(sk_generic_search_request_add_query_feature(generic, "feature", 2.5,
                                                          error.data(), ERROR_CAPACITY), "generic feature");
    expect_invalid(sk_generic_search_request_add_query_feature(
                       generic, "vespa.matching.numthreadspersearch", 2.0, error.data(),
                       ERROR_CAPACITY),
                   "generic reserved query feature");
    expect_ok(sk_generic_search_request_add_rank_property(generic, "custom", "value",
                                                          error.data(), ERROR_CAPACITY), "generic property");
    expect_ok(sk_generic_search_request_add_grouping(generic, "qty", 10, SK_GROUP_ORDER_NONE,
                                                     error.data(), ERROR_CAPACITY), "generic grouping");
    sk_generic_search_result *generic_result = nullptr;
    expect_ok(sk_engine_search_generic(engine, query, generic, &generic_result,
                                       error.data(), ERROR_CAPACITY), "generic search");
    assert(sk_generic_search_result_hit_count(generic_result) == 1);
    assert(sk_generic_search_result_coverage_active(generic_result) > 0);
    assert(sk_generic_search_result_coverage_covered(generic_result) > 0);
    assert(sk_generic_search_result_coverage_target_active(generic_result) > 0);
    assert(sk_generic_search_result_degradation_flags(generic_result) == 0);
    assert(sk_generic_search_result_issue_count(generic_result) == 0);
    assert(sk_generic_search_result_grouping_count(generic_result) == 1);
    assert(sk_generic_search_result_group_count(generic_result, 0) >= 1);
    const char *generic_id = nullptr;
    uint32_t generic_id_length = 0;
    double generic_score = 0;
    expect_ok(sk_generic_search_result_hit(generic_result, 0, &generic_id, &generic_id_length,
                                           &generic_score, error.data(), ERROR_CAPACITY),
              "generic hit");
    assert(generic_id_length != 0);
    expect_invalid(sk_generic_search_result_hit(generic_result, 1, &generic_id, &generic_id_length,
                                                &generic_score, error.data(), ERROR_CAPACITY),
                   "generic hit bounds");
    uint32_t key_kind = 0;
    const char *key_string = nullptr;
    uint32_t key_string_length = 0;
    int64_t key_integer = 0;
    uint64_t group_count = 0;
    expect_ok(sk_generic_search_result_group(generic_result, 0, 0, &key_kind, &key_string,
                                             &key_string_length, &key_integer, &group_count,
                                             error.data(), ERROR_CAPACITY), "generic group");
    assert(key_kind == SK_VALUE_INTEGER && group_count != 0);
    expect_invalid(sk_generic_search_result_group(generic_result, 1, 0, &key_kind, &key_string,
                                                  &key_string_length, &key_integer, &group_count,
                                                  error.data(), ERROR_CAPACITY), "generic group bounds");
    const char *issue_message = nullptr;
    uint32_t issue_length = 0;
    expect_invalid(sk_generic_search_result_issue_message(
                       generic_result, 0, &issue_message, &issue_length,
                       error.data(), ERROR_CAPACITY),
                   "generic issue bounds");
    sk_generic_search_result_destroy(generic_result);
    sk_generic_search_request_destroy(generic);
    sk_query_destroy(query);

    for (uint32_t index = 0; index < 20; ++index) {
        const auto document_id = "id:test:typea::degraded-" + std::to_string(index);
        sk_document *degraded_document = nullptr;
        expect_ok(sk_document_create_empty(document_id.c_str(), &degraded_document,
                                           error.data(), ERROR_CAPACITY),
                  "degraded document create");
        expect_ok(sk_document_add_integer(degraded_document, "qty", 18,
                                          error.data(), ERROR_CAPACITY),
                  "degraded document value");
        expect_ok(sk_engine_put(engine, degraded_document, error.data(), ERROR_CAPACITY),
                  "degraded document put");
        sk_document_destroy(degraded_document);
    }
    query = make_equals_query(engine);
    generic = nullptr;
    expect_ok(sk_generic_search_request_create("default", 10, 0, SK_SUMMARY_NONE, &generic,
                                               error.data(), ERROR_CAPACITY),
              "degraded request");
    expect_ok(sk_generic_search_request_add_sort(generic, "qty", SK_SORT_ASCENDING,
                                                 SK_SORT_MISSING_DEFAULT, error.data(),
                                                 ERROR_CAPACITY),
              "degraded sort");
    expect_ok(sk_generic_search_request_add_rank_property(
                  generic, "vespa.matchphase.degradation.maxfiltercoverage", "0.9",
                  error.data(), ERROR_CAPACITY),
              "degraded match-phase property");
    generic_result = nullptr;
    expect_ok(sk_engine_search_generic(engine, query, generic, &generic_result,
                                       error.data(), ERROR_CAPACITY),
              "valid degraded search");
    assert(generic_result != nullptr);
    assert(sk_generic_search_result_hit_count(generic_result) > 0);
    assert(sk_generic_search_result_coverage_active(generic_result) > 0);
    assert(sk_generic_search_result_coverage_covered(generic_result) > 0);
    assert(sk_generic_search_result_coverage_target_active(generic_result) > 0);
    assert((sk_generic_search_result_degradation_flags(generic_result) &
            SK_DEGRADATION_MATCH_PHASE) != 0);
    sk_generic_search_result_destroy(generic_result);
    sk_generic_search_request_destroy(generic);
    sk_query_destroy(query);

    query = make_equals_query(engine);
    generic = nullptr;
    expect_ok(sk_generic_search_request_create("missing-rank-profile", 10, 0,
                                               SK_SUMMARY_NONE, &generic,
                                               error.data(), ERROR_CAPACITY),
              "hard failure request");
    generic_result = reinterpret_cast<sk_generic_search_result *>(1);
    expect(sk_engine_search_generic(engine, query, generic, &generic_result,
                                    error.data(), ERROR_CAPACITY),
           SK_NATIVE_STATUS_INVALID_ARGUMENT, "hard native search failure");
    assert(generic_result == nullptr);
    assert(error.bytes[0] != 0);
    sk_generic_search_request_destroy(generic);
    sk_query_destroy(query);

    sk_attribute_document *attributes = nullptr;
    expect_ok(sk_attribute_document_create("id:test:typea::attributes", &attributes,
                                           error.data(), ERROR_CAPACITY), "attribute document");
    expect_ok(sk_attribute_document_add_integer(attributes, "qty", 19, error.data(), ERROR_CAPACITY),
              "attribute integer");
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    expect_ok(sk_test_failpoint_set(engine, "attribute_mutation_before_native_call", 1,
                                    SK_TEST_FAILPOINT_ERROR, error.data(), ERROR_CAPACITY),
              "attribute before failpoint");
    expect(sk_engine_put_attributes(engine, attributes, error.data(), ERROR_CAPACITY),
           SK_NATIVE_STATUS_INVALID_ARGUMENT, "attribute before failure");
    expect_ok(sk_test_failpoint_set(engine, "attribute_mutation_after_native_call", 1,
                                    SK_TEST_FAILPOINT_STD_EXCEPTION, error.data(), ERROR_CAPACITY),
              "attribute after failpoint");
    expect(sk_engine_put_attributes(engine, attributes, error.data(), ERROR_CAPACITY),
           SK_NATIVE_STATUS_INTERNAL, "attribute after failure");
#endif
    expect_ok(sk_engine_put_attributes(engine, attributes, error.data(), ERROR_CAPACITY),
              "attribute put");
    sk_attribute_document_destroy(attributes);

    expect_ok(sk_attribute_document_create("id:test:typea::attribute-builders", &attributes,
                                           error.data(), ERROR_CAPACITY), "attribute builder document");
    expect_ok(sk_attribute_document_add_string(attributes, "qty", "19",
                                               error.data(), ERROR_CAPACITY), "attribute string");
    expect_ok(sk_attribute_document_add_float(attributes, "qty", 2.0, error.data(), ERROR_CAPACITY),
              "attribute float");
    expect_ok(sk_attribute_document_add_boolean(attributes, "qty", 1, error.data(), ERROR_CAPACITY),
              "attribute boolean");
    sk_attribute_document_destroy(attributes);

    sk_attribute_update *attribute_update = nullptr;
    expect_ok(sk_attribute_update_create("id:test:typea::attributes", &attribute_update,
                                         error.data(), ERROR_CAPACITY), "attribute update");
    expect_ok(sk_attribute_update_add_string(attribute_update, "qty", "20",
                                             error.data(), ERROR_CAPACITY), "attribute update string");
    expect_ok(sk_attribute_update_add_integer(attribute_update, "qty", 20,
                                              error.data(), ERROR_CAPACITY), "attribute update integer");
    expect_ok(sk_attribute_update_add_float(attribute_update, "qty", 3.0,
                                            error.data(), ERROR_CAPACITY), "attribute update float");
    expect_ok(sk_attribute_update_add_boolean(attribute_update, "qty", 0,
                                              error.data(), ERROR_CAPACITY), "attribute update boolean");
    sk_attribute_update_destroy(attribute_update);

    expect_ok(sk_attribute_update_create("id:test:typea::attributes", &attribute_update,
                                         error.data(), ERROR_CAPACITY), "attribute integer update");
    expect_ok(sk_attribute_update_add_integer(attribute_update, "qty", 20,
                                              error.data(), ERROR_CAPACITY), "attribute update native integer");
    expect_ok(sk_engine_update_attributes(engine, attribute_update, error.data(), ERROR_CAPACITY),
              "attribute update call");
    sk_attribute_update_destroy(attribute_update);

    sk_text_document *text_document = nullptr;
    const auto *body = reinterpret_cast<const uint8_t *>(QUICK_FOX_TEXT);
    const auto *terms = reinterpret_cast<const uint8_t *>(QUICKFOX_TERM);
    const uint32_t term_lengths[] = {5, 3};
    const uint32_t starts[] = {0, 6};
    const uint32_t lengths[] = {5, 3};
    expect_ok(sk_text_document_create("id:test:typea::text", TITLE_FIELD, body,
                                      sizeof(QUICK_FOX_TEXT) - 1,
                                      &text_document, error.data(), ERROR_CAPACITY), "text document");
    expect_ok(sk_text_document_add_occurrences(text_document, terms, sizeof(QUICKFOX_TERM) - 1,
                                               term_lengths, starts, lengths, 2,
                                               error.data(), ERROR_CAPACITY), "text occurrences");
    expect_ok(sk_engine_put_text(engine, text_document, error.data(), ERROR_CAPACITY), "text put");
    sk_text_document_destroy(text_document);

    sk_text_request *text_request = nullptr;
    expect_ok(sk_text_request_create(SK_TEXT_QUERY_TERM, TITLE_FIELD, "default", &text_request,
                                     error.data(), ERROR_CAPACITY), "text request");
    const auto *query_terms = reinterpret_cast<const uint8_t *>(QUICK_TERM);
    const uint32_t query_lengths[] = {5};
    expect_ok(sk_text_request_add_terms(text_request, query_terms, sizeof(QUICK_TERM) - 1,
                                        query_lengths, 1, error.data(), ERROR_CAPACITY), "text terms");
    sk_text_result *text_result = nullptr;
    expect_ok(sk_engine_search_text(engine, text_request, &text_result,
                                    error.data(), ERROR_CAPACITY), "text search");
    if (sk_text_result_size(text_result) != 0) {
        expect_ok(sk_text_result_hit(text_result, 0, &id, &id_length, &score,
                                     error.data(), ERROR_CAPACITY), "text hit");
    }
    sk_text_result_destroy(text_result);
    sk_text_request_destroy(text_request);

    expect_ok(sk_engine_close(engine, error.data(), ERROR_CAPACITY), "engine close");
    expect_invalid(sk_engine_close(engine, error.data(), ERROR_CAPACITY), "engine close twice");
    sk_engine_destroy(engine);
}

} // namespace

int main(int argc, char **argv) {
    highlighting_decoder_unit();
    packed_document_buffer_contract();
    if (std::getenv("SK_NATIVE_HIGHLIGHTING_UNIT_ONLY") != nullptr) {
        std::cout << "native highlighting decoder unit passed\n";
        return 0;
    }
    if (std::getenv("SK_NATIVE_PREDICATE_CONTRACT_ONLY") != nullptr) {
        if (argc != 3) {
            std::cerr << "usage: native_adapter_coverage_test DATA_DIR CONFIG_DIR\n";
            return 2;
        }
        assert(sk_native_abi_version() == SK_NATIVE_ABI_VERSION);
        predicate_query_buffer_contract_with_engine(std::filesystem::path(argv[1]),
                                                    std::filesystem::path(argv[2]));
        std::cout << "native predicate packed-buffer ABI contract passed\n";
        return 0;
    }
    if (argc != 3) {
        std::cerr << "usage: native_adapter_coverage_test DATA_DIR CONFIG_DIR\n";
        return 2;
    }
    assert(sk_native_abi_version() == SK_NATIVE_ABI_VERSION);
    invalid_argument_surface();
    exercise_native_engine(std::filesystem::path(argv[1]), std::filesystem::path(argv[2]));
    std::cout << "native adapter coverage harness passed\n";
    return 0;
}
