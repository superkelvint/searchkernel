// Search Kernel native adapter: document and text conversion helpers.
// This file is included by abi.cc so the private adapter remains one native
// boundary while responsibilities stay physically searchable by subsystem.

template <typename Add>
int32_t add_document_value(sk_document *document, const char *field,
                           uint8_t *error, uint32_t error_capacity, Add &&add) noexcept {
    if (!document || !field) return set_error(error, error_capacity,
                                              SK_NATIVE_STATUS_INVALID_ARGUMENT,
                                              "document value arguments are invalid");
    return invoke(error, error_capacity, [&] {
        validate_field_name(field);
        if (*field == '\0') throw std::invalid_argument("document field must not be empty");
        OwnedValue value = add();
        auto existing = std::find_if(document->fields.begin(), document->fields.end(),
                                     [field](const auto &entry) { return entry.name == field; });
        if (existing != document->fields.end()) {
            existing->value = std::move(value);
        } else {
            document->fields.push_back({field, std::move(value), {}, {}});
        }
    });
}

std::vector<std::string> copy_string_array(const uint8_t *bytes,
                                           const uint32_t *lengths,
                                           uint32_t count,
                                           const char *what) {
    if (count != 0 && (!bytes || !lengths)) {
        throw std::invalid_argument(std::string(what) + " arrays are invalid");
    }
    std::vector<std::string> result;
    result.reserve(count);
    size_t offset = 0;
    for (uint32_t index = 0; index < count; ++index) {
        const size_t length = lengths[index];
        if (offset > std::numeric_limits<size_t>::max() - length) {
            throw std::invalid_argument(std::string(what) + " data is too large");
        }
        result.emplace_back(reinterpret_cast<const char *>(bytes + offset), length);
        offset += length;
    }
    return result;
}

std::vector<std::pair<std::string, std::string>> copy_predicate_features(
    const uint8_t *key_bytes, const uint32_t *key_lengths,
    const uint8_t *value_bytes, const uint32_t *value_lengths,
    uint32_t count) {
    if (count == 0 || !key_bytes || !key_lengths || !value_bytes || !value_lengths) {
        throw std::invalid_argument("predicate feature arrays are invalid");
    }
    std::vector<std::pair<std::string, std::string>> result;
    result.reserve(count);
    size_t key_offset = 0;
    size_t value_offset = 0;
    for (uint32_t index = 0; index < count; ++index) {
        const size_t key_length = key_lengths[index];
        const size_t value_length = value_lengths[index];
        if (key_offset > std::numeric_limits<size_t>::max() - key_length ||
            value_offset > std::numeric_limits<size_t>::max() - value_length) {
            throw std::invalid_argument("predicate feature data is too large");
        }
        result.emplace_back(
            std::string(reinterpret_cast<const char *>(key_bytes + key_offset), key_length),
            std::string(reinterpret_cast<const char *>(value_bytes + value_offset), value_length));
        key_offset += key_length;
        value_offset += value_length;
    }
    return result;
}



std::string copy_text_bytes(const uint8_t *bytes, uint32_t length,
                            size_t maximum, const char *what) {
    if (length > maximum) {
        throw std::invalid_argument(std::string(what) + " is too long");
    }
    if (length == 0) {
        return {};
    }
    if (bytes == nullptr) {
        throw std::invalid_argument(std::string(what) + " must not be null");
    }
    std::string result(reinterpret_cast<const char *>(bytes), length);
    if (!is_valid_utf8(result)) {
        throw std::invalid_argument(std::string(what) + " must be valid UTF-8");
    }
    return result;
}

void initialize_text_document(TextDocument &document, const char *id, const char *field,
                              const uint8_t *text, uint32_t text_length) {
    validate_document_id(id);
    validate_field_name(field);
    if (*id == '\0' || *field == '\0') {
        throw std::invalid_argument("text document ID and field must not be empty");
    }
    document.id = id;
    document.field = field;
    document.text = copy_text_bytes(text, text_length, MAX_TEXT_BYTES, "text");
}

void append_text_occurrences(TextDocument &document, const uint8_t *term_bytes,
                             uint32_t term_bytes_length, const uint32_t *term_lengths,
                             const uint32_t *byte_starts, const uint32_t *byte_lengths,
                             uint32_t occurrence_count) {
    if (document.occurrences_added) {
        throw std::invalid_argument("text occurrences may only be added once");
    }
    if (document.occurrences.size() > MAX_TEXT_OCCURRENCES ||
        occurrence_count > MAX_TEXT_OCCURRENCES - document.occurrences.size()) {
        throw std::invalid_argument("text occurrence count is too large");
    }
    if (occurrence_count == 0) {
        if (term_bytes_length != 0) {
            throw std::invalid_argument("text occurrence term bytes are not fully described");
        }
        document.occurrences_added = true;
        return;
    }
    if (!term_lengths || !byte_starts || !byte_lengths) {
        throw std::invalid_argument("text occurrence arrays must not be null");
    }
    if (term_bytes_length != 0 && !term_bytes) {
        throw std::invalid_argument("text occurrence term bytes must not be null");
    }
    size_t total_term_bytes = 0;
    for (uint32_t index = 0; index < occurrence_count; ++index) {
        if (term_lengths[index] == 0) {
            throw std::invalid_argument("text occurrence term must not be empty");
        }
        if (term_lengths[index] > MAX_TEXT_TERM_BYTES ||
            term_lengths[index] > MAX_TOTAL_TEXT_TERM_BYTES - total_term_bytes) {
            throw std::invalid_argument("text occurrence term bytes are too large");
        }
        total_term_bytes += term_lengths[index];
        if (byte_starts[index] > document.text.size() ||
            byte_lengths[index] > document.text.size() - byte_starts[index] ||
            !is_utf8_boundary(document.text, byte_starts[index]) ||
            !is_utf8_boundary(document.text, byte_starts[index] + byte_lengths[index])) {
            throw std::invalid_argument("text occurrence span is not on UTF-8 boundaries");
        }
    }
    if (total_term_bytes != term_bytes_length) {
        throw std::invalid_argument("text occurrence term lengths do not match term bytes");
    }

    std::vector<TextOccurrence> additions;
    additions.reserve(occurrence_count);
    size_t term_offset = 0;
    for (uint32_t index = 0; index < occurrence_count; ++index) {
        const auto term_length = term_lengths[index];
        additions.push_back({
            copy_text_bytes(term_bytes + term_offset, term_length,
                            MAX_TEXT_TERM_BYTES, "text occurrence term"),
            byte_starts[index],
            byte_lengths[index],
            static_cast<uint32_t>(document.occurrences.size() + index)});
        term_offset += term_length;
    }
    document.occurrences.insert(document.occurrences.end(),
                                std::make_move_iterator(additions.begin()),
                                std::make_move_iterator(additions.end()));
    document.occurrences_added = true;
}

void initialize_text_request(TextRequest &request, uint32_t kind, const char *field,
                             const char *rank_profile) {
    validate_field_name(field);
    validate_rank_profile(rank_profile);
    if (*field == '\0' || *rank_profile == '\0') {
        throw std::invalid_argument("text query field and rank profile must not be empty");
    }
    if (kind != SK_TEXT_QUERY_TERM && kind != SK_TEXT_QUERY_PHRASE) {
        throw std::invalid_argument("unsupported text query kind");
    }
    request.kind = kind;
    request.field = field;
    request.rank_profile = rank_profile;
}

void append_text_terms(TextRequest &request, const uint8_t *term_bytes,
                       uint32_t term_bytes_length, const uint32_t *term_lengths,
                       uint32_t term_count) {
    if (request.terms_added) {
        throw std::invalid_argument("text query terms may only be added once");
    }
    if (request.terms.size() > MAX_TEXT_TERMS ||
        term_count > MAX_TEXT_TERMS - request.terms.size()) {
        throw std::invalid_argument("text term count is too large");
    }
    const auto resulting_count = request.terms.size() + term_count;
    if ((request.kind == SK_TEXT_QUERY_TERM && resulting_count != 1) ||
        (request.kind == SK_TEXT_QUERY_PHRASE && resulting_count == 0)) {
        throw std::invalid_argument("invalid number of terms for text query kind");
    }
    if (term_count == 0) {
        if (term_bytes_length != 0) {
            throw std::invalid_argument("text query term bytes are not fully described");
        }
        request.terms_added = true;
        return;
    }
    if (!term_lengths) {
        throw std::invalid_argument("text query term lengths must not be null");
    }
    if (term_bytes_length != 0 && !term_bytes) {
        throw std::invalid_argument("text query term bytes must not be null");
    }
    size_t total_term_bytes = 0;
    for (uint32_t index = 0; index < term_count; ++index) {
        if (term_lengths[index] == 0) {
            throw std::invalid_argument("text query term must not be empty");
        }
        if (term_lengths[index] > MAX_TEXT_TERM_BYTES ||
            term_lengths[index] > MAX_TOTAL_TEXT_TERM_BYTES - total_term_bytes) {
            throw std::invalid_argument("text query term bytes are too large");
        }
        total_term_bytes += term_lengths[index];
    }
    if (total_term_bytes != term_bytes_length) {
        throw std::invalid_argument("text query term lengths do not match term bytes");
    }
    std::vector<std::string> additions;
    additions.reserve(term_count);
    size_t term_offset = 0;
    for (uint32_t index = 0; index < term_count; ++index) {
        const auto term_length = term_lengths[index];
        additions.push_back(copy_text_bytes(term_bytes + term_offset, term_length,
                                            MAX_TEXT_TERM_BYTES, "text query term"));
        term_offset += term_length;
    }
    request.terms.insert(request.terms.end(),
                         std::make_move_iterator(additions.begin()),
                         std::make_move_iterator(additions.end()));
    request.positions.reserve(request.terms.size());
    for (size_t index = request.positions.size(); index < request.terms.size(); ++index) {
        request.positions.push_back(static_cast<uint32_t>(index));
    }
    request.terms_added = true;
}
