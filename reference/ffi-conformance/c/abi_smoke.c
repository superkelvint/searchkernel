#include "searchkernel_ffi.h"

#include <assert.h>
#include <stdint.h>
#include <stdio.h>

_Static_assert(sizeof(sk_handle_t) == 8, "sk_handle_t must be 64-bit");
_Static_assert(sizeof(sk_status_t) == 4, "sk_status_t must be 32-bit");
_Static_assert(offsetof(sk_buffer_t, data) == 0, "data offset");
_Static_assert(offsetof(sk_buffer_t, len) == sizeof(void *), "len offset");
_Static_assert(offsetof(sk_buffer_t, allocation_id) == sizeof(void *) * 2,
               "allocation_id offset");
_Static_assert(sizeof(sk_buffer_t) == sizeof(void *) * 2 + sizeof(uint64_t),
               "sk_buffer_t layout");

int main(void) {
    assert(SK_ABI_VERSION == 1u);
    assert(SK_STATUS_OK == 0u);
    assert(SK_STATUS_INVALID_ARGUMENT == 1u);
    assert(SK_STATUS_INVALID_HANDLE == 2u);
    assert(SK_STATUS_INVALID_REQUEST == 3u);
    assert(SK_STATUS_UNSUPPORTED_PROTOCOL == 4u);
    assert(SK_STATUS_UNSUPPORTED_OPERATION == 5u);
    assert(SK_STATUS_INTERNAL == 6u);
    assert(SK_STATUS_PANIC == 7u);

    assert(sk_abi_version() == SK_ABI_VERSION);

    sk_buffer_free(NULL);

    sk_buffer_t empty = {0};
    sk_buffer_free(&empty);
    assert(empty.data == NULL);
    assert(empty.len == 0u);
    assert(empty.allocation_id == 0u);

    assert(sk_close(0u) == SK_STATUS_INVALID_HANDLE);

    // sk_call treats the output as write-only on entry, so ordinary
    // uninitialized C out-parameter storage is valid.
    sk_buffer_t out;
    assert(sk_call(0u, NULL, 0u, &out) == SK_STATUS_INVALID_HANDLE);
    assert(out.data == NULL);
    assert(out.len == 0u);
    assert(out.allocation_id == 0u);

    assert(sk_call(0u, NULL, 0u, NULL) == SK_STATUS_INVALID_ARGUMENT);

    puts("C ABI smoke PASS");
    return 0;
}
