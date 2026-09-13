#include <searchkernel_vespa.h>

#include <stdio.h>
#include <string.h>

struct sk_engine {
    int marker;
};

static unsigned int open_count;

static const char *mode(void) {
    const char *value = getenv("SK_M0_FAKE_MODE");
    return value ? value : "success";
}

static void clear_error(sk_error *error) {
    if (error) {
        error->code = 0;
        error->message[0] = '\0';
    }
}

static void set_error(sk_error *error, int32_t code, const char *message) {
    size_t length;

    if (!error) {
        return;
    }
    clear_error(error);
    error->code = code;
    if (!message) {
        return;
    }
    length = strlen(message);
    if (length >= sizeof(error->message)) {
        length = sizeof(error->message) - 1;
    }
    memcpy(error->message, message, length);
    error->message[length] = '\0';
}

uint32_t sk_native_abi_version(void) {
    return strcmp(mode(), "abi-mismatch") == 0 ? 2U : 1U;
}

int32_t sk_engine_open(const char *data_dir, const char *config_dir,
                       sk_engine **out_engine, sk_error *out_error) {
    const char *selected_mode = mode();

    (void)data_dir;
    (void)config_dir;
    open_count++;
    if (strcmp(selected_mode, "open-error") == 0) {
        set_error(out_error, 7, "injected open error");
        return 7;
    }
    if (strcmp(selected_mode, "open-null-message") == 0) {
        set_error(out_error, 8, NULL);
        return 8;
    }
    if (strcmp(selected_mode, "open-null-engine") == 0) {
        clear_error(out_error);
        *out_engine = NULL;
        return 0;
    }
    if (strcmp(selected_mode, "second-open-error") == 0 && open_count == 2) {
        set_error(out_error, 9, "injected second open error");
        return 9;
    }

    clear_error(out_error);
    *out_engine = malloc(sizeof(**out_engine));
    if (*out_engine) {
        (*out_engine)->marker = 1;
        return 0;
    }
    set_error(out_error, 10, "injected allocation failure");
    return 10;
}

void sk_engine_destroy(sk_engine *engine) {
    free(engine);
}
