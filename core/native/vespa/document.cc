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
                                           uint32_t count, uint32_t bytes_length,
                                           const char *what) {
    if (count > MAX_PACKED_DOCUMENT_ITEMS) {
        throw std::invalid_argument(std::string(what) + " has too many items");
    }
    if (count == 0 && bytes_length != 0) {
        throw std::invalid_argument(std::string(what) + " arrays are invalid");
    }
    if (bytes_length > MAX_PACKED_DOCUMENT_TOTAL_BYTES) {
        throw std::invalid_argument(std::string(what) + " data is too large");
    }
    if (count != 0 && (!bytes || !lengths)) {
        throw std::invalid_argument(std::string(what) + " arrays are invalid");
    }
    std::vector<std::string> result;
    result.reserve(count);
    size_t offset = 0;
    for (uint32_t index = 0; index < count; ++index) {
        const size_t length = lengths[index];
        if (length > MAX_PACKED_DOCUMENT_ELEMENT_BYTES) {
            throw std::invalid_argument(std::string(what) + " item is too large");
        }
        if (offset > bytes_length || length > bytes_length - offset) {
            throw std::invalid_argument(std::string(what) + " lengths exceed packed data");
        }
        if (length == 0) {
            result.emplace_back();
        } else {
            result.emplace_back(reinterpret_cast<const char *>(bytes + offset), length);
        }
        offset += length;
    }
    if (offset != bytes_length) {
        throw std::invalid_argument(std::string(what) + " lengths do not match packed data");
    }
    return result;
}

std::vector<std::pair<std::string, std::string>> copy_predicate_features(
    const uint8_t *key_bytes, const uint32_t *key_lengths, uint32_t key_bytes_length,
    const uint8_t *value_bytes, const uint32_t *value_lengths, uint32_t value_bytes_length,
    uint32_t count) {
    if (count == 0 || count > MAX_PACKED_DOCUMENT_ITEMS) {
        throw std::invalid_argument("predicate feature count is invalid");
    }
    if (key_bytes_length > MAX_PACKED_DOCUMENT_TOTAL_BYTES ||
        value_bytes_length > MAX_PACKED_DOCUMENT_TOTAL_BYTES) {
        throw std::invalid_argument("predicate feature data is too large");
    }
    if (!key_bytes || !key_lengths || !value_bytes || !value_lengths) {
        throw std::invalid_argument("predicate feature arrays are invalid");
    }
    std::vector<std::pair<std::string, std::string>> result;
    result.reserve(count);
    size_t key_offset = 0;
    size_t value_offset = 0;
    for (uint32_t index = 0; index < count; ++index) {
        const size_t key_length = key_lengths[index];
        const size_t value_length = value_lengths[index];
        if (key_length > MAX_PACKED_DOCUMENT_ELEMENT_BYTES ||
            value_length > MAX_PACKED_DOCUMENT_ELEMENT_BYTES) {
            throw std::invalid_argument("predicate feature item is too large");
        }
        if (key_offset > key_bytes_length || key_length > key_bytes_length - key_offset ||
            value_offset > value_bytes_length ||
            value_length > value_bytes_length - value_offset) {
            throw std::invalid_argument("predicate feature lengths exceed packed data");
        }
        std::string key;
        if (key_length != 0) {
            key.assign(reinterpret_cast<const char *>(key_bytes + key_offset), key_length);
        }
        std::string value;
        if (value_length != 0) {
            value.assign(reinterpret_cast<const char *>(value_bytes + value_offset), value_length);
        }
        result.emplace_back(std::move(key), std::move(value));
        key_offset += key_length;
        value_offset += value_length;
    }
    if (key_offset != key_bytes_length || value_offset != value_bytes_length) {
        throw std::invalid_argument("predicate feature lengths do not match packed data");
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

std::vector<std::vector<TextOccurrence>> decode_text_array_occurrences(
    const std::vector<std::string> &values, const uint8_t *term_bytes,
    uint32_t term_bytes_length, const uint32_t *term_lengths,
    const uint32_t *byte_starts, const uint32_t *byte_lengths,
    const uint32_t *element_offsets, uint32_t element_count,
    uint32_t occurrence_count) {
    if (element_count != values.size()) {
        throw std::invalid_argument("array annotation element count does not match array");
    }
    if (!element_offsets || element_offsets[0] != 0 ||
        element_offsets[element_count] != occurrence_count) {
        throw std::invalid_argument("array annotation offsets are invalid");
    }
    if (occurrence_count != 0 && (!term_lengths || !byte_starts || !byte_lengths)) {
        throw std::invalid_argument("array annotation occurrence arrays are invalid");
    }
    if (term_bytes_length != 0 && !term_bytes) {
        throw std::invalid_argument("array annotation term bytes must not be null");
    }

    std::vector<std::vector<TextOccurrence>> result(element_count);
    for (uint32_t element = 0; element < element_count; ++element) {
        const auto begin = element_offsets[element];
        const auto end = element_offsets[element + 1];
        if (begin > end || end > occurrence_count) {
            throw std::invalid_argument("array annotation offsets are invalid");
        }
        std::vector<uint32_t> lengths;
        std::vector<uint32_t> starts;
        std::vector<uint32_t> byte_lengths_for_element;
        if (begin != end) {
            lengths.assign(term_lengths + begin, term_lengths + end);
            starts.assign(byte_starts + begin, byte_starts + end);
            byte_lengths_for_element.assign(byte_lengths + begin, byte_lengths + end);
        }
        size_t term_offset = 0;
        for (uint32_t prior = 0; prior < begin; ++prior) {
            const auto length = term_lengths[prior];
            if (length == 0) {
                throw std::invalid_argument("array annotation term must not be empty");
            }
            if (term_offset > term_bytes_length || length > term_bytes_length - term_offset) {
                throw std::invalid_argument("array annotation term bytes are invalid");
            }
            term_offset += length;
        }
        std::vector<uint8_t> element_term_bytes;
        for (uint32_t occurrence = begin; occurrence < end; ++occurrence) {
            const auto length = term_lengths[occurrence];
            if (length == 0) {
                throw std::invalid_argument("array annotation term must not be empty");
            }
            if (term_offset > term_bytes_length || length > term_bytes_length - term_offset) {
                throw std::invalid_argument("array annotation term bytes are invalid");
            }
            element_term_bytes.insert(element_term_bytes.end(),
                                       term_bytes + term_offset,
                                       term_bytes + term_offset + length);
            term_offset += length;
        }
        TextDocument analyzed;
        analyzed.text = values[element];
        append_text_occurrences(analyzed, element_term_bytes.data(),
                                static_cast<uint32_t>(element_term_bytes.size()), lengths.data(),
                                starts.data(), byte_lengths_for_element.data(), end - begin);
        result[element] = std::move(analyzed.occurrences);
    }
    return result;
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
