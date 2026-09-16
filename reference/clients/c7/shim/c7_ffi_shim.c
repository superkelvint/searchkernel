#define _POSIX_C_SOURCE 200809L
#include "c5_abi_adapter.h"

#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

typedef struct handle_node {
    sk_handle_t id;
    int closed;
    struct handle_node *next;
} handle_node;

typedef struct allocation_node {
    uint8_t *ptr;
    uint64_t id;
    size_t len;
    struct allocation_node *next;
} allocation_node;

static sk_handle_t next_handle_id = 1;
static uint64_t next_allocation_id = 1;
static uint64_t open_count = 0;
static uint64_t close_count = 0;
static uint64_t call_count = 0;
static uint64_t alloc_count = 0;
static uint64_t free_count = 0;
static uint64_t double_free_count = 0;
static handle_node *handles = NULL;
static allocation_node *allocations = NULL;

static const char *trace_path(void) {
    const char *p = getenv("C7_SHIM_TRACE");
    return (p && *p) ? p : "/tmp/searchkernel-c7-shim.trace";
}

static uint32_t configured_abi_version(void) {
    const char *v = getenv("C7_SHIM_ABI_VERSION");
    if (!v || !*v) return 2u;
    return (uint32_t)strtoul(v, NULL, 10);
}

static void trace_line(const char *line) {
    FILE *f = fopen(trace_path(), "a");
    if (!f) return;
    fputs(line, f);
    fputc('\n', f);
    fclose(f);
}

static void trace_event(const char *event, uint64_t a, uint64_t b) {
    char buf[256];
    snprintf(buf, sizeof(buf),
             "{\"event\":\"%s\",\"a\":%" PRIu64 ",\"b\":%" PRIu64 "}",
             event, a, b);
    trace_line(buf);
}

static handle_node *find_handle(sk_handle_t id) {
    for (handle_node *p = handles; p; p = p->next) {
        if (p->id == id) return p;
    }
    return NULL;
}

static void track_alloc(uint8_t *ptr, size_t len, uint64_t id) {
    allocation_node *n = (allocation_node *)malloc(sizeof(*n));
    if (!n) abort();
    n->ptr = ptr;
    n->id = id;
    n->len = len;
    n->next = allocations;
    allocations = n;
    alloc_count++;
}

static int untrack_alloc(uint8_t *ptr, uint64_t id, size_t *len) {
    allocation_node **cur = &allocations;
    while (*cur) {
        if ((*cur)->ptr == ptr && (*cur)->id == id) {
            allocation_node *victim = *cur;
            *cur = victim->next;
            if (len) *len = victim->len;
            free(victim);
            free_count++;
            return 1;
        }
        cur = &(*cur)->next;
    }
    double_free_count++;
    return 0;
}

static uint64_t outstanding_count(void) {
    uint64_t count = 0;
    for (allocation_node *p = allocations; p; p = p->next) count++;
    return count;
}

__attribute__((constructor))
static void c7_init(void) {
    unlink(trace_path());
    trace_event("shim_init", configured_abi_version(), 0);
}

__attribute__((destructor))
static void c7_fini(void) {
    char buf[512];
    snprintf(buf, sizeof(buf),
             "{\"event\":\"summary\",\"open\":%" PRIu64
             ",\"close\":%" PRIu64 ",\"call\":%" PRIu64
             ",\"alloc\":%" PRIu64 ",\"free\":%" PRIu64
             ",\"double_free\":%" PRIu64 ",\"outstanding\":%" PRIu64 "}",
             open_count, close_count, call_count, alloc_count, free_count,
             double_free_count, outstanding_count());
    trace_line(buf);
}

uint32_t sk_abi_version(void) {
    uint32_t version = configured_abi_version();
    trace_event("abi_version", version, 0);
    return version;
}

sk_status_t sk_open(const char *path, sk_handle_t *out_handle,
                    uint8_t *error, size_t error_capacity) {
    (void)path;
    (void)error;
    (void)error_capacity;
    if (!out_handle) return SK_STATUS_INVALID_ARGUMENT;
    *out_handle = 0;
    handle_node *handle = (handle_node *)calloc(1, sizeof(*handle));
    if (!handle) return SK_STATUS_INVALID_ARGUMENT;
    handle->id = next_handle_id++;
    handle->next = handles;
    handles = handle;
    *out_handle = handle->id;
    open_count++;
    trace_event("open", handle->id, 0);
    return SK_STATUS_OK;
}

sk_status_t sk_call(sk_handle_t handle, const uint8_t *request, size_t request_len,
                    uint8_t **out_data, size_t *out_len,
                    uint64_t *out_allocation_id) {
    if (!out_data || !out_len || !out_allocation_id) return SK_STATUS_INVALID_ARGUMENT;
    *out_data = NULL;
    *out_len = 0;
    *out_allocation_id = 0;
    handle_node *entry = find_handle(handle);
    if (!entry || entry->closed) return SK_STATUS_INVALID_HANDLE;
    if (request_len > 0 && !request) return SK_STATUS_INVALID_ARGUMENT;
    call_count++;
    trace_event("call", handle, (uint64_t)request_len);
    if (request_len == 4 && memcmp(request, "FAIL", 4) == 0) return 77u;

    if (request_len > 0) {
        uint8_t *response = (uint8_t *)malloc(request_len);
        if (!response) return SK_STATUS_INVALID_ARGUMENT;
        for (size_t i = 0; i < request_len; ++i) response[i] = request[request_len - 1 - i];
        uint64_t allocation_id = next_allocation_id++;
        track_alloc(response, request_len, allocation_id);
        *out_data = response;
        *out_len = request_len;
        *out_allocation_id = allocation_id;
        trace_event("alloc", allocation_id, (uint64_t)request_len);
    }
    return SK_STATUS_OK;
}

sk_status_t sk_close(sk_handle_t handle, uint8_t *error, size_t error_capacity) {
    (void)error;
    (void)error_capacity;
    handle_node *entry = find_handle(handle);
    if (!entry || entry->closed) return SK_STATUS_INVALID_HANDLE;
    entry->closed = 1;
    close_count++;
    trace_event("close", handle, 0);
    return SK_STATUS_OK;
}

void sk_buffer_free(uint8_t *data, uint64_t allocation_id) {
    if (!data || !allocation_id) return;
    size_t len = 0;
    if (untrack_alloc(data, allocation_id, &len)) {
        trace_event("free", allocation_id, (uint64_t)len);
        free(data);
    } else {
        trace_event("double_free", allocation_id, 0);
    }
}
