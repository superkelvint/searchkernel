#include <searchkernel_vespa.h>

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

static int open_and_close(const char *data_dir, const char *config_dir) {
    sk_engine *engine = NULL;
    uint8_t error[512] = {0};
    int32_t rc = sk_engine_open(data_dir, config_dir, &engine, error, sizeof(error));
    if (rc != 0 || engine == NULL) {
        fprintf(stderr, "sk_engine_open failed: status=%d message=%s\n",
                (int) rc, error[0] != '\0' ? (char *) error : "<none>");
        return 1;
    }
    sk_engine_destroy(engine);
    return 0;
}

int main(int argc, char **argv) {
    if (argc != 3) {
        fprintf(stderr, "usage: probe DATA_DIR CONFIG_DIR\n");
        return 2;
    }

    uint32_t abi = sk_native_abi_version();
    if (abi != 4) {
        fprintf(stderr, "unexpected native ABI: %u\n", abi);
        return 1;
    }

    if (open_and_close(argv[1], argv[2]) != 0) return 1;
    if (open_and_close(argv[1], argv[2]) != 0) return 1;

    printf("{\"scenario\":\"native-open-close\",\"abi_version\":%u,\"open_count\":2,\"status\":\"ok\"}\n", abi);
    return 0;
}
