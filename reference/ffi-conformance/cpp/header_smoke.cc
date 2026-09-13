#include "searchkernel_ffi.h"

#include <cstddef>
#include <cstdint>
#include <type_traits>

static_assert(sizeof(sk_handle_t) == 8);
static_assert(sizeof(sk_status_t) == 4);
static_assert(std::is_standard_layout_v<sk_buffer_t>);
static_assert(offsetof(sk_buffer_t, data) == 0);
static_assert(offsetof(sk_buffer_t, len) == sizeof(void *));
static_assert(offsetof(sk_buffer_t, allocation_id) == sizeof(void *) * 2);
static_assert(sizeof(sk_buffer_t) == sizeof(void *) * 2 + sizeof(uint64_t));

int main() {
    sk_buffer_t buffer{nullptr, 0, 0};
    sk_buffer_free(&buffer);
    return sk_abi_version() == SK_ABI_VERSION ? 0 : 1;
}
