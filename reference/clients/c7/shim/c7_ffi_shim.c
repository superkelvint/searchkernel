#define _POSIX_C_SOURCE 200809L
#include "c5_abi_adapter.h"

#include <errno.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

struct sk_handle {
    uint64_t id;
    int closed;
};

typedef struct allocation_node {
    uint8_t *ptr;
    size_t len;
    struct allocation_node *next;
} allocation_node;

static uint64_t next_handle_id = 1;
static uint64_t open_count = 0;
static uint64_t close_count = 0;
static uint64_t call_count = 0;
static uint64_t alloc_count = 0;
static uint64_t free_count = 0;
static uint64_t double_free_count = 0;
static allocation_node *allocations = NULL;

static const char *trace_path(void) {
    const char *p = getenv("C7_SHIM_TRACE");
    return (p && *p) ? p : "/tmp/searchkernel-c7-shim.trace";
}

static uint32_t configured_abi_version(void) {
    const char *v = getenv("C7_SHIM_ABI_VERSION");
    if (!v || !*v) return 1u;
    unsigned long parsed = strtoul(v, NULL, 10);
    return (uint32_t)parsed;
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

static void track_alloc(uint8_t *ptr, size_t len) {
    allocation_node *n = (allocation_node *)malloc(sizeof(*n));
    if (!n) abort();
    n->ptr = ptr;
    n->len = len;
    n->next = allocations;
    allocations = n;
    alloc_count++;
}

static int untrack_alloc(uint8_t *ptr) {
    allocation_node **cur = &allocations;
    while (*cur) {
        if ((*cur)->ptr == ptr) {
            allocation_node *victim = *cur;
            *cur = victim->next;
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
    uint64_t n = 0;
    for (allocation_node *p = allocations; p; p = p->next) n++;
    return n;
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
             ",\"close\":%" PRIu64
             ",\"call\":%" PRIu64
             ",\"alloc\":%" PRIu64
             ",\"free\":%" PRIu64
             ",\"double_free\":%" PRIu64
             ",\"outstanding\":%" PRIu64 "}",
             open_count, close_count, call_count,
             alloc_count, free_count, double_free_count, outstanding_count());
    trace_line(buf);
}

uint32_t sk_abi_version(void) {
    uint32_t v = configured_abi_version();
    trace_event("abi_version", v, 0);
    return v;
}

int32_t sk_open(const uint8_t *payload, size_t payload_len, sk_handle **out_handle) {
    (void)payload;
    if (!out_handle) return -10;
    sk_handle *h = (sk_handle *)calloc(1, sizeof(*h));
    if (!h) return -11;
    h->id = next_handle_id++;
    *out_handle = h;
    open_count++;
    trace_event("open", h->id, (uint64_t)payload_len);
    return 0;
}

int32_t sk_call(sk_handle *handle,
                const uint8_t *request,
                size_t request_len,
                sk_buffer *out_response) {
    if (!handle || handle->closed || !out_response) return -20;
    call_count++;
    trace_event("call", handle->id, (uint64_t)request_len);

    /* Reserved verifier input: ASCII FAIL => native transport failure. */
    if (request_len == 4 && request && memcmp(request, "FAIL", 4) == 0) {
        out_response->data = NULL;
        out_response->len = 0;
        return -77;
    }

    uint8_t *response = NULL;
    if (request_len > 0) {
        response = (uint8_t *)malloc(request_len);
        if (!response) return -21;
        for (size_t i = 0; i < request_len; i++) {
            response[i] = request[request_len - 1 - i];
        }
        track_alloc(response, request_len);
    }

    out_response->data = response;
    out_response->len = request_len;
    if (response) trace_event("alloc", (uint64_t)(uintptr_t)response, (uint64_t)request_len);
    return 0;
}

void sk_buffer_free(sk_buffer *buffer) {
    if (!buffer) return;
    if (buffer->data) {
        uint8_t *p = buffer->data;
        if (untrack_alloc(p)) {
            trace_event("free", (uint64_t)(uintptr_t)p, (uint64_t)buffer->len);
            free(p);
        } else {
            trace_event("double_free", (uint64_t)(uintptr_t)p, 0);
        }
    }
    buffer->data = NULL;
    buffer->len = 0;
}

void sk_close(sk_handle *handle) {
    if (!handle) return;
    if (handle->closed) {
        trace_event("native_double_close", handle->id, 0);
        return;
    }
    handle->closed = 1;
    close_count++;
    trace_event("close", handle->id, 0);
    free(handle);
}
