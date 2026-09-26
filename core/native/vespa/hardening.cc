// Search Kernel native adapter: hardening and timing support.
#include <thread>
// This file is included by abi.cc so the private adapter remains one native
// boundary while responsibilities stay physically searchable by subsystem.

#ifdef SK_ENABLE_HARDENING_FAILPOINTS
class ResultFaultController {
    mutable std::mutex _mutex;
    uint32_t _fault = 0;
    bool _armed = false;

    static bool known(uint32_t fault) noexcept {
        return fault >= SK_TEST_RESULT_FAULT_NULL_RESULT &&
               fault <= SK_TEST_RESULT_FAULT_NULL_GROUPING_VALUE;
    }

public:
    void set(uint32_t fault) {
        if (!known(fault)) {
            throw std::invalid_argument("unknown native result fault");
        }
        std::lock_guard<std::mutex> guard(_mutex);
        _fault = fault;
        _armed = true;
    }

    void clear() noexcept {
        std::lock_guard<std::mutex> guard(_mutex);
        _armed = false;
    }

    bool take(uint32_t fault) const noexcept {
        auto *self = const_cast<ResultFaultController *>(this);
        std::lock_guard<std::mutex> guard(self->_mutex);
        if (!self->_armed || self->_fault != fault) {
            return false;
        }
        self->_armed = false;
        return true;
    }
};
#endif

#ifdef SK_ENABLE_HARDENING_FAILPOINTS
class FailpointController {
    struct Entry {
        uint32_t trigger = 0;
        uint32_t action = 0;
        uint32_t hits = 0;
        bool blocked = false;
        bool armed = false;
        std::thread::id owner;
    };

    bool _thread_affine;
    mutable std::mutex _mutex;
    std::condition_variable _condition;
    std::map<std::string, Entry> _entries;

    static bool known(std::string_view name) noexcept {
        static constexpr std::array<std::string_view, 18> names = {
            "open_before_native_create", "open_after_native_create",
            "open_constructor", "open_initialize",
            "mutation_before_native_call", "mutation_after_native_call",
            "search_before_match", "search_after_match", "result_before_copy",
            "result_after_allocation", "destroy_before_native_release",
            "attribute_mutation_before_native_call",
            "attribute_mutation_after_native_call", "group_before_match",
            "group_after_match", "search_after_id_resolution", "search_after_docsums"};
        return std::find(names.begin(), names.end(), name) != names.end();
    }

public:
    explicit FailpointController(bool thread_affine = false) noexcept
        : _thread_affine(thread_affine) {}

    void set(const char *name, uint32_t trigger, uint32_t action) {
        if (!name || !known(name)) {
            throw std::invalid_argument("unknown hardening failpoint");
        }
        if (trigger == 0) {
            throw std::invalid_argument("hardening failpoint hit must be non-zero");
        }
        if (action < SK_TEST_FAILPOINT_ERROR || action > SK_TEST_FAILPOINT_BLOCK) {
            throw std::invalid_argument("unsupported hardening failpoint action");
        }
        std::lock_guard<std::mutex> guard(_mutex);
        _entries[name] = Entry{
            trigger, action, 0, false, true,
            _thread_affine ? std::this_thread::get_id() : std::thread::id{}
        };
    }

    void clear(const char *name) {
        if (!name) {
            throw std::invalid_argument("hardening failpoint name must not be null");
        }
        std::lock_guard<std::mutex> guard(_mutex);
        auto entry = _entries.find(name);
        if (entry != _entries.end()) {
            entry->second.armed = false;
            entry->second.blocked = false;
        }
        _condition.notify_all();
    }

    void release(const char *name) {
        if (!name) {
            throw std::invalid_argument("hardening failpoint name must not be null");
        }
        std::lock_guard<std::mutex> guard(_mutex);
        auto entry = _entries.find(name);
        if (entry == _entries.end() || !entry->second.blocked) {
            throw std::invalid_argument("hardening failpoint is not blocked");
        }
        entry->second.blocked = false;
        entry->second.armed = false;
        _condition.notify_all();
    }

    bool is_blocked(const char *name) const {
        if (!name) {
            throw std::invalid_argument("hardening failpoint name must not be null");
        }
        std::lock_guard<std::mutex> guard(_mutex);
        auto entry = _entries.find(name);
        return entry != _entries.end() && entry->second.blocked;
    }

    void hit(std::string_view name) const {
        auto *self = const_cast<FailpointController *>(this);
        std::unique_lock<std::mutex> guard(self->_mutex);
        auto entry = self->_entries.find(std::string(name));
        if (entry == self->_entries.end() || !entry->second.armed) {
            return;
        }
        if (self->_thread_affine && entry->second.owner != std::this_thread::get_id()) {
            return;
        }
        ++entry->second.hits;
        if (entry->second.hits != entry->second.trigger) {
            return;
        }
        const auto action = entry->second.action;
        entry->second.armed = false;
        if (action == SK_TEST_FAILPOINT_BLOCK) {
            entry->second.blocked = true;
            self->_condition.notify_all();
            self->_condition.wait(guard, [&] { return !entry->second.blocked; });
            return;
        }
        guard.unlock();
        switch (action) {
        case SK_TEST_FAILPOINT_ERROR:
            throw std::invalid_argument("hardening failpoint error");
        case SK_TEST_FAILPOINT_STD_EXCEPTION:
            throw std::runtime_error("hardening failpoint std::exception");
        case SK_TEST_FAILPOINT_BAD_ALLOC:
            throw std::bad_alloc();
        case SK_TEST_FAILPOINT_UNKNOWN_EXCEPTION:
            throw 0;
        default:
            throw std::invalid_argument("unsupported hardening failpoint action");
        }
    }
};

// Open failpoints are process-global because no Engine exists yet. Make them
// thread-affine so parallel tests cannot consume a failpoint armed by another
// test thread. Engine-owned failpoints remain non-affine and retain their
// cross-thread lifecycle/concurrency semantics.
FailpointController OPEN_FAILPOINTS(true);

class TimingProbe {
public:
    using Clock = std::chrono::steady_clock;

    class Scope {
        const char *_name;
        Clock::time_point _start;

    public:
        explicit Scope(const char *name) noexcept
            : _name(name), _start(Clock::now()) {}

        ~Scope() noexcept {
            TimingProbe::record(_name, Clock::now() - _start);
        }
    };

    static bool enabled() noexcept {
        static const bool value = [] {
            const char *setting = std::getenv("SK_ENGINE_TIMING");
            return setting && std::string_view(setting) == "1";
        }();
        return value;
    }

    static Clock::time_point now() noexcept { return Clock::now(); }

    static void record(const char *name, Clock::duration elapsed) noexcept {
        if (!enabled()) return;
        const auto elapsed_us = std::chrono::duration_cast<std::chrono::microseconds>(elapsed).count();
        std::fprintf(stderr, "searchkernel_timing phase=%s elapsed_us=%lld\n", name,
                     static_cast<long long>(elapsed_us));
    }
};
#endif
