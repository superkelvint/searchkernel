#include "searchkernel_vespa.h"
#include "searchkernel_ffi.h"

static_assert(SK_NATIVE_STATUS_OK == 0);
static_assert(SK_NATIVE_STATUS_INVALID_ARGUMENT == 1);
static_assert(SK_NATIVE_STATUS_INTERNAL == 2);
static_assert(SK_NATIVE_STATUS_OUT_OF_MEMORY == 3);

static_assert(SK_STATUS_OK == 0);
static_assert(SK_STATUS_INVALID_ARGUMENT == 1);
static_assert(SK_STATUS_INTERNAL == 6);
