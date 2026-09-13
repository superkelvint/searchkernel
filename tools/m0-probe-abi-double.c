#include <searchkernel_vespa.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

struct sk_engine {
    int marker;
};

static unsigned int open_count;

static const char *mode(void) {
    const char *value = getenv("SK_M0_FAKE_MODE");
    return value ? value : "success";
}

static void clear_error(uint8_t *error, uint32_t capacity) {
    if (error && capacity != 0) {
        error[0] = '\0';
    }
}

static void set_error(uint8_t *error, uint32_t capacity, const char *message) {
    size_t length;

    if (!error || capacity == 0 || !message) {
        return;
    }
    clear_error(error, capacity);
    length = strlen(message);
    if (length >= capacity) {
        length = capacity - 1;
    }
    memcpy(error, message, length);
    error[length] = '\0';
}

uint32_t sk_native_abi_version(void) {
    return strcmp(mode(), "abi-mismatch") == 0 ? 1U : 2U;
}

int32_t sk_engine_open(const char *data_dir, const char *config_dir,
                       sk_engine **out_engine, uint8_t *out_error,
                       uint32_t out_error_capacity) {
    const char *selected_mode = mode();

    (void)data_dir;
    (void)config_dir;
    open_count++;
    if (strcmp(selected_mode, "open-error") == 0) {
        set_error(out_error, out_error_capacity, "injected open error");
        return 7;
    }
    if (strcmp(selected_mode, "open-null-message") == 0) {
        set_error(out_error, out_error_capacity, NULL);
        return 8;
    }
    if (strcmp(selected_mode, "open-null-engine") == 0) {
        clear_error(out_error, out_error_capacity);
        *out_engine = NULL;
        return 0;
    }
    if (strcmp(selected_mode, "second-open-error") == 0 && open_count == 2) {
        set_error(out_error, out_error_capacity, "injected second open error");
        return 9;
    }

    clear_error(out_error, out_error_capacity);
    *out_engine = malloc(sizeof(**out_engine));
    if (*out_engine) {
        (*out_engine)->marker = 1;
        return 0;
    }
    set_error(out_error, out_error_capacity, "injected allocation failure");
    return 10;
}

void sk_engine_destroy(sk_engine *engine) {
    free(engine);
}
