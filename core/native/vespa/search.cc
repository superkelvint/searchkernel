// Search Kernel native adapter: search result and feature helpers.
// This file is included by abi.cc so the private adapter remains one native
// boundary while responsibilities stay physically searchable by subsystem.

std::chrono::milliseconds remaining_search_timeout(
    std::chrono::milliseconds budget,
    std::chrono::steady_clock::duration elapsed) noexcept {
    if (elapsed >= budget) {
        return 0ms;
    }
    return std::chrono::duration_cast<std::chrono::milliseconds>(budget - elapsed);
}

void copy_search_reply_metadata(const SearchReply &reply, LocalSearchResultState &result) {
    result.total_hit_count = reply.totalHitCount;
    result.coverage_active = reply.coverage.getActive();
    result.coverage_covered = reply.coverage.getCovered();
    result.coverage_target_active = reply.coverage.getTargetActive();
    if (reply.coverage.wasDegradedByMatchPhase()) {
        result.degradation_flags |= SK_DEGRADATION_MATCH_PHASE;
    }
    if (reply.coverage.wasDegradedByTimeout()) {
        result.degradation_flags |= SK_DEGRADATION_TIMEOUT;
    }
    if (reply.coverage.was_degraded_by_ann_timeout()) {
        result.degradation_flags |= SK_DEGRADATION_ANN_TIMEOUT;
    }
    if (reply.my_issues) {
        reply.my_issues->for_each_message([&result](const std::string &message) {
            result.issues.push_back(message);
        });
    }
}
void add_query_feature(LocalSearchRequestState &request, const char *name, double value) {
    validate_string_size(name, MAX_SCHEMA_IDENTIFIER_BYTES, "query feature name");
    if (*name == '\0') throw std::invalid_argument("query feature name must not be empty");
    if (std::string_view(name) == RESERVED_RANK_PROPERTY) {
        throw std::invalid_argument("query feature is reserved by Search Kernel");
    }
    if (!std::isfinite(value)) throw std::invalid_argument("query feature value must be finite");
    if (std::find_if(request.query_features.begin(), request.query_features.end(),
                     [name](const auto &entry) { return entry.first == name; }) !=
        request.query_features.end()) {
        throw std::invalid_argument("duplicate query feature");
    }
    if (std::find_if(request.rank_properties.begin(), request.rank_properties.end(),
                     [name](const auto &entry) { return entry.first == name; }) !=
        request.rank_properties.end()) {
        throw std::invalid_argument("rank property collision");
    }
    if (std::find_if(request.query_tensors.begin(), request.query_tensors.end(),
                     [name](const auto &entry) { return entry.first == name; }) !=
        request.query_tensors.end()) {
        throw std::invalid_argument("query tensor collision");
    }
    request.query_features.emplace_back(name, value);
}

void add_rank_property(LocalSearchRequestState &request, const char *name, const char *value) {
    validate_string_size(name, MAX_SCHEMA_IDENTIFIER_BYTES, "rank property name");
    validate_string_size(value, MAX_RANK_PROFILE_BYTES, "rank property value");
    if (*name == '\0') throw std::invalid_argument("rank property name must not be empty");
    if (*value == '\0') throw std::invalid_argument("rank property value must not be empty");
    if (std::string_view(name) == RESERVED_RANK_PROPERTY) {
        throw std::invalid_argument("rank property is reserved by Search Kernel");
    }
    if (std::find_if(request.rank_properties.begin(), request.rank_properties.end(),
                     [name](const auto &entry) { return entry.first == name; }) !=
        request.rank_properties.end()) {
        throw std::invalid_argument("duplicate rank property");
    }
    if (std::find_if(request.query_features.begin(), request.query_features.end(),
                     [name](const auto &entry) { return entry.first == name; }) !=
        request.query_features.end()) {
        throw std::invalid_argument("rank property collision");
    }
    if (std::find_if(request.query_tensors.begin(), request.query_tensors.end(),
                     [name](const auto &entry) { return entry.first == name; }) !=
        request.query_tensors.end()) {
        throw std::invalid_argument("query tensor collision");
    }
    request.rank_properties.emplace_back(name, value);
}

uint32_t match_feature_count(const LocalSearchResultState &result) {
    return static_cast<uint32_t>(result.match_feature_names.size());
}

void match_feature_name(const LocalSearchResultState &result, uint32_t feature_index,
                        const char **out_name, uint32_t *out_length) {
    if (!out_name || !out_length || feature_index >= result.match_feature_names.size()) {
        throw std::invalid_argument("match feature name arguments are invalid");
    }
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    static const uint8_t invalid_utf8[] = {0xff};
    if (result.result_faults &&
        result.result_faults->take(SK_TEST_RESULT_FAULT_NULL_MATCH_FEATURE_NAME)) {
        *out_name = nullptr;
        *out_length = 1;
        return;
    }
    if (result.result_faults &&
        result.result_faults->take(SK_TEST_RESULT_FAULT_INVALID_MATCH_FEATURE_NAME)) {
        *out_name = reinterpret_cast<const char *>(invalid_utf8);
        *out_length = sizeof(invalid_utf8);
        return;
    }
#endif
    const auto &name = result.match_feature_names[feature_index];
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    if (feature_index > 0 && result.result_faults &&
        result.result_faults->take(SK_TEST_RESULT_FAULT_DUPLICATE_MATCH_FEATURE_NAME)) {
        *out_name = result.match_feature_names[0].data();
        *out_length = static_cast<uint32_t>(result.match_feature_names[0].size());
        return;
    }
#endif
    *out_name = name.data();
    *out_length = static_cast<uint32_t>(name.size());
}

const sk_feature_value *match_feature_value_ref(const LocalSearchResultState &result,
                                                uint32_t hit_index,
                                                uint32_t feature_index) {
    if (hit_index >= result.hits.size() || feature_index >= result.match_feature_names.size()) {
        throw std::invalid_argument("match feature value arguments are invalid");
    }
    const size_t index = static_cast<size_t>(hit_index) * result.match_feature_names.size() +
                         feature_index;
    if (index >= result.match_feature_values.size()) {
        throw std::runtime_error("native match feature matrix is incomplete");
    }
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    if (result.result_faults &&
        result.result_faults->take(SK_TEST_RESULT_FAULT_MATCH_FEATURE_VALUE_ERROR)) {
        throw std::runtime_error("injected match feature accessor failure");
    }
#endif
    return &result.match_feature_values[index];
}

void match_feature_value_double(const LocalSearchResultState &result, uint32_t hit_index,
                                uint32_t feature_index, double *out_value) {
    if (!out_value) {
        throw std::invalid_argument("match feature value arguments are invalid");
    }
    const auto *value = match_feature_value_ref(result, hit_index, feature_index);
    if (!value->value.is_double()) {
        throw TypeErrorNativeError("match feature value is data, not double");
    }
    *out_value = value->value.as_double();
}
