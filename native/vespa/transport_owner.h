#ifndef SEARCHKERNEL_TRANSPORT_OWNER_H
#define SEARCHKERNEL_TRANSPORT_OWNER_H

#include <stdexcept>

template <typename Transport>
class TransportOwner {
    Transport _transport;
    bool _started = false;

public:
    TransportOwner() = default;
    TransportOwner(const TransportOwner &) = delete;
    TransportOwner &operator=(const TransportOwner &) = delete;

    void start() {
        if (_started) {
            return;
        }
        // Set ownership before calling into FNET. If Start() throws after
        // starting any worker, this fully constructed subobject's destructor
        // still owns the responsibility to stop the transport.
        _started = true;
        if (!_transport.Start()) {
            _started = false;
            throw std::runtime_error("could not start embedded transport");
        }
    }

    void shutdown() {
        if (!_started) {
            return;
        }
        _transport.ShutDown(true);
        _started = false;
    }

    void shutdown_noexcept() noexcept {
        if (!_started) {
            return;
        }
        _started = false;
        try {
            _transport.ShutDown(true);
        } catch (...) {
        }
    }

    Transport &transport() noexcept { return _transport; }

    ~TransportOwner() noexcept { shutdown_noexcept(); }
};

#endif
