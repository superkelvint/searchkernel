// Search Kernel native adapter: hardening and timing support.
#include <thread>
#include <type_traits>
#include <deque>
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
        static constexpr std::array<std::string_view, 22> names = {
            "open_before_native_create", "open_after_native_create",
            "open_constructor", "open_initialize",
            "mutation_before_native_call", "mutation_after_native_call",
            "search_before_match", "search_after_match", "result_before_copy",
            "result_after_allocation", "destroy_before_native_release",
            "attribute_mutation_before_native_call",
            "attribute_mutation_after_native_call", "group_before_match",
            "group_after_match", "search_after_id_resolution", "search_after_docsums",
            "mutation_before_visibility_wait", "mutation_before_search_publication_wait",
            "mutation_after_search_publication_start",
            "matcher_publication_before_feedview_force_commit"};
        return std::find(names.begin(), names.end(), name) != names.end();
    }

public:
    enum class DeferredBlock { NotBlocking, Continue, Deferred };

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

    DeferredBlock defer_blocking_hit(std::string_view name) const {
        auto *self = const_cast<FailpointController *>(this);
        std::lock_guard<std::mutex> guard(self->_mutex);
        auto entry = self->_entries.find(std::string(name));
        if (entry == self->_entries.end() ||
            entry->second.action != SK_TEST_FAILPOINT_BLOCK ||
            (self->_thread_affine && entry->second.owner != std::this_thread::get_id())) {
            return DeferredBlock::NotBlocking;
        }
        if (entry->second.blocked) return DeferredBlock::Deferred;
        if (!entry->second.armed) return DeferredBlock::NotBlocking;
        ++entry->second.hits;
        if (entry->second.hits != entry->second.trigger) {
            return DeferredBlock::Continue;
        }
        entry->second.armed = false;
        entry->second.blocked = true;
        self->_condition.notify_all();
        return DeferredBlock::Deferred;
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

// Hardening builds install this forwarding view as FeedHandler's active view.
// Automatic FeedHandler commits and Search Kernel's explicit publication fence
// therefore pass through the same gate. The delegate preserves Vespa's real
// DocumentSubDBCollection feed topology; only forceCommit scheduling is held.
class PublicationGateFeedView final : public proton::IFeedView {
    proton::IFeedView::SP _delegate;
    std::shared_ptr<FailpointController> _failpoints;
    std::mutex _pending_mutex;
    std::deque<std::pair<CommitParam, DoneCallback>> _pending_commits;

public:
    PublicationGateFeedView(proton::IFeedView::SP delegate,
                             std::shared_ptr<FailpointController> failpoints) noexcept
        : _delegate(std::move(delegate)), _failpoints(std::move(failpoints)) {}

    const std::shared_ptr<const document::DocumentTypeRepo>& getDocumentTypeRepo() const override {
        return _delegate->getDocumentTypeRepo();
    }
    const proton::ISimpleDocumentMetaStore* getDocumentMetaStorePtr() const override {
        return _delegate->getDocumentMetaStorePtr();
    }
    void preparePut(proton::PutOperation& op) override { _delegate->preparePut(op); }
    void handlePut(proton::FeedToken token, const proton::PutOperation& op) override {
        _delegate->handlePut(std::move(token), op);
    }
    void prepareUpdate(proton::UpdateOperation& op) override { _delegate->prepareUpdate(op); }
    void handleUpdate(proton::FeedToken token, const proton::UpdateOperation& op) override {
        _delegate->handleUpdate(std::move(token), op);
    }
    void prepareRemove(proton::RemoveOperation& op) override { _delegate->prepareRemove(op); }
    void handleRemove(proton::FeedToken token, const proton::RemoveOperation& op) override {
        _delegate->handleRemove(std::move(token), op);
    }
    void prepareDeleteBucket(proton::DeleteBucketOperation& op) override {
        _delegate->prepareDeleteBucket(op);
    }
    bool isMoveStillValid(const proton::MoveOperation& op) const override {
        return _delegate->isMoveStillValid(op);
    }
    void prepareMove(proton::MoveOperation& op) override { _delegate->prepareMove(op); }
    void handleDeleteBucket(const proton::DeleteBucketOperation& op,
                            const DoneCallback& on_done) override {
        _delegate->handleDeleteBucket(op, on_done);
    }
    void handleMove(const proton::MoveOperation& op, const DoneCallback& on_done) override {
        _delegate->handleMove(op, on_done);
    }
    void heartBeat(search::SerialNum serial, const DoneCallback& on_done) override {
        _delegate->heartBeat(serial, on_done);
    }
    void forceCommit(const CommitParam& param, const DoneCallback& on_done) override {
        // A blocking failpoint must not hold FeedHandler's master thread: the
        // accepted feed operation may still need that thread to publish its
        // source document. Keep the real commit pending while allowing feed
        // processing to continue, then delegate it from the master thread on
        // release.
        constexpr std::string_view name =
            "matcher_publication_before_feedview_force_commit";
        const auto decision = _failpoints->defer_blocking_hit(name);
        if (decision == FailpointController::DeferredBlock::Deferred) {
            std::lock_guard<std::mutex> guard(_pending_mutex);
            _pending_commits.emplace_back(param, on_done);
            return;
        }
        if (decision == FailpointController::DeferredBlock::NotBlocking) {
            _failpoints->hit(name);
        }
        _delegate->forceCommit(param, on_done);
    }

    void release_pending_commits() {
        std::deque<std::pair<CommitParam, DoneCallback>> pending;
        {
            std::lock_guard<std::mutex> guard(_pending_mutex);
            pending.swap(_pending_commits);
        }
        for (const auto& [param, on_done] : pending) {
            _delegate->forceCommit(param, on_done);
        }
    }
    void handlePruneRemovedDocuments(const proton::PruneRemovedDocumentsOperation& op,
                                     const DoneCallback& on_done) override {
        _delegate->handlePruneRemovedDocuments(op, on_done);
    }
    void handleCompactLidSpace(const proton::CompactLidSpaceOperation& op,
                               const DoneCallback& on_done) override {
        _delegate->handleCompactLidSpace(op, on_done);
    }
};

static_assert(std::is_nothrow_constructible_v<PublicationGateFeedView,
                                              proton::IFeedView::SP,
                                              std::shared_ptr<FailpointController>>);

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
