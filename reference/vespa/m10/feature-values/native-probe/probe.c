#include "searchkernel_vespa.h"

#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define ERR_CAP SK_ERROR_MESSAGE_CAPACITY

static uint8_t error_buf[ERR_CAP];

static void die(const char *what, int32_t status) {
    fprintf(stderr, "%s failed: status=%" PRId32 " error=%s\n", what, status,
            (const char *)error_buf);
    exit(2);
}

static void ok(const char *what, int32_t status) {
    if (status != SK_NATIVE_STATUS_OK) die(what, status);
}

static void clear_error(void) { memset(error_buf, 0, sizeof(error_buf)); }

static void json_string(const char *value, uint32_t length) {
    putchar('"');
    for (uint32_t i = 0; i < length; ++i) {
        const unsigned char c = (unsigned char)value[i];
        if (c == '"' || c == '\\') {
            putchar('\\');
            putchar((char)c);
        } else if (c == '\n') {
            fputs("\\n", stdout);
        } else if (c == '\r') {
            fputs("\\r", stdout);
        } else if (c == '\t') {
            fputs("\\t", stdout);
        } else if (c < 0x20) {
            printf("\\u%04x", c);
        } else {
            putchar((char)c);
        }
    }
    putchar('"');
}

static void emit_data(const uint8_t *data, size_t size) {
    fputs("{\"kind\":\"data\",\"hex\":\"", stdout);
    for (size_t i = 0; i < size; ++i) printf("%02x", data[i]);
    fputs("\"}", stdout);
}

static void emit_feature(const sk_generic_search_result *result, uint32_t hit,
                         uint32_t feature) {
    const sk_feature_value *generic_value = NULL;
    const sk_feature_value *local_value = NULL;
    clear_error();
    ok("generic feature ref", sk_generic_search_result_match_feature_value_ref(
                                  result, hit, feature, &generic_value, error_buf, ERR_CAP));
    clear_error();
    ok("local feature ref", sk_local_search_result_match_feature_value_ref(
                                (const sk_local_search_result *)result, hit, feature,
                                &local_value, error_buf, ERR_CAP));
    if (!generic_value || !local_value ||
        sk_feature_value_kind(generic_value) != sk_feature_value_kind(local_value)) {
        fprintf(stderr, "generic/local feature refs disagree\n");
        exit(2);
    }

    const uint32_t kind = sk_feature_value_kind(generic_value);
    if (kind == SK_FEATURE_VALUE_DOUBLE) {
        double generic_double = 0.0;
        double local_double = 0.0;
        clear_error();
        ok("generic feature double", sk_feature_value_double(
                                           generic_value, &generic_double, error_buf, ERR_CAP));
        clear_error();
        ok("local feature double", sk_feature_value_double(
                                         local_value, &local_double, error_buf, ERR_CAP));
        uint64_t generic_bits = 0;
        uint64_t local_bits = 0;
        memcpy(&generic_bits, &generic_double, sizeof(generic_bits));
        memcpy(&local_bits, &local_double, sizeof(local_bits));
        if (generic_bits != local_bits) {
            fprintf(stderr, "generic/local double values disagree\n");
            exit(2);
        }
        double legacy = 0.0;
        clear_error();
        ok("generic legacy scalar feature", sk_generic_search_result_match_feature_value(
                                                  result, hit, feature, &legacy, error_buf, ERR_CAP));
        uint64_t legacy_bits = 0;
        memcpy(&legacy_bits, &legacy, sizeof(legacy_bits));
        if (legacy_bits != generic_bits) {
            fprintf(stderr, "legacy scalar value differs from typed value\n");
            exit(2);
        }
        double local_legacy = 0.0;
        clear_error();
        ok("local legacy scalar feature", sk_local_search_result_match_feature_value(
                                                   (const sk_local_search_result *)result, hit, feature,
                                                   &local_legacy, error_buf, ERR_CAP));
        uint64_t local_legacy_bits = 0;
        memcpy(&local_legacy_bits, &local_legacy, sizeof(local_legacy_bits));
        if (local_legacy_bits != generic_bits) {
            fprintf(stderr, "local legacy scalar value differs from typed value\n");
            exit(2);
        }
        printf("{\"kind\":\"double\",\"bits\":\"0x%016" PRIx64 "\"}", generic_bits);
    } else if (kind == SK_FEATURE_VALUE_DATA) {
        const uint8_t *generic_data = NULL;
        const uint8_t *local_data = NULL;
        size_t generic_size = 0;
        size_t local_size = 0;
        clear_error();
        ok("generic feature data", sk_feature_value_data(
                                        generic_value, &generic_data, &generic_size,
                                        error_buf, ERR_CAP));
        clear_error();
        ok("local feature data", sk_feature_value_data(
                                      local_value, &local_data, &local_size,
                                      error_buf, ERR_CAP));
        if (generic_size != local_size ||
            (generic_size != 0 && memcmp(generic_data, local_data, generic_size) != 0)) {
            fprintf(stderr, "generic/local data values disagree\n");
            exit(2);
        }
        double legacy = 0.0;
        clear_error();
        const int32_t generic_status = sk_generic_search_result_match_feature_value(
            result, hit, feature, &legacy, error_buf, ERR_CAP);
        if (generic_status != SK_NATIVE_STATUS_TYPE_ERROR) {
            die("generic legacy data rejection", generic_status);
        }
        clear_error();
        const int32_t local_status = sk_local_search_result_match_feature_value(
            (const sk_local_search_result *)result, hit, feature, &legacy,
            error_buf, ERR_CAP);
        if (local_status != SK_NATIVE_STATUS_TYPE_ERROR) {
            die("local legacy data rejection", local_status);
        }
        emit_data(generic_data, generic_size);
    } else {
        fprintf(stderr, "unexpected feature value kind: %" PRIu32 "\n", kind);
        exit(2);
    }
}

int main(int argc, char **argv) {
    if (argc != 3) {
        fprintf(stderr, "usage: probe DATA_DIR CONFIG_DIR\n");
        return 2;
    }

    sk_engine *engine = NULL;
    sk_query *query = NULL;
    sk_generic_search_request *request = NULL;
    sk_generic_search_result *result = NULL;
    const uint8_t terms[] = {'f', 'o', 'x'};
    const uint32_t lengths[] = {3};

    clear_error();
    ok("engine open", sk_engine_open(argv[1], argv[2], &engine, error_buf, ERR_CAP));
    clear_error();
    ok("query create", sk_query_create(engine, &query, error_buf, ERR_CAP));
    clear_error();
    ok("query text", sk_query_add_text(query, SK_QUERY_TEXT_TERM, "title", terms,
                                        sizeof(terms), lengths, 1, error_buf, ERR_CAP));
    clear_error();
    ok("request create", sk_generic_search_request_create(
                              "feature_values", 10, 0, SK_SUMMARY_NONE,
                              &request, error_buf, ERR_CAP));
    clear_error();
    const double query_tensor[] = {1.25, -2.5};
    ok("query tensor", sk_generic_search_request_add_query_tensor_dense(
                              request, "embedding", "tensor<float>(x[2])",
                              query_tensor, 2, error_buf, ERR_CAP));
    clear_error();
    ok("generic search", sk_engine_search_generic(
                              engine, query, request, &result, error_buf, ERR_CAP));
    if (sk_generic_search_result_hit_count(result) != 1 ||
        sk_generic_search_result_match_feature_count(result) != 2) {
        fprintf(stderr, "unexpected feature-value result shape\n");
        return 2;
    }

    const char *id = NULL;
    uint32_t id_length = 0;
    double score = 0.0;
    clear_error();
    ok("result hit", sk_generic_search_result_hit(result, 0, &id, &id_length,
                                                    &score, error_buf, ERR_CAP));
    uint64_t score_bits = 0;
    memcpy(&score_bits, &score, sizeof(score_bits));
    printf("{\"id\":\"double-and-data-feature-values\",\"steps\":[{\"id\":\"search\",\"op\":\"search\",\"total_hit_count\":%" PRIu64 ",\"hits\":[{\"document_id\":",
           sk_generic_search_result_total_hit_count(result));
    json_string(id, id_length);
    printf(",\"score_bits\":\"0x%016" PRIx64 "\",\"match_features\":{", score_bits);
    for (uint32_t feature = 0; feature < 2; ++feature) {
        if (feature) putchar(',');
        const char *name = NULL;
        uint32_t name_length = 0;
        clear_error();
        ok("feature name", sk_generic_search_result_match_feature_name(
                                  result, feature, &name, &name_length, error_buf, ERR_CAP));
        json_string(name, name_length);
        putchar(':');
        emit_feature(result, 0, feature);
    }
    fputs("}}]}]}\n", stdout);

    sk_generic_search_result_destroy(result);
    sk_generic_search_request_destroy(request);
    sk_query_destroy(query);
    sk_engine_destroy(engine);
    return 0;
}
