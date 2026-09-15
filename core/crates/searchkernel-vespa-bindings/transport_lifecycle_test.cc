#include "transport_owner.h"

namespace {

struct RecordingTransport {
    static bool shutdown_called;

    bool Start() noexcept { return true; }
    void ShutDown(bool) noexcept { shutdown_called = true; }
};

bool RecordingTransport::shutdown_called = false;

struct ConstructorFailure {
    TransportOwner<RecordingTransport> transport;

    ConstructorFailure() {
        transport.start();
        throw 1;
    }
};

} // namespace

extern "C" int searchkernel_transport_destructor_regression() {
    RecordingTransport::shutdown_called = false;
    try {
        ConstructorFailure failure;
        (void)failure;
    } catch (...) {
    }
    return RecordingTransport::shutdown_called ? 0 : 1;
}
