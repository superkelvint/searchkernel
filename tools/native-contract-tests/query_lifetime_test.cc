#include "searchkernel_vespa.h"

#include <algorithm>
#include <cstdint>
#include <cmath>
#include <iomanip>
#include <limits>
#include <locale>
#include <mutex>
#include <sstream>
#include <string>

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

extern "C" int searchkernel_query_single_use_regression(const char *data_dir,
                                                               const char *config_dir) {
    uint8_t error[SK_ERROR_MESSAGE_CAPACITY] = {};
    sk_engine *engine = nullptr;
    if (sk_engine_open(data_dir, config_dir, &engine, error, sizeof(error)) !=
        SK_NATIVE_STATUS_OK) {
        return 1;
    }

    auto make_true_query = [&](sk_query **out) -> int32_t {
        if (sk_query_create(engine, out, error, sizeof(error)) != SK_NATIVE_STATUS_OK) {
            return 2;
        }
        if (sk_query_add_true(*out, error, sizeof(error)) != SK_NATIVE_STATUS_OK) {
            sk_query_destroy(*out);
            *out = nullptr;
            return 3;
        }
        return 0;
    };

    sk_query *query = nullptr;
    if (const auto code = make_true_query(&query); code != 0) {
        sk_engine_destroy(engine);
        return code;
    }

    sk_text_result *text_result = nullptr;
    if (sk_engine_search_query(engine, query, "default", "*", &text_result, error,
                               sizeof(error)) != SK_NATIVE_STATUS_OK ||
        !text_result) {
        sk_query_destroy(query);
        sk_engine_destroy(engine);
        if (text_result) sk_text_result_destroy(text_result);
        return 4;
    }
    sk_text_result_destroy(text_result);
    text_result = nullptr;

    if (sk_engine_search_query(engine, query, "default", "*", &text_result, error,
                               sizeof(error)) != SK_NATIVE_STATUS_INVALID_ARGUMENT ||
        text_result != nullptr) {
        if (text_result) sk_text_result_destroy(text_result);
        sk_query_destroy(query);
        sk_engine_destroy(engine);
        return 5;
    }
    if (sk_query_add_true(query, error, sizeof(error)) !=
        SK_NATIVE_STATUS_INVALID_ARGUMENT) {
        sk_query_destroy(query);
        sk_engine_destroy(engine);
        return 6;
    }
    sk_query_destroy(query);

    query = nullptr;
    if (const auto code = make_true_query(&query); code != 0) {
        sk_engine_destroy(engine);
        return code + 10;
    }
    if (sk_engine_search_query(nullptr, query, "default", "*", &text_result, error,
                               sizeof(error)) != SK_NATIVE_STATUS_INVALID_ARGUMENT) {
        sk_query_destroy(query);
        sk_engine_destroy(engine);
        return 14;
    }
    if (sk_query_add_true(query, error, sizeof(error)) !=
        SK_NATIVE_STATUS_INVALID_ARGUMENT) {
        sk_query_destroy(query);
        sk_engine_destroy(engine);
        return 15;
    }
    sk_query_destroy(query);

    query = nullptr;
    if (const auto code = make_true_query(&query); code != 0) {
        sk_engine_destroy(engine);
        return code + 20;
    }
    sk_generic_search_request *request = nullptr;
    if (sk_generic_search_request_create("default", 10, 0, SK_SUMMARY_NONE, &request,
                                         error, sizeof(error)) != SK_NATIVE_STATUS_OK) {
        sk_query_destroy(query);
        sk_engine_destroy(engine);
        return 24;
    }
    sk_generic_search_result *generic_result = nullptr;
    if (sk_engine_search_generic(engine, query, request, &generic_result, error,
                                 sizeof(error)) != SK_NATIVE_STATUS_OK ||
        !generic_result) {
        if (generic_result) sk_generic_search_result_destroy(generic_result);
        sk_generic_search_request_destroy(request);
        sk_query_destroy(query);
        sk_engine_destroy(engine);
        return 25;
    }
    sk_generic_search_result_destroy(generic_result);
    generic_result = nullptr;
    if (sk_query_add_true(query, error, sizeof(error)) !=
        SK_NATIVE_STATUS_INVALID_ARGUMENT) {
        sk_generic_search_request_destroy(request);
        sk_query_destroy(query);
        sk_engine_destroy(engine);
        return 26;
    }
    if (sk_engine_search_generic(engine, query, request, &generic_result, error,
                                 sizeof(error)) != SK_NATIVE_STATUS_INVALID_ARGUMENT ||
        generic_result != nullptr) {
        if (generic_result) sk_generic_search_result_destroy(generic_result);
        sk_generic_search_request_destroy(request);
        sk_query_destroy(query);
        sk_engine_destroy(engine);
        return 27;
    }

    sk_generic_search_request_destroy(request);
    sk_query_destroy(query);
    sk_engine_destroy(engine);
    return 0;
}

namespace {

struct CommaDecimalNumPut final : std::num_put<char> {
    iter_type do_put(iter_type output, std::ios_base &, char_type, double value) const override {
        std::ostringstream encoded;
        encoded.imbue(std::locale::classic());
        encoded << std::setprecision(std::numeric_limits<double>::max_digits10) << value;
        auto text = encoded.str();
        std::replace(text.begin(), text.end(), '.', ',');
        return std::copy(text.begin(), text.end(), output);
    }
};

struct GlobalLocaleGuard {
    explicit GlobalLocaleGuard(const std::locale &replacement)
        : previous(std::locale::global(replacement)) {}

    ~GlobalLocaleGuard() noexcept { std::locale::global(previous); }

    std::locale previous;
};

} // namespace

extern "C" int searchkernel_query_feature_locale_regression(const char *data_dir,
                                                              const char *config_dir) {
    static std::mutex locale_mutex;
    std::lock_guard<std::mutex> lock(locale_mutex);

    uint8_t error[SK_ERROR_MESSAGE_CAPACITY] = {};
    sk_engine *engine = nullptr;
    if (sk_engine_open(data_dir, config_dir, &engine, error, sizeof(error)) !=
        SK_NATIVE_STATUS_OK) {
        return 1;
    }

    sk_query *query = nullptr;
    if (sk_query_create(engine, &query, error, sizeof(error)) != SK_NATIVE_STATUS_OK) {
        sk_engine_destroy(engine);
        return 2;
    }
    if (sk_query_add_true(query, error, sizeof(error)) != SK_NATIVE_STATUS_OK) {
        sk_query_destroy(query);
        sk_engine_destroy(engine);
        return 3;
    }

    sk_generic_search_request *request = nullptr;
    if (sk_generic_search_request_create("locale", 10, 0, SK_SUMMARY_NONE, &request, error,
                                         sizeof(error)) != SK_NATIVE_STATUS_OK) {
        sk_query_destroy(query);
        sk_engine_destroy(engine);
        return 4;
    }
    if (sk_generic_search_request_add_query_feature(request, "query(locale_probe)", 1.5, error,
                                                    sizeof(error)) != SK_NATIVE_STATUS_OK) {
        sk_generic_search_request_destroy(request);
        sk_query_destroy(query);
        sk_engine_destroy(engine);
        return 5;
    }

    sk_generic_search_result *result = nullptr;
    int32_t status = SK_NATIVE_STATUS_INTERNAL;
    {
        const std::locale comma_locale(std::locale::classic(), new CommaDecimalNumPut);
        GlobalLocaleGuard locale_guard(comma_locale);
        status = sk_engine_search_generic(engine, query, request, &result, error, sizeof(error));
    }
    sk_generic_search_request_destroy(request);
    sk_query_destroy(query);
    if (status != SK_NATIVE_STATUS_OK || !result) {
        sk_engine_destroy(engine);
        if (result) sk_generic_search_result_destroy(result);
        return 6;
    }

    if (sk_generic_search_result_hit_count(result) != 1) {
        sk_generic_search_result_destroy(result);
        sk_engine_destroy(engine);
        return 7;
    }
    const char *id = nullptr;
    uint32_t id_length = 0;
    double score = 0.0;
    if (sk_generic_search_result_hit(result, 0, &id, &id_length, &score, error,
                                     sizeof(error)) != SK_NATIVE_STATUS_OK) {
        sk_generic_search_result_destroy(result);
        sk_engine_destroy(engine);
        return 8;
    }
    if (std::string(id, id_length) != "locale-1") {
        sk_generic_search_result_destroy(result);
        sk_engine_destroy(engine);
        return 9;
    }
    if (std::abs(score - 1.5) > 1e-12) {
        sk_generic_search_result_destroy(result);
        sk_engine_destroy(engine);
        return 10;
    }
    sk_generic_search_result_destroy(result);
    sk_engine_destroy(engine);
    return 0;
}
