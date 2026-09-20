// Search Kernel native adapter: native error translation.
// This file is included by abi.cc so the private adapter remains one native
// boundary while responsibilities stay physically searchable by subsystem.

void clear_error(uint8_t *error, uint32_t error_capacity) noexcept {
    if (!error || error_capacity == 0) {
        return;
    }
    error[0] = '\0';
}
int32_t set_error(uint8_t *error, uint32_t error_capacity, int32_t code,
                 const char *message, const char *detail_code = nullptr) noexcept {
    if (!error || error_capacity == 0 || !message) {
        return code;
    }
    // Keep the diagnostic path allocation-free: this function is called from
    // noexcept exception translation, including out-of-memory paths.
    size_t copied = 0;
    const auto append = [&](const char *value) {
        const size_t remaining = static_cast<size_t>(error_capacity - 1) - copied;
        const size_t length = std::strlen(value);
        const size_t count = std::min(length, remaining);
        std::memcpy(error + copied, value, count);
        copied += count;
    };
    if (detail_code) {
        // Stable machine-readable prefix used by the Rust adapter. The
        // human-readable message remains after the second colon.
        append("SK_DETAIL_V1:");
        append(detail_code);
        append(":");
    }
    append(message);
    error[copied] = '\0';
    return code;
}

template <typename Function>
int32_t invoke(uint8_t *error, uint32_t error_capacity, Function &&function) noexcept {
    clear_error(error, error_capacity);
    try {
        function();
        return SK_NATIVE_STATUS_OK;
    } catch (const std::bad_alloc &) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_OUT_OF_MEMORY, nullptr);
    } catch (const UnsupportedNativeError &exception) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_UNSUPPORTED, exception.what(),
                         exception.detail_code());
    } catch (const DeadlineExceededNativeError &exception) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_DEADLINE_EXCEEDED, exception.what());
    } catch (const TypeErrorNativeError &exception) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_TYPE_ERROR, exception.what());
    } catch (const TensorValidationError &exception) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         exception.what(), exception.detail_code());
    } catch (const std::invalid_argument &exception) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, exception.what());
    } catch (const std::exception &exception) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INTERNAL, exception.what());
    } catch (...) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INTERNAL, "unknown native exception");
    }
}

template <typename Handle>
void destroy_noexcept(Handle *handle) noexcept {
    try {
        delete handle;
    } catch (...) {
    }
}
