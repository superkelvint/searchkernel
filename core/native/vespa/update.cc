// Search Kernel native adapter: update and attribute helpers.
// This file is included by abi.cc so the private adapter remains one native
// boundary while responsibilities stay physically searchable by subsystem.

template <typename Add>
int32_t add_update_value(sk_document_update *update, const char *field,
                         uint8_t *error, uint32_t error_capacity, Add &&add) noexcept {
    if (!update || !field) return set_error(error, error_capacity,
                                            SK_NATIVE_STATUS_INVALID_ARGUMENT,
                                            "update value arguments are invalid");
    return invoke(error, error_capacity, [&] {
        validate_field_name(field);
        if (*field == '\0') throw std::invalid_argument("update field must not be empty");
        auto value = add();
        auto existing = std::find_if(update->fields.begin(), update->fields.end(),
                                     [field](const auto &entry) { return entry.first == field; });
        if (existing != update->fields.end()) existing->second = std::move(value);
        else update->fields.emplace_back(field, std::move(value));
        update->arithmetic.erase(
            std::remove_if(update->arithmetic.begin(), update->arithmetic.end(),
                           [field](const auto &entry) { return entry.field == field; }),
            update->arithmetic.end());
    });
}

template <typename Container>
void append_owned_value(Container &container, const char *field, OwnedValue value) {
    validate_field_name(field);
    if (*field == '\0') throw std::invalid_argument("attribute field must not be empty");
    container.emplace_back(field, std::move(value));
}
