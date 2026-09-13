#ifndef SEARCHKERNEL_FILE_DESCRIPTOR_H
#define SEARCHKERNEL_FILE_DESCRIPTOR_H

#include <unistd.h>

class FileDescriptor {
    int _fd;

public:
    explicit FileDescriptor(int fd) noexcept : _fd(fd) {}
    FileDescriptor(const FileDescriptor &) = delete;
    FileDescriptor &operator=(const FileDescriptor &) = delete;

    int get() const noexcept { return _fd; }

    int close() noexcept {
        const int fd = _fd;
        _fd = -1;
        return fd >= 0 ? ::close(fd) : 0;
    }

    ~FileDescriptor() noexcept {
        (void)close();
    }
};

#endif
