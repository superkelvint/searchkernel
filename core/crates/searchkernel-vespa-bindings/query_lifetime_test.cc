#include "searchkernel_vespa.h"

#include <cstdint>

extern "C" int searchkernel_query_outlives_engine_regression(const char *data_dir,
                                                               const char *config_dir) {
    uint8_t error[SK_ERROR_MESSAGE_CAPACITY] = {};
    sk_engine *engine = nullptr;
    sk_query *query = nullptr;

    if (sk_engine_open(data_dir, config_dir, &engine, error, sizeof(error)) !=
        SK_NATIVE_STATUS_OK) {
        return 1;
    }
    if (sk_query_create(engine, &query, error, sizeof(error)) != SK_NATIVE_STATUS_OK) {
        sk_engine_destroy(engine);
        return 2;
    }

    // The query must own all state needed by these operations. In particular,
    // no operation below may dereference the destroyed engine.
    sk_engine_destroy(engine);

    if (sk_query_validate_field(query, SK_QUERY_FIELD_TEXT, "title", error, sizeof(error)) !=
        SK_NATIVE_STATUS_OK) {
        sk_query_destroy(query);
        return 3;
    }

    const uint8_t term[] = {'f', 'o', 'x'};
    const uint32_t term_length[] = {3};
    if (sk_query_add_text(query, SK_QUERY_TEXT_TERM, "title", term, sizeof(term),
                          term_length, 1, error, sizeof(error)) != SK_NATIVE_STATUS_OK) {
        sk_query_destroy(query);
        return 4;
    }
    sk_query_destroy(query);
    return 0;
}
