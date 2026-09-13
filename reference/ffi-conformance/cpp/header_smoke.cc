#include "searchkernel_ffi.h"

#include <cstdint>

static_assert(sizeof(sk_handle_t) == 8);
static_assert(sizeof(sk_status_t) == 4);

int main() {
    sk_buffer_free(nullptr, 0);
    return sk_abi_version() == SK_ABI_VERSION ? 0 : 1;
}
