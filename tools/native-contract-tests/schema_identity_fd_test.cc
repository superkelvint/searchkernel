#include "file_descriptor.h"

#include <fcntl.h>

static int check_reuse(int original, int replacement_flags, const char *replacement_path) {
    int replacement = -1;
    {
        FileDescriptor descriptor(original);
        if (descriptor.close() != 0) {
            return 1;
        }
        replacement = ::open(replacement_path, replacement_flags);
        if (replacement != original) {
            if (replacement >= 0) ::close(replacement);
            return 2;
        }
    }

    const bool replacement_is_open = (::fcntl(replacement, F_GETFD) >= 0);
    ::close(replacement);
    return replacement_is_open ? 0 : 3;
}

extern "C" int searchkernel_schema_identity_fd_reuse_regression() {
    const int file_fd = ::open("/dev/null", O_RDONLY);
    if (file_fd < 0) {
        return 1;
    }
    const int file_result = check_reuse(file_fd, O_RDONLY, "/dev/null");
    if (file_result != 0) {
        return file_result;
    }

    const int directory_fd = ::open(".", O_RDONLY | O_DIRECTORY);
    if (directory_fd < 0) {
        return 4;
    }
    return check_reuse(directory_fd, O_RDONLY | O_DIRECTORY, ".");
}
