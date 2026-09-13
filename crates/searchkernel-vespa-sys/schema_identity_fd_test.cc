#include "file_descriptor.h"

#include <fcntl.h>

extern "C" int searchkernel_schema_identity_fd_reuse_regression() {
    const int original = ::open("/dev/null", O_RDONLY);
    if (original < 0) {
        return 1;
    }

    int replacement = -1;
    {
        FileDescriptor descriptor(original);
        if (descriptor.close() != 0) {
            return 2;
        }
        replacement = ::open("/dev/null", O_RDONLY);
        if (replacement != original) {
            if (replacement >= 0) ::close(replacement);
            return 3;
        }
    }

    const bool replacement_is_open = (::fcntl(replacement, F_GETFD) >= 0);
    ::close(replacement);
    return replacement_is_open ? 0 : 4;
}
