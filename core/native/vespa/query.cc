// Search Kernel native adapter: query construction helpers.
// This file is included by abi.cc so the private adapter remains one native
// boundary while responsibilities stay physically searchable by subsystem.

std::string query_stack_dump(int64_t value, const std::string &field) {
    VespaQueryBuilder builder;
    builder.addNumberTerm(std::to_string(value), field, 1, Weight(1));
    auto tree = builder.build();
    if (!tree) {
        throw std::runtime_error("could not build number query");
    }
    return StackDumpCreator::create(*tree);
}


std::string copy_text_bytes(const uint8_t *bytes, uint32_t length,
                            size_t maximum, const char *what);

void validate_query_field(const NativeQueryState &query, uint32_t kind,
                          const char *field) {
    query.ensure_active();
    if (!field) {
        throw std::invalid_argument("query field must not be null");
    }
    validate_field_name(field);
    if (*field == '\0' && !(query.same_element_parent && kind == SK_QUERY_FIELD_TEXT)) {
        throw std::invalid_argument("query field must not be empty");
    }
    std::string qualified_field;
    auto metadata = std::find_if(
        query.fields.begin(), query.fields.end(),
        [field](const auto &candidate) { return candidate.name == field; });
    if (metadata == query.fields.end() && query.same_element_parent && kind == SK_QUERY_FIELD_TEXT) {
        qualified_field = *query.same_element_parent;
        if (*field != '\0') {
            qualified_field += ".";
            qualified_field += field;
        }
        metadata = std::find_if(
            query.fields.begin(), query.fields.end(),
            [&qualified_field](const auto &candidate) { return candidate.name == qualified_field; });
    }
    if (metadata == query.fields.end()) {
        throw std::invalid_argument("unknown generated schema field: " + std::string(field));
    }
    switch (kind) {
    case SK_QUERY_FIELD_TEXT:
        if ((metadata->type != "text" && metadata->type != "array<string>") || !metadata->index) {
            throw std::invalid_argument(
                "text operation requires an indexed text field: " + std::string(field));
        }
        break;
    case SK_QUERY_FIELD_INTEGER:
        if (!metadata->attribute) {
            throw std::invalid_argument(
                "integer operation requires an integer attribute field: " + std::string(field));
        }
        if (metadata->type != "integer") {
            throw std::invalid_argument("wrong value type for query field: " + std::string(field));
        }
        break;
    case SK_QUERY_FIELD_STRING:
        if (!metadata->attribute) {
            throw std::invalid_argument("string operation requires a string attribute field: " + std::string(field));
        }
        if (metadata->type != "string") {
            throw std::invalid_argument("wrong value type for query field: " + std::string(field));
        }
        break;
    case SK_QUERY_FIELD_FLOAT:
        if (!metadata->attribute) {
            throw std::invalid_argument("float operation requires a float attribute field: " + std::string(field));
        }
        if (metadata->type != "float") {
            throw std::invalid_argument("wrong value type for query field: " + std::string(field));
        }
        break;
    case SK_QUERY_FIELD_BOOLEAN:
        if (!metadata->attribute) {
            throw std::invalid_argument("boolean operation requires a boolean attribute field: " + std::string(field));
        }
        if (metadata->type != "boolean") {
            throw std::invalid_argument("wrong value type for query field: " + std::string(field));
        }
        break;
    case SK_QUERY_FIELD_ATTRIBUTE:
        if (!metadata->attribute || metadata->type == "text") {
            throw std::invalid_argument("attribute operation requires an attribute field: " + std::string(field));
        }
        break;
    case SK_QUERY_FIELD_TENSOR:
        if (!metadata->attribute || metadata->type != "tensor") {
            throw std::invalid_argument(
                "nearest-neighbor operation requires a tensor attribute field: " + std::string(field));
        }
        break;
    case SK_QUERY_FIELD_ARRAY:
        if (metadata->type != "array<string>" && metadata->type != "array<struct>") {
            throw std::invalid_argument(
                "same-element operation requires an array field: " + std::string(field));
        }
        if (metadata->type == "array<string>" && !metadata->index) {
            throw std::invalid_argument(
                "same-element operation requires an indexed array field: " + std::string(field));
        }
        break;
    case SK_QUERY_FIELD_POSITION:
        if (!metadata->attribute || metadata->type != "position") {
            throw std::invalid_argument(
                "geo operation requires a position attribute field: " + std::string(field));
        }
        break;
    case SK_QUERY_FIELD_PREDICATE:
        if (!metadata->attribute || metadata->type != "predicate") {
            throw std::invalid_argument(
                "predicate operation requires a predicate attribute field: " + std::string(field));
        }
        break;
    default:
        throw std::invalid_argument("unsupported query field kind");
    }
}

std::string query_index_field(const NativeQueryState &query, const char *field) {
    if (query.same_element_parent) {
        std::string qualified = *query.same_element_parent;
        if (*field != '\0') {
            qualified += ".";
            qualified += field;
        }
        const auto relative_metadata = std::find_if(
            query.fields.begin(), query.fields.end(),
            [&qualified](const auto &candidate) { return candidate.name == qualified; });
        if (relative_metadata != query.fields.end()) return field;
    }
    const auto metadata = std::find_if(
        query.fields.begin(), query.fields.end(),
        [field](const auto &candidate) { return candidate.name == field; });
    if (metadata == query.fields.end()) return field;
    if (!metadata->virtual_field) return field;
    std::string result;
    for (const auto character : metadata->name) {
        if (character == '.') result += "__";
        else result += character;
    }
    return result;
}

void add_query_operator(NativeQueryState &query, uint32_t kind, uint32_t child_count) {
    query.ensure_active();
    if (child_count > static_cast<uint32_t>(std::numeric_limits<int>::max())) {
        throw std::invalid_argument("query child count cannot be represented by Vespa");
    }
    const auto count = static_cast<int>(child_count);
    switch (kind) {
    case SK_QUERY_NODE_AND:
        query.builder.addAnd(count);
        break;
    case SK_QUERY_NODE_OR:
        query.builder.addOr(count);
        break;
    case SK_QUERY_NODE_NOT:
        if (child_count != 1) {
            throw std::invalid_argument("NOT query must contain exactly one child");
        }
        query.builder.addAndNot(2);
        query.builder.add_true_node();
        break;
    default:
        throw std::invalid_argument("unsupported query operator");
    }
}

void add_query_text(NativeQueryState &query, uint32_t kind, const char *field,
                    const uint8_t *term_bytes, uint32_t term_bytes_length,
                    const uint32_t *term_lengths, uint32_t term_count) {
    if (!field) {
        throw std::invalid_argument("query field must not be null");
    }
    validate_field_name(field);
    if (*field == '\0' && !query.same_element_parent) {
        throw std::invalid_argument("query field must not be empty");
    }
    validate_query_field(query, SK_QUERY_FIELD_TEXT, field);
    if (kind != SK_QUERY_TEXT_TERM && kind != SK_QUERY_TEXT_PHRASE) {
        throw std::invalid_argument("unsupported query text kind");
    }
    if ((kind == SK_QUERY_TEXT_TERM && term_count != 1) ||
        (kind == SK_QUERY_TEXT_PHRASE && term_count == 0)) {
        throw std::invalid_argument("invalid number of terms for query text kind");
    }
    if (term_count > MAX_TEXT_TERMS) {
        throw std::invalid_argument("query text term count is too large");
    }
    if (term_count > 0 && !term_lengths) {
        throw std::invalid_argument("query text term lengths must not be null");
    }
    if (term_count > 0 && !term_bytes) {
        throw std::invalid_argument("query text term bytes must not be null");
    }
    size_t total_bytes = 0;
    for (uint32_t index = 0; index < term_count; ++index) {
        const auto length = term_lengths[index];
        if (length == 0 || length > MAX_TEXT_TERM_BYTES ||
            length > MAX_TOTAL_TEXT_TERM_BYTES - total_bytes) {
            throw std::invalid_argument("query text term bytes are too large or empty");
        }
        total_bytes += length;
    }
    if (total_bytes != term_bytes_length) {
        throw std::invalid_argument("query text term lengths do not match term bytes");
    }
    if (term_bytes_length > 0 && !term_bytes) {
        throw std::invalid_argument("query text term bytes must not be null");
    }
    std::vector<std::string> terms;
    terms.reserve(term_count);
    size_t term_offset = 0;
    for (uint32_t index = 0; index < term_count; ++index) {
        const auto length = term_lengths[index];
        terms.push_back(copy_text_bytes(term_bytes + term_offset, length,
                                        MAX_TEXT_TERM_BYTES, "query text term"));
        term_offset += length;
    }
    const auto index_field = query_index_field(query, field);
    if (kind == SK_QUERY_TEXT_TERM) {
        query.builder.addStringTerm(terms[0], index_field, query.allocate_term_id(), Weight(1));
    } else {
        query.builder.addPhrase(static_cast<int>(terms.size()), index_field,
                                query.allocate_term_id(), Weight(1));
        for (const auto &term : terms) {
            query.builder.addStringTerm(term, index_field, query.allocate_term_id(), Weight(1));
        }
    }
}

void validate_query_multi_field(const NativeQueryState &query, const char *field) {
    query.ensure_active();
    if (!field) {
        throw std::invalid_argument("query field must not be null");
    }
    validate_field_name(field);
    if (*field == '\0') {
        throw std::invalid_argument("query field must not be empty");
    }
    const auto metadata = std::find_if(
        query.fields.begin(), query.fields.end(),
        [field](const auto &candidate) { return candidate.name == field; });
    if (metadata == query.fields.end()) {
        throw std::invalid_argument("unknown generated schema field: " + std::string(field));
    }
    if ((!metadata->index && !metadata->attribute) ||
        (metadata->type == "text" && !metadata->index)) {
        throw std::invalid_argument(
            "weighted query requires an indexed or attribute field: " + std::string(field));
    }
}

void validate_fuzzy_query_field(const NativeQueryState &query, const char *field) {
    query.ensure_active();
    if (!field) {
        throw std::invalid_argument("query field must not be null");
    }
    validate_field_name(field);
    if (*field == '\0') {
        throw std::invalid_argument("query field must not be empty");
    }
    auto metadata = query.fields.end();
    if (query.same_element_parent) {
        std::string qualified = *query.same_element_parent;
        qualified += ".";
        qualified += field;
        metadata = std::find_if(
            query.fields.begin(), query.fields.end(),
            [&qualified](const auto &candidate) { return candidate.name == qualified; });
    }
    if (metadata == query.fields.end()) {
        metadata = std::find_if(
            query.fields.begin(), query.fields.end(),
            [field](const auto &candidate) { return candidate.name == field; });
    }
    if (metadata == query.fields.end()) {
        throw std::invalid_argument("unknown generated schema field: " + std::string(field));
    }
    if ((metadata->type == "text" && metadata->index) ||
        (metadata->type == "string" && metadata->attribute)) {
        return;
    }
    throw std::invalid_argument(
        "fuzzy query requires an indexed text or string attribute field: " + std::string(field));
}

search::query::Weight query_item_weight(int64_t value) {
    if (value < std::numeric_limits<int32_t>::min() ||
        value > std::numeric_limits<int32_t>::max()) {
        throw std::invalid_argument("weighted query item weight is outside Vespa's int32 range");
    }
    return Weight(static_cast<int32_t>(value));
}

void validate_weighted_query_inputs(const uint8_t *key_bytes, uint32_t key_bytes_length,
                                    const uint32_t *key_lengths, const int64_t *weights,
                                    uint32_t item_count) {
    if (item_count == 0) {
        throw std::invalid_argument("weighted query requires at least one item");
    }
    if (!key_lengths || !weights) {
        throw std::invalid_argument("weighted query arrays must not be null");
    }
    if (!key_bytes && key_bytes_length != 0) {
        throw std::invalid_argument("weighted query key bytes must not be null");
    }
    if (item_count > MAX_TEXT_TERMS) {
        throw std::invalid_argument("weighted query item count is too large");
    }
    size_t total_bytes = 0;
    for (uint32_t index = 0; index < item_count; ++index) {
        const auto length = key_lengths[index];
        if (length == 0 || length > MAX_TEXT_TERM_BYTES ||
            length > MAX_TOTAL_TEXT_TERM_BYTES - total_bytes) {
            throw std::invalid_argument("weighted query key is too large or empty");
        }
        total_bytes += length;
        (void)query_item_weight(weights[index]);
    }
    if (total_bytes != key_bytes_length) {
        throw std::invalid_argument("weighted query key lengths do not match key bytes");
    }
}

void add_weighted_query_strings(NativeQueryState &query, uint32_t kind, const char *field,
                                const uint8_t *key_bytes, uint32_t key_bytes_length,
                                const uint32_t *key_lengths, const int64_t *weights,
                                uint32_t item_count, uint32_t wand_target_num_hits = 100,
                                int64_t wand_score_threshold = 0,
                                double wand_threshold_boost_factor = 1.0) {
    validate_query_multi_field(query, field);
    validate_weighted_query_inputs(key_bytes, key_bytes_length, key_lengths, weights, item_count);
    auto terms = std::make_unique<search::query::WeightedStringTermVector>(item_count);
    size_t offset = 0;
    for (uint32_t index = 0; index < item_count; ++index) {
        const auto length = key_lengths[index];
        terms->addTerm(std::string_view(reinterpret_cast<const char *>(key_bytes + offset), length),
                       query_item_weight(weights[index]));
        offset += length;
    }
    const auto type = search::query::MultiTerm::Type::WEIGHTED_STRING;
    if (kind == 1) {
        query.builder.addWandTerm(std::move(terms), type, field, query.allocate_term_id(), Weight(100),
                                  wand_target_num_hits, wand_score_threshold,
                                  wand_threshold_boost_factor);
    } else if (kind == 2) {
        query.builder.addDotProduct(std::move(terms), type, field, query.allocate_term_id(), Weight(100));
    } else {
        query.builder.addWeightedSetTerm(std::move(terms), type, field, query.allocate_term_id(), Weight(100));
    }
}

void add_weighted_query_integers(NativeQueryState &query, uint32_t kind, const char *field,
                                 const int64_t *keys, const int64_t *weights,
                                 uint32_t item_count, uint32_t wand_target_num_hits = 100,
                                 int64_t wand_score_threshold = 0,
                                 double wand_threshold_boost_factor = 1.0) {
    validate_query_multi_field(query, field);
    if (item_count == 0) {
        throw std::invalid_argument("weighted query requires at least one item");
    }
    if (!keys || !weights) {
        throw std::invalid_argument("weighted query arrays must not be null");
    }
    if (item_count > MAX_TEXT_TERMS) {
        throw std::invalid_argument("weighted query item count is too large");
    }
    auto terms = std::make_unique<search::query::WeightedIntegerTermVector>(item_count);
    for (uint32_t index = 0; index < item_count; ++index) {
        terms->addTerm(keys[index], query_item_weight(weights[index]));
    }
    const auto type = search::query::MultiTerm::Type::WEIGHTED_INTEGER;
    if (kind == 1) {
        query.builder.addWandTerm(std::move(terms), type, field, query.allocate_term_id(), Weight(100),
                                  wand_target_num_hits, wand_score_threshold,
                                  wand_threshold_boost_factor);
    } else if (kind == 2) {
        query.builder.addDotProduct(std::move(terms), type, field, query.allocate_term_id(), Weight(100));
    } else {
        query.builder.addWeightedSetTerm(std::move(terms), type, field, query.allocate_term_id(), Weight(100));
    }
}

std::unique_ptr<search::NumericRangeSpec> make_query_range(
    uint32_t lower_kind, int64_t lower_value,
    uint32_t upper_kind, int64_t upper_value) {
    auto spec = std::make_unique<search::NumericRangeSpec>();
    spec->valid = true;
    spec->valid_integers = true;
    const auto set_bound = [](uint32_t kind, int64_t value, bool lower,
                              search::NumericRangeSpec &range) {
        auto &integer_limit = lower ? range.int64_lower_limit : range.int64_upper_limit;
        auto &fp_limit = lower ? range.fp_lower_limit : range.fp_upper_limit;
        auto &inclusive = lower ? range.lower_inclusive : range.upper_inclusive;
        if (kind == SK_QUERY_BOUND_UNBOUNDED) {
            integer_limit = lower ? std::numeric_limits<int64_t>::min()
                                   : std::numeric_limits<int64_t>::max();
            fp_limit = lower ? -std::numeric_limits<double>::infinity()
                              : std::numeric_limits<double>::infinity();
            inclusive = true;
        } else if (kind == SK_QUERY_BOUND_INCLUDED || kind == SK_QUERY_BOUND_EXCLUDED) {
            integer_limit = value;
            fp_limit = value;
            inclusive = (kind == SK_QUERY_BOUND_INCLUDED);
        } else {
            throw std::invalid_argument("unsupported query range bound");
        }
    };
    set_bound(lower_kind, lower_value, true, *spec);
    set_bound(upper_kind, upper_value, false, *spec);
    return spec;
}
