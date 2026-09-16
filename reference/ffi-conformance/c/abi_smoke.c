#include "searchkernel_ffi.h"

#include <assert.h>
#include <stdint.h>
#include <stdio.h>

_Static_assert(sizeof(sk_handle_t) == 8, "sk_handle_t must be 64-bit");
_Static_assert(sizeof(sk_status_t) == 4, "sk_status_t must be 32-bit");

int main(void) {
    assert(SK_ABI_VERSION == 2u);
    assert(SK_STATUS_OK == 0u);
    assert(SK_STATUS_INVALID_ARGUMENT == 1u);
    assert(SK_STATUS_INVALID_HANDLE == 2u);
    assert(SK_STATUS_INVALID_REQUEST == 3u);
    assert(SK_STATUS_UNSUPPORTED_PROTOCOL == 4u);
    assert(SK_STATUS_UNSUPPORTED_OPERATION == 5u);
    assert(SK_STATUS_INTERNAL == 6u);
    assert(SK_STATUS_PANIC == 7u);
    assert(SK_STATUS_INDEX_CLOSED == 12u);

    assert(sk_abi_version() == SK_ABI_VERSION);

    sk_buffer_free(NULL, 0u);

    assert(sk_close(0u, NULL, 0u) == SK_STATUS_INVALID_HANDLE);

    // sk_call treats the output as write-only on entry, so ordinary
    // uninitialized C out-parameter storage is valid.
    uint8_t *out_data;
    size_t out_len;
    uint64_t out_allocation_id;
    assert(sk_call(0u, NULL, 0u, &out_data, &out_len, &out_allocation_id) ==
           SK_STATUS_INVALID_HANDLE);
    assert(out_data == NULL);
    assert(out_len == 0u);
    assert(out_allocation_id == 0u);

    out_len = SIZE_MAX;
    out_allocation_id = UINT64_MAX;
    assert(sk_call(0u, NULL, 0u, NULL, &out_len, &out_allocation_id) ==
           SK_STATUS_INVALID_ARGUMENT);
    assert(out_len == 0u);
    assert(out_allocation_id == 0u);

    puts("C ABI smoke PASS");
    return 0;
}
