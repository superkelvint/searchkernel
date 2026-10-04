#include "searchkernel_vespa_internal.h"

// The adapter is intentionally built as one unity translation unit. The
// included subsystem files are physically separate for maintenance/search,
// while this keeps the private C++ object graph and exception boundary
// identical to the pre-split implementation.
namespace {
std::string encode_dense_tensor(const char *type_spec, const double *values,
                                uint32_t value_count);
std::vector<double> dense_tensor_cells(const vespalib::eval::Value &tensor);
OwnedValue make_dense_tensor_value(const char *type_spec, const double *values,
                                   uint32_t value_count);
std::chrono::milliseconds remaining_search_timeout(
    std::chrono::milliseconds budget,
    std::chrono::steady_clock::duration elapsed) noexcept;
std::string query_stack_dump(int64_t value, const std::string &field);
void copy_search_reply_metadata(const SearchReply &reply, LocalSearchResultState &result);
void append_text_occurrences(TextDocument &, const uint8_t *, uint32_t,
                             const uint32_t *, const uint32_t *, const uint32_t *,
                             uint32_t);
std::vector<std::vector<TextOccurrence>> decode_text_array_occurrences(
    const std::vector<std::string> &, const uint8_t *, uint32_t, const uint32_t *,
    const uint32_t *, const uint32_t *, const uint32_t *, uint32_t, uint32_t);
std::unique_ptr<sk_grouping_tree_node> grouping_tree_node(
    const Group &group, bool include_key);
}

#include "hardening.cc"

namespace {
#include "engine.cc"
#include "tensor.cc"
#include "errors.cc"
#include "query.cc"
#include "grouping.cc"
#include "search.cc"
}

struct sk_engine {
    std::unique_ptr<EmbeddedEngine> impl;
};
struct sk_document : ValueDocument {};
struct sk_document_update : ValueUpdate {};
struct sk_search_request : ValueRequest {};
struct sk_search_result : ErrorResult {};
struct sk_text_document : TextDocument {};
struct sk_text_update : TextUpdate {};
struct sk_text_request : TextRequest {};
struct sk_text_result : TextResult {};
struct sk_query : NativeQueryState {};
struct sk_local_search_request : LocalSearchRequestState {};
struct sk_local_search_result : LocalSearchResultState {
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    ~sk_local_search_result() {
        if (result_destroyed) {
            result_destroyed->fetch_add(1);
        }
    }
#endif
};
struct sk_attribute_document : AttributeDocumentState {};
struct sk_attribute_update : AttributeUpdateState {};
struct sk_local_grouping_request : LocalGroupingRequestState {};
struct sk_local_grouping_result : LocalGroupingResultState {
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    ~sk_local_grouping_result() {
        if (result_destroyed) {
            result_destroyed->fetch_add(1);
        }
    }
#endif
};
struct sk_generic_search_request : sk_local_search_request {};
struct sk_generic_grouping_request : sk_local_grouping_request {};

template <typename Function>
void run_open_stage(const char *stage, Function &&function) {
    try {
        function();
    } catch (const std::bad_alloc &) {
        // Preserve the allocation-failure domain. Constructing contextual
        // diagnostics here could itself allocate and would obscure OOM.
        throw;
    } catch (const UnsupportedNativeError &exception) {
        throw UnsupportedNativeError(
            exception.detail_code(), std::string(stage) + ": " + exception.what());
    } catch (const DeadlineExceededNativeError &exception) {
        throw DeadlineExceededNativeError(std::string(stage) + ": " + exception.what());
    } catch (const TypeErrorNativeError &exception) {
        throw TypeErrorNativeError(std::string(stage) + ": " + exception.what());
    } catch (const TensorValidationError &exception) {
        throw TensorValidationError(
            exception.detail_code(), std::string(stage) + ": " + exception.what());
    } catch (const std::invalid_argument &exception) {
        throw std::invalid_argument(std::string(stage) + ": " + exception.what());
    } catch (const std::exception &exception) {
        throw std::runtime_error(std::string(stage) + ": " + exception.what());
    }
}

namespace {
sk_grouping_aggregation clone_grouping_aggregation(const sk_grouping_aggregation &source) {
    sk_grouping_aggregation result;
    result.kind = source.kind;
    if (source.expression) result.expression.reset(source.expression->clone());
    result.quantiles = source.quantiles;
    result.summary_class = source.summary_class;
    result.max_hits = source.max_hits;
    return result;
}

std::unique_ptr<sk_grouping_plan> clone_grouping_plan(const sk_grouping_plan &source) {
    auto result = std::make_unique<sk_grouping_plan>();
    result->all = source.all;
    for (const auto &source_level : source.levels) {
        if (!source_level || !source_level->classifier) {
            throw std::invalid_argument("structured grouping level is incomplete");
        }
        auto level = std::make_unique<sk_grouping_level>();
        level->classifier.reset(source_level->classifier->clone());
        level->max_groups = source_level->max_groups;
        level->precision = source_level->precision;
        for (const auto &aggregation : source_level->aggregations) {
            level->aggregations.push_back(clone_grouping_aggregation(aggregation));
        }
        for (const auto &order : source_level->order_by) {
            if (!order.expression) {
                throw std::invalid_argument("structured grouping order is incomplete");
            }
            level->order_by.push_back({
                std::unique_ptr<search::expression::ExpressionNode>(order.expression->clone()),
                order.ascending,
            });
        }
        if (source_level->filter) {
            const auto &source_filter = *source_level->filter;
            if (!source_filter.predicate) {
                throw std::invalid_argument("structured grouping filter is incomplete");
            }
            sk_grouping_filter filter;
            filter.pattern = source_filter.pattern;
            if (source_filter.expression) {
                filter.expression.reset(source_filter.expression->clone());
            }
            filter.predicate.reset(source_filter.predicate->clone());
            level->filter = std::move(filter);
        }
        result->levels.push_back(std::move(level));
    }
    for (const auto &aggregation : source.root_aggregations) {
        result->root_aggregations.push_back(clone_grouping_aggregation(aggregation));
    }
    return result;
}

}

namespace {
#include "document.cc"
#include "update.cc"
}

extern "C" uint32_t sk_native_abi_version(void) SK_NOEXCEPT {
    return SK_NATIVE_ABI_VERSION;
}

extern "C" int32_t sk_engine_open(const char *data_dir, const char *config_dir,
                                   sk_engine **out_engine, uint8_t *error,
                                   uint32_t error_capacity) SK_NOEXCEPT {
    if (out_engine) {
        *out_engine = nullptr;
    }
    if (!out_engine || !data_dir || !config_dir) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "engine open arguments must not be null");
    }
    return invoke(error, error_capacity, [&] {
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        TimingProbe::Scope open_timing("open_total");
#endif
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        OPEN_FAILPOINTS.hit("open_before_native_create");
#endif
        std::error_code filesystem_error;
        const std::filesystem::path data_path(data_dir);
        if (std::filesystem::exists(data_path, filesystem_error) &&
            !std::filesystem::is_directory(data_path, filesystem_error)) {
            throw std::invalid_argument("engine data path is not a directory: " + data_path.string());
        }
        if (filesystem_error) {
            throw std::runtime_error("could not inspect engine data path '" + data_path.string() + "': " +
                                     filesystem_error.message());
        }
        const std::filesystem::path config_path(config_dir);
        if (!std::filesystem::is_directory(config_path, filesystem_error)) {
            if (filesystem_error) {
                throw std::runtime_error("could not inspect engine config path '" + config_path.string() + "': " +
                                         filesystem_error.message());
            }
            throw std::invalid_argument("engine config path is not a directory: " + config_path.string());
        }
        auto engine = std::make_unique<sk_engine>();
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        {
            TimingProbe::Scope constructor_timing("constructor");
#endif
        run_open_stage("engine construction", [&] {
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
            OPEN_FAILPOINTS.hit("open_constructor");
#endif
            engine->impl = std::make_unique<EmbeddedEngine>(data_dir,
                                                            config_path.string());
        });
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        }
#endif
        run_open_stage("engine initialization", [&] {
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
            OPEN_FAILPOINTS.hit("open_initialize");
#endif
            engine->impl->initialize(data_dir, config_path.string());
        });
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        OPEN_FAILPOINTS.hit("open_after_native_create");
#endif
        *out_engine = engine.release();
    });
}

extern "C" int32_t sk_engine_close(sk_engine *engine, uint8_t *error,
                                    uint32_t error_capacity) SK_NOEXCEPT {
    if (!engine || !engine->impl) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "engine close arguments are invalid");
    }
    return invoke(error, error_capacity, [&] { engine->impl->close(); });
}

extern "C" void sk_engine_destroy(sk_engine *engine) SK_NOEXCEPT {
    try {
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        if (engine && engine->impl) {
            try {
                engine->impl->failpoint("destroy_before_native_release");
            } catch (...) {
            }
        }
#endif
        delete engine;
    } catch (...) {
    }
}

extern "C" int32_t sk_document_create(const char *id, const char *field,
                                        int64_t value, sk_document **out,
                                        uint8_t *error,
                                        uint32_t error_capacity) SK_NOEXCEPT {
    if (out) {
        *out = nullptr;
    }
    if (!out || !id || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "document arguments must not be null");
    }
    return invoke(error, error_capacity, [&] {
        validate_document_id(id);
        validate_field_name(field);
        if (*id == '\0' || *field == '\0') {
            throw std::invalid_argument("document ID and field must not be empty");
        }
        auto document = std::make_unique<sk_document>();
        document->id = id;
        document->field = field;
        document->value = value;
        *out = document.release();
    });
}

extern "C" void sk_document_destroy(sk_document *document) SK_NOEXCEPT {
    destroy_noexcept(document);
}

extern "C" SK_API int32_t sk_document_create_empty(const char *id, sk_document **out,
                                              uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out) *out = nullptr;
    if (!out || !id) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                                      "document arguments must not be null");
    return invoke(error, error_capacity, [&] {
        validate_document_id(id);
        if (*id == '\0') throw std::invalid_argument("document ID must not be empty");
        auto document = std::make_unique<sk_document>();
        document->id = id;
        *out = document.release();
    });
}

extern "C" SK_API int32_t sk_document_add_string(sk_document *document, const char *field,
                                            const char *value, uint8_t *error,
                                            uint32_t error_capacity) SK_NOEXCEPT {
    if (!value) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                                 "document string value must not be null");
    return add_document_value(document, field, error, error_capacity, [&] {
        OwnedValue result; result.kind = SK_VALUE_STRING; result.string = value; return result;
});
}

extern "C" SK_API int32_t sk_document_add_integer(sk_document *document, const char *field,
                                             int64_t value, uint8_t *error,
                                             uint32_t error_capacity) SK_NOEXCEPT {
    return add_document_value(document, field, error, error_capacity, [&] {
        OwnedValue result; result.kind = SK_VALUE_INTEGER; result.integer = value; return result;
    });
}

extern "C" SK_API int32_t sk_document_add_float(sk_document *document, const char *field,
                                           double value, uint8_t *error,
                                           uint32_t error_capacity) SK_NOEXCEPT {
    return add_document_value(document, field, error, error_capacity, [&] {
        OwnedValue result; result.kind = SK_VALUE_FLOAT; result.floating = value; return result;
    });
}

extern "C" SK_API int32_t sk_document_add_boolean(sk_document *document, const char *field,
                                             uint8_t value, uint8_t *error,
                                             uint32_t error_capacity) SK_NOEXCEPT {
    return add_document_value(document, field, error, error_capacity, [&] {
        OwnedValue result; result.kind = SK_VALUE_BOOLEAN; result.boolean = value != 0; return result;
    });
}

extern "C" SK_API int32_t sk_document_add_tensor_dense(
    sk_document *document, const char *field, const char *type_spec,
    const double *values, uint32_t value_count, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!type_spec || !values) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "document tensor arguments are invalid");
    }
    return add_document_value(document, field, error, error_capacity, [&] {
        return make_dense_tensor_value(type_spec, values, value_count);
    });
}

extern "C" SK_API int32_t sk_document_add_string_array(
    sk_document *document, const char *field, const uint8_t *value_bytes,
    const uint32_t *value_lengths, uint32_t value_count, uint32_t value_bytes_length,
    uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    return add_document_value(document, field, error, error_capacity, [&] {
        OwnedValue result;
        result.kind = SK_VALUE_STRING_ARRAY;
        result.strings = copy_string_array(value_bytes, value_lengths, value_count, value_bytes_length, "string array");
        return result;
    });
}

extern "C" SK_API int32_t sk_struct_array_create(
    sk_struct_array **out_array, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out_array) *out_array = nullptr;
    if (!out_array) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "struct array output is null");
    }
    return invoke(error, error_capacity, [&] {
        *out_array = new sk_struct_array();
    });
}

extern "C" SK_API int32_t sk_struct_array_add_element(
    sk_struct_array *array, uint32_t *out_element_index, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out_element_index) *out_element_index = 0;
    if (!array || !out_element_index) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "struct array element arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        if (array->elements.size() >= std::numeric_limits<uint32_t>::max()) {
            throw std::invalid_argument("struct array has too many elements");
        }
        *out_element_index = static_cast<uint32_t>(array->elements.size());
        array->elements.emplace_back();
    });
}

int32_t set_struct_array_value(
    sk_struct_array *array, uint32_t element_index, const char *field,
    sk_struct_array_value value, uint8_t *error, uint32_t error_capacity) {
    if (!array || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "struct array value arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        validate_field_name(field);
        if (*field == '\0' || element_index >= array->elements.size()) {
            throw std::invalid_argument("struct array value arguments are invalid");
        }
        auto &fields = array->elements[element_index];
        if (std::any_of(fields.begin(), fields.end(),
                        [field](const auto &entry) { return entry.first == field; })) {
            throw std::invalid_argument("struct array field is assigned more than once");
        }
        fields.emplace_back(field, std::move(value));
    });
}

extern "C" SK_API int32_t sk_struct_array_set_string(
    sk_struct_array *array, uint32_t element_index, const char *field,
    const char *value, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!value) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                                 "struct array string value must not be null");
    sk_struct_array_value result;
    result.kind = SK_VALUE_STRING;
    result.string = value;
    return set_struct_array_value(array, element_index, field, std::move(result),
                                  error, error_capacity);
}

extern "C" SK_API int32_t sk_struct_array_set_integer(
    sk_struct_array *array, uint32_t element_index, const char *field,
    int64_t value, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    sk_struct_array_value result;
    result.kind = SK_VALUE_INTEGER;
    result.integer = value;
    return set_struct_array_value(array, element_index, field, std::move(result),
                                  error, error_capacity);
}

extern "C" SK_API int32_t sk_struct_array_set_float(
    sk_struct_array *array, uint32_t element_index, const char *field,
    double value, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!std::isfinite(value)) return set_error(error, error_capacity,
                                                SK_NATIVE_STATUS_INVALID_ARGUMENT,
                                                "struct array float value must be finite");
    sk_struct_array_value result;
    result.kind = SK_VALUE_FLOAT;
    result.floating = value;
    return set_struct_array_value(array, element_index, field, std::move(result),
                                  error, error_capacity);
}

extern "C" SK_API int32_t sk_struct_array_set_boolean(
    sk_struct_array *array, uint32_t element_index, const char *field,
    uint8_t value, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    sk_struct_array_value result;
    result.kind = SK_VALUE_BOOLEAN;
    result.boolean = value != 0;
    return set_struct_array_value(array, element_index, field, std::move(result),
                                  error, error_capacity);
}

extern "C" SK_API void sk_struct_array_destroy(sk_struct_array *array) SK_NOEXCEPT {
    destroy_noexcept(array);
}

extern "C" SK_API int32_t sk_document_add_struct_array(
    sk_document *document, const char *field, const sk_struct_array *array,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!array) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                                 "struct array must not be null");
    return add_document_value(document, field, error, error_capacity, [&] {
        OwnedValue result;
        result.kind = SK_VALUE_STRUCT_ARRAY;
        result.struct_elements.clear();
        result.struct_elements.reserve(array->elements.size());
        for (const auto &element : array->elements) {
            std::vector<std::pair<std::string, OwnedValue>> converted;
            converted.reserve(element.size());
            for (const auto &[name, value] : element) {
                OwnedValue owned;
                owned.kind = value.kind;
                owned.string = value.string;
                owned.integer = value.integer;
                owned.floating = value.floating;
                owned.boolean = value.boolean;
                converted.emplace_back(name, std::move(owned));
            }
            result.struct_elements.push_back(std::move(converted));
        }
        return result;
    });
}

extern "C" SK_API int32_t sk_document_add_position(
    sk_document *document, const char *field, double latitude, double longitude,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    return add_document_value(document, field, error, error_capacity, [&] {
        OwnedValue result;
        result.kind = SK_VALUE_POSITION;
        result.latitude = latitude;
        result.longitude = longitude;
        return result;
    });
}

extern "C" SK_API int32_t sk_document_add_predicate(
    sk_document *document, const char *field, const uint8_t *feature_key_bytes,
    const uint32_t *feature_key_lengths, uint32_t feature_key_bytes_length,
    const uint8_t *feature_value_bytes, const uint32_t *feature_value_lengths,
    uint32_t feature_value_bytes_length, uint32_t feature_count, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    return add_document_value(document, field, error, error_capacity, [&] {
        OwnedValue result;
        result.kind = SK_VALUE_PREDICATE;
        result.predicate_features = copy_predicate_features(
            feature_key_bytes, feature_key_lengths, feature_key_bytes_length,
            feature_value_bytes, feature_value_lengths, feature_value_bytes_length, feature_count);
        return result;
    });
}

extern "C" SK_API int32_t sk_document_add_predicate_json(
    sk_document *document, const char *field, const char *expression_json,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!expression_json) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "predicate expression must not be null");
    }
    return add_document_value(document, field, error, error_capacity, [&] {
        OwnedValue result;
        result.kind = SK_VALUE_PREDICATE;
        result.predicate_json = expression_json;
        return result;
    });
}

extern "C" SK_API int32_t sk_document_annotate_text(
    sk_document *document, const char *field, const uint8_t *term_bytes,
    uint32_t term_bytes_length, const uint32_t *term_lengths,
    const uint32_t *byte_starts, const uint32_t *byte_lengths,
    uint32_t occurrence_count, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!document || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "document annotation arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        auto existing = std::find_if(document->fields.begin(), document->fields.end(),
                                     [field](const auto &entry) { return entry.name == field; });
        if (existing == document->fields.end()) {
            throw std::invalid_argument("document text field must be added before annotation");
        }
        if (existing->value.kind != SK_VALUE_STRING) {
            throw std::invalid_argument("document annotation requires a string field");
        }
        TextDocument analyzed;
        analyzed.text = existing->value.string;
        append_text_occurrences(analyzed, term_bytes, term_bytes_length, term_lengths,
                                byte_starts, byte_lengths, occurrence_count);
        existing->occurrences = std::move(analyzed.occurrences);
    });
}

extern "C" SK_API int32_t sk_document_annotate_string_array(
    sk_document *document, const char *field, const uint8_t *term_bytes,
    uint32_t term_bytes_length, const uint32_t *term_lengths,
    const uint32_t *byte_starts, const uint32_t *byte_lengths,
    const uint32_t *element_offsets, uint32_t element_count,
    uint32_t occurrence_count, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!document || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "array annotation arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        auto existing = std::find_if(document->fields.begin(), document->fields.end(),
                                     [field](const auto &entry) { return entry.name == field; });
        if (existing == document->fields.end() || existing->value.kind != SK_VALUE_STRING_ARRAY) {
            throw std::invalid_argument("a string array assignment is required before annotation");
        }
        if (element_count != existing->value.strings.size()) {
            throw std::invalid_argument("array annotation element count does not match array");
        }
        existing->array_occurrences = decode_text_array_occurrences(
            existing->value.strings, term_bytes, term_bytes_length, term_lengths, byte_starts,
            byte_lengths, element_offsets, element_count, occurrence_count);
    });
}

extern "C" int32_t sk_document_update_create(const char *id, const char *field,
                                               int64_t value,
                                               sk_document_update **out,
                                               uint8_t *error,
                                               uint32_t error_capacity) SK_NOEXCEPT {
    if (out) {
        *out = nullptr;
    }
    if (!out || !id || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "update arguments must not be null");
    }
    return invoke(error, error_capacity, [&] {
        validate_document_id(id);
        validate_field_name(field);
        if (*id == '\0' || *field == '\0') {
            throw std::invalid_argument("document ID and field must not be empty");
        }
        auto update = std::make_unique<sk_document_update>();
        update->id = id;
        update->field = field;
        update->value = value;
        *out = update.release();
    });
}

extern "C" void sk_document_update_destroy(sk_document_update *update) SK_NOEXCEPT {
    destroy_noexcept(update);
}

extern "C" SK_API int32_t sk_document_update_create_empty(
    const char *id, sk_document_update **out, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out) *out = nullptr;
    if (!out || !id) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                                      "update arguments must not be null");
    return invoke(error, error_capacity, [&] {
        validate_document_id(id);
        if (*id == '\0') throw std::invalid_argument("document ID must not be empty");
        auto update = std::make_unique<sk_document_update>();
        update->id = id;
        *out = update.release();
    });
}

extern "C" SK_API int32_t sk_document_update_set_create_if_nonexistent(
    sk_document_update *update, uint8_t create_if_nonexistent, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!update) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "update arguments must not be null");
    }
    return invoke(error, error_capacity, [&] {
        update->create_if_nonexistent = create_if_nonexistent != 0;
    });
}

extern "C" SK_API int32_t sk_document_update_assign_string(
    sk_document_update *update, const char *field, const char *value,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!value) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                                 "update string value must not be null");
    return add_update_value(update, field, error, error_capacity, [&] {
        OwnedValue result; result.kind = SK_VALUE_STRING; result.string = value; return std::optional<OwnedValue>(std::move(result));
    });
}

extern "C" SK_API int32_t sk_document_update_assign_integer(
    sk_document_update *update, const char *field, int64_t value,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    return add_update_value(update, field, error, error_capacity, [&] {
        OwnedValue result; result.kind = SK_VALUE_INTEGER; result.integer = value; return std::optional<OwnedValue>(std::move(result));
    });
}

extern "C" SK_API int32_t sk_document_update_assign_float(
    sk_document_update *update, const char *field, double value,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    return add_update_value(update, field, error, error_capacity, [&] {
        OwnedValue result; result.kind = SK_VALUE_FLOAT; result.floating = value; return std::optional<OwnedValue>(std::move(result));
    });
}

extern "C" SK_API int32_t sk_document_update_assign_boolean(
    sk_document_update *update, const char *field, uint8_t value,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    return add_update_value(update, field, error, error_capacity, [&] {
        OwnedValue result; result.kind = SK_VALUE_BOOLEAN; result.boolean = value != 0; return std::optional<OwnedValue>(std::move(result));
    });
}

extern "C" SK_API int32_t sk_document_update_assign_tensor_dense(
    sk_document_update *update, const char *field, const char *type_spec,
    const double *values, uint32_t value_count, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!type_spec || !values) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "update tensor arguments are invalid");
    }
    return add_update_value(update, field, error, error_capacity, [&] {
        return std::optional<OwnedValue>(
            make_dense_tensor_value(type_spec, values, value_count));
    });
}

extern "C" SK_API int32_t sk_document_update_assign_string_array(
    sk_document_update *update, const char *field, const uint8_t *value_bytes,
    const uint32_t *value_lengths, uint32_t value_count, uint32_t value_bytes_length,
    uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    return add_update_value(update, field, error, error_capacity, [&] {
        OwnedValue result;
        result.kind = SK_VALUE_STRING_ARRAY;
        result.strings = copy_string_array(value_bytes, value_lengths, value_count, value_bytes_length, "string array");
        return std::optional<OwnedValue>(std::move(result));
    });
}

extern "C" SK_API int32_t sk_document_update_assign_struct_array(
    sk_document_update *update, const char *field, const sk_struct_array *array,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!array) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                                 "update struct array must not be null");
    return add_update_value(update, field, error, error_capacity, [&] {
        OwnedValue result;
        result.kind = SK_VALUE_STRUCT_ARRAY;
        result.struct_elements.reserve(array->elements.size());
        for (const auto &element : array->elements) {
            std::vector<std::pair<std::string, OwnedValue>> converted;
            converted.reserve(element.size());
            for (const auto &[name, value] : element) {
                OwnedValue owned;
                owned.kind = value.kind;
                owned.string = value.string;
                owned.integer = value.integer;
                owned.floating = value.floating;
                owned.boolean = value.boolean;
                converted.emplace_back(name, std::move(owned));
            }
            result.struct_elements.push_back(std::move(converted));
        }
        return std::optional<OwnedValue>(std::move(result));
    });
}

extern "C" SK_API int32_t sk_document_update_assign_position(
    sk_document_update *update, const char *field, double latitude, double longitude,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    return add_update_value(update, field, error, error_capacity, [&] {
        OwnedValue result;
        result.kind = SK_VALUE_POSITION;
        result.latitude = latitude;
        result.longitude = longitude;
        return std::optional<OwnedValue>(std::move(result));
    });
}

extern "C" SK_API int32_t sk_document_update_assign_predicate(
    sk_document_update *update, const char *field, const uint8_t *feature_key_bytes,
    const uint32_t *feature_key_lengths, uint32_t feature_key_bytes_length,
    const uint8_t *feature_value_bytes, const uint32_t *feature_value_lengths,
    uint32_t feature_value_bytes_length, uint32_t feature_count, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    return add_update_value(update, field, error, error_capacity, [&] {
        OwnedValue result;
        result.kind = SK_VALUE_PREDICATE;
        result.predicate_features = copy_predicate_features(
            feature_key_bytes, feature_key_lengths, feature_key_bytes_length,
            feature_value_bytes, feature_value_lengths, feature_value_bytes_length, feature_count);
        return std::optional<OwnedValue>(std::move(result));
    });
}

extern "C" SK_API int32_t sk_document_update_assign_predicate_json(
    sk_document_update *update, const char *field, const char *expression_json,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!expression_json) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "predicate expression must not be null");
    }
    return add_update_value(update, field, error, error_capacity, [&] {
        OwnedValue result;
        result.kind = SK_VALUE_PREDICATE;
        result.predicate_json = expression_json;
        return std::optional<OwnedValue>(std::move(result));
    });
}

extern "C" SK_API int32_t sk_document_update_clear(sk_document_update *update, const char *field,
                                              uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    return add_update_value(update, field, error, error_capacity,
                            [] { return std::optional<OwnedValue>(); });
}

extern "C" SK_API int32_t sk_document_update_arithmetic(
    sk_document_update *update, const char *field, uint32_t arithmetic_operator,
    double operand, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!update || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "arithmetic update arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        validate_field_name(field);
        if (!std::isfinite(operand)) throw std::invalid_argument("arithmetic operand must be finite");
        if (arithmetic_operator > SK_ARITHMETIC_SUB) {
            throw std::invalid_argument("unknown arithmetic update operator");
        }
        update->fields.erase(std::remove_if(update->fields.begin(), update->fields.end(),
                                            [field](const auto &entry) { return entry.first == field; }),
                             update->fields.end());
        auto existing = std::find_if(update->arithmetic.begin(), update->arithmetic.end(),
                                     [field](const auto &entry) { return entry.field == field; });
        ValueUpdate::Arithmetic value{field, arithmetic_operator, operand};
        if (existing == update->arithmetic.end()) update->arithmetic.push_back(std::move(value));
        else *existing = std::move(value);
    });
}

extern "C" SK_API int32_t sk_document_update_annotate_text(
    sk_document_update *update, const char *field, const uint8_t *term_bytes,
    uint32_t term_bytes_length, const uint32_t *term_lengths,
    const uint32_t *byte_starts, const uint32_t *byte_lengths,
    uint32_t occurrence_count, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!update || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "update annotation arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        auto existing = std::find_if(update->fields.begin(), update->fields.end(),
                                     [field](const auto &entry) { return entry.first == field; });
        if (existing == update->fields.end() || !existing->second ||
            existing->second->kind != SK_VALUE_STRING) {
            throw std::invalid_argument("a string assignment is required before annotation");
        }
        TextDocument analyzed;
        analyzed.text = existing->second->string;
        append_text_occurrences(analyzed, term_bytes, term_bytes_length, term_lengths,
                                byte_starts, byte_lengths, occurrence_count);
        auto annotated = std::find_if(update->annotated_fields.begin(),
                                      update->annotated_fields.end(),
                                      [field](const auto &entry) { return entry.first == field; });
        if (annotated == update->annotated_fields.end()) {
            update->annotated_fields.emplace_back(field, std::move(analyzed.occurrences));
        } else {
            annotated->second = std::move(analyzed.occurrences);
        }
    });
}

extern "C" SK_API int32_t sk_document_update_annotate_string_array(
    sk_document_update *update, const char *field, const uint8_t *term_bytes,
    uint32_t term_bytes_length, const uint32_t *term_lengths,
    const uint32_t *byte_starts, const uint32_t *byte_lengths,
    const uint32_t *element_offsets, uint32_t element_count,
    uint32_t occurrence_count, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!update || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "array annotation arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        auto existing = std::find_if(update->fields.begin(), update->fields.end(),
                                     [field](const auto &entry) { return entry.first == field; });
        if (existing == update->fields.end() || !existing->second ||
            existing->second->kind != SK_VALUE_STRING_ARRAY) {
            throw std::invalid_argument("a string array assignment is required before annotation");
        }
        if (element_count != existing->second->strings.size()) {
            throw std::invalid_argument("array annotation element count does not match array");
        }
        auto analyzed_elements = decode_text_array_occurrences(
            existing->second->strings, term_bytes, term_bytes_length, term_lengths, byte_starts,
            byte_lengths, element_offsets, element_count, occurrence_count);
        auto annotated = std::find_if(update->annotated_array_fields.begin(),
                                      update->annotated_array_fields.end(),
                                      [field](const auto &entry) { return entry.first == field; });
        if (annotated == update->annotated_array_fields.end()) {
            update->annotated_array_fields.emplace_back(field, std::move(analyzed_elements));
        } else {
            annotated->second = std::move(analyzed_elements);
        }
    });
}

extern "C" int32_t sk_search_request_create(const char *field, int64_t value,
                                              uint32_t limit,
                                              sk_search_request **out,
                                              uint8_t *error,
                                              uint32_t error_capacity) SK_NOEXCEPT {
    if (out) {
        *out = nullptr;
    }
    if (!out || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "search arguments must not be null");
    }
    return invoke(error, error_capacity, [&] {
        validate_field_name(field);
        if (*field == '\0') {
            throw std::invalid_argument("search field must not be empty");
        }
        if (limit > MAX_SEARCH_LIMIT) {
            throw std::invalid_argument("search limit is too large");
        }
        auto request = std::make_unique<sk_search_request>();
        request->field = field;
        request->value = value;
        request->limit = limit;
        *out = request.release();
    });
}

extern "C" void sk_search_request_destroy(sk_search_request *request) SK_NOEXCEPT {
    destroy_noexcept(request);
}

extern "C" int32_t sk_engine_put(sk_engine *engine, const sk_document *document,
                                  uint8_t *error,
                                  uint32_t error_capacity) SK_NOEXCEPT {
    if (!engine || !engine->impl || !document) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "put arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        if (!document->fields.empty()) {
            engine->impl->put_document(*document);
        } else {
            engine->impl->put(document->id, document->field, document->value);
        }
    });
}

extern "C" int32_t sk_engine_update(sk_engine *engine,
                                     const sk_document_update *update,
                                     uint8_t *error,
                                     uint32_t error_capacity) SK_NOEXCEPT {
    if (!engine || !engine->impl || !update) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "update arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        if (!update->fields.empty() || !update->arithmetic.empty()) {
            engine->impl->update_document(*update);
        } else {
            engine->impl->update(update->id, update->field, update->value);
        }
    });
}

extern "C" SK_API int32_t sk_engine_get(const sk_engine *engine, const char *id,
                                  sk_document **out_document, uint8_t *error,
                                  uint32_t error_capacity) SK_NOEXCEPT {
    if (out_document) *out_document = nullptr;
    if (!engine || !engine->impl || !id || !out_document) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "get arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        auto document = engine->impl->get_value_document_for_abi(id);
        if (document.exists) {
            auto result = std::make_unique<sk_document>(std::move(document));
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
            result->result_faults = engine->impl->result_faults();
#endif
            *out_document = result.release();
        }
    });
}

extern "C" int32_t sk_document_field_count(const sk_document *document,
                                             uint32_t *out_count, uint8_t *error,
                                             uint32_t error_capacity) SK_NOEXCEPT {
    if (!document || !out_count) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "document field count arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        if (document->fields.size() > std::numeric_limits<uint32_t>::max()) {
            throw std::runtime_error("document field count exceeds native capacity");
        }
        *out_count = static_cast<uint32_t>(document->fields.size());
    });
}

extern "C" int32_t sk_document_field(const sk_document *document, uint32_t index,
                                      const char **out_name, uint32_t *out_name_length,
                                      uint32_t *out_kind, const char **out_string,
                                      uint32_t *out_string_length, int64_t *out_integer,
                                      double *out_float, uint8_t *out_boolean,
                                      uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!document || !out_name || !out_name_length || !out_kind || !out_string ||
        !out_string_length || !out_integer || !out_float || !out_boolean) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "document field arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        if (index >= document->fields.size()) {
            throw std::invalid_argument("document field index is out of range");
        }
        const auto &field = document->fields[index];
        if (field.name.size() > std::numeric_limits<uint32_t>::max() ||
            field.value.string.size() > std::numeric_limits<uint32_t>::max()) {
            throw std::runtime_error("document field value exceeds native capacity");
        }
        *out_name = field.name.data();
        *out_name_length = static_cast<uint32_t>(field.name.size());
        *out_kind = field.value.kind;
        *out_string = field.value.string.data();
        *out_string_length = static_cast<uint32_t>(field.value.string.size());
        *out_integer = field.value.integer;
        *out_float = field.value.floating;
        *out_boolean = field.value.boolean ? 1 : 0;
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        static const uint8_t invalid_utf8[] = {0xff};
        if (document->result_faults &&
            document->result_faults->take(SK_TEST_RESULT_FAULT_DOCUMENT_ACCESSOR_ERROR)) {
            throw std::runtime_error("injected document accessor failure");
        }
        if (document->result_faults &&
            document->result_faults->take(SK_TEST_RESULT_FAULT_NULL_DOCUMENT_FIELD_NAME)) {
            *out_name = nullptr;
            *out_name_length = 1;
        } else if (document->result_faults &&
                   document->result_faults->take(SK_TEST_RESULT_FAULT_INVALID_DOCUMENT_FIELD_NAME)) {
            *out_name = reinterpret_cast<const char *>(invalid_utf8);
            *out_name_length = sizeof(invalid_utf8);
        }
        if (field.value.kind == SK_VALUE_STRING && document->result_faults &&
            document->result_faults->take(SK_TEST_RESULT_FAULT_NULL_DOCUMENT_STRING)) {
            *out_string = nullptr;
            *out_string_length = 1;
        } else if (field.value.kind == SK_VALUE_STRING && document->result_faults &&
                   document->result_faults->take(SK_TEST_RESULT_FAULT_INVALID_DOCUMENT_STRING)) {
            *out_string = reinterpret_cast<const char *>(invalid_utf8);
            *out_string_length = sizeof(invalid_utf8);
        }
        if (document->result_faults &&
            document->result_faults->take(SK_TEST_RESULT_FAULT_UNKNOWN_DOCUMENT_KIND)) {
            *out_kind = 99;
        }
#endif
    });
}

extern "C" SK_API int32_t sk_document_field_tensor_dense(
    const sk_document *document, uint32_t index,
    const char **out_type_spec, uint32_t *out_type_spec_length,
    const double **out_cells, uint32_t *out_cell_count,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!document || !out_type_spec || !out_type_spec_length || !out_cells ||
        !out_cell_count) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "document tensor field arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        if (index >= document->fields.size()) {
            throw std::invalid_argument("document field index is out of range");
        }
        const auto &field = document->fields[index];
        if (field.value.kind != SK_VALUE_TENSOR) {
            throw std::invalid_argument("document field is not a tensor");
        }
        if (field.value.tensor_type.size() > std::numeric_limits<uint32_t>::max() ||
            field.value.tensor_cells.size() > std::numeric_limits<uint32_t>::max()) {
            throw std::runtime_error("document tensor field exceeds native capacity");
        }
        *out_type_spec = field.value.tensor_type.data();
        *out_type_spec_length = static_cast<uint32_t>(field.value.tensor_type.size());
        *out_cells = field.value.tensor_cells.data();
        *out_cell_count = static_cast<uint32_t>(field.value.tensor_cells.size());
    });
}

extern "C" int32_t sk_engine_remove(sk_engine *engine, const char *id,
                                     int32_t *out_was_found, uint8_t *error,
                                     uint32_t error_capacity) SK_NOEXCEPT {
    if (out_was_found) {
        *out_was_found = 0;
    }
    if (!engine || !engine->impl || !id || !out_was_found) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "remove arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        validate_document_id(id);
        *out_was_found = engine->impl->remove(id) ? 1 : 0;
    });
}

extern "C" int32_t sk_engine_search(const sk_engine *engine,
                                     const sk_search_request *request,
                                     sk_search_result **out_result,
                                     uint8_t *error,
                                     uint32_t error_capacity) SK_NOEXCEPT {
    if (out_result) {
        *out_result = nullptr;
    }
    if (!engine || !engine->impl || !request || !out_result) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "search arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        if (request->limit == 0) {
            throw std::invalid_argument("search limit must be greater than zero");
        }
        auto result = std::make_unique<sk_search_result>();
        result->ids = engine->impl->search(request->field, request->value,
                                           request->limit);
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        engine->impl->failpoint("result_after_allocation");
        result->failpoints = engine->impl->failpoints();
#endif
        *out_result = result.release();
    });
}

extern "C" uint32_t sk_search_result_size(const sk_search_result *result) SK_NOEXCEPT {
    return result ? static_cast<uint32_t>(result->ids.size()) : 0;
}

extern "C" int32_t sk_search_result_id(const sk_search_result *result,
                                        uint32_t index, const char **out_id,
                                        uint32_t *out_length, uint8_t *error,
                                        uint32_t error_capacity) SK_NOEXCEPT {
    if (out_id) {
        *out_id = nullptr;
    }
    if (out_length) {
        *out_length = 0;
    }
    if (!result || !out_id || !out_length) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "result arguments are invalid");
    }
    if (index >= result->ids.size()) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "result index is out of bounds");
    }
    return invoke(error, error_capacity, [&] {
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        if (result->failpoints) {
            result->failpoints->hit("result_before_copy");
        }
#endif
        *out_id = result->ids[index].data();
        *out_length = static_cast<uint32_t>(result->ids[index].size());
    });
}

extern "C" void sk_search_result_destroy(sk_search_result *result) SK_NOEXCEPT {
    destroy_noexcept(result);
}

extern "C" int32_t sk_local_search_request_create(
    const char *rank_profile, uint32_t limit, uint32_t offset,
    uint32_t summary_kind, sk_local_search_request **out_request,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_request) *out_request = nullptr;
    if (!rank_profile || !out_request) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "local search request arguments are invalid");
    return invoke(error, error_capacity, [&] {
        validate_rank_profile(rank_profile);
        if (*rank_profile == '\0') throw std::invalid_argument("rank profile must not be empty");
        if (limit > MAX_SEARCH_LIMIT) throw std::invalid_argument("search limit is too large");
        if (summary_kind != SK_SUMMARY_NONE && summary_kind != SK_SUMMARY_DEFAULT && summary_kind != SK_SUMMARY_FIELDS) throw std::invalid_argument("unsupported summary kind");
        auto request = std::make_unique<sk_local_search_request>();
        request->rank_profile = rank_profile;
        request->limit = limit;
        request->offset = offset;
        request->summary_kind = summary_kind;
        *out_request = request.release();
    });
}

extern "C" int32_t sk_local_search_request_add_sort(
    sk_local_search_request *request, const char *field, uint32_t order,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!request || !field) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "local sort arguments are invalid");
    return invoke(error, error_capacity, [&] {
        validate_field_name(field);
        if (order != SK_SORT_ASCENDING && order != SK_SORT_DESCENDING) throw std::invalid_argument("unsupported sort order");
        request->sort.push_back({field, order, SK_SORT_MISSING_DEFAULT});
    });
}

extern "C" int32_t sk_local_search_request_add_summary_field(
    sk_local_search_request *request, const char *field,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!request || !field) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "local summary arguments are invalid");
    return invoke(error, error_capacity, [&] {
        validate_field_name(field);
        if (request->summary_kind != SK_SUMMARY_FIELDS) throw std::invalid_argument("summary fields require Fields summary mode");
        if (std::find(request->summary_fields.begin(), request->summary_fields.end(), field) != request->summary_fields.end()) throw std::invalid_argument("summary fields must not contain duplicates");
        request->summary_fields.emplace_back(field);
    });
}

extern "C" int32_t sk_local_search_request_add_highlight_field(
    sk_local_search_request *request, const char *field,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!request || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "local highlight arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        validate_field_name(field);
        if (std::find(request->highlight_fields.begin(), request->highlight_fields.end(), field) !=
            request->highlight_fields.end()) {
            throw std::invalid_argument("highlight fields must not contain duplicates");
        }
        request->highlight_fields.emplace_back(field);
    });
}

extern "C" int32_t sk_local_search_request_set_summary_class(
    sk_local_search_request *request, const char *name,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!request || !name) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "local summary class arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        validate_string_size(name, MAX_SCHEMA_IDENTIFIER_BYTES, "summary class");
        if (*name == '\0') throw std::invalid_argument("summary class must not be empty");
        request->summary_class = name;
    });
}

extern "C" int32_t sk_local_search_request_add_query_feature(
    sk_local_search_request *request, const char *name, double value,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!request || !name) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "query feature arguments are invalid");
    }
    return invoke(error, error_capacity, [&] { add_query_feature(*request, name, value); });
}

extern "C" int32_t sk_local_search_request_add_rank_property(
    sk_local_search_request *request, const char *name, const char *value,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!request || !name || !value) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "rank property arguments are invalid");
    }
    return invoke(error, error_capacity, [&] { add_rank_property(*request, name, value); });
}

extern "C" void sk_local_search_request_destroy(sk_local_search_request *request) SK_NOEXCEPT { destroy_noexcept(request); }

extern "C" int32_t sk_engine_search_local(
    const sk_engine *engine, const sk_query *query,
    const sk_local_search_request *request, sk_local_search_result **out_result,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_result) *out_result = nullptr;
    if (!engine || !engine->impl || !query || !request || !out_result) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "local search arguments are invalid");
    return invoke(error, error_capacity, [&] {
        auto result = std::make_unique<sk_local_search_result>();
        static_cast<LocalSearchResultState &>(*result) = engine->impl->search_local_result(*query, *request);
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        engine->impl->failpoint("result_after_allocation");
        result->failpoints = engine->impl->failpoints();
        result->result_faults = engine->impl->result_faults();
        result->result_destroyed = engine->impl->result_destroyed_counter();
        for (auto &grouping : result->groupings) {
            grouping.failpoints = result->failpoints;
            grouping.result_faults = result->result_faults;
        }
#endif
        *out_result = result.release();
    });
}

extern "C" uint64_t sk_local_search_result_total_hit_count(const sk_local_search_result *result) SK_NOEXCEPT { return result ? result->total_hit_count : 0; }
extern "C" uint32_t sk_local_search_result_hit_count(const sk_local_search_result *result) SK_NOEXCEPT { return result ? static_cast<uint32_t>(result->hits.size()) : 0; }

extern "C" int32_t sk_local_search_result_hit(
    const sk_local_search_result *result, uint32_t index, const char **out_id,
    uint32_t *out_id_length, double *out_score, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out_id) *out_id = nullptr;
    if (out_id_length) *out_id_length = 0;
    if (out_score) *out_score = 0.0;
    if (!result || !out_id || !out_id_length || !out_score) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "local hit arguments are invalid");
    if (index >= result->hits.size()) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "local hit index is out of bounds");
    return invoke(error, error_capacity, [&] {
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        if (result->failpoints) result->failpoints->hit("result_before_copy");
        static const uint8_t invalid_utf8[] = {0xff};
        if (result->result_faults &&
            result->result_faults->take(SK_TEST_RESULT_FAULT_NULL_HIT_ID)) {
            *out_id = nullptr;
            *out_id_length = 1;
            return;
        }
        if (result->result_faults &&
            result->result_faults->take(SK_TEST_RESULT_FAULT_INVALID_HIT_ID)) {
            *out_id = reinterpret_cast<const char *>(invalid_utf8);
            *out_id_length = sizeof(invalid_utf8);
            return;
        }
#endif
        *out_id = result->hits[index].id.data();
        *out_id_length = static_cast<uint32_t>(result->hits[index].id.size());
        *out_score = result->hits[index].score;
    });
}

extern "C" uint32_t sk_local_search_result_summary_count(const sk_local_search_result *result, uint32_t hit_index) SK_NOEXCEPT {
    return result && hit_index < result->hits.size() ? static_cast<uint32_t>(result->hits[hit_index].summary.size()) : 0;
}

extern "C" int32_t sk_local_search_result_summary_name(
    const sk_local_search_result *result, uint32_t hit_index, uint32_t summary_index,
    const char **out_name, uint32_t *out_length, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out_name) *out_name = nullptr;
    if (out_length) *out_length = 0;
    if (!result || !out_name || !out_length || hit_index >= result->hits.size() || summary_index >= result->hits[hit_index].summary.size()) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "summary name arguments are invalid");
    return invoke(error, error_capacity, [&] {
        const auto &name = result->hits[hit_index].summary[summary_index].first;
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        static const uint8_t invalid_utf8[] = {0xff};
        if (result->result_faults &&
            result->result_faults->take(SK_TEST_RESULT_FAULT_NULL_SUMMARY_NAME)) {
            *out_name = nullptr;
            *out_length = 1;
            return;
        }
        if (result->result_faults &&
            result->result_faults->take(SK_TEST_RESULT_FAULT_INVALID_SUMMARY_NAME)) {
            *out_name = reinterpret_cast<const char *>(invalid_utf8);
            *out_length = sizeof(invalid_utf8);
            return;
        }
#endif
        *out_name = name.data();
        *out_length = static_cast<uint32_t>(name.size());
    });
}

extern "C" int32_t sk_local_search_result_summary_value(
    const sk_local_search_result *result, uint32_t hit_index, uint32_t summary_index,
    uint32_t *out_kind, const char **out_string, uint32_t *out_string_length,
    int64_t *out_integer, double *out_float, uint8_t *out_boolean,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_kind) *out_kind = 0;
    if (out_string) *out_string = nullptr;
    if (out_string_length) *out_string_length = 0;
    if (out_integer) *out_integer = 0;
    if (out_float) *out_float = 0.0;
    if (out_boolean) *out_boolean = 0;
    if (!result || !out_kind || !out_string || !out_string_length || !out_integer || !out_float || !out_boolean || hit_index >= result->hits.size() || summary_index >= result->hits[hit_index].summary.size()) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "summary value arguments are invalid");
    return invoke(error, error_capacity, [&] {
        const auto &value = result->hits[hit_index].summary[summary_index].second;
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        static const uint8_t invalid_utf8[] = {0xff};
        if (value.kind == SK_VALUE_STRING && result->result_faults &&
            result->result_faults->take(SK_TEST_RESULT_FAULT_NULL_SUMMARY_STRING)) {
            *out_kind = SK_VALUE_STRING;
            *out_string = nullptr;
            *out_string_length = 1;
            return;
        }
        if (value.kind == SK_VALUE_STRING && result->result_faults &&
            result->result_faults->take(SK_TEST_RESULT_FAULT_INVALID_SUMMARY_STRING)) {
            *out_kind = SK_VALUE_STRING;
            *out_string = reinterpret_cast<const char *>(invalid_utf8);
            *out_string_length = sizeof(invalid_utf8);
            return;
        }
        *out_kind = value.kind;
#else
        *out_kind = value.kind;
#endif
        *out_string = value.string.data();
        *out_string_length = static_cast<uint32_t>(value.string.size());
        *out_integer = value.integer;
        *out_float = value.floating;
        *out_boolean = value.boolean;
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        if (result->result_faults &&
            result->result_faults->take(SK_TEST_RESULT_FAULT_UNKNOWN_SUMMARY_KIND)) {
            *out_kind = 99;
        }
#endif
    });
}

extern "C" uint32_t sk_local_search_result_highlight_count(
    const sk_local_search_result *result, uint32_t hit_index) SK_NOEXCEPT {
    return result && hit_index < result->hits.size()
               ? static_cast<uint32_t>(result->hits[hit_index].highlights.size())
               : 0;
}

extern "C" int32_t sk_local_search_result_highlight_field(
    const sk_local_search_result *result, uint32_t hit_index,
    uint32_t highlight_index, const char **out_field, uint32_t *out_length,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_field) *out_field = nullptr;
    if (out_length) *out_length = 0;
    if (!result || !out_field || !out_length || hit_index >= result->hits.size() ||
        highlight_index >= result->hits[hit_index].highlights.size()) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "highlight field arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        const auto &field = result->hits[hit_index].highlights[highlight_index].field;
        *out_field = field.data();
        *out_length = static_cast<uint32_t>(field.size());
    });
}

extern "C" uint32_t sk_local_search_result_highlight_fragment_count(
    const sk_local_search_result *result, uint32_t hit_index,
    uint32_t highlight_index) SK_NOEXCEPT {
    return result && hit_index < result->hits.size() &&
                   highlight_index < result->hits[hit_index].highlights.size()
               ? static_cast<uint32_t>(result->hits[hit_index].highlights[highlight_index]
                                           .fragments.size())
               : 0;
}

extern "C" int32_t sk_local_search_result_highlight_fragment_text(
    const sk_local_search_result *result, uint32_t hit_index,
    uint32_t highlight_index, uint32_t fragment_index, const char **out_text,
    uint32_t *out_length, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_text) *out_text = nullptr;
    if (out_length) *out_length = 0;
    if (!result || !out_text || !out_length || hit_index >= result->hits.size() ||
        highlight_index >= result->hits[hit_index].highlights.size() ||
        fragment_index >= result->hits[hit_index].highlights[highlight_index].fragments.size()) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "highlight fragment arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        const auto &text = result->hits[hit_index].highlights[highlight_index]
                               .fragments[fragment_index]
                               .text;
        *out_text = text.data();
        *out_length = static_cast<uint32_t>(text.size());
    });
}

extern "C" uint32_t sk_local_search_result_highlight_match_count(
    const sk_local_search_result *result, uint32_t hit_index,
    uint32_t highlight_index, uint32_t fragment_index) SK_NOEXCEPT {
    return result && hit_index < result->hits.size() &&
                   highlight_index < result->hits[hit_index].highlights.size() &&
                   fragment_index < result->hits[hit_index].highlights[highlight_index].fragments.size()
               ? static_cast<uint32_t>(result->hits[hit_index].highlights[highlight_index]
                                           .fragments[fragment_index]
                                           .matches.size())
               : 0;
}

extern "C" int32_t sk_local_search_result_highlight_match(
    const sk_local_search_result *result, uint32_t hit_index,
    uint32_t highlight_index, uint32_t fragment_index, uint32_t match_index,
    uint32_t *out_start_byte, uint32_t *out_end_byte,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_start_byte) *out_start_byte = 0;
    if (out_end_byte) *out_end_byte = 0;
    if (!result || !out_start_byte || !out_end_byte || hit_index >= result->hits.size() ||
        highlight_index >= result->hits[hit_index].highlights.size() ||
        fragment_index >= result->hits[hit_index].highlights[highlight_index].fragments.size() ||
        match_index >= result->hits[hit_index].highlights[highlight_index]
                           .fragments[fragment_index]
                           .matches.size()) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "highlight match arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        const auto &match = result->hits[hit_index].highlights[highlight_index]
                                .fragments[fragment_index]
                                .matches[match_index];
        *out_start_byte = match.start_byte;
        *out_end_byte = match.end_byte;
    });
}

extern "C" uint32_t sk_local_search_result_match_feature_count(
    const sk_local_search_result *result) SK_NOEXCEPT {
    return result ? match_feature_count(*result) : 0;
}

extern "C" int32_t sk_local_search_result_match_feature_name(
    const sk_local_search_result *result, uint32_t feature_index,
    const char **out_name, uint32_t *out_length, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out_name) *out_name = nullptr;
    if (out_length) *out_length = 0;
    if (!result || !out_name || !out_length) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "match feature name arguments are invalid");
    }
    return invoke(error, error_capacity,
                  [&] { match_feature_name(*result, feature_index, out_name, out_length); });
}

extern "C" int32_t sk_local_search_result_match_feature_value(
    const sk_local_search_result *result, uint32_t hit_index,
    uint32_t feature_index, double *out_value, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out_value) *out_value = 0.0;
    if (!result || !out_value) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "match feature value arguments are invalid");
    }
    return invoke(error, error_capacity,
                  [&] { match_feature_value_double(*result, hit_index, feature_index, out_value); });
}

extern "C" int32_t sk_local_search_result_match_feature_value_ref(
    const sk_local_search_result *result, uint32_t hit_index,
    uint32_t feature_index, const sk_feature_value **out_value,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_value) *out_value = nullptr;
    if (!result || !out_value) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "match feature value arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        *out_value = match_feature_value_ref(*result, hit_index, feature_index);
    });
}

extern "C" uint32_t sk_feature_value_kind(const sk_feature_value *value) SK_NOEXCEPT {
    if (!value) return SK_FEATURE_VALUE_KIND_INVALID;
    return value->value.is_double() ? SK_FEATURE_VALUE_DOUBLE : SK_FEATURE_VALUE_DATA;
}

extern "C" int32_t sk_feature_value_double(
    const sk_feature_value *value, double *out_value, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out_value) *out_value = 0.0;
    if (!value || !out_value) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "feature double accessor arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        if (!value->value.is_double()) {
            throw TypeErrorNativeError("feature value is data, not double");
        }
        *out_value = value->value.as_double();
    });
}

extern "C" int32_t sk_feature_value_data(
    const sk_feature_value *value, const uint8_t **out_data, size_t *out_size,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_data) *out_data = nullptr;
    if (out_size) *out_size = 0;
    if (!value || !out_data || !out_size) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "feature data accessor arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        if (!value->value.is_data()) {
            throw TypeErrorNativeError("feature value is double, not data");
        }
        const auto data = value->value.as_data();
        *out_data = reinterpret_cast<const uint8_t *>(data.data);
        *out_size = data.size;
    });
}

extern "C" void sk_local_search_result_destroy(sk_local_search_result *result) SK_NOEXCEPT { destroy_noexcept(result); }

extern "C" int32_t sk_attribute_document_create(
    const char *id, sk_attribute_document **out, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out) *out = nullptr;
    if (!id || !out) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "attribute document arguments are invalid");
    return invoke(error, error_capacity, [&] { validate_document_id(id); auto document = std::make_unique<sk_attribute_document>(); document->id = id; *out = document.release(); });
}

extern "C" int32_t sk_attribute_document_add_string(sk_attribute_document *document, const char *field, const char *value, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!document || !field || !value) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "attribute string arguments are invalid");
    return invoke(error, error_capacity, [&] { OwnedValue v; v.kind = SK_VALUE_STRING; v.string = value; append_owned_value(document->fields, field, std::move(v)); });
}
extern "C" int32_t sk_attribute_document_add_integer(sk_attribute_document *document, const char *field, int64_t value, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!document || !field) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "attribute integer arguments are invalid");
    return invoke(error, error_capacity, [&] { OwnedValue v; v.kind = SK_VALUE_INTEGER; v.integer = value; append_owned_value(document->fields, field, std::move(v)); });
}
extern "C" int32_t sk_attribute_document_add_float(sk_attribute_document *document, const char *field, double value, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!document || !field) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "attribute float arguments are invalid");
    return invoke(error, error_capacity, [&] { if (!std::isfinite(value)) throw std::invalid_argument("attribute float value must be finite"); OwnedValue v; v.kind = SK_VALUE_FLOAT; v.floating = value; append_owned_value(document->fields, field, std::move(v)); });
}
extern "C" int32_t sk_attribute_document_add_boolean(sk_attribute_document *document, const char *field, uint8_t value, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!document || !field) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "attribute boolean arguments are invalid");
    return invoke(error, error_capacity, [&] { OwnedValue v; v.kind = SK_VALUE_BOOLEAN; v.boolean = value != 0; append_owned_value(document->fields, field, std::move(v)); });
}
extern "C" void sk_attribute_document_destroy(sk_attribute_document *document) SK_NOEXCEPT { destroy_noexcept(document); }

extern "C" int32_t sk_attribute_update_create(const char *id, sk_attribute_update **out, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out) *out = nullptr;
    if (!id || !out) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "attribute update arguments are invalid");
    return invoke(error, error_capacity, [&] { validate_document_id(id); auto update = std::make_unique<sk_attribute_update>(); update->id = id; *out = update.release(); });
}
extern "C" int32_t sk_attribute_update_add_string(sk_attribute_update *update, const char *field, const char *value, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!update || !field || !value) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "attribute string update arguments are invalid");
    return invoke(error, error_capacity, [&] { OwnedValue v; v.kind = SK_VALUE_STRING; v.string = value; append_owned_value(update->assignments, field, std::move(v)); });
}
extern "C" int32_t sk_attribute_update_add_integer(sk_attribute_update *update, const char *field, int64_t value, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!update || !field) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "attribute integer update arguments are invalid");
    return invoke(error, error_capacity, [&] { OwnedValue v; v.kind = SK_VALUE_INTEGER; v.integer = value; append_owned_value(update->assignments, field, std::move(v)); });
}
extern "C" int32_t sk_attribute_update_add_float(sk_attribute_update *update, const char *field, double value, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!update || !field) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "attribute float update arguments are invalid");
    return invoke(error, error_capacity, [&] { if (!std::isfinite(value)) throw std::invalid_argument("attribute float value must be finite"); OwnedValue v; v.kind = SK_VALUE_FLOAT; v.floating = value; append_owned_value(update->assignments, field, std::move(v)); });
}
extern "C" int32_t sk_attribute_update_add_boolean(sk_attribute_update *update, const char *field, uint8_t value, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!update || !field) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "attribute boolean update arguments are invalid");
    return invoke(error, error_capacity, [&] { OwnedValue v; v.kind = SK_VALUE_BOOLEAN; v.boolean = value != 0; append_owned_value(update->assignments, field, std::move(v)); });
}
extern "C" void sk_attribute_update_destroy(sk_attribute_update *update) SK_NOEXCEPT { destroy_noexcept(update); }

extern "C" int32_t sk_engine_put_attributes(sk_engine *engine, const sk_attribute_document *document, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!engine || !engine->impl || !document) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "attribute put arguments are invalid");
    return invoke(error, error_capacity, [&] { engine->impl->put_attributes(*document); });
}
extern "C" int32_t sk_engine_update_attributes(sk_engine *engine, const sk_attribute_update *update, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!engine || !engine->impl || !update) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "attribute update arguments are invalid");
    return invoke(error, error_capacity, [&] { engine->impl->update_attributes(*update); });
}

extern "C" int32_t sk_grouping_plan_create(
    sk_grouping_plan **out_plan, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out_plan) *out_plan = nullptr;
    if (!out_plan) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "grouping plan arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        auto plan = std::make_unique<sk_grouping_plan>();
        *out_plan = plan.release();
    });
}

extern "C" int32_t sk_grouping_plan_set_all(
    sk_grouping_plan *plan, uint8_t all, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!plan) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "grouping plan is null");
    }
    return invoke(error, error_capacity, [&] { plan->all = all != 0; });
}

extern "C" int32_t sk_grouping_expression_attribute(
    const char *field, sk_grouping_expression **out_expression,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_expression) *out_expression = nullptr;
    if (!field || !out_expression) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "grouping expression arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        auto expression = make_grouping_attribute_expression(field);
        *out_expression = expression.release();
    });
}

extern "C" int32_t sk_grouping_expression_string(
    const char *value, sk_grouping_expression **out_expression,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_expression) *out_expression = nullptr;
    if (!value || !out_expression) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "grouping string literal arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        *out_expression = make_grouping_literal_string_expression(value).release();
    });
}

extern "C" int32_t sk_grouping_expression_integer(
    int64_t value, sk_grouping_expression **out_expression,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_expression) *out_expression = nullptr;
    if (!out_expression) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "grouping integer literal output is null");
    }
    return invoke(error, error_capacity, [&] {
        *out_expression = make_grouping_literal_integer_expression(value).release();
    });
}

extern "C" int32_t sk_grouping_expression_float(
    double value, sk_grouping_expression **out_expression,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_expression) *out_expression = nullptr;
    if (!out_expression) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "grouping float literal output is null");
    }
    return invoke(error, error_capacity, [&] {
        *out_expression = make_grouping_literal_float_expression(value).release();
    });
}

extern "C" int32_t sk_grouping_expression_boolean(
    uint8_t value, sk_grouping_expression **out_expression,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_expression) *out_expression = nullptr;
    if (!out_expression) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "grouping boolean literal output is null");
    }
    return invoke(error, error_capacity, [&] {
        *out_expression = make_grouping_literal_boolean_expression(value != 0).release();
    });
}

extern "C" int32_t sk_grouping_expression_negative(
    const sk_grouping_expression *input, sk_grouping_expression **out_expression,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_expression) *out_expression = nullptr;
    if (!input || !out_expression) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "grouping unary expression arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        *out_expression = make_grouping_unary_expression(*input).release();
    });
}

extern "C" int32_t sk_grouping_expression_binary(
    uint32_t operator_kind, const sk_grouping_expression *left,
    const sk_grouping_expression *right, sk_grouping_expression **out_expression,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_expression) *out_expression = nullptr;
    if (!left || !right || !out_expression) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "grouping binary expression arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        *out_expression = make_grouping_binary_expression(operator_kind, *left, *right).release();
    });
}

extern "C" SK_API int32_t sk_grouping_expression_function(
    const char *name, const sk_grouping_expression *const *arguments,
    uint32_t argument_count, sk_grouping_expression **out_expression,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_expression) *out_expression = nullptr;
    if (!name || !out_expression) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "grouping function arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        *out_expression = make_grouping_function_expression(
            name, arguments, argument_count).release();
    });
}

extern "C" int32_t sk_grouping_expression_aggregation_reference(
    uint32_t index, sk_grouping_expression **out_expression,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_expression) *out_expression = nullptr;
    if (!out_expression) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "grouping expression output is null");
    }
    return invoke(error, error_capacity, [&] {
        *out_expression = make_grouping_aggregation_reference_expression(index).release();
    });
}

extern "C" void sk_grouping_expression_destroy(
    sk_grouping_expression *expression) SK_NOEXCEPT {
    destroy_noexcept(expression);
}

extern "C" int32_t sk_grouping_aggregation_count(
    sk_grouping_aggregation **out, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out) *out = nullptr;
    if (!out) {
        return invoke(error, error_capacity, [] {
            throw std::invalid_argument("grouping aggregation output is null");
        });
    }
    return invoke(error, error_capacity, [&] {
        auto aggregation = std::make_unique<sk_grouping_aggregation>();
        aggregation->kind = GroupingAggregationKind::COUNT;
        *out = aggregation.release();
    });
}

extern "C" int32_t sk_grouping_aggregation_average(
    const sk_grouping_expression *expression,
    sk_grouping_aggregation **out, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out) *out = nullptr;
    if (!expression || !out) {
        return invoke(error, error_capacity, [] {
            throw std::invalid_argument("average grouping aggregation arguments are invalid");
        });
    }
    return invoke(error, error_capacity, [&] {
        if (!expression->expression) {
            throw std::invalid_argument("grouping expression is empty");
        }
        auto aggregation = std::make_unique<sk_grouping_aggregation>();
        aggregation->kind = GroupingAggregationKind::AVERAGE;
        aggregation->expression.reset(expression->expression->clone());
        *out = aggregation.release();
    });
}

int32_t grouping_expression_aggregation(
    GroupingAggregationKind kind, const sk_grouping_expression *expression,
    sk_grouping_aggregation **out, uint8_t *error, uint32_t error_capacity) {
    if (out) *out = nullptr;
    if (!expression || !out) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "grouping aggregation arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        if (!expression->expression) throw std::invalid_argument("grouping expression is empty");
        auto aggregation = std::make_unique<sk_grouping_aggregation>();
        aggregation->kind = kind;
        aggregation->expression.reset(expression->expression->clone());
        *out = aggregation.release();
    });
}

extern "C" int32_t sk_grouping_aggregation_sum(
    const sk_grouping_expression *expression, sk_grouping_aggregation **out,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    return grouping_expression_aggregation(GroupingAggregationKind::SUM, expression, out, error, error_capacity);
}
extern "C" int32_t sk_grouping_aggregation_min(
    const sk_grouping_expression *expression, sk_grouping_aggregation **out,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    return grouping_expression_aggregation(GroupingAggregationKind::MIN, expression, out, error, error_capacity);
}
extern "C" int32_t sk_grouping_aggregation_max(
    const sk_grouping_expression *expression, sk_grouping_aggregation **out,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    return grouping_expression_aggregation(GroupingAggregationKind::MAX, expression, out, error, error_capacity);
}
extern "C" int32_t sk_grouping_aggregation_stddev(
    const sk_grouping_expression *expression, sk_grouping_aggregation **out,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    return grouping_expression_aggregation(GroupingAggregationKind::STDDEV, expression, out, error, error_capacity);
}
extern "C" int32_t sk_grouping_aggregation_xor(
    const sk_grouping_expression *expression, sk_grouping_aggregation **out,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    return grouping_expression_aggregation(GroupingAggregationKind::XOR, expression, out, error, error_capacity);
}

extern "C" int32_t sk_grouping_aggregation_quantiles(
    const sk_grouping_expression *expression, const double *quantiles,
    uint32_t quantile_count, sk_grouping_aggregation **out,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out) *out = nullptr;
    if (!expression || !quantiles || !out || quantile_count == 0) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "quantile grouping aggregation arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        if (!expression->expression) throw std::invalid_argument("grouping expression is empty");
        if (quantile_count > 1024) throw std::invalid_argument("too many grouping quantiles");
        auto aggregation = std::make_unique<sk_grouping_aggregation>();
        aggregation->kind = GroupingAggregationKind::QUANTILES;
        aggregation->expression.reset(expression->expression->clone());
        aggregation->quantiles.assign(quantiles, quantiles + quantile_count);
        for (double quantile : aggregation->quantiles) {
            if (!std::isfinite(quantile) || quantile < 0.0 || quantile > 1.0) {
                throw std::invalid_argument("grouping quantiles must be finite values in [0, 1]");
            }
        }
        *out = aggregation.release();
    });
}

extern "C" int32_t sk_grouping_aggregation_summary(
    const char *summary_class, uint32_t max_hits, sk_grouping_aggregation **out,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out) *out = nullptr;
    if (!summary_class || !out) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "summary grouping aggregation arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        validate_string_size(summary_class, MAX_SCHEMA_IDENTIFIER_BYTES, "summary class");
        auto aggregation = std::make_unique<sk_grouping_aggregation>();
        aggregation->kind = GroupingAggregationKind::SUMMARY;
        aggregation->summary_class = summary_class;
        aggregation->max_hits = max_hits;
        *out = aggregation.release();
    });
}

extern "C" void sk_grouping_aggregation_destroy(
    sk_grouping_aggregation *aggregation) SK_NOEXCEPT {
    destroy_noexcept(aggregation);
}

extern "C" int32_t sk_grouping_plan_add_level(
    sk_grouping_plan *plan, const sk_grouping_expression *classifier,
    sk_grouping_level **out_level, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out_level) *out_level = nullptr;
    if (!plan || !classifier || !out_level) {
        return invoke(error, error_capacity, [] {
            throw std::invalid_argument("grouping level arguments are invalid");
        });
    }
    return invoke(error, error_capacity, [&] {
        if (!classifier->expression) {
            throw std::invalid_argument("grouping classifier expression is empty");
        }
        auto level = std::make_unique<sk_grouping_level>();
        level->classifier.reset(classifier->expression->clone());
        plan->levels.push_back(std::move(level));
        *out_level = plan->levels.back().get();
    });
}

extern "C" int32_t sk_grouping_level_set_max_groups(
    sk_grouping_level *level, int64_t max_groups, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!level) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                                 "grouping level is null");
    return invoke(error, error_capacity, [&] {
        if (max_groups < -1) throw std::invalid_argument("grouping max groups must be -1 or non-negative");
        level->max_groups = max_groups;
    });
}

extern "C" int32_t sk_grouping_level_set_precision(
    sk_grouping_level *level, int64_t precision, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!level) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                                 "grouping level is null");
    return invoke(error, error_capacity, [&] {
        if (precision < -1) throw std::invalid_argument("grouping precision must be -1 or non-negative");
        level->precision = precision;
    });
}

std::unique_ptr<sk_grouping_filter> make_grouping_regex_filter(
    const char *pattern, std::unique_ptr<search::expression::ExpressionNode> expression) {
    validate_string_size(pattern, MAX_RANK_PROFILE_BYTES, "grouping regex");
    if (!expression) throw std::invalid_argument("grouping filter expression is empty");
    auto filter = std::make_unique<sk_grouping_filter>();
    filter->pattern = pattern;
    filter->expression.reset(expression->clone());
    filter->predicate = std::make_unique<search::expression::RegexPredicateNode>(
        filter->pattern, std::move(expression));
    return filter;
}

std::unique_ptr<sk_grouping_filter> make_grouping_filter(
    std::unique_ptr<search::expression::FilterPredicateNode> predicate) {
    if (!predicate) throw std::invalid_argument("grouping filter predicate is empty");
    auto filter = std::make_unique<sk_grouping_filter>();
    filter->predicate = std::move(predicate);
    return filter;
}

extern "C" int32_t sk_grouping_filter_regex(
    const char *pattern, const char *field, sk_grouping_filter **out_filter,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_filter) *out_filter = nullptr;
    if (!pattern || !field || !out_filter) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "grouping filter arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        auto expression = make_grouping_attribute_expression(field);
        auto filter = make_grouping_regex_filter(
            pattern, std::unique_ptr<search::expression::ExpressionNode>(
                expression->expression->clone()));
        *out_filter = filter.release();
    });
}

extern "C" int32_t sk_grouping_filter_regex_expression(
    const char *pattern, const sk_grouping_expression *expression,
    sk_grouping_filter **out_filter, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out_filter) *out_filter = nullptr;
    if (!pattern || !expression || !expression->expression || !out_filter) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "grouping regex filter arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        *out_filter = make_grouping_regex_filter(
            pattern, std::unique_ptr<search::expression::ExpressionNode>(
                expression->expression->clone())).release();
    });
}

template <typename Predicate>
int32_t grouping_filter_multi(
    const sk_grouping_filter *const *filters, uint32_t filter_count,
    sk_grouping_filter **out_filter, uint8_t *error, uint32_t error_capacity) {
    if (out_filter) *out_filter = nullptr;
    if (!out_filter || (filter_count != 0 && !filters) || filter_count == 0) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "grouping filter list is invalid");
    }
    return invoke(error, error_capacity, [&] {
        auto predicate = std::make_unique<Predicate>();
        for (uint32_t index = 0; index < filter_count; ++index) {
            if (!filters[index] || !filters[index]->predicate) {
                throw std::invalid_argument("grouping filter list contains an empty predicate");
            }
            predicate->args().emplace_back(filters[index]->predicate->clone());
        }
        *out_filter = make_grouping_filter(std::move(predicate)).release();
    });
}

extern "C" int32_t sk_grouping_filter_and(
    const sk_grouping_filter *const *filters, uint32_t filter_count,
    sk_grouping_filter **out_filter, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    return grouping_filter_multi<search::expression::AndPredicateNode>(
        filters, filter_count, out_filter, error, error_capacity);
}

extern "C" int32_t sk_grouping_filter_or(
    const sk_grouping_filter *const *filters, uint32_t filter_count,
    sk_grouping_filter **out_filter, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    return grouping_filter_multi<search::expression::OrPredicateNode>(
        filters, filter_count, out_filter, error, error_capacity);
}

extern "C" int32_t sk_grouping_filter_not(
    const sk_grouping_filter *filter, sk_grouping_filter **out_filter,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_filter) *out_filter = nullptr;
    if (!filter || !filter->predicate || !out_filter) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "grouping not filter arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        auto predicate = std::make_unique<search::expression::NotPredicateNode>(
            std::unique_ptr<search::expression::FilterPredicateNode>(
                filter->predicate->clone()));
        *out_filter = make_grouping_filter(std::move(predicate)).release();
    });
}

extern "C" int32_t sk_grouping_filter_range(
    double lower, double upper, const sk_grouping_expression *expression,
    uint8_t include_lower, uint8_t include_upper,
    sk_grouping_filter **out_filter, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out_filter) *out_filter = nullptr;
    if (!expression || !expression->expression || !out_filter ||
        !std::isfinite(lower) || !std::isfinite(upper)) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "grouping range filter arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        auto predicate = std::make_unique<search::expression::RangePredicateNode>(
            lower, upper,
            std::unique_ptr<search::expression::ExpressionNode>(expression->expression->clone()),
            include_lower != 0, include_upper != 0);
        *out_filter = make_grouping_filter(std::move(predicate)).release();
    });
}

extern "C" int32_t sk_grouping_filter_is_true(
    const sk_grouping_expression *expression, sk_grouping_filter **out_filter,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_filter) *out_filter = nullptr;
    if (!expression || !expression->expression || !out_filter) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "grouping is-true filter arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        auto predicate = std::make_unique<search::expression::IsTruePredicateNode>(
            std::unique_ptr<search::expression::ExpressionNode>(expression->expression->clone()));
        *out_filter = make_grouping_filter(std::move(predicate)).release();
    });
}

extern "C" void sk_grouping_filter_destroy(sk_grouping_filter *filter) SK_NOEXCEPT {
    destroy_noexcept(filter);
}

extern "C" int32_t sk_grouping_level_set_filter(
    sk_grouping_level *level, const sk_grouping_filter *filter,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!level || !filter) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                                           "grouping filter arguments are invalid");
    return invoke(error, error_capacity, [&] {
        sk_grouping_filter copy;
        copy.pattern = filter->pattern;
        copy.expression.reset(filter->expression ? filter->expression->clone() : nullptr);
        copy.predicate.reset(filter->predicate ? filter->predicate->clone() : nullptr);
        level->filter = std::move(copy);
    });
}

extern "C" int32_t sk_grouping_level_add_order_by(
    sk_grouping_level *level, const sk_grouping_expression *expression,
    uint8_t ascending, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!level || !expression || !expression->expression) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "grouping order arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        level->order_by.push_back({std::unique_ptr<search::expression::ExpressionNode>(expression->expression->clone()), ascending != 0});
    });
}

extern "C" int32_t sk_grouping_level_add_count(
    sk_grouping_level *level, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!level) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "grouping level is null");
    }
    return invoke(error, error_capacity, [&] {
        sk_grouping_aggregation aggregation;
        aggregation.kind = GroupingAggregationKind::COUNT;
        attach_grouping_aggregation(*level, aggregation);
    });
}

extern "C" int32_t sk_grouping_level_add_aggregation(
    sk_grouping_level *level, uint32_t kind, const char *field,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!level || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "grouping aggregation arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        if (kind != SK_AGGREGATION_AVERAGE && kind != SK_AGGREGATION_SUM &&
            kind != SK_AGGREGATION_MIN && kind != SK_AGGREGATION_MAX &&
            kind != SK_AGGREGATION_STDDEV && kind != SK_AGGREGATION_XOR) {
            throw UnsupportedNativeError("grouping_aggregation_kind", "unsupported grouping aggregation kind");
        }
        sk_grouping_aggregation aggregation;
        switch (kind) {
        case SK_AGGREGATION_SUM: aggregation.kind = GroupingAggregationKind::SUM; break;
        case SK_AGGREGATION_MIN: aggregation.kind = GroupingAggregationKind::MIN; break;
        case SK_AGGREGATION_MAX: aggregation.kind = GroupingAggregationKind::MAX; break;
        case SK_AGGREGATION_STDDEV: aggregation.kind = GroupingAggregationKind::STDDEV; break;
        case SK_AGGREGATION_XOR: aggregation.kind = GroupingAggregationKind::XOR; break;
        default: aggregation.kind = GroupingAggregationKind::AVERAGE; break;
        }
        aggregation.expression.reset(
            make_grouping_attribute_expression(field)->expression->clone());
        attach_grouping_aggregation(*level, aggregation);
    });
}

extern "C" int32_t sk_grouping_level_add_aggregation_handle(
    sk_grouping_level *level, const sk_grouping_aggregation *aggregation,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!level || !aggregation) {
        return invoke(error, error_capacity, [] {
            throw std::invalid_argument("grouping aggregation handle arguments are invalid");
        });
    }
    return invoke(error, error_capacity, [&] {
        if (aggregation->kind != GroupingAggregationKind::COUNT &&
            aggregation->kind != GroupingAggregationKind::SUMMARY &&
            !aggregation->expression) {
            throw std::invalid_argument("grouping aggregation expression is empty");
        }
        if (aggregation->kind == GroupingAggregationKind::QUANTILES && aggregation->quantiles.empty()) {
            throw std::invalid_argument("quantile grouping aggregation has no quantiles");
        }
        attach_grouping_aggregation(*level, *aggregation);
    });
}

extern "C" int32_t sk_grouping_plan_add_root_aggregation_handle(
    sk_grouping_plan *plan, const sk_grouping_aggregation *aggregation,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!plan || !aggregation) {
        return invoke(error, error_capacity, [] {
            throw std::invalid_argument("root grouping aggregation handle arguments are invalid");
        });
    }
    return invoke(error, error_capacity, [&] {
        if (aggregation->kind != GroupingAggregationKind::COUNT &&
            aggregation->kind != GroupingAggregationKind::SUMMARY &&
            !aggregation->expression) {
            throw std::invalid_argument("root grouping aggregation expression is empty");
        }
        sk_grouping_aggregation copy;
        copy.kind = aggregation->kind;
        if (aggregation->expression) copy.expression.reset(aggregation->expression->clone());
        copy.quantiles = aggregation->quantiles;
        copy.summary_class = aggregation->summary_class;
        copy.max_hits = aggregation->max_hits;
        plan->root_aggregations.push_back(std::move(copy));
    });
}

extern "C" void sk_grouping_plan_destroy(sk_grouping_plan *plan) SK_NOEXCEPT {
    destroy_noexcept(plan);
}

extern "C" int32_t sk_engine_group_tree(
    const sk_engine *engine, const sk_query *query,
    const sk_grouping_plan *plan, sk_grouping_tree_result **out_result,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    // EmbeddedEngine::group_tree maps GroupingLevel and Grouping classifiers and
    // their AggregationResult, CountAggregationResult, and
    // AverageAggregationResult prototypes from AttributeNode expressions into
    // groupSpec, then recursively decodes Group, getChildrenSize, getChild, and
    // groupResult from the real Vespa grouping response.
    if (out_result) *out_result = nullptr;
    if (!engine || !engine->impl || !query || !plan || !out_result) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "general grouping arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        auto result = std::make_unique<sk_grouping_tree_result>(
            engine->impl->group_tree(*query, *plan));
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        result->result_faults = engine->impl->result_faults();
        result->result_destroyed = engine->impl->result_destroyed_counter();
        if (result->root) {
            attach_grouping_faults(*result->root, result->result_faults);
        }
#endif
        *out_result = result.release();
    });
}

extern "C" int32_t sk_engine_group_tree_with_request(
    const sk_engine *engine, const sk_query *query,
    const sk_generic_search_request *request, const sk_grouping_plan *plan,
    sk_grouping_tree_result **out_result, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out_result) *out_result = nullptr;
    if (!engine || !engine->impl || !query || !request || !plan || !out_result) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "general grouping request arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        auto result = std::make_unique<sk_grouping_tree_result>(
            engine->impl->group_tree(*query, *plan,
                                     static_cast<const LocalSearchRequestState &>(*request)));
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        result->result_faults = engine->impl->result_faults();
        result->result_destroyed = engine->impl->result_destroyed_counter();
        if (result->root) {
            attach_grouping_faults(*result->root, result->result_faults);
        }
#endif
        *out_result = result.release();
    });
}

extern "C" int32_t sk_grouping_tree_result_root(
    const sk_grouping_tree_result *result,
    const sk_grouping_tree_node **out_root, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out_root) *out_root = nullptr;
    if (!result || !result->root || !out_root) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "grouping result root arguments are invalid");
    }
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    if (result->result_faults &&
        result->result_faults->take(SK_TEST_RESULT_FAULT_GROUPING_TREE_ROOT_ACCESSOR_ERROR)) {
        return invoke(error, error_capacity, [] {
            throw std::runtime_error("injected grouping tree root accessor failure");
        });
    }
#endif
    *out_root = result->root.get();
    return SK_NATIVE_STATUS_OK;
}

extern "C" uint32_t sk_grouping_tree_node_group_count(
    const sk_grouping_tree_node *node) SK_NOEXCEPT {
    return node ? static_cast<uint32_t>(node->groups.size()) : 0;
}

extern "C" int32_t sk_grouping_tree_node_group(
    const sk_grouping_tree_node *node, uint32_t group_index,
    const sk_grouping_tree_node **out_group, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out_group) *out_group = nullptr;
    if (!node || !out_group || group_index >= node->groups.size()) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "grouping child arguments are invalid");
    }
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    if (node->result_faults &&
        node->result_faults->take(SK_TEST_RESULT_FAULT_GROUPING_TREE_GROUP_ACCESSOR_ERROR)) {
        return invoke(error, error_capacity, [] {
            throw std::runtime_error("injected grouping tree child accessor failure");
        });
    }
    if (node->result_faults &&
        node->result_faults->take(SK_TEST_RESULT_FAULT_NULL_GROUPING_CHILD)) {
        return SK_NATIVE_STATUS_OK;
    }
#endif
    *out_group = node->groups[group_index].get();
    return SK_NATIVE_STATUS_OK;
}

extern "C" int32_t sk_grouping_tree_node_key(
    const sk_grouping_tree_node *node,
    const sk_grouping_result_value **out_value, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out_value) *out_value = nullptr;
    if (!node || !node->key || !out_value) {
        return invoke(error, error_capacity, [] {
            throw std::invalid_argument("grouping node key arguments are invalid");
        });
    }
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    if (node->result_faults &&
        node->result_faults->take(SK_TEST_RESULT_FAULT_GROUPING_TREE_KEY_ACCESSOR_ERROR)) {
        return invoke(error, error_capacity, [] {
            throw std::runtime_error("injected grouping tree key accessor failure");
        });
    }
    if (node->result_faults &&
        node->result_faults->take(SK_TEST_RESULT_FAULT_NULL_GROUPING_VALUE)) {
        return SK_NATIVE_STATUS_OK;
    }
#endif
    *out_value = node->key.get();
    return SK_NATIVE_STATUS_OK;
}

extern "C" uint32_t sk_grouping_result_value_kind(
    const sk_grouping_result_value *value) SK_NOEXCEPT {
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    if (value && value->result_faults &&
        value->result_faults->take(SK_TEST_RESULT_FAULT_UNKNOWN_GROUPING_VALUE_KIND)) {
        return 99;
    }
#endif
    return value ? value->kind : 0;
}

extern "C" int32_t sk_grouping_result_value_string(
    const sk_grouping_result_value *value, const char **out_string,
    uint32_t *out_length, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_string) *out_string = nullptr;
    if (out_length) *out_length = 0;
    if (!value || !out_string || !out_length) {
        return invoke(error, error_capacity, [] {
            throw std::invalid_argument("grouping string value arguments are invalid");
        });
    }
    return invoke(error, error_capacity, [&] {
        if (value->kind != SK_GROUPING_RESULT_VALUE_STRING) {
            throw std::invalid_argument("grouping result value is not a string");
        }
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        if (value->result_faults &&
            value->result_faults->take(SK_TEST_RESULT_FAULT_INVALID_GROUPING_STRING)) {
            static const char invalid_utf8[] = {static_cast<char>(0xff)};
            *out_string = invalid_utf8;
            *out_length = sizeof(invalid_utf8);
            return;
        }
#endif
        if (value->string.size() > std::numeric_limits<uint32_t>::max()) {
            throw std::runtime_error("grouping string value is too large");
        }
        *out_string = value->string.data();
        *out_length = static_cast<uint32_t>(value->string.size());
    });
}

extern "C" int32_t sk_grouping_result_value_integer(
    const sk_grouping_result_value *value, int64_t *out_integer,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_integer) *out_integer = 0;
    if (!value || !out_integer) {
        return invoke(error, error_capacity, [] {
            throw std::invalid_argument("grouping integer value arguments are invalid");
        });
    }
    return invoke(error, error_capacity, [&] {
        if (value->kind != SK_GROUPING_RESULT_VALUE_INTEGER) {
            throw std::invalid_argument("grouping result value is not an integer");
        }
        *out_integer = value->integer;
    });
}

extern "C" int32_t sk_grouping_result_value_float(
    const sk_grouping_result_value *value, double *out_float,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_float) *out_float = 0.0;
    if (!value || !out_float) {
        return invoke(error, error_capacity, [] {
            throw std::invalid_argument("grouping float value arguments are invalid");
        });
    }
    return invoke(error, error_capacity, [&] {
        if (value->kind != SK_GROUPING_RESULT_VALUE_FLOAT) {
            throw std::invalid_argument("grouping result value is not a float");
        }
        *out_float = value->floating;
    });
}

extern "C" int32_t sk_grouping_result_value_boolean(
    const sk_grouping_result_value *value, uint8_t *out_boolean,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_boolean) *out_boolean = 0;
    if (!value || !out_boolean) {
        return invoke(error, error_capacity, [] {
            throw std::invalid_argument("grouping boolean value arguments are invalid");
        });
    }
    return invoke(error, error_capacity, [&] {
        if (value->kind != SK_GROUPING_RESULT_VALUE_BOOLEAN) {
            throw std::invalid_argument("grouping result value is not a boolean");
        }
        *out_boolean = value->boolean ? 1 : 0;
    });
}

extern "C" uint32_t sk_grouping_result_value_quantile_count(
    const sk_grouping_result_value *value) SK_NOEXCEPT {
    return value && value->kind == SK_GROUPING_RESULT_VALUE_QUANTILES
               ? static_cast<uint32_t>(value->quantiles.size())
               : 0;
}

extern "C" int32_t sk_grouping_result_value_quantile(
    const sk_grouping_result_value *value, uint32_t index,
    double *out_quantile, double *out_value, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out_quantile) *out_quantile = 0.0;
    if (out_value) *out_value = 0.0;
    if (!value || value->kind != SK_GROUPING_RESULT_VALUE_QUANTILES ||
        !out_quantile || !out_value || index >= value->quantiles.size()) {
        return invoke(error, error_capacity, [] {
            throw std::invalid_argument("grouping quantile value arguments are invalid");
        });
    }
    return invoke(error, error_capacity, [&] {
        *out_quantile = value->quantiles[index].first;
        *out_value = value->quantiles[index].second;
    });
}

extern "C" uint32_t sk_grouping_tree_node_output_count(
    const sk_grouping_tree_node *node) SK_NOEXCEPT {
    return node ? static_cast<uint32_t>(node->outputs.size()) : 0;
}

extern "C" int32_t sk_grouping_tree_node_output(
    const sk_grouping_tree_node *node, uint32_t output_index,
    const sk_grouping_tree_output **out_output, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out_output) *out_output = nullptr;
    if (!node || !out_output || output_index >= node->outputs.size()) {
        return invoke(error, error_capacity, [] {
            throw std::invalid_argument("grouping output arguments are invalid");
        });
    }
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    if (node->result_faults &&
        node->result_faults->take(SK_TEST_RESULT_FAULT_GROUPING_TREE_OUTPUT_ACCESSOR_ERROR)) {
        return invoke(error, error_capacity, [] {
            throw std::runtime_error("injected grouping tree output accessor failure");
        });
    }
    if (node->result_faults &&
        node->result_faults->take(SK_TEST_RESULT_FAULT_NULL_GROUPING_OUTPUT)) {
        return SK_NATIVE_STATUS_OK;
    }
#endif
    *out_output = node->outputs[output_index].get();
    return SK_NATIVE_STATUS_OK;
}

extern "C" uint32_t sk_grouping_tree_output_kind(
    const sk_grouping_tree_output *output) SK_NOEXCEPT {
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    if (output && output->result_faults &&
        output->result_faults->take(SK_TEST_RESULT_FAULT_HIT_LIST_GROUPING_OUTPUT)) {
        return SK_GROUPING_TREE_OUTPUT_HIT_LIST;
    }
    if (output && output->result_faults &&
        output->result_faults->take(SK_TEST_RESULT_FAULT_UNKNOWN_GROUPING_OUTPUT_KIND)) {
        return 99;
    }
#endif
    return output ? output->kind : 0;
}

extern "C" int32_t sk_grouping_tree_output_value(
    const sk_grouping_tree_output *output,
    const sk_grouping_result_value **out_value, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out_value) *out_value = nullptr;
    if (!output || !output->value || !out_value) {
        return invoke(error, error_capacity, [] {
            throw std::invalid_argument("grouping output value arguments are invalid");
        });
    }
    if (output->kind != SK_GROUPING_TREE_OUTPUT_VALUE) {
        return invoke(error, error_capacity, [] {
            throw std::invalid_argument("grouping output is not a value");
        });
    }
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    if (output->result_faults &&
        output->result_faults->take(SK_TEST_RESULT_FAULT_GROUPING_OUTPUT_VALUE_ACCESSOR_ERROR)) {
        return invoke(error, error_capacity, [] {
            throw std::runtime_error("injected grouping output value accessor failure");
        });
    }
    if (output->result_faults &&
        output->result_faults->take(SK_TEST_RESULT_FAULT_NULL_GROUPING_VALUE)) {
        return SK_NATIVE_STATUS_OK;
    }
#endif
    *out_value = output->value.get();
    return SK_NATIVE_STATUS_OK;
}

extern "C" uint32_t sk_grouping_tree_output_hit_count(
    const sk_grouping_tree_output *output) SK_NOEXCEPT {
    return (output && output->kind == SK_GROUPING_TREE_OUTPUT_HIT_LIST)
               ? static_cast<uint32_t>(output->hits.size())
               : 0;
}

extern "C" int32_t sk_grouping_tree_output_hit_id(
    const sk_grouping_tree_output *output, uint32_t hit_index,
    const char **out_id, uint32_t *out_id_length, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out_id) *out_id = nullptr;
    if (out_id_length) *out_id_length = 0;
    if (!output || output->kind != SK_GROUPING_TREE_OUTPUT_HIT_LIST ||
        !out_id || !out_id_length || hit_index >= output->hits.size()) {
        return invoke(error, error_capacity, [] {
            throw std::invalid_argument("grouping hit-list arguments are invalid");
        });
    }
    const auto &id = output->hits[hit_index].id;
    *out_id = id.data();
    *out_id_length = static_cast<uint32_t>(id.size());
    return SK_NATIVE_STATUS_OK;
}

extern "C" int32_t sk_grouping_tree_output_hit_score(
    const sk_grouping_tree_output *output, uint32_t hit_index, double *out_score,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!output || output->kind != SK_GROUPING_TREE_OUTPUT_HIT_LIST || !out_score ||
        hit_index >= output->hits.size()) {
        return invoke(error, error_capacity, [] {
            throw std::invalid_argument("grouping hit score arguments are invalid");
        });
    }
    *out_score = output->hits[hit_index].score;
    return SK_NATIVE_STATUS_OK;
}

extern "C" uint32_t sk_grouping_tree_output_hit_summary_count(
    const sk_grouping_tree_output *output, uint32_t hit_index) SK_NOEXCEPT {
    return (output && output->kind == SK_GROUPING_TREE_OUTPUT_HIT_LIST &&
            hit_index < output->hits.size())
               ? static_cast<uint32_t>(output->hits[hit_index].summary.size())
               : 0;
}

extern "C" int32_t sk_grouping_tree_output_hit_summary_name(
    const sk_grouping_tree_output *output, uint32_t hit_index,
    uint32_t summary_index, const char **out_name, uint32_t *out_length,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_name) *out_name = nullptr;
    if (out_length) *out_length = 0;
    if (!output || output->kind != SK_GROUPING_TREE_OUTPUT_HIT_LIST || !out_name ||
        !out_length || hit_index >= output->hits.size() ||
        summary_index >= output->hits[hit_index].summary.size()) {
        return invoke(error, error_capacity, [] {
            throw std::invalid_argument("grouping hit summary name arguments are invalid");
        });
    }
    const auto &name = output->hits[hit_index].summary[summary_index].name;
    *out_name = name.data();
    *out_length = static_cast<uint32_t>(name.size());
    return SK_NATIVE_STATUS_OK;
}

extern "C" int32_t sk_grouping_tree_output_hit_summary_value(
    const sk_grouping_tree_output *output, uint32_t hit_index,
    uint32_t summary_index, uint32_t *out_kind, const char **out_string,
    uint32_t *out_string_length, int64_t *out_integer, double *out_float,
    uint8_t *out_boolean, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_kind) *out_kind = 0;
    if (out_string) *out_string = nullptr;
    if (out_string_length) *out_string_length = 0;
    if (out_integer) *out_integer = 0;
    if (out_float) *out_float = 0.0;
    if (out_boolean) *out_boolean = 0;
    if (!output || output->kind != SK_GROUPING_TREE_OUTPUT_HIT_LIST || !out_kind ||
        !out_string || !out_string_length || !out_integer || !out_float ||
        !out_boolean || hit_index >= output->hits.size() ||
        summary_index >= output->hits[hit_index].summary.size()) {
        return invoke(error, error_capacity, [] {
            throw std::invalid_argument("grouping hit summary value arguments are invalid");
        });
    }
    const auto &value = output->hits[hit_index].summary[summary_index];
    *out_kind = value.kind;
    *out_string = value.string.data();
    *out_string_length = static_cast<uint32_t>(value.string.size());
    *out_integer = value.integer;
    *out_float = value.floating;
    *out_boolean = value.boolean;
    return SK_NATIVE_STATUS_OK;
}

extern "C" void sk_grouping_tree_result_destroy(
    sk_grouping_tree_result *result) SK_NOEXCEPT {
    destroy_noexcept(result);
}

extern "C" int32_t sk_local_grouping_request_create(const char *field, sk_local_grouping_request **out, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out) *out = nullptr;
    if (!field || !out) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "grouping request arguments are invalid");
    return invoke(error, error_capacity, [&] { validate_field_name(field); auto request = std::make_unique<sk_local_grouping_request>(); request->group_field = field; *out = request.release(); });
}
extern "C" int32_t sk_local_grouping_request_add_aggregation(sk_local_grouping_request *request, uint32_t kind, const char *field, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!request || !field) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "grouping aggregation arguments are invalid");
    return invoke(error, error_capacity, [&] { validate_field_name(field); if (kind < SK_AGGREGATION_SUM || kind > SK_AGGREGATION_AVERAGE) throw std::invalid_argument("unsupported aggregation kind"); request->aggregations.push_back({kind, field}); });
}
extern "C" void sk_local_grouping_request_destroy(sk_local_grouping_request *request) SK_NOEXCEPT { destroy_noexcept(request); }
extern "C" int32_t sk_engine_group_local(const sk_engine *engine, const sk_query *query, const sk_local_grouping_request *request, sk_local_grouping_result **out, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out) *out = nullptr;
    if (!engine || !engine->impl || !query || !request || !out) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "local grouping arguments are invalid");
    return invoke(error, error_capacity, [&] { auto result = std::make_unique<sk_local_grouping_result>(); static_cast<LocalGroupingResultState &>(*result) = engine->impl->group_local_result(*query, *request); *out = result.release(); });
}
extern "C" uint32_t sk_local_grouping_result_group_count(const sk_local_grouping_result *result) SK_NOEXCEPT { return result ? static_cast<uint32_t>(result->groups.size()) : 0; }

int32_t grouping_result_group(const LocalGroupingResultState *result, uint32_t index,
                              uint32_t *out_key_kind, const char **out_key_string,
                              uint32_t *out_key_string_length, int64_t *out_key_integer,
                              uint64_t *out_count, uint8_t *error,
                              uint32_t error_capacity) noexcept {
    if (out_key_kind) *out_key_kind = 0;
    if (out_key_string) *out_key_string = nullptr;
    if (out_key_string_length) *out_key_string_length = 0;
    if (out_key_integer) *out_key_integer = 0;
    if (out_count) *out_count = 0;
    if (!result || !out_key_kind || !out_key_string || !out_key_string_length ||
        !out_key_integer || !out_count || index >= result->groups.size()) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "group result arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        const auto &group = result->groups[index];
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        static const uint8_t invalid_utf8[] = {0xff};
        if (result->result_faults &&
            result->result_faults->take(SK_TEST_RESULT_FAULT_GROUP_ACCESSOR_ERROR)) {
            throw std::runtime_error("injected grouping accessor failure");
        }
        if (result->result_faults &&
            result->result_faults->take(SK_TEST_RESULT_FAULT_UNSUPPORTED_GROUP_KEY)) {
            *out_key_kind = 99;
            *out_key_string = nullptr;
            *out_key_string_length = 0;
            *out_key_integer = 0;
            *out_count = group.count;
            return;
        }
        if (result->result_faults &&
            result->result_faults->take(SK_TEST_RESULT_FAULT_INVALID_GROUP_KEY)) {
            *out_key_kind = SK_VALUE_STRING;
            *out_key_string = reinterpret_cast<const char *>(invalid_utf8);
            *out_key_string_length = sizeof(invalid_utf8);
            *out_key_integer = 0;
            *out_count = group.count;
            return;
        }
#endif
        *out_key_kind = group.key.kind;
        *out_key_string = group.key.string.data();
        *out_key_string_length = static_cast<uint32_t>(group.key.string.size());
        *out_key_integer = group.key.kind == SK_VALUE_BOOLEAN
                               ? (group.key.boolean ? 1 : 0)
                               : group.key.integer;
        *out_count = group.count;
    });
}

extern "C" int32_t sk_local_grouping_result_group(const sk_local_grouping_result *result, uint32_t index, uint32_t *out_key_kind, const char **out_key_string, uint32_t *out_key_string_length, int64_t *out_key_integer, uint64_t *out_count, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    return grouping_result_group(result, index, out_key_kind, out_key_string,
                                 out_key_string_length, out_key_integer, out_count,
                                 error, error_capacity);
}
extern "C" uint32_t sk_local_grouping_result_aggregation_count(const sk_local_grouping_result *result, uint32_t index) SK_NOEXCEPT { return result && index < result->groups.size() ? static_cast<uint32_t>(result->groups[index].aggregations.size()) : 0; }
extern "C" int32_t sk_local_grouping_result_aggregation(const sk_local_grouping_result *result, uint32_t group_index, uint32_t aggregation_index, uint32_t *out_kind, int64_t *out_integer, double *out_float, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_kind) *out_kind = 0;
    if (out_integer) *out_integer = 0;
    if (out_float) *out_float = 0.0;
    if (!result || !out_kind || !out_integer || !out_float || group_index >= result->groups.size() || aggregation_index >= result->groups[group_index].aggregations.size()) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "aggregation result arguments are invalid");
    return invoke(error, error_capacity, [&] {
        const auto &value = result->groups[group_index].aggregations[aggregation_index];
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        if (result->result_faults &&
            result->result_faults->take(SK_TEST_RESULT_FAULT_AGGREGATION_ACCESSOR_ERROR)) {
            throw std::runtime_error("injected aggregation accessor failure");
        }
#endif
        *out_kind = value.kind;
        *out_integer = value.integer;
        *out_float = value.floating;
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        if (result->result_faults &&
            result->result_faults->take(SK_TEST_RESULT_FAULT_UNKNOWN_AGGREGATION_KIND)) {
            *out_kind = 99;
        }
#endif
    });
}
extern "C" void sk_local_grouping_result_destroy(sk_local_grouping_result *result) SK_NOEXCEPT { destroy_noexcept(result); }

extern "C" int32_t sk_generic_search_request_create(
    const char *rank_profile, uint32_t limit, uint32_t offset,
    uint32_t summary_kind, sk_generic_search_request **out_request,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_request) *out_request = nullptr;
    if (!rank_profile || !out_request) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "generic search request arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        validate_rank_profile(rank_profile);
        if (*rank_profile == '\0') throw std::invalid_argument("rank profile must not be empty");
        if (limit > MAX_SEARCH_LIMIT) throw std::invalid_argument("search limit is too large");
        if (summary_kind != SK_SUMMARY_NONE && summary_kind != SK_SUMMARY_DEFAULT &&
            summary_kind != SK_SUMMARY_FIELDS) {
            throw std::invalid_argument("unsupported summary kind");
        }
        auto request = std::make_unique<sk_generic_search_request>();
        request->rank_profile = rank_profile;
        request->limit = limit;
        request->offset = offset;
        request->summary_kind = summary_kind;
        *out_request = request.release();
    });
}

extern "C" int32_t sk_generic_search_request_set_timeout(
    sk_generic_search_request *request, uint64_t timeout_millis,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!request) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "generic search request is null");
    }
    return invoke(error, error_capacity, [&] {
        if (timeout_millis > static_cast<uint64_t>(std::numeric_limits<int64_t>::max())) {
            throw std::invalid_argument("search timeout is too large");
        }
        request->timeout_millis = timeout_millis;
    });
}

extern "C" int32_t sk_generic_search_request_set_diagnostics(
    sk_generic_search_request *request, uint32_t trace_level,
    uint32_t matching_depth, uint32_t first_phase_depth,
    uint32_t second_phase_depth, uint32_t raw_bytes,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!request || trace_level > 9 || matching_depth > 16 ||
        first_phase_depth > 16 || second_phase_depth > 16 ||
        raw_bytes == 0 || raw_bytes > 262144) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "diagnostic controls exceed safe bounds");
    }
    request->diagnostic_trace_level = trace_level;
    request->diagnostic_matching_depth = matching_depth;
    request->diagnostic_first_phase_depth = first_phase_depth;
    request->diagnostic_second_phase_depth = second_phase_depth;
    request->diagnostic_raw_bytes = raw_bytes;
    return SK_NATIVE_STATUS_OK;
}

extern "C" int32_t sk_generic_search_request_add_sort(
    sk_generic_search_request *request, const char *field, uint32_t order,
    uint32_t missing, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!request || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "generic sort arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        if (request->raw_sort_spec) throw std::invalid_argument("structured sort is mutually exclusive with raw sort spec");
        validate_field_name(field);
        if (order != SK_SORT_ASCENDING && order != SK_SORT_DESCENDING) {
            throw std::invalid_argument("unsupported sort order");
        }
        if (missing != SK_SORT_MISSING_DEFAULT && missing != SK_SORT_MISSING_FIRST &&
            missing != SK_SORT_MISSING_LAST) {
            throw std::invalid_argument("unsupported sort missing placement");
        }
        request->sort.push_back({field, order, missing});
    });
}

extern "C" int32_t sk_generic_search_request_set_sort_spec(
    sk_generic_search_request *request, const char *sort_spec,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!request || !sort_spec) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "raw sort arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        validate_string_size(sort_spec, MAX_RANK_PROFILE_BYTES, "raw sort spec");
        if (*sort_spec == '\0') throw std::invalid_argument("raw sort spec must not be empty");
        if (!request->sort.empty()) throw std::invalid_argument("raw sort is mutually exclusive with structured sort");
        request->raw_sort_spec = sort_spec;
    });
}

extern "C" int32_t sk_generic_search_request_add_summary_field(
    sk_generic_search_request *request, const char *field, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    return sk_local_search_request_add_summary_field(
        static_cast<sk_local_search_request *>(request), field, error, error_capacity);
}

extern "C" int32_t sk_generic_search_request_add_highlight_field(
    sk_generic_search_request *request, const char *field,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    return sk_local_search_request_add_highlight_field(
        static_cast<sk_local_search_request *>(request), field, error, error_capacity);
}

extern "C" int32_t sk_generic_search_request_set_summary_class(
    sk_generic_search_request *request, const char *name, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    return sk_local_search_request_set_summary_class(
        static_cast<sk_local_search_request *>(request), name, error, error_capacity);
}

extern "C" int32_t sk_generic_search_request_add_query_feature(
    sk_generic_search_request *request, const char *name, double value,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!request || !name) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "query feature arguments are invalid");
    }
    return invoke(error, error_capacity,
                  [&] { add_query_feature(*request, name, value); });
}

extern "C" int32_t sk_generic_search_request_add_query_tensor_dense(
    sk_generic_search_request *request, const char *name, const char *type_spec,
    const double *values, uint32_t value_count, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!request || !name || !type_spec || !values) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "query tensor arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        validate_string_size(name, MAX_SCHEMA_IDENTIFIER_BYTES, "query tensor name");
        if (*name == '\0') throw std::invalid_argument("query tensor name must not be empty");
        if (std::find_if(request->query_tensors.begin(), request->query_tensors.end(),
                         [name](const auto &entry) { return entry.first == name; }) !=
            request->query_tensors.end()) {
            throw std::invalid_argument("duplicate query tensor");
        }
        if (std::find_if(request->query_features.begin(), request->query_features.end(),
                         [name](const auto &entry) { return entry.first == name; }) !=
            request->query_features.end()) {
            throw std::invalid_argument("query tensor collides with query feature");
        }
        if (std::find_if(request->rank_properties.begin(), request->rank_properties.end(),
                         [name](const auto &entry) { return entry.first == name; }) !=
            request->rank_properties.end()) {
            throw std::invalid_argument("query tensor collides with rank property");
        }
        auto encoded = encode_dense_tensor(type_spec, values, value_count);
        request->query_tensors.emplace_back(name, std::move(encoded));
    });
}

extern "C" int32_t sk_generic_search_request_add_rank_property(
    sk_generic_search_request *request, const char *name, const char *value,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!request || !name || !value) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "rank property arguments are invalid");
    }
    return invoke(error, error_capacity,
                  [&] { add_rank_property(*request, name, value); });
}

extern "C" int32_t sk_generic_search_request_add_grouping(
    sk_generic_search_request *request, const char *group_field,
    uint32_t max_groups, uint32_t order, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!request || !group_field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "generic grouping arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        validate_field_name(group_field);
        if (order > SK_GROUP_ORDER_VALUE_DESC) {
            throw std::invalid_argument("unsupported grouping order");
        }
        request->groupings.push_back(
            LocalGroupingRequestState{std::string(group_field), max_groups, order, {}});
    });
}

extern "C" int32_t sk_generic_search_request_add_grouping_plan(
    sk_generic_search_request *request, const sk_grouping_plan *plan,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!request || !plan) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "structured grouping plan arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        request->grouping_plans.push_back(clone_grouping_plan(*plan));
    });
}

extern "C" void sk_generic_search_request_destroy(
    sk_generic_search_request *request) SK_NOEXCEPT { destroy_noexcept(request); }

extern "C" int32_t sk_engine_search_generic(
    const sk_engine *engine, const sk_query *query,
    const sk_generic_search_request *request,
    sk_generic_search_result **out_result, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out_result) *out_result = nullptr;
    if (query && !query->consume()) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "query handle has already been consumed");
    }
    if (!engine || !engine->impl || !query || !request || !out_result) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "generic search arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        auto result = std::make_unique<sk_local_search_result>();
        static_cast<LocalSearchResultState &>(*result) =
            engine->impl->search_local_result(*query, *request);
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        engine->impl->failpoint("result_after_allocation");
        result->failpoints = engine->impl->failpoints();
        result->result_faults = engine->impl->result_faults();
        result->result_destroyed = engine->impl->result_destroyed_counter();
        for (auto &grouping : result->groupings) {
            grouping.failpoints = result->failpoints;
            grouping.result_faults = result->result_faults;
        }
        for (auto &tree : result->grouping_trees) {
            tree->result_faults = result->result_faults;
            tree->result_destroyed = result->result_destroyed;
            if (tree->root) attach_grouping_faults(*tree->root, result->result_faults);
        }
        if (result->result_faults->take(SK_TEST_RESULT_FAULT_NULL_RESULT)) {
            return;
        }
#endif
        *out_result = result.release();
    });
}

extern "C" uint64_t sk_generic_search_result_total_hit_count(
    const sk_generic_search_result *result) SK_NOEXCEPT {
    return sk_local_search_result_total_hit_count(
        static_cast<const sk_local_search_result *>(result));
}

extern "C" int32_t sk_diagnostic_tensor_decode(
    const uint8_t *data, uint32_t length, char *type_spec,
    uint32_t type_capacity, uint32_t *out_type_length, double *cells,
    uint32_t cell_capacity, uint32_t *out_cell_count,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_type_length) *out_type_length = 0;
    if (out_cell_count) *out_cell_count = 0;
    return guarded(error, error_capacity, [&] {
        if (!data || !type_spec || !cells || !out_type_length || !out_cell_count ||
            length == 0 || length > 262144 || cell_capacity == 0 || cell_capacity > 4096) {
            throw std::invalid_argument("diagnostic tensor arguments exceed bounds");
        }
        vespalib::nbostream stream(reinterpret_cast<const char *>(data), length);
        auto tensor = vespalib::eval::decode_value(stream, vespalib::eval::FastValueBuilderFactory::get());
        if (tensor->type().count_mapped_dimensions() != 0 || tensor->type().count_indexed_dimensions() != 1) {
            throw UnsupportedNativeError("diagnostic_tensor_shape_unsupported", "this integration normalizes only single-indexed dense ranking tensors");
        }
        if (tensor->cells().size > cell_capacity) {
            throw TensorValidationError("diagnostic_tensor_truncated", "ranking tensor exceeds the diagnostic cell budget");
        }
        const auto type = tensor->type().to_spec();
        if (type.size() > type_capacity) throw std::invalid_argument("tensor type exceeds output capacity");
        const auto values = dense_tensor_cells(*tensor);
        std::memcpy(type_spec, type.data(), type.size());
        std::copy(values.begin(), values.end(), cells);
        *out_type_length = static_cast<uint32_t>(type.size());
        *out_cell_count = static_cast<uint32_t>(values.size());
    });
}

extern "C" int32_t sk_generic_search_result_diagnostics(
    const sk_generic_search_result *result, const uint8_t **out_trace,
    uint32_t *out_length, uint32_t *out_truncated,
    double *out_matching_seconds, double *out_summary_seconds,
    double *out_query_seconds, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!result || !out_trace || !out_length || !out_truncated ||
        !out_matching_seconds || !out_summary_seconds || !out_query_seconds) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "diagnostic result arguments are invalid");
    }
    *out_trace = reinterpret_cast<const uint8_t *>(result->diagnostic_trace.data());
    *out_length = static_cast<uint32_t>(result->diagnostic_trace.size());
    *out_truncated = result->diagnostic_truncated ? 1U : 0U;
    *out_matching_seconds = result->diagnostic_matching_seconds;
    *out_summary_seconds = result->diagnostic_summary_seconds;
    *out_query_seconds = result->diagnostic_query_seconds;
    return SK_NATIVE_STATUS_OK;
}
extern "C" uint32_t sk_generic_search_result_hit_count(
    const sk_generic_search_result *result) SK_NOEXCEPT {
    return sk_local_search_result_hit_count(static_cast<const sk_local_search_result *>(result));
}
extern "C" uint64_t sk_generic_search_result_coverage_active(
    const sk_generic_search_result *result) SK_NOEXCEPT {
    return result ? result->coverage_active : 0;
}
extern "C" uint64_t sk_generic_search_result_coverage_covered(
    const sk_generic_search_result *result) SK_NOEXCEPT {
    return result ? result->coverage_covered : 0;
}
extern "C" uint64_t sk_generic_search_result_coverage_target_active(
    const sk_generic_search_result *result) SK_NOEXCEPT {
    return result ? result->coverage_target_active : 0;
}
extern "C" uint32_t sk_generic_search_result_degradation_flags(
    const sk_generic_search_result *result) SK_NOEXCEPT {
    return result ? result->degradation_flags : 0;
}
extern "C" uint32_t sk_generic_search_result_issue_count(
    const sk_generic_search_result *result) SK_NOEXCEPT {
    return result ? static_cast<uint32_t>(result->issues.size()) : 0;
}
extern "C" int32_t sk_generic_search_result_issue_message(
    const sk_generic_search_result *result, uint32_t index, const char **out_message,
    uint32_t *out_length, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_message) *out_message = nullptr;
    if (out_length) *out_length = 0;
    if (!result || !out_message || !out_length || index >= result->issues.size()) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "search result issue arguments are invalid");
    }
    const auto &message = result->issues[index];
    *out_message = message.data();
    *out_length = static_cast<uint32_t>(message.size());
    return SK_NATIVE_STATUS_OK;
}
extern "C" int32_t sk_generic_search_result_hit(
    const sk_generic_search_result *result, uint32_t index, const char **out_id,
    uint32_t *out_id_length, double *out_score, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    return sk_local_search_result_hit(static_cast<const sk_local_search_result *>(result), index,
                                      out_id, out_id_length, out_score, error, error_capacity);
}
extern "C" uint32_t sk_generic_search_result_summary_count(
    const sk_generic_search_result *result, uint32_t hit_index) SK_NOEXCEPT {
    return sk_local_search_result_summary_count(static_cast<const sk_local_search_result *>(result),
                                                hit_index);
}
extern "C" int32_t sk_generic_search_result_summary_name(
    const sk_generic_search_result *result, uint32_t hit_index, uint32_t summary_index,
    const char **out_name, uint32_t *out_length, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    return sk_local_search_result_summary_name(static_cast<const sk_local_search_result *>(result),
                                               hit_index, summary_index, out_name, out_length,
                                               error, error_capacity);
}
extern "C" int32_t sk_generic_search_result_summary_value(
    const sk_generic_search_result *result, uint32_t hit_index, uint32_t summary_index,
    uint32_t *out_kind, const char **out_string, uint32_t *out_string_length,
    int64_t *out_integer, double *out_float, uint8_t *out_boolean, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    return sk_local_search_result_summary_value(
        static_cast<const sk_local_search_result *>(result), hit_index, summary_index, out_kind,
        out_string, out_string_length, out_integer, out_float, out_boolean, error,
        error_capacity);
}
extern "C" uint32_t sk_generic_search_result_highlight_count(
    const sk_generic_search_result *result, uint32_t hit_index) SK_NOEXCEPT {
    return sk_local_search_result_highlight_count(
        static_cast<const sk_local_search_result *>(result), hit_index);
}
extern "C" int32_t sk_generic_search_result_highlight_field(
    const sk_generic_search_result *result, uint32_t hit_index,
    uint32_t highlight_index, const char **out_field, uint32_t *out_length,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    return sk_local_search_result_highlight_field(
        static_cast<const sk_local_search_result *>(result), hit_index,
        highlight_index, out_field, out_length, error, error_capacity);
}
extern "C" uint32_t sk_generic_search_result_highlight_fragment_count(
    const sk_generic_search_result *result, uint32_t hit_index,
    uint32_t highlight_index) SK_NOEXCEPT {
    return sk_local_search_result_highlight_fragment_count(
        static_cast<const sk_local_search_result *>(result), hit_index,
        highlight_index);
}
extern "C" int32_t sk_generic_search_result_highlight_fragment_text(
    const sk_generic_search_result *result, uint32_t hit_index,
    uint32_t highlight_index, uint32_t fragment_index, const char **out_text,
    uint32_t *out_length, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    return sk_local_search_result_highlight_fragment_text(
        static_cast<const sk_local_search_result *>(result), hit_index,
        highlight_index, fragment_index, out_text, out_length, error,
        error_capacity);
}
extern "C" uint32_t sk_generic_search_result_highlight_match_count(
    const sk_generic_search_result *result, uint32_t hit_index,
    uint32_t highlight_index, uint32_t fragment_index) SK_NOEXCEPT {
    return sk_local_search_result_highlight_match_count(
        static_cast<const sk_local_search_result *>(result), hit_index,
        highlight_index, fragment_index);
}
extern "C" int32_t sk_generic_search_result_highlight_match(
    const sk_generic_search_result *result, uint32_t hit_index,
    uint32_t highlight_index, uint32_t fragment_index, uint32_t match_index,
    uint32_t *out_start_byte, uint32_t *out_end_byte,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    return sk_local_search_result_highlight_match(
        static_cast<const sk_local_search_result *>(result), hit_index,
        highlight_index, fragment_index, match_index, out_start_byte,
        out_end_byte, error, error_capacity);
}
extern "C" uint32_t sk_generic_search_result_match_feature_count(
    const sk_generic_search_result *result) SK_NOEXCEPT {
    return result ? match_feature_count(*result) : 0;
}
extern "C" int32_t sk_generic_search_result_match_feature_name(
    const sk_generic_search_result *result, uint32_t feature_index, const char **out_name,
    uint32_t *out_length, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_name) *out_name = nullptr;
    if (out_length) *out_length = 0;
    if (!result || !out_name || !out_length) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "match feature name arguments are invalid");
    }
    return invoke(error, error_capacity,
                  [&] { match_feature_name(*result, feature_index, out_name, out_length); });
}
extern "C" int32_t sk_generic_search_result_match_feature_value(
    const sk_generic_search_result *result, uint32_t hit_index, uint32_t feature_index,
    double *out_value, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_value) *out_value = 0.0;
    if (!result || !out_value) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "match feature value arguments are invalid");
    }
    return invoke(error, error_capacity,
                  [&] { match_feature_value_double(*result, hit_index, feature_index, out_value); });
}
extern "C" int32_t sk_generic_search_result_match_feature_value_ref(
    const sk_generic_search_result *result, uint32_t hit_index,
    uint32_t feature_index, const sk_feature_value **out_value,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_value) *out_value = nullptr;
    if (!result || !out_value) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "match feature value arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        *out_value = match_feature_value_ref(*result, hit_index, feature_index);
    });
}
extern "C" uint32_t sk_generic_search_result_grouping_count(
    const sk_generic_search_result *result) SK_NOEXCEPT {
    return result ? static_cast<uint32_t>(result->groupings.size()) : 0;
}
extern "C" uint32_t sk_generic_search_result_grouping_tree_count(
    const sk_generic_search_result *result) SK_NOEXCEPT {
    return result ? static_cast<uint32_t>(result->grouping_trees.size()) : 0;
}

extern "C" int32_t sk_generic_search_result_grouping_tree_root(
    const sk_generic_search_result *result, uint32_t grouping_index,
    const sk_grouping_tree_node **out_root, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out_root) *out_root = nullptr;
    if (!result || !out_root || grouping_index >= result->grouping_trees.size()) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "structured grouping result arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        *out_root = result->grouping_trees[grouping_index]->root.get();
    });
}
extern "C" uint32_t sk_generic_search_result_group_count(
    const sk_generic_search_result *result, uint32_t grouping_index) SK_NOEXCEPT {
    return result && grouping_index < result->groupings.size()
               ? static_cast<uint32_t>(result->groupings[grouping_index].groups.size())
               : 0;
}
extern "C" int32_t sk_generic_search_result_group(
    const sk_generic_search_result *result, uint32_t grouping_index,
    uint32_t group_index, uint32_t *out_key_kind, const char **out_key_string,
    uint32_t *out_key_string_length, int64_t *out_key_integer,
    uint64_t *out_count, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!result || grouping_index >= result->groupings.size()) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "grouping result index is out of bounds");
    }
    return grouping_result_group(&result->groupings[grouping_index], group_index,
                                 out_key_kind, out_key_string,
                                 out_key_string_length, out_key_integer,
                                 out_count, error, error_capacity);
}
extern "C" void sk_generic_search_result_destroy(
    sk_generic_search_result *result) SK_NOEXCEPT { destroy_noexcept(result); }

extern "C" int32_t sk_generic_grouping_request_create(
    const char *field, uint32_t max_groups, uint32_t order,
    sk_generic_grouping_request **out, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out) *out = nullptr;
    if (!field || !out) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "generic grouping request arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        validate_field_name(field);
        if (order > SK_GROUP_ORDER_VALUE_DESC) {
            throw std::invalid_argument("unsupported grouping order");
        }
        auto request = std::make_unique<sk_generic_grouping_request>();
        request->group_field = field;
        request->max_groups = max_groups;
        request->order = order;
        *out = request.release();
    });
}

extern "C" int32_t sk_generic_grouping_request_add_aggregation(
    sk_generic_grouping_request *request, uint32_t kind, const char *field,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    return sk_local_grouping_request_add_aggregation(
        static_cast<sk_local_grouping_request *>(request), kind, field, error, error_capacity);
}
extern "C" void sk_generic_grouping_request_destroy(
    sk_generic_grouping_request *request) SK_NOEXCEPT { destroy_noexcept(request); }
extern "C" int32_t sk_engine_group_generic(
    const sk_engine *engine, const sk_query *query,
    const sk_generic_grouping_request *request,
    sk_generic_grouping_result **out, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out) *out = nullptr;
    if (!engine || !engine->impl || !query || !request || !out) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "generic grouping arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        auto result = std::make_unique<sk_local_grouping_result>();
        static_cast<LocalGroupingResultState &>(*result) =
            engine->impl->group_local_result(*query, *request);
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        result->failpoints = engine->impl->failpoints();
        result->result_faults = engine->impl->result_faults();
        result->result_destroyed = engine->impl->result_destroyed_counter();
        if (result->result_faults->take(SK_TEST_RESULT_FAULT_NULL_RESULT)) {
            return;
        }
#endif
        *out = result.release();
    });
}
extern "C" uint32_t sk_generic_grouping_result_group_count(
    const sk_generic_grouping_result *result) SK_NOEXCEPT {
    return sk_local_grouping_result_group_count(
        static_cast<const sk_local_grouping_result *>(result));
}
extern "C" int32_t sk_generic_grouping_result_group(
    const sk_generic_grouping_result *result, uint32_t group_index,
    uint32_t *out_key_kind, const char **out_key_string,
    uint32_t *out_key_string_length, int64_t *out_key_integer,
    uint64_t *out_count, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    return sk_local_grouping_result_group(
        static_cast<const sk_local_grouping_result *>(result), group_index, out_key_kind,
        out_key_string, out_key_string_length, out_key_integer, out_count, error, error_capacity);
}
extern "C" uint32_t sk_generic_grouping_result_aggregation_count(
    const sk_generic_grouping_result *result, uint32_t group_index) SK_NOEXCEPT {
    return sk_local_grouping_result_aggregation_count(
        static_cast<const sk_local_grouping_result *>(result), group_index);
}
extern "C" int32_t sk_generic_grouping_result_aggregation(
    const sk_generic_grouping_result *result, uint32_t group_index,
    uint32_t aggregation_index, uint32_t *out_kind, int64_t *out_integer,
    double *out_float, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    return sk_local_grouping_result_aggregation(
        static_cast<const sk_local_grouping_result *>(result), group_index, aggregation_index,
        out_kind, out_integer, out_float, error, error_capacity);
}
extern "C" void sk_generic_grouping_result_destroy(
    sk_generic_grouping_result *result) SK_NOEXCEPT { destroy_noexcept(result); }

extern "C" int32_t sk_text_document_create(
    const char *id, const char *field, const uint8_t *text, uint32_t text_length,
    sk_text_document **out, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out) {
        *out = nullptr;
    }
    if (!out || !id || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "text document arguments must not be null");
    }
    return invoke(error, error_capacity, [&] {
        auto document = std::make_unique<sk_text_document>();
        initialize_text_document(*document, id, field, text, text_length);
        *out = document.release();
    });
}

extern "C" int32_t sk_text_document_add_occurrences(
    sk_text_document *document, const uint8_t *term_bytes,
    uint32_t term_bytes_length, const uint32_t *term_lengths,
    const uint32_t *byte_starts, const uint32_t *byte_lengths,
    uint32_t occurrence_count, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!document) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "text document is null");
    }
    return invoke(error, error_capacity, [&] {
        append_text_occurrences(*document, term_bytes, term_bytes_length, term_lengths,
                                byte_starts, byte_lengths, occurrence_count);
    });
}

extern "C" void sk_text_document_destroy(sk_text_document *document) SK_NOEXCEPT {
    destroy_noexcept(document);
}

extern "C" int32_t sk_text_update_create(
    const char *id, const char *field, const uint8_t *text, uint32_t text_length,
    sk_text_update **out, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out) {
        *out = nullptr;
    }
    if (!out || !id || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "text update arguments must not be null");
    }
    return invoke(error, error_capacity, [&] {
        auto update = std::make_unique<sk_text_update>();
        initialize_text_document(*update, id, field, text, text_length);
        *out = update.release();
    });
}

extern "C" int32_t sk_text_update_add_occurrences(
    sk_text_update *update, const uint8_t *term_bytes,
    uint32_t term_bytes_length, const uint32_t *term_lengths,
    const uint32_t *byte_starts, const uint32_t *byte_lengths,
    uint32_t occurrence_count, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!update) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "text update is null");
    }
    return invoke(error, error_capacity, [&] {
        append_text_occurrences(*update, term_bytes, term_bytes_length, term_lengths,
                                byte_starts, byte_lengths, occurrence_count);
    });
}

extern "C" void sk_text_update_destroy(sk_text_update *update) SK_NOEXCEPT {
    destroy_noexcept(update);
}

extern "C" int32_t sk_text_request_create(
    uint32_t kind, const char *field, const char *rank_profile,
    sk_text_request **out, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out) {
        *out = nullptr;
    }
    if (!out || !field || !rank_profile) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "text request arguments must not be null");
    }
    return invoke(error, error_capacity, [&] {
        auto request = std::make_unique<sk_text_request>();
        initialize_text_request(*request, kind, field, rank_profile);
        *out = request.release();
    });
}

extern "C" int32_t sk_text_request_add_terms(
    sk_text_request *request, const uint8_t *term_bytes,
    uint32_t term_bytes_length, const uint32_t *term_lengths,
    uint32_t term_count, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!request) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "text request is null");
    }
    return invoke(error, error_capacity, [&] {
        append_text_terms(*request, term_bytes, term_bytes_length, term_lengths, term_count);
    });
}

extern "C" void sk_text_request_destroy(sk_text_request *request) SK_NOEXCEPT {
    destroy_noexcept(request);
}

extern "C" int32_t sk_engine_put_text(sk_engine *engine,
                                        const sk_text_document *document,
                                        uint8_t *error,
                                        uint32_t error_capacity) SK_NOEXCEPT {
    if (!engine || !engine->impl || !document) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "text put arguments are invalid");
    }
    return invoke(error, error_capacity, [&] { engine->impl->put_text(*document); });
}

extern "C" int32_t sk_engine_update_text(sk_engine *engine,
                                           const sk_text_update *update,
                                           uint8_t *error,
                                           uint32_t error_capacity) SK_NOEXCEPT {
    if (!engine || !engine->impl || !update) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "text update arguments are invalid");
    }
    return invoke(error, error_capacity, [&] { engine->impl->update_text(*update); });
}

extern "C" int32_t sk_engine_search_text(const sk_engine *engine,
                                           const sk_text_request *request,
                                           sk_text_result **out_result,
                                           uint8_t *error,
                                           uint32_t error_capacity) SK_NOEXCEPT {
    if (out_result) {
        *out_result = nullptr;
    }
    if (!engine || !engine->impl || !request || !out_result) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "text search arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        auto result = std::make_unique<sk_text_result>();
        result->hits = engine->impl->search_text(*request);
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        engine->impl->failpoint("result_after_allocation");
        result->failpoints = engine->impl->failpoints();
#endif
        *out_result = result.release();
    });
}

extern "C" uint32_t sk_text_result_size(const sk_text_result *result) SK_NOEXCEPT {
    return result ? static_cast<uint32_t>(result->hits.size()) : 0;
}

extern "C" int32_t sk_text_result_hit(const sk_text_result *result,
                                       uint32_t index, const char **out_id,
                                       uint32_t *out_length, double *out_score,
                                       uint8_t *error,
                                       uint32_t error_capacity) SK_NOEXCEPT {
    if (out_id) {
        *out_id = nullptr;
    }
    if (out_length) {
        *out_length = 0;
    }
    if (out_score) {
        *out_score = 0.0;
    }
    if (!result || !out_id || !out_length || !out_score) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "text result arguments are invalid");
    }
    if (index >= result->hits.size()) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "text result index is out of bounds");
    }
    return invoke(error, error_capacity, [&] {
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        if (result->failpoints) {
            result->failpoints->hit("result_before_copy");
        }
#endif
        *out_id = result->hits[index].id.data();
        *out_length = static_cast<uint32_t>(result->hits[index].id.size());
        *out_score = result->hits[index].score;
    });
}

extern "C" void sk_text_result_destroy(sk_text_result *result) SK_NOEXCEPT {
    destroy_noexcept(result);
}

extern "C" int32_t sk_query_create(const sk_engine *engine, sk_query **out_query,
                                    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_query) {
        *out_query = nullptr;
    }
    if (!engine || !engine->impl || !out_query) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "query create arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        auto query = std::make_unique<sk_query>();
        query->fields = engine->impl->query_field_metadata();
        *out_query = query.release();
    });
}

extern "C" int32_t sk_query_add_operator(sk_query *query, uint32_t kind,
                                           uint32_t child_count, uint8_t *error,
                                           uint32_t error_capacity) SK_NOEXCEPT {
    if (!query) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "query operator arguments are invalid");
    }
    return invoke(error, error_capacity, [&] { add_query_operator(*query, kind, child_count); });
}

extern "C" int32_t sk_query_validate_field(const sk_query *query, uint32_t kind,
                                              const char *field, uint8_t *error,
                                              uint32_t error_capacity) SK_NOEXCEPT {
    if (!query) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "query field arguments are invalid");
    }
    return invoke(error, error_capacity,
                  [&] { validate_query_field(*query, kind, field); });
}

extern "C" int32_t sk_query_add_true(sk_query *query, uint8_t *error,
                                      uint32_t error_capacity) SK_NOEXCEPT {
    if (!query) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "query true arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        query->ensure_active();
        query->builder.add_true_node();
    });
}

extern "C" int32_t sk_query_add_text(sk_query *query, uint32_t kind,
                                      const char *field, const uint8_t *term_bytes,
                                      uint32_t term_bytes_length,
                                      const uint32_t *term_lengths,
                                      uint32_t term_count, uint8_t *error,
                                      uint32_t error_capacity) SK_NOEXCEPT {
    if (!query) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "query text arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        add_query_text(*query, kind, field, term_bytes, term_bytes_length,
                       term_lengths, term_count);
    });
}

extern "C" int32_t sk_query_add_in_strings(
    sk_query *query, const char *field, const uint8_t *value_bytes,
    uint32_t value_bytes_length, const uint32_t *value_lengths,
    uint32_t value_count, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "in query arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        validate_query_field(*query, SK_QUERY_FIELD_STRING, field);
        if (value_count == 0 || value_count > MAX_TEXT_TERMS || !value_lengths ||
            (!value_bytes && value_bytes_length != 0)) {
            throw std::invalid_argument("in query string values are invalid");
        }
        size_t total_bytes = 0;
        for (uint32_t index = 0; index < value_count; ++index) {
            const auto length = value_lengths[index];
            if (length == 0 || length > MAX_TEXT_TERM_BYTES ||
                length > MAX_TOTAL_TEXT_TERM_BYTES - total_bytes) {
                throw std::invalid_argument("in query string value is too large or empty");
            }
            total_bytes += length;
        }
        if (total_bytes != value_bytes_length) {
            throw std::invalid_argument("in query string lengths do not match value bytes");
        }
        auto terms = std::make_unique<search::query::StringTermVector>(value_count);
        size_t offset = 0;
        for (uint32_t index = 0; index < value_count; ++index) {
            const auto length = value_lengths[index];
            terms->addTerm(std::string_view(reinterpret_cast<const char *>(value_bytes + offset), length));
            offset += length;
        }
        query->builder.add_in_term(std::move(terms), search::query::MultiTerm::Type::STRING,
                                   field, query->allocate_term_id(), Weight(1));
    });
}

extern "C" int32_t sk_query_add_in_integers(
    sk_query *query, const char *field, const int64_t *values,
    uint32_t value_count, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "in query arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        validate_query_field(*query, SK_QUERY_FIELD_INTEGER, field);
        if (!values || value_count == 0 || value_count > MAX_TEXT_TERMS) {
            throw std::invalid_argument("in query integer values are invalid");
        }
        auto terms = std::make_unique<search::query::IntegerTermVector>(value_count);
        for (uint32_t index = 0; index < value_count; ++index) {
            if (values[index] == std::numeric_limits<int64_t>::min()) {
                throw std::invalid_argument("in query integer value INT64_MIN is reserved");
            }
            terms->addTerm(values[index]);
        }
        query->builder.add_in_term(std::move(terms), search::query::MultiTerm::Type::INTEGER,
                                   field, query->allocate_term_id(), Weight(1));
    });
}

extern "C" int32_t sk_query_add_proximity(
    sk_query *query, uint8_t ordered, const char *field, uint32_t child_count,
    uint32_t distance, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "proximity query arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        validate_query_field(*query, SK_QUERY_FIELD_TEXT, field);
        if (child_count < 2) {
            throw std::invalid_argument("proximity query requires at least two children");
        }
        if (distance == 0) {
            throw std::invalid_argument("proximity query distance must be positive");
        }
        if (child_count > static_cast<uint32_t>(std::numeric_limits<int>::max())) {
            throw std::invalid_argument("query child count cannot be represented by Vespa");
        }
        if (ordered) {
            query->builder.addONear(static_cast<int>(child_count), distance, 0, 0);
        } else {
            query->builder.addNear(static_cast<int>(child_count), distance, 0, 0);
        }
    });
}

extern "C" int32_t sk_query_add_equiv(
    sk_query *query, uint32_t child_count, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!query) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "equiv query arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        if (child_count < 2) {
            throw std::invalid_argument("equiv query requires at least two children");
        }
        if (child_count > static_cast<uint32_t>(std::numeric_limits<int>::max())) {
            throw std::invalid_argument("equiv query child count is invalid");
        }
        query->builder.addEquiv(static_cast<int>(child_count), query->allocate_term_id(), Weight(100));
    });
}

extern "C" int32_t sk_query_add_weak_and(
    sk_query *query, const char *field, uint32_t child_count,
    uint32_t target_num_hits, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "weak-and query arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        query->ensure_active();
        if (child_count < 1 || child_count > static_cast<uint32_t>(std::numeric_limits<int>::max())) {
            throw std::invalid_argument("weak-and query child count is invalid");
        }
        if (target_num_hits == 0) {
            throw std::invalid_argument("weak-and target hits must be positive");
        }
        // Keep the legacy ABI field argument for compatibility. WeakAnd is a
        // boolean-like operator; its children carry their own field views.
        query->builder.addWeakAnd(static_cast<int>(child_count), target_num_hits, "");
    });
}

extern "C" int32_t sk_query_add_rank(
    sk_query *query, uint32_t child_count, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!query) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "rank query arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        query->ensure_active();
        if (child_count < 1 || child_count > static_cast<uint32_t>(std::numeric_limits<int>::max())) {
            throw std::invalid_argument("rank query child count is invalid");
        }
        query->builder.addRank(static_cast<int>(child_count));
    });
}

extern "C" int32_t sk_query_add_fuzzy(
    sk_query *query, const char *field, const char *term,
    uint32_t max_edit_distance, uint32_t prefix_lock_length,
    uint8_t prefix_match, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field || !term) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "fuzzy query arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        validate_fuzzy_query_field(*query, field);
        validate_string_size(term, MAX_TEXT_TERM_BYTES, "fuzzy query term");
        if (*term == '\0') {
            throw std::invalid_argument("fuzzy query term must not be empty");
        }
        query->builder.addFuzzyTerm(term, field, query->allocate_term_id(), Weight(100),
                                    max_edit_distance, prefix_lock_length, prefix_match != 0);
    });
}

extern "C" int32_t sk_query_add_alternatives(
    sk_query *query, const char *field, const uint8_t *term_bytes,
    uint32_t term_bytes_length, const uint32_t *term_lengths,
    const int64_t *exactness_weights, uint32_t term_count, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "alternatives query arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        validate_query_field(*query, SK_QUERY_FIELD_TEXT, field);
        validate_weighted_query_inputs(term_bytes, term_bytes_length, term_lengths,
                                       exactness_weights, term_count);
        auto terms = std::make_unique<search::query::WeightedStringTermVector>(term_count);
        size_t offset = 0;
        for (uint32_t index = 0; index < term_count; ++index) {
            const auto length = term_lengths[index];
            terms->addTerm(std::string_view(reinterpret_cast<const char *>(term_bytes + offset), length),
                           query_item_weight(exactness_weights[index]));
            offset += length;
        }
        query->builder.add_word_alternatives(
            std::move(terms), field, query->allocate_term_id(), Weight(100));
    });
}

extern "C" int32_t sk_query_add_matches(
    sk_query *query, const char *field, const char *pattern, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field || !pattern) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "matches query arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        validate_fuzzy_query_field(*query, field);
        validate_string_size(pattern, MAX_TEXT_TERM_BYTES, "matches query pattern");
        if (*pattern == '\0') {
            throw std::invalid_argument("matches query pattern must not be empty");
        }
        const auto index_field = query_index_field(*query, field);
        query->builder.addRegExpTerm(pattern, index_field, query->allocate_term_id(), Weight(100));
    });
}

extern "C" int32_t sk_query_add_labeled(
    sk_query *query, int32_t unique_id, double score, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!query) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "labeled query arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        query->ensure_active();
        if (unique_id < 0 || !std::isfinite(score)) {
            throw std::invalid_argument("labeled query id and score are invalid");
        }
        query->builder.add_label_wrapper(unique_id, score);
    });
}

extern "C" int32_t sk_query_add_weighted_set_strings(
    sk_query *query, const char *field, const uint8_t *key_bytes,
    uint32_t key_bytes_length, const uint32_t *key_lengths,
    const int64_t *weights, uint32_t item_count, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "weighted-set query arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        add_weighted_query_strings(*query, 0, field, key_bytes, key_bytes_length,
                                   key_lengths, weights, item_count);
    });
}

extern "C" int32_t sk_query_add_weighted_set_integers(
    sk_query *query, const char *field, const int64_t *keys,
    const int64_t *weights, uint32_t item_count, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "weighted-set query arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        add_weighted_query_integers(*query, 0, field, keys, weights, item_count);
    });
}

extern "C" int32_t sk_query_add_dot_product_strings(
    sk_query *query, const char *field, const uint8_t *key_bytes,
    uint32_t key_bytes_length, const uint32_t *key_lengths,
    const int64_t *weights, uint32_t item_count, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "dot-product query arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        add_weighted_query_strings(*query, 2, field, key_bytes, key_bytes_length,
                                   key_lengths, weights, item_count);
    });
}

extern "C" int32_t sk_query_add_dot_product_integers(
    sk_query *query, const char *field, const int64_t *keys,
    const int64_t *weights, uint32_t item_count, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "dot-product query arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        add_weighted_query_integers(*query, 2, field, keys, weights, item_count);
    });
}

extern "C" int32_t sk_query_add_wand_strings(
    sk_query *query, const char *field, const uint8_t *key_bytes,
    uint32_t key_bytes_length, const uint32_t *key_lengths,
    const int64_t *weights, uint32_t item_count, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "wand query arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        add_weighted_query_strings(*query, 1, field, key_bytes, key_bytes_length,
                                   key_lengths, weights, item_count);
    });
}

extern "C" int32_t sk_query_add_wand_integers(
    sk_query *query, const char *field, const int64_t *keys,
    const int64_t *weights, uint32_t item_count, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "wand query arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        add_weighted_query_integers(*query, 1, field, keys, weights, item_count);
    });
}

extern "C" int32_t sk_query_add_wand_strings_with_options(
    sk_query *query, const char *field, const uint8_t *key_bytes,
    uint32_t key_bytes_length, const uint32_t *key_lengths,
    const int64_t *weights, uint32_t item_count, uint32_t target_num_hits,
    int64_t score_threshold, double threshold_boost_factor, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "wand query arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        if (target_num_hits == 0 || !std::isfinite(threshold_boost_factor) ||
            threshold_boost_factor <= 0.0) {
            throw std::invalid_argument("wand query options are invalid");
        }
        add_weighted_query_strings(*query, 1, field, key_bytes, key_bytes_length,
                                   key_lengths, weights, item_count, target_num_hits,
                                   score_threshold, threshold_boost_factor);
    });
}

extern "C" int32_t sk_query_add_wand_integers_with_options(
    sk_query *query, const char *field, const int64_t *keys,
    const int64_t *weights, uint32_t item_count, uint32_t target_num_hits,
    int64_t score_threshold, double threshold_boost_factor, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "wand query arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        if (target_num_hits == 0 || !std::isfinite(threshold_boost_factor) ||
            threshold_boost_factor <= 0.0) {
            throw std::invalid_argument("wand query options are invalid");
        }
        add_weighted_query_integers(*query, 1, field, keys, weights, item_count,
                                    target_num_hits, score_threshold,
                                    threshold_boost_factor);
    });
}

extern "C" int32_t sk_query_add_equals(sk_query *query, const char *field,
                                        int64_t value, uint8_t *error,
                                        uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "query equality arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        validate_field_name(field);
        validate_query_field(*query, SK_QUERY_FIELD_INTEGER, field);
        if (value == std::numeric_limits<int64_t>::min()) {
            throw std::invalid_argument(
                "integer value INT64_MIN is reserved as Vespa's undefined attribute sentinel: " +
                std::string(field));
        }
        query->builder.addNumberTerm(std::to_string(value), field,
                                     query->allocate_term_id(), Weight(1));
    });
}

extern "C" int32_t sk_query_add_range(sk_query *query, const char *field,
                                       uint32_t lower_kind, int64_t lower_value,
                                       uint32_t upper_kind, int64_t upper_value,
                                       uint8_t *error,
                                       uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "query range arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        validate_field_name(field);
        validate_query_field(*query, SK_QUERY_FIELD_INTEGER, field);
        if ((lower_kind != SK_QUERY_BOUND_UNBOUNDED &&
             lower_value == std::numeric_limits<int64_t>::min()) ||
            (upper_kind != SK_QUERY_BOUND_UNBOUNDED &&
             upper_value == std::numeric_limits<int64_t>::min())) {
            throw std::invalid_argument(
                "integer value INT64_MIN is reserved as Vespa's undefined attribute sentinel: " +
                std::string(field));
        }
        auto range = search::query::Range(make_query_range(
            lower_kind, lower_value, upper_kind, upper_value));
        query->builder.addRangeTerm(range, field, query->allocate_term_id(), Weight(1));
    });
}

extern "C" int32_t sk_query_add_nearest_neighbor(
    sk_query *query, const char *document_field, const char *query_tensor,
    uint32_t target_num_hits, uint8_t allow_approximate, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !document_field || !query_tensor) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "nearest-neighbor arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        validate_field_name(query_tensor);
        if (*query_tensor == '\0') throw std::invalid_argument("query tensor must not be empty");
        if (target_num_hits == 0) throw std::invalid_argument("nearest-neighbor target hits must be positive");
        validate_query_field(*query, SK_QUERY_FIELD_TENSOR, document_field);
        query->builder.add_nearest_neighbor_term(
            query_tensor, document_field, query->allocate_term_id(), Weight(1),
            target_num_hits, allow_approximate != 0);
    });
}

extern "C" int32_t sk_nearest_neighbor_options_create(
    sk_nearest_neighbor_options **out_options, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out_options) *out_options = nullptr;
    if (!out_options) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                                       "nearest-neighbor options output is null");
    return invoke(error, error_capacity, [&] {
        *out_options = new sk_nearest_neighbor_options();
    });
}

extern "C" void sk_nearest_neighbor_options_destroy(
    sk_nearest_neighbor_options *options) SK_NOEXCEPT {
    destroy_noexcept(options);
}

extern "C" int32_t sk_nearest_neighbor_options_set_distance_threshold(
    sk_nearest_neighbor_options *options, double distance_threshold,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!options) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                                   "nearest-neighbor options are null");
    return invoke(error, error_capacity, [&] {
        if (!std::isfinite(distance_threshold) && !std::isinf(distance_threshold)) {
            throw std::invalid_argument("nearest-neighbor distance threshold must be finite or infinity");
        }
        if (distance_threshold < 0.0) throw std::invalid_argument("nearest-neighbor distance threshold must not be negative");
        options->distance_threshold = distance_threshold;
    });
}

extern "C" int32_t sk_nearest_neighbor_options_set_explore_additional_hits(
    sk_nearest_neighbor_options *options, uint32_t explore_additional_hits,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!options) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                                   "nearest-neighbor options are null");
    return invoke(error, error_capacity, [&] { options->explore_additional_hits = explore_additional_hits; });
}

extern "C" int32_t sk_query_add_nearest_neighbor_with_options_handle(
    sk_query *query, const char *document_field, const char *query_tensor,
    uint32_t target_num_hits, uint8_t allow_approximate,
    const sk_nearest_neighbor_options *options, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !document_field || !query_tensor || !options) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "nearest-neighbor handle arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        validate_field_name(query_tensor);
        if (*query_tensor == '\0' || target_num_hits == 0) {
            throw std::invalid_argument("nearest-neighbor arguments are invalid");
        }
        validate_query_field(*query, SK_QUERY_FIELD_TENSOR, document_field);
        search::query::NearestNeighborTerm::HnswParams params;
        if (options->distance_threshold) params.distance_threshold = *options->distance_threshold;
        if (options->explore_additional_hits != 0) {
            params.explore_additional_hits = options->explore_additional_hits;
        }
        query->builder.add_nearest_neighbor_term(
            query_tensor, document_field, query->allocate_term_id(), Weight(1),
            target_num_hits, allow_approximate != 0, std::move(params));
    });
}

extern "C" int32_t sk_query_add_same_element_with_filter(
    sk_query *query, const char *field, uint32_t child_count,
    const uint32_t *element_filter, uint32_t element_filter_count,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "same-element arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        if (child_count < 2 || child_count > static_cast<uint32_t>(std::numeric_limits<int>::max())) {
            throw std::invalid_argument("same-element query child count is invalid");
        }
        if (element_filter_count != 0 && !element_filter) {
            throw std::invalid_argument("same-element element filter must not be null");
        }
        validate_query_field(*query, SK_QUERY_FIELD_ARRAY, field);
        std::vector<uint32_t> filter;
        if (element_filter_count != 0) {
            filter.assign(element_filter, element_filter + element_filter_count);
        }
        query->builder.addSameElement(static_cast<int>(child_count), field,
                                      query->allocate_term_id(), Weight(100), std::move(filter));
        query->same_element_parent = field;
    });
}

extern "C" int32_t sk_query_add_same_element(
    sk_query *query, const char *field, uint32_t child_count, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    return sk_query_add_same_element_with_filter(
        query, field, child_count, nullptr, 0, error, error_capacity);
}

uint32_t parse_geo_radius_microdegrees(const char *radius) {
    if (!radius || *radius == '\0') throw std::invalid_argument("geo radius must not be empty");
    std::string value(radius);
    size_t first = value.find_first_not_of(" \t\r\n");
    size_t last = value.find_last_not_of(" \t\r\n");
    if (first == std::string::npos) throw std::invalid_argument("geo radius must not be empty");
    value = value.substr(first, last - first + 1);
    double multiplier = 1.0;
    std::string_view number = value;
    if (value.ends_with("km")) {
        multiplier = 1.0 / 111.1950802335329;
        number = std::string_view(value).substr(0, value.size() - 2);
    } else if (value.ends_with("m")) {
        multiplier = 1.0 / 111195.0802335329;
        number = std::string_view(value).substr(0, value.size() - 1);
    }
    std::string number_text(number);
    char *end = nullptr;
    const double amount = std::strtod(number_text.c_str(), &end);
    if (!std::isfinite(amount) || amount <= 0.0 || !end || *end != '\0') {
        throw std::invalid_argument("geo radius must be a positive number with optional m or km suffix");
    }
    const double microdegrees = amount * multiplier * 1'000'000.0;
    if (!std::isfinite(microdegrees) || microdegrees > std::numeric_limits<uint32_t>::max()) {
        throw std::invalid_argument("geo radius is outside the supported range");
    }
    return static_cast<uint32_t>(std::llround(microdegrees));
}

int32_t geo_microdegrees(double value, const char *name) {
    if (!std::isfinite(value)) throw std::invalid_argument(std::string("geo ") + name + " must be finite");
    const double result = value * 1'000'000.0;
    if (result < std::numeric_limits<int32_t>::min() || result > std::numeric_limits<int32_t>::max()) {
        throw std::invalid_argument(std::string("geo ") + name + " is outside the supported range");
    }
    return static_cast<int32_t>(std::llround(result));
}

extern "C" int32_t sk_query_add_geo_location(
    sk_query *query, const char *field, double latitude, double longitude,
    const char *radius, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field || !radius) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "geo-location arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        if (latitude < -90.0 || latitude > 90.0 || longitude < -180.0 || longitude > 180.0) {
            throw std::invalid_argument("geo coordinates are outside their valid ranges");
        }
        validate_query_field(*query, SK_QUERY_FIELD_POSITION, field);
        const auto point = search::query::Point{geo_microdegrees(longitude, "longitude"),
                                                  geo_microdegrees(latitude, "latitude")};
        const auto radius_microdegrees = parse_geo_radius_microdegrees(radius);
        const auto aspect = static_cast<uint32_t>(std::llround(
            std::min(1.0, std::max(0.0, std::cos(latitude * M_PI / 180.0))) *
            4'294'967'295.0));
        query->builder.addLocationTerm(
            search::query::Location(point, radius_microdegrees, aspect), field,
            query->allocate_term_id(), Weight(100));
    });
}

extern "C" int32_t sk_query_add_geo_bounding_box(
    sk_query *query, const char *field, double south, double west,
    double north, double east, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "geo bounding-box arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        if (south < -90.0 || north > 90.0 || south > north || west < -180.0 || east > 180.0 || west > east) {
            throw std::invalid_argument("geo bounding-box coordinates are invalid");
        }
        validate_query_field(*query, SK_QUERY_FIELD_POSITION, field);
        const auto rectangle = search::query::Rectangle(
            geo_microdegrees(west, "west"), geo_microdegrees(south, "south"),
            geo_microdegrees(east, "east"), geo_microdegrees(north, "north"));
        query->builder.addLocationTerm(search::query::Location(rectangle), field,
                                       query->allocate_term_id(), Weight(100));
    });
}

extern "C" int32_t sk_query_add_predicate(
    sk_query *query, const char *field,
    const uint8_t *feature_key_bytes, uint32_t feature_key_bytes_length,
    const uint32_t *feature_key_lengths, const uint8_t *feature_value_bytes,
    uint32_t feature_value_bytes_length, const uint32_t *feature_value_lengths,
    const uint64_t *feature_masks, uint32_t feature_count,
    const uint8_t *range_key_bytes, uint32_t range_key_bytes_length,
    const uint32_t *range_key_lengths, const uint64_t *range_values,
    const uint64_t *range_masks,
    uint32_t range_count, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "predicate arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        validate_query_field(*query, SK_QUERY_FIELD_PREDICATE, field);
        if (feature_count == 0 && range_count == 0) {
            throw std::invalid_argument("predicate query requires at least one feature");
        }
        // Totals are part of the trust boundary: reject impossible descriptors before
        // any packed-buffer pointer arithmetic. Per-item checks below use the
        // remaining advertised bytes, so offset addition cannot wrap.
        if (feature_count > MAX_TEXT_TERMS || range_count > MAX_TEXT_TERMS ||
            feature_key_bytes_length > MAX_TEXT_BYTES ||
            feature_value_bytes_length > MAX_TEXT_BYTES ||
            range_key_bytes_length > MAX_TEXT_BYTES ||
            (feature_count && (!feature_key_lengths || !feature_value_lengths || !feature_masks)) ||
            (range_count && (!range_key_lengths || !range_values || !range_masks)) ||
            (!feature_key_bytes && feature_key_bytes_length != 0) ||
            (!feature_value_bytes && feature_value_bytes_length != 0) ||
            (!range_key_bytes && range_key_bytes_length != 0)) {
            throw std::invalid_argument("predicate feature arrays are invalid");
        }
        auto term = std::make_unique<search::query::PredicateQueryTerm>();
        size_t key_offset = 0;
        size_t value_offset = 0;
        for (uint32_t index = 0; index < feature_count; ++index) {
            const auto key_length = feature_key_lengths[index];
            const auto value_length = feature_value_lengths[index];
            if (key_length == 0 || value_length == 0 || key_length > MAX_TEXT_TERM_BYTES ||
                value_length > MAX_TEXT_TERM_BYTES || key_offset > feature_key_bytes_length ||
                key_length > feature_key_bytes_length - key_offset ||
                value_offset > feature_value_bytes_length ||
                value_length > feature_value_bytes_length - value_offset) {
                throw std::invalid_argument("predicate feature key or value is invalid");
            }
            term->addFeature(
                std::string(reinterpret_cast<const char *>(feature_key_bytes + key_offset), key_length),
                std::string(reinterpret_cast<const char *>(feature_value_bytes + value_offset), value_length),
                feature_masks[index]);
            key_offset += key_length;
            value_offset += value_length;
        }
        if (key_offset != feature_key_bytes_length || value_offset != feature_value_bytes_length ||
            key_offset > MAX_TEXT_BYTES || value_offset > MAX_TEXT_BYTES) {
            throw std::invalid_argument("predicate feature lengths do not match packed bytes");
        }
        key_offset = 0;
        for (uint32_t index = 0; index < range_count; ++index) {
            const auto key_length = range_key_lengths[index];
            if (key_length == 0 || key_length > MAX_TEXT_TERM_BYTES ||
                key_offset > range_key_bytes_length ||
                key_length > range_key_bytes_length - key_offset) {
                throw std::invalid_argument("predicate range feature key is invalid");
            }
            term->addRangeFeature(
                std::string(reinterpret_cast<const char *>(range_key_bytes + key_offset), key_length),
                range_values[index], range_masks[index]);
            key_offset += key_length;
        }
        if (key_offset != range_key_bytes_length || key_offset > MAX_TEXT_BYTES) {
            throw std::invalid_argument("predicate range key lengths do not match packed bytes");
        }
        query->builder.addPredicateQuery(std::move(term), field,
                                         query->allocate_term_id(), Weight(100));
    });
}

extern "C" int32_t sk_query_add_attribute_string_equals(
    sk_query *query, const char *field, const char *value,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field || !value) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "string equality arguments are invalid");
    return invoke(error, error_capacity, [&] {
        validate_query_field(*query, SK_QUERY_FIELD_STRING, field);
        query->builder.addStringTerm(value, field, query->allocate_term_id(), Weight(1));
    });
}

extern "C" int32_t sk_query_add_attribute_float_equals(
    sk_query *query, const char *field, double value,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "float equality arguments are invalid");
    return invoke(error, error_capacity, [&] {
        if (!std::isfinite(value)) throw std::invalid_argument("attribute float value must be finite");
        validate_query_field(*query, SK_QUERY_FIELD_FLOAT, field);
        std::ostringstream text;
        text.imbue(std::locale::classic());
        text << std::setprecision(std::numeric_limits<double>::max_digits10) << value;
        query->builder.addNumberTerm(text.str(), field, query->allocate_term_id(), Weight(1));
    });
}

extern "C" int32_t sk_query_add_attribute_boolean_equals(
    sk_query *query, const char *field, uint8_t value,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "boolean equality arguments are invalid");
    return invoke(error, error_capacity, [&] {
        validate_query_field(*query, SK_QUERY_FIELD_BOOLEAN, field);
        query->builder.addNumberTerm(value ? "1" : "0", field, query->allocate_term_id(), Weight(1));
    });
}

extern "C" int32_t sk_query_add_attribute_exists(
    sk_query *query, const char *field, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "attribute exists arguments are invalid");
    return invoke(error, error_capacity, [&] {
        validate_query_field(*query, SK_QUERY_FIELD_ATTRIBUTE, field);
        const auto metadata = std::find_if(
            query->fields.begin(), query->fields.end(),
            [field](const auto &candidate) { return candidate.name == field; });
        if (metadata == query->fields.end()) {
            throw std::invalid_argument("unknown generated schema field: " + std::string(field));
        }
        if (metadata->type == "string") {
            auto spec = std::make_unique<search::StringRangeSpec>();
            // Vespa's single-string attribute uses the empty string as its
            // undefined value. A strict lower bound excludes that sentinel
            // while retaining every actual non-empty keyword.
            spec->left = "";
            spec->left_closed = false;
            spec->right_unbounded = true;
            query->builder.add_string_range_term(
                StringRange(std::move(spec)), field, query->allocate_term_id(), Weight(1));
        } else if (metadata->type == "boolean") {
            // Vespa BOOL attributes have no undefined sentinel and expose
            // false for documents that do not contain the source field.
            query->builder.addOr(2);
            query->builder.addNumberTerm("0", field, query->allocate_term_id(), Weight(1));
            query->builder.addNumberTerm("1", field, query->allocate_term_id(), Weight(1));
        } else if (metadata->type == "integer" || metadata->type == "float") {
            query->builder.addRangeTerm(search::query::Range("[;]"), field,
                                         query->allocate_term_id(), Weight(1));
        } else {
            throw UnsupportedNativeError(
                "attribute_exists_type_not_supported",
                "unsupported attribute exists field type: " + metadata->type);
        }
    });
}

extern "C" int32_t sk_query_add_attribute_integer_range(
    sk_query *query, const char *field, uint32_t lower_kind,
    int64_t lower_value, uint32_t upper_kind, int64_t upper_value,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "integer range arguments are invalid");
    return invoke(error, error_capacity, [&] {
        validate_query_field(*query, SK_QUERY_FIELD_INTEGER, field);
        if ((lower_kind != SK_QUERY_BOUND_UNBOUNDED &&
             lower_value == std::numeric_limits<int64_t>::min()) ||
            (upper_kind != SK_QUERY_BOUND_UNBOUNDED &&
             upper_value == std::numeric_limits<int64_t>::min())) {
            throw std::invalid_argument(
                "integer value INT64_MIN is reserved as Vespa's undefined attribute sentinel: " +
                std::string(field));
        }
        query->builder.addRangeTerm(search::query::Range(make_query_range(lower_kind, lower_value, upper_kind, upper_value)), field, query->allocate_term_id(), Weight(1));
    });
}

extern "C" int32_t sk_query_add_attribute_float_range(
    sk_query *query, const char *field, uint32_t lower_kind,
    double lower_value, uint32_t upper_kind, double upper_value,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "float range arguments are invalid");
    return invoke(error, error_capacity, [&] {
        if ((lower_kind != SK_QUERY_BOUND_UNBOUNDED && !std::isfinite(lower_value)) ||
            (upper_kind != SK_QUERY_BOUND_UNBOUNDED && !std::isfinite(upper_value))) {
            throw std::invalid_argument("attribute float bound must be finite");
        }
        validate_query_field(*query, SK_QUERY_FIELD_FLOAT, field);
        const auto delimiter = [](uint32_t kind, bool lower) {
            if (kind == SK_QUERY_BOUND_UNBOUNDED) return lower ? '[' : ']';
            if (kind == SK_QUERY_BOUND_INCLUDED) return lower ? '[' : ']';
            if (kind == SK_QUERY_BOUND_EXCLUDED) return lower ? '<' : '>';
            throw std::invalid_argument("unsupported attribute range bound kind");
        };
        std::ostringstream text;
        text.imbue(std::locale::classic());
        text << delimiter(lower_kind, true);
        if (lower_kind != SK_QUERY_BOUND_UNBOUNDED) text << std::setprecision(std::numeric_limits<double>::max_digits10) << lower_value;
        text << ';';
        if (upper_kind != SK_QUERY_BOUND_UNBOUNDED) text << std::setprecision(std::numeric_limits<double>::max_digits10) << upper_value;
        text << delimiter(upper_kind, false);
        query->builder.addRangeTerm(search::query::Range(text.str()), field, query->allocate_term_id(), Weight(1));
    });
}

extern "C" void sk_query_destroy(sk_query *query) SK_NOEXCEPT {
    destroy_noexcept(query);
}

extern "C" int32_t sk_engine_search_query(const sk_engine *engine,
                                           const sk_query *query,
                                           const char *rank_profile,
                                           const char *source,
                                           sk_text_result **out_result,
                                           uint8_t *error,
                                           uint32_t error_capacity) SK_NOEXCEPT {
    if (out_result) {
        *out_result = nullptr;
    }
    if (query && !query->consume()) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "query handle has already been consumed");
    }
    if (!engine || !engine->impl || !query || !rank_profile || !source || !out_result) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "query search arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        auto result = std::make_unique<sk_text_result>();
        result->hits = engine->impl->search_query(query->builder, rank_profile, source);
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        engine->impl->failpoint("result_after_allocation");
        result->failpoints = engine->impl->failpoints();
#endif
        *out_result = result.release();
    });
}

#ifdef SK_ENABLE_HARDENING_FAILPOINTS
extern "C" SK_API int32_t sk_test_failpoint_set(sk_engine *engine, const char *name,
                                                   uint32_t hit, uint32_t action,
                                                   uint8_t *error,
                                                   uint32_t error_capacity) SK_NOEXCEPT {
    if (!engine || !engine->impl || !name) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "failpoint set arguments are invalid");
    }
    return invoke(error, error_capacity,
                  [&] { engine->impl->set_failpoint(name, hit, action); });
}

extern "C" SK_API int32_t sk_test_failpoint_set_open(const char *name, uint32_t hit,
                                                        uint32_t action, uint8_t *error,
                                                        uint32_t error_capacity) SK_NOEXCEPT {
    if (!name) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "open failpoint arguments are invalid");
    }
    return invoke(error, error_capacity,
                  [&] { OPEN_FAILPOINTS.set(name, hit, action); });
}

extern "C" SK_API int32_t sk_test_failpoint_clear(sk_engine *engine, const char *name,
                                                     uint8_t *error,
                                                     uint32_t error_capacity) SK_NOEXCEPT {
    if (!engine || !engine->impl || !name) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "failpoint clear arguments are invalid");
    }
    return invoke(error, error_capacity,
                  [&] { engine->impl->clear_failpoint(name); });
}

extern "C" SK_API int32_t sk_test_failpoint_clear_open(const char *name, uint8_t *error,
                                                          uint32_t error_capacity) SK_NOEXCEPT {
    if (!name) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "open failpoint arguments are invalid");
    }
    return invoke(error, error_capacity, [&] { OPEN_FAILPOINTS.clear(name); });
}

extern "C" SK_API int32_t sk_test_failpoint_release(sk_engine *engine, const char *name,
                                                       uint8_t *error,
                                                       uint32_t error_capacity) SK_NOEXCEPT {
    if (!engine || !engine->impl || !name) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "failpoint release arguments are invalid");
    }
    return invoke(error, error_capacity,
                  [&] { engine->impl->release_failpoint(name); });
}

extern "C" SK_API int32_t sk_test_failpoint_is_blocked(const sk_engine *engine,
                                                          const char *name,
                                                          uint8_t *out_blocked,
                                                          uint8_t *error,
                                                          uint32_t error_capacity) SK_NOEXCEPT {
    if (out_blocked) {
        *out_blocked = 0;
    }
    if (!engine || !engine->impl || !name || !out_blocked) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "failpoint status arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        *out_blocked = engine->impl->failpoint_is_blocked(name) ? 1 : 0;
    });
}

extern "C" SK_API int32_t sk_test_result_fault_set(sk_engine *engine, uint32_t fault,
                                                      uint8_t *error,
                                                      uint32_t error_capacity) SK_NOEXCEPT {
    if (!engine || !engine->impl) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "result fault set arguments are invalid");
    }
    return invoke(error, error_capacity,
                  [&] { engine->impl->set_result_fault(fault); });
}

extern "C" SK_API int32_t sk_test_result_fault_clear(sk_engine *engine, uint8_t *error,
                                                        uint32_t error_capacity) SK_NOEXCEPT {
    if (!engine || !engine->impl) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "result fault clear arguments are invalid");
    }
    return invoke(error, error_capacity,
                  [&] { engine->impl->clear_result_fault(); });
}

extern "C" SK_API int32_t sk_test_search_reply_metadata(
    uint32_t degradation_flags, const char *const *issue_messages,
    uint32_t issue_count, sk_generic_search_result **out_result,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_result) {
        *out_result = nullptr;
    }
    if (!out_result || (issue_count != 0 && !issue_messages) ||
        (degradation_flags &
         ~(SK_DEGRADATION_MATCH_PHASE | SK_DEGRADATION_TIMEOUT |
           SK_DEGRADATION_ANN_TIMEOUT))) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "SearchReply metadata test arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        search::engine::SearchReply reply;
        reply.coverage.setActive(17).setCovered(11).setTargetActive(19);
        if (degradation_flags & SK_DEGRADATION_MATCH_PHASE) {
            reply.coverage.degradeMatchPhase();
        }
        if (degradation_flags & SK_DEGRADATION_TIMEOUT) {
            reply.coverage.degradeTimeout();
        }
        if (degradation_flags & SK_DEGRADATION_ANN_TIMEOUT) {
            reply.coverage.degrade_ann_timeout();
        }
        if (issue_count != 0) {
            reply.my_issues = std::make_unique<search::UniqueIssues>();
            for (uint32_t index = 0; index < issue_count; ++index) {
                if (!issue_messages[index]) {
                    throw std::invalid_argument("SearchReply issue message is null");
                }
                reply.my_issues->handle(vespalib::Issue(issue_messages[index]));
            }
        }
        auto result = std::make_unique<sk_local_search_result>();
        copy_search_reply_metadata(reply, *result);
        *out_result = result.release();
    });
}

extern "C" SK_API int32_t sk_test_result_destroy_count(const sk_engine *engine,
                                                          uint64_t *out_count,
                                                          uint8_t *error,
                                                          uint32_t error_capacity) SK_NOEXCEPT {
    if (out_count) {
        *out_count = 0;
    }
    if (!engine || !engine->impl || !out_count) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "result destroy count arguments are invalid");
    }
    return invoke(error, error_capacity,
                  [&] { *out_count = engine->impl->result_destroyed(); });
}
#endif
