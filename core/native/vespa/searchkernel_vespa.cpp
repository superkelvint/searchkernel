#include "searchkernel_vespa.h"
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
#include "searchkernel_vespa_test.h"
#endif
#include "file_descriptor.h"
#include "transport_owner.h"

#include <vespa/config-bucketspaces.h>
#include <vespa/config-stor-distribution.h>
#include <vespa/config/subscription/sourcespec.h>
#include <vespa/document/config/documenttypes_config_fwd.h>
#include <vespa/document/annotation/annotation.h>
#include <vespa/document/annotation/span.h>
#include <vespa/document/annotation/spanlist.h>
#include <vespa/document/annotation/spantree.h>
#include <vespa/document/datatype/datatype.h>
#include <vespa/document/datatype/documenttype.h>
#include <vespa/document/fieldvalue/document.h>
#include <vespa/document/fieldvalue/intfieldvalue.h>
#include <vespa/document/fieldvalue/longfieldvalue.h>
#include <vespa/document/fieldvalue/boolfieldvalue.h>
#include <vespa/document/fieldvalue/doublefieldvalue.h>
#include <vespa/document/fieldvalue/stringfieldvalue.h>
#include <vespa/document/repo/documenttyperepo.h>
#include <vespa/document/repo/fixedtyperepo.h>
#include <vespa/document/repo/newconfigbuilder.h>
#include <vespa/document/update/assignvalueupdate.h>
#include <vespa/document/update/clearvalueupdate.h>
#include <vespa/document/update/documentupdate.h>
#include <vespa/document/update/fieldupdate.h>
#include <vespa/fnet/transport.h>
#include <vespa/searchcore/proton/matching/querylimiter.h>
#include <vespa/searchcore/proton/matching/querynodes.h>
#include <vespa/searchcore/proton/metrics/metricswireservice.h>
#include <vespa/searchcore/proton/persistenceengine/i_resource_write_filter.h>
#include <vespa/searchcore/proton/persistenceengine/ipersistenceengineowner.h>
#include <vespa/searchcore/proton/persistenceengine/persistenceengine.h>
#include <vespa/searchcore/proton/persistenceengine/i_document_retriever.h>
#include <vespa/searchcore/proton/server/bootstrapconfig.h>
#include <vespa/searchcore/proton/server/configstore.h>
#include <vespa/searchcore/proton/server/documentdb.h>
#include <vespa/searchcore/proton/server/documentdbconfigmanager.h>
#include <vespa/searchcore/proton/server/fileconfigmanager.h>
#include <vespa/searchcore/proton/server/feedhandler.h>
#include <vespa/searchcore/proton/server/idocumentdbowner.h>
#include <vespa/searchcore/proton/server/idocumentsubdb.h>
#include <vespa/searchcore/proton/documentmetastore/i_document_meta_store_context.h>
#include <vespa/searchcore/proton/server/memoryconfigstore.h>
#include <vespa/searchcore/proton/server/persistencehandlerproxy.h>
#include <vespa/searchcore/proton/matching/sessionmanager.h>
#include <vespa/searchcore/proton/reference/document_db_reference_registry.h>
#include <vespa/searchcore/proton/server/maintenance_job_token_source.h>
#include <vespa/searchcore/proton/server/i_resource_usage_listener.h>
#include <vespa/searchcore/proton/server/i_resource_usage_notifier.h>
#include <vespa/searchcore/proton/server/i_shared_threading_service.h>
#include <vespa/searchlib/attribute/interlock.h>
#include <vespa/searchlib/common/i_document_id_provider.h>
#include <vespa/searchlib/engine/searchreply.h>
#include <vespa/searchlib/engine/searchrequest.h>
#include <vespa/searchlib/engine/docsumreply.h>
#include <vespa/searchlib/engine/docsumrequest.h>
#include <vespa/searchlib/aggregation/averageaggregationresult.h>
#include <vespa/searchlib/aggregation/countaggregationresult.h>
#include <vespa/searchlib/aggregation/group.h>
#include <vespa/searchlib/aggregation/grouping.h>
#include <vespa/searchlib/aggregation/groupinglevel.h>
#include <vespa/searchlib/aggregation/maxaggregationresult.h>
#include <vespa/searchlib/aggregation/minaggregationresult.h>
#include <vespa/searchlib/aggregation/sumaggregationresult.h>
#include <vespa/searchlib/expression/attributenode.h>
#include <vespa/searchlib/expression/floatresultnode.h>
#include <vespa/searchlib/expression/integerresultnode.h>
#include <vespa/searchlib/expression/stringresultnode.h>
#include <vespa/searchlib/common/fileheadercontext.h>
#include <vespa/searchlib/util/fileheadertk.h>
#include <vespa/searchlib/common/serialized_query_tree.h>
#include <vespa/searchlib/query/numeric_range_spec.h>
#include <vespa/searchlib/query/tree/querybuilder.h>
#include <vespa/searchlib/query/tree/range.h>
#include <vespa/searchlib/query/tree/stackdumpcreator.h>
#include <vespa/searchlib/transactionlog/translogserver.h>
#include <vespa/vdslib/distribution/distribution.h>
#include <vespa/vdslib/state/clusterstate.h>
#include <vespa/vespalib/net/socket_spec.h>
#include <vespa/vespalib/util/hw_info.h>
#include <vespa/vespalib/util/threadstackexecutor.h>
#include <vespa/vespalib/data/fileheader.h>
#include <vespa/vespalib/data/slime/slime.h>
#include <vespa/vespalib/objects/nbostream.h>
#include <vespa/vespalib/objects/nboserializer.h>
#include <vespa/vespalib/util/host_name.h>
#include <vespa/vespalib/util/invokeserviceimpl.h>
#include <vespa/vespalib/util/sequencedtaskexecutor.h>
#include <vespa/vespalib/util/shared_operation_throttler.h>
#include <vespa/persistence/dummyimpl/dummy_bucket_executor.h>

#include <algorithm>
#include <array>
#include <atomic>
#include <chrono>
#include <cerrno>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <condition_variable>
#include <cmath>
#include <filesystem>
#include <fcntl.h>
#include <fstream>
#include <iomanip>
#include <iterator>
#include <initializer_list>
#include <locale>
#include <limits>
#include <map>
#include <memory>
#include <mutex>
#include <new>
#include <optional>
#include <set>
#include <sstream>
#include <stdexcept>
#include <string>
#include <string_view>
#include <shared_mutex>
#include <tuple>
#include <utility>
#include <vector>

#include <netinet/in.h>
#include <sys/file.h>
#include <sys/socket.h>
#include <sys/types.h>
#include <unistd.h>

using namespace std::chrono_literals;
using document::Document;
using document::DocumentId;
using document::DocumentType;
using document::DocumentTypeRepo;
using proton::DocTypeName;
using proton::DocumentDB;
using proton::DocumentDBConfigHelper;
using proton::PersistenceEngine;
using proton::PersistenceHandlerProxy;
using vespa::config::search::core::ProtonConfigBuilder;
using search::engine::SearchReply;
using search::engine::SearchRequest;
using search::engine::DocsumReply;
using search::engine::DocsumRequest;
using search::aggregation::AggregationResult;
using search::aggregation::AverageAggregationResult;
using search::aggregation::CountAggregationResult;
using search::aggregation::Group;
using search::aggregation::Grouping;
using search::aggregation::GroupingLevel;
using search::aggregation::MaxAggregationResult;
using search::aggregation::MinAggregationResult;
using search::aggregation::SumAggregationResult;
using search::expression::AttributeNode;
using search::expression::FloatResultNode;
using search::expression::HoldString;
using search::expression::Int64ResultNode;
using search::expression::StringResultNode;
using search::SerializedQueryTree;
using search::query::QueryBuilder;
using VespaQueryBuilder = QueryBuilder<proton::matching::ProtonNodeTypes>;
using search::query::StackDumpCreator;
using search::query::Weight;
using search::transactionlog::TransLogServer;
using storage::spi::Bucket;
using storage::spi::BucketInfo;
using storage::spi::Timestamp;
using vespalib::HwInfo;
using vespalib::ThreadExecutor;
using vespa::config::content::core::BucketspacesConfig;
using vespa::config::search::core::ProtonConfig;
using cloud::config::filedistribution::FiledistributorrpcConfig;

namespace {

#ifdef SK_ENABLE_HARDENING_FAILPOINTS
class FailpointController {
    struct Entry {
        uint32_t trigger = 0;
        uint32_t action = 0;
        uint32_t hits = 0;
        bool blocked = false;
        bool armed = false;
    };

    mutable std::mutex _mutex;
    std::condition_variable _condition;
    std::map<std::string, Entry> _entries;

    static bool known(std::string_view name) noexcept {
        static constexpr std::array<std::string_view, 9> names = {
            "open_before_native_create", "open_after_native_create",
            "mutation_before_native_call", "mutation_after_native_call",
            "search_before_match", "search_after_match", "result_before_copy",
            "result_after_allocation", "destroy_before_native_release"};
        return std::find(names.begin(), names.end(), name) != names.end();
    }

public:
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
        _entries[name] = Entry{trigger, action, 0, false, true};
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

FailpointController OPEN_FAILPOINTS;

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

constexpr size_t MAX_DOCUMENT_ID_BYTES = 4096;
constexpr size_t MAX_SCHEMA_IDENTIFIER_BYTES = 256;
constexpr size_t MAX_RANK_PROFILE_BYTES = 256;
constexpr size_t MAX_TEXT_BYTES = 16 * 1024 * 1024;
constexpr size_t MAX_TEXT_TERM_BYTES = 1024 * 1024;
constexpr size_t MAX_TOTAL_TEXT_TERM_BYTES = 64 * 1024 * 1024;
constexpr size_t MAX_TEXT_TERMS = 65536;
constexpr size_t MAX_TEXT_OCCURRENCES = 65536;
constexpr uint32_t MAX_SEARCH_LIMIT = 1'000'000;
constexpr std::array<std::string_view, 18> RESERVED_FIELD_NAMES = {
    "index",          "index_url",      "summary",       "attribute", "select_input",
    "host",           "documentid",     "position",      "split_foreach", "tokenize",
    "if",             "else",           "switch",        "case",      "sddocname",
    "relevancy",      "rankfeatures",  "summaryfeatures"};

struct QueryFieldMetadata {
    std::string name;
    std::string type;
    bool index = false;
    bool attribute = false;
};

bool is_identifier(std::string_view value) noexcept {
    if (value.empty()) return false;
    const auto first = static_cast<unsigned char>(value.front());
    if (first != '_' && !(first >= 'A' && first <= 'Z') && !(first >= 'a' && first <= 'z')) {
        return false;
    }
    for (const auto byte : value.substr(1)) {
        const auto character = static_cast<unsigned char>(byte);
        if (character != '_' && !(character >= 'A' && character <= 'Z') &&
            !(character >= 'a' && character <= 'z') && !(character >= '0' && character <= '9')) {
            return false;
        }
    }
    return true;
}

bool is_slime_type(const vespalib::slime::Inspector &value, uint32_t type) noexcept {
    return value.valid() && value.type().getId() == type;
}

void validate_manifest_identifier(const std::string &value, const char *what) {
    if (value.size() > MAX_SCHEMA_IDENTIFIER_BYTES || !is_identifier(value)) {
        throw std::invalid_argument(std::string("generated schema ") + what +
                                    " is not a valid identifier");
    }
}

void validate_manifest_field_name(const std::string &value) {
    validate_manifest_identifier(value, "field name");
    if (std::find(RESERVED_FIELD_NAMES.begin(), RESERVED_FIELD_NAMES.end(), value) !=
        RESERVED_FIELD_NAMES.end()) {
        throw std::invalid_argument("generated schema field name is reserved");
    }
}

bool is_utf8_continuation(uint8_t byte) noexcept {
    return (byte & 0xc0) == 0x80;
}

bool is_valid_utf8(std::string_view value) noexcept {
    for (size_t index = 0; index < value.size();) {
        const auto byte = static_cast<uint8_t>(value[index]);
        if (byte <= 0x7f) {
            ++index;
            continue;
        }
        if (byte >= 0xc2 && byte <= 0xdf) {
            if (index + 1 >= value.size() ||
                !is_utf8_continuation(static_cast<uint8_t>(value[index + 1]))) {
                return false;
            }
            index += 2;
            continue;
        }
        if (byte == 0xe0) {
            if (index + 2 >= value.size() ||
                static_cast<uint8_t>(value[index + 1]) < 0xa0 ||
                static_cast<uint8_t>(value[index + 1]) > 0xbf ||
                !is_utf8_continuation(static_cast<uint8_t>(value[index + 2]))) {
                return false;
            }
            index += 3;
            continue;
        }
        if ((byte >= 0xe1 && byte <= 0xec) || (byte >= 0xee && byte <= 0xef)) {
            if (index + 2 >= value.size() ||
                !is_utf8_continuation(static_cast<uint8_t>(value[index + 1])) ||
                !is_utf8_continuation(static_cast<uint8_t>(value[index + 2]))) {
                return false;
            }
            index += 3;
            continue;
        }
        if (byte == 0xed) {
            if (index + 2 >= value.size() ||
                static_cast<uint8_t>(value[index + 1]) < 0x80 ||
                static_cast<uint8_t>(value[index + 1]) > 0x9f ||
                !is_utf8_continuation(static_cast<uint8_t>(value[index + 2]))) {
                return false;
            }
            index += 3;
            continue;
        }
        if (byte == 0xf0) {
            if (index + 3 >= value.size() ||
                static_cast<uint8_t>(value[index + 1]) < 0x90 ||
                static_cast<uint8_t>(value[index + 1]) > 0xbf ||
                !is_utf8_continuation(static_cast<uint8_t>(value[index + 2])) ||
                !is_utf8_continuation(static_cast<uint8_t>(value[index + 3]))) {
                return false;
            }
            index += 4;
            continue;
        }
        if (byte >= 0xf1 && byte <= 0xf3) {
            if (index + 3 >= value.size() ||
                !is_utf8_continuation(static_cast<uint8_t>(value[index + 1])) ||
                !is_utf8_continuation(static_cast<uint8_t>(value[index + 2])) ||
                !is_utf8_continuation(static_cast<uint8_t>(value[index + 3]))) {
                return false;
            }
            index += 4;
            continue;
        }
        if (byte == 0xf4) {
            if (index + 3 >= value.size() ||
                static_cast<uint8_t>(value[index + 1]) < 0x80 ||
                static_cast<uint8_t>(value[index + 1]) > 0x8f ||
                !is_utf8_continuation(static_cast<uint8_t>(value[index + 2])) ||
                !is_utf8_continuation(static_cast<uint8_t>(value[index + 3]))) {
                return false;
            }
            index += 4;
            continue;
        }
        return false;
    }
    return true;
}

bool is_utf8_boundary(std::string_view value, size_t offset) noexcept {
    return offset <= value.size() &&
           (offset == 0 || offset == value.size() ||
            !is_utf8_continuation(static_cast<uint8_t>(value[offset])));
}

size_t bounded_string_length(const char *value, size_t maximum) noexcept {
    for (size_t length = 0; length <= maximum; ++length) {
        if (value[length] == '\0') {
            return length;
        }
    }
    return maximum + 1;
}

void validate_string_size(const char *value, size_t maximum, const char *what) {
    if (value == nullptr) {
        throw std::invalid_argument(std::string(what) + " must not be null");
    }
    if (bounded_string_length(value, maximum) > maximum) {
        throw std::invalid_argument(std::string(what) + " is too long");
    }
}

void validate_document_id(const char *value) {
    validate_string_size(value, MAX_DOCUMENT_ID_BYTES, "document ID");
}

void validate_field_name(const char *value) {
    validate_string_size(value, MAX_SCHEMA_IDENTIFIER_BYTES, "field name");
}

void validate_rank_profile(const char *value) {
    validate_string_size(value, MAX_RANK_PROFILE_BYTES, "rank profile");
}

constexpr document::BucketSpace bucket_space() noexcept {
    return document::BucketSpace(1);
}

class EmbeddedWireService final : public proton::MetricsWireService {
public:
    void set_attributes(proton::AttributeMetrics &, std::vector<std::string>) override {}
    void set_index_fields(proton::IndexMetrics &, std::vector<std::string>) override {}
    void addRankProfile(proton::DocumentDBTaggedMetrics &, const std::string &, size_t) override {}
    void cleanRankProfiles(proton::DocumentDBTaggedMetrics &) override {}
};

class EmbeddedResourceUsageNotifier final : public proton::IResourceUsageNotifier {
public:
    void add_resource_usage_listener(proton::IResourceUsageListener *listener) override {
        listener->notify_resource_usage(proton::ResourceUsageState());
    }
    void remove_resource_usage_listener(proton::IResourceUsageListener *) override {}
};

class EmbeddedDBOwner final : public proton::IDocumentDBOwner {
    std::shared_ptr<proton::IDocumentDBReferenceRegistry> _registry =
        std::make_shared<proton::DocumentDBReferenceRegistry>();
    std::unique_ptr<proton::matching::SessionManager> _session_manager =
        std::make_unique<proton::matching::SessionManager>(10);
    std::shared_ptr<proton::MaintenanceJobTokenSource> _job_tokens =
        std::make_shared<proton::MaintenanceJobTokenSource>();
    std::shared_ptr<vespalib::SharedOperationThrottler> _replay_throttler =
        vespalib::SharedOperationThrottler::make_unlimited_throttler();

public:
    bool isInitializing() const override { return false; }
    uint32_t getDistributionKey() const override { return -1; }
    uint32_t getNumThreadsPerSearch() const override { return 1; }
    std::shared_ptr<proton::IDocumentDBReferenceRegistry> getDocumentDBReferenceRegistry() const override {
        return _registry;
    }
    proton::matching::SessionManager &session_manager() override { return *_session_manager; }
    std::shared_ptr<proton::MaintenanceJobTokenSource> get_lid_space_compaction_job_token_source() override {
        return _job_tokens;
    }
    std::shared_ptr<vespalib::SharedOperationThrottler> shared_replay_throttler() const override {
        return _replay_throttler;
    }
};

using EmbeddedTransportOwner = TransportOwner<FNET_Transport>;

class EmbeddedThreadingService final : public proton::ISharedThreadingService {
    vespalib::ThreadExecutor &_shared;
    std::unique_ptr<vespalib::ISequencedTaskExecutor> _field_writer =
        vespalib::SequencedTaskExecutor::create([](vespalib::Runnable &worker) {
            worker.run();
            return 1;
        }, 1);
    vespalib::InvokeServiceImpl _invoke_service{10ms};
    EmbeddedTransportOwner _transport;
    storage::spi::dummy::DummyBucketExecutor _bucket_executor{2};

public:
    explicit EmbeddedThreadingService(vespalib::ThreadExecutor &shared) : _shared(shared) { _transport.start(); }
    ThreadExecutor &shared() override { return _shared; }
    vespalib::ISequencedTaskExecutor &field_writer() override { return *_field_writer; }
    vespalib::InvokeService &invokeService() override { return _invoke_service; }
    FNET_Transport &transport() override { return _transport.transport(); }
    void shutdown() { _transport.shutdown(); }
    void shutdown_noexcept() noexcept { _transport.shutdown_noexcept(); }
    storage::spi::BucketExecutor &bucket_executor() override { return _bucket_executor; }
    const std::atomic<vespalib::steady_time> &nowRef() const override { return _invoke_service.nowRef(); }
};

class EmbeddedFileHeaderContext final : public search::common::FileHeaderContext {
    std::string _host_name = vespalib::HostName::get();
public:
    void addTags(vespalib::GenericHeader &header, const std::string &name) const override {
        using Tag = vespalib::GenericHeader::Tag;
        search::FileHeaderTk::addVersionTags(header);
        header.putTag(Tag("fileName", name));
        addCreateAndFreezeTime(header);
        header.putTag(Tag("hostName", _host_name));
    }
};

document::BucketId bucket_id_for(const document::DocumentId &id) {
    return id.getGlobalId().convertToBucketId();
}

storage::spi::Bucket bucket_for(const document::DocumentId &id) {
    return storage::spi::Bucket(document::Bucket(bucket_space(), bucket_id_for(id)));
}

std::mutex DATA_DIRECTORY_LOCK_MUTEX;
std::set<std::string> DATA_DIRECTORY_LOCKS;

class DataDirectoryLock {
    int _fd = -1;
    std::string _key;
    bool _registered = false;
    bool _had_existing_state = false;

    void unregister_path() noexcept {
        if (!_registered) return;
        try {
            std::lock_guard<std::mutex> guard(DATA_DIRECTORY_LOCK_MUTEX);
            DATA_DIRECTORY_LOCKS.erase(_key);
        } catch (...) {
        }
        _registered = false;
    }

public:
    explicit DataDirectoryLock(const std::string &data_dir) {
        const auto path = std::filesystem::path(data_dir);
        std::error_code filesystem_error;
        const bool existed = std::filesystem::exists(path, filesystem_error);
        if (filesystem_error) {
            throw std::runtime_error("could not inspect index data path '" + path.string() + "': " +
                                     filesystem_error.message());
        }
        if (existed && !std::filesystem::is_directory(path, filesystem_error)) {
            if (filesystem_error) {
                throw std::runtime_error("could not inspect index data path '" + path.string() + "': " +
                                         filesystem_error.message());
            }
            throw std::invalid_argument("engine data path is not a directory: " + path.string());
        }
        if (existed) {
            for (std::filesystem::directory_iterator entry(path, filesystem_error), end;
                 entry != end; entry.increment(filesystem_error)) {
                if (filesystem_error) {
                    throw std::runtime_error("could not inspect index data path '" + path.string() + "': " +
                                             filesystem_error.message());
                }
                const auto filename = entry->path().filename().string();
                const bool is_directory = entry->is_directory(filesystem_error);
                if (filesystem_error) {
                    throw std::runtime_error("could not inspect index data path '" + path.string() + "': " +
                                             filesystem_error.message());
                }
                // Files placed beside the index (for example a driver's redirected
                // JSON output) are not Vespa state.  Engine state is rooted in the
                // document-type and transaction-log directories.
                if (is_directory && filename != ".searchkernel.lock" &&
                    !filename.starts_with(".searchkernel-schema.tmp")) {
                    _had_existing_state = true;
                    break;
                }
            }
            if (filesystem_error) {
                throw std::runtime_error("could not inspect index data path '" + path.string() + "': " +
                                         filesystem_error.message());
            }
        }
        std::filesystem::create_directories(path);
        _key = std::filesystem::weakly_canonical(path).string();
        {
            std::lock_guard<std::mutex> guard(DATA_DIRECTORY_LOCK_MUTEX);
            if (!DATA_DIRECTORY_LOCKS.insert(_key).second) {
                throw std::invalid_argument("index is already open");
            }
            _registered = true;
        }

        try {
            const auto lock_path = path / ".searchkernel.lock";
            _fd = ::open(lock_path.c_str(), O_RDWR | O_CREAT, 0600);
            if (_fd < 0) {
                throw std::runtime_error("could not open index lock '" + lock_path.string() + "': " +
                                         std::strerror(errno));
            }
            if (::flock(_fd, LOCK_EX | LOCK_NB) != 0) {
                const auto error = errno;
                ::close(_fd);
                _fd = -1;
                if (error == EWOULDBLOCK || error == EAGAIN) {
                    throw std::invalid_argument("index is already open");
                }
                throw std::runtime_error("could not lock index '" + path.string() + "': " +
                                         std::strerror(error));
            }
        } catch (...) {
            unregister_path();
            throw;
        }
    }

    DataDirectoryLock(const DataDirectoryLock &) = delete;
    DataDirectoryLock &operator=(const DataDirectoryLock &) = delete;

    ~DataDirectoryLock() noexcept {
        if (_fd >= 0) {
            ::flock(_fd, LOCK_UN);
            ::close(_fd);
        }
        unregister_path();
    }

    bool had_existing_state() const noexcept { return _had_existing_state; }
};

class PortReservation {
    int _fd = -1;
    unsigned int _port = 0;

public:
    PortReservation() {
        _fd = ::socket(AF_INET, SOCK_STREAM, 0);
        if (_fd < 0) {
            throw std::runtime_error("could not create transaction log port reservation: " +
                                     std::string(std::strerror(errno)));
        }
        int reuse = 1;
        if (::setsockopt(_fd, SOL_SOCKET, SO_REUSEADDR, &reuse, sizeof(reuse)) != 0) {
            const auto error = errno;
            ::close(_fd);
            _fd = -1;
            throw std::runtime_error("could not configure transaction log port reservation: " +
                                     std::string(std::strerror(error)));
        }
        sockaddr_in address{};
        address.sin_family = AF_INET;
        address.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
        address.sin_port = htons(0);
        if (::bind(_fd, reinterpret_cast<const sockaddr *>(&address), sizeof(address)) != 0) {
            const auto error = errno;
            ::close(_fd);
            _fd = -1;
            throw std::runtime_error("could not reserve transaction log port: " +
                                     std::string(std::strerror(error)));
        }
        socklen_t length = sizeof(address);
        if (::getsockname(_fd, reinterpret_cast<sockaddr *>(&address), &length) != 0) {
            const auto error = errno;
            ::close(_fd);
            _fd = -1;
            throw std::runtime_error("could not read transaction log port: " +
                                     std::string(std::strerror(error)));
        }
        _port = ntohs(address.sin_port);
    }

    PortReservation(const PortReservation &) = delete;
    PortReservation &operator=(const PortReservation &) = delete;

    ~PortReservation() noexcept {
        if (_fd >= 0) {
            ::close(_fd);
        }
    }

    unsigned int port() const noexcept { return _port; }
};

void validate_raw_document_id(std::string_view id, std::string_view document_type) {
    if (!id.starts_with("id:")) {
        return;
    }
    const auto namespace_end = id.find(':', 3);
    if (namespace_end <= 3 || namespace_end + 1 >= id.size()) {
        throw std::invalid_argument("raw document ID is malformed");
    }
    const auto document_type_start = namespace_end + 1;
    const auto document_type_end = id.find("::", document_type_start);
    if (document_type_end == std::string_view::npos || document_type_end == document_type_start ||
        id.substr(document_type_start, document_type_end - document_type_start) != document_type) {
        throw std::invalid_argument("raw document ID belongs to a different document type");
    }
}

std::string internal_document_id(std::string_view id, std::string_view document_type) {
    validate_raw_document_id(id, document_type);
    if (id.starts_with("id:")) {
        return std::string(id);
    }
    return "id:searchkernel:" + std::string(document_type) + "::" + std::string(id);
}

std::string external_document_id(std::string_view id, std::string_view document_type) {
    const std::string prefix = "id:searchkernel:" + std::string(document_type) + "::";
    if (id.starts_with(prefix)) {
        return std::string(id.substr(prefix.size()));
    }
    return std::string(id);
}

class PersistenceOwner final : public proton::IPersistenceEngineOwner {
public:
    void setClusterState(document::BucketSpace,
                         const storage::spi::ClusterState &) override {}
};

class AcceptAllWrites final : public proton::IResourceWriteFilter {
public:
    bool acceptWriteOperation() const override { return true; }
    State getAcceptState() const override { return State(true, ""); }
};

storage::spi::ClusterState single_node_cluster_state() {
    using storage::lib::Distribution;
    using storage::lib::Node;
    using storage::lib::NodeState;
    using storage::lib::NodeType;
    using storage::lib::State;
    using vespa::config::content::StorDistributionConfigBuilder;
    using Group = StorDistributionConfigBuilder::Group;

    storage::lib::ClusterState state;
    state.setNodeState(Node(NodeType::STORAGE, 0),
                       NodeState(NodeType::STORAGE, State::UP, "embedded", 1.0));
    state.setClusterState(State::UP);

    StorDistributionConfigBuilder config;
    config.redundancy = 1;
    config.readyCopies = 1;
    config.group.emplace_back();
    Group &group = config.group.back();
    group.index = "invalid";
    group.name = "embedded";
    group.capacity = 1.0;
    group.partitions = "";
    group.nodes.emplace_back();
    group.nodes.back().index = 0;
    return storage::spi::ClusterState(state, 0, Distribution(config));
}

struct TextOccurrence {
    std::string term;
    uint32_t byte_start;
    uint32_t byte_length;
    uint32_t position;
};

struct TextDocument {
    std::string id;
    std::string field;
    std::string text;
    std::vector<TextOccurrence> occurrences;
    // The public API is a one-transfer structure-of-arrays builder. Keeping
    // this state in the opaque object makes that contract enforceable instead
    // of relying on every caller to make exactly one call.
    bool occurrences_added = false;
};

struct TextUpdate : TextDocument {};

struct TextRequest {
    uint32_t kind;
    std::string field;
    std::vector<std::string> terms;
    std::vector<uint32_t> positions;
    std::string rank_profile;
    // Terms are supplied by one structure-of-arrays transfer only. This
    // prevents repeated calls from bypassing the aggregate resource limits.
    bool terms_added = false;
};

struct TextHit {
    std::string id;
    double score;
};

struct TextResult {
    std::vector<TextHit> hits;
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    const FailpointController *failpoints = nullptr;
#endif
};

struct OwnedValue {
    uint32_t kind = 0;
    std::string string;
    int64_t integer = 0;
    double floating = 0.0;
    bool boolean = false;
};

struct DocumentFieldState {
    std::string name;
    OwnedValue value;
    std::vector<TextOccurrence> occurrences;
};

struct ValueDocument {
    std::string id;
    std::string field;
    int64_t value = 0;
    std::vector<DocumentFieldState> fields;
};

struct ValueUpdate {
    std::string id;
    std::string field;
    int64_t value = 0;
    std::vector<std::pair<std::string, std::optional<OwnedValue>>> fields;
    std::vector<std::pair<std::string, std::vector<TextOccurrence>>> annotated_fields;
};

void append_text_occurrences(TextDocument &, const uint8_t *, uint32_t,
                             const uint32_t *, const uint32_t *, const uint32_t *,
                             uint32_t);

struct AttributeDocumentState {
    std::string id;
    std::vector<std::pair<std::string, OwnedValue>> fields;
};

struct AttributeUpdateState {
    std::string id;
    std::vector<std::pair<std::string, OwnedValue>> assignments;
};

struct LocalSearchRequestState {
    std::string rank_profile;
    uint32_t limit = 10;
    uint32_t offset = 0;
    uint32_t summary_kind = SK_SUMMARY_NONE;
    std::vector<std::pair<std::string, uint32_t>> sort;
    std::vector<std::string> summary_fields;
};

struct LocalHitState {
    std::string id;
    double score = 0.0;
    std::vector<std::pair<std::string, OwnedValue>> summary;
};

struct LocalSearchResultState {
    uint64_t total_hit_count = 0;
    std::vector<LocalHitState> hits;
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    const FailpointController *failpoints = nullptr;
#endif
};

struct LocalAggregationSpec {
    uint32_t kind = 0;
    std::string field;
};

struct LocalGroupingRequestState {
    std::string group_field;
    std::vector<LocalAggregationSpec> aggregations;
};

struct LocalGroupState {
    OwnedValue key;
    uint64_t count = 0;
    std::vector<OwnedValue> aggregations;
};

struct LocalGroupingResultState {
    std::vector<LocalGroupState> groups;
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    const FailpointController *failpoints = nullptr;
#endif
};

struct NativeQueryState {
    std::vector<QueryFieldMetadata> fields;
    mutable VespaQueryBuilder builder;
    uint32_t next_term_id = 1;

    int32_t allocate_term_id() {
        if (next_term_id > static_cast<uint32_t>(std::numeric_limits<int32_t>::max())) {
            throw std::invalid_argument("query term ID cannot be represented by Vespa");
        }
        return static_cast<int32_t>(next_term_id++);
    }
};

std::string query_stack_dump(int64_t value, const std::string &field) {
    VespaQueryBuilder builder;
    builder.addNumberTerm(std::to_string(value), field, 1, Weight(1));
    auto tree = builder.build();
    if (!tree) {
        throw std::runtime_error("could not build number query");
    }
    return StackDumpCreator::create(*tree);
}

class EmbeddedEngine {
    struct ManifestField {
        std::string name;
        std::string type;
        bool index = false;
        bool attribute = false;
        bool summary = false;
    };
    struct SchemaManifest {
        std::string document_type;
        std::vector<ManifestField> fields;
    };

    SchemaManifest _manifest;
    DataDirectoryLock _data_lock;
    std::string _schema_identity;
    std::string _document_type;
    std::shared_ptr<const DocumenttypesConfig> _document_types;
    std::shared_ptr<DocumentTypeRepo> _repo;
    const DocumentType *_doc_type;
    EmbeddedWireService _wire_service;
    EmbeddedDBOwner _db_owner;
    vespalib::ThreadStackExecutor _summary_executor;
    EmbeddedThreadingService _shared_service;
    HwInfo _hw_info;
    EmbeddedFileHeaderContext _file_header_context;
    std::string _tls_dir;
    PortReservation _tls_port_reservation;
    const unsigned int _tls_port;
    TransLogServer _tls;
    proton::matching::QueryLimiter _query_limiter;
    DocumentDB::SP _db;
    // Non-owning while _db owns the config store. DocumentDB performs the
    // normal snapshot pruning before it closes the store.
    proton::ConfigStore *_config_store = nullptr;
    PersistenceOwner _persistence_owner;
    AcceptAllWrites _write_filter;
    EmbeddedResourceUsageNotifier _resource_usage_notifier;
    PersistenceEngine _persistence;
    std::shared_ptr<PersistenceHandlerProxy> _handler;
    std::vector<uint64_t> _prepared_buckets;
    std::filesystem::path _runtime_config_dir;
    std::atomic<uint64_t> _next_timestamp{1};
    bool _initialized = false;
    enum class State { Open, Closing, Closed, Failed };
    std::atomic<State> _state{State::Open};
    // The lifecycle gate protects the lifetime of _db and the other native
    // resources. Shared holders are deliberately allowed to overlap; Vespa
    // owns operation-level concurrency. Close and cleanup take the exclusive
    // holder and therefore cannot race an active operation.
    mutable std::shared_mutex _lifecycle_mutex;
    // This container is owned by this adapter rather than Vespa. Hold this
    // narrow lock only while preparing a previously unseen bucket; native feed
    // calls themselves remain concurrent and are serialized by Vespa as needed.
    mutable std::mutex _bucket_mutex;
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    mutable FailpointController _failpoints;
#endif

    static std::string read_identity_file(const std::filesystem::path &path) {
        std::ifstream input(path, std::ios::binary);
        if (!input) {
            throw std::runtime_error("could not read generated configuration: " + path.string());
        }
        const std::string contents((std::istreambuf_iterator<char>(input)),
                                   std::istreambuf_iterator<char>());
        if (!input.good() && !input.eof()) {
            throw std::runtime_error("could not read generated configuration: " + path.string());
        }
        return contents;
    }

    static std::string schema_identity(const SchemaManifest &manifest,
                                       const std::filesystem::path &config_dir) {
        std::string identity = "version=2\ndocument_type=" + manifest.document_type + "\n";
        for (const auto &field : manifest.fields) {
            identity += "field=" + field.name + "\t" + field.type + "\t" +
                        (field.index ? "1" : "0") + "\t" +
                        (field.attribute ? "1" : "0") + "\t" +
                        (field.summary ? "1" : "0") + "\n";
        }
        for (const char *name : {"indexschema.cfg", "attributes.cfg", "summary.cfg",
                                 "rank-profiles.cfg", "juniperrc.cfg"}) {
            const auto contents = read_identity_file(config_dir / name);
            identity += "file=" + std::string(name) + "\nlength=" +
                        std::to_string(contents.size()) + "\n";
            identity += contents;
            identity.push_back('\n');
        }
        return identity;
    }

    static std::string validate_or_persist_schema_identity(
        const std::string &data_dir, const std::string &config_dir,
        const SchemaManifest &manifest, bool had_existing_state) {
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        TimingProbe::Scope timing("schema_identity");
#endif
        const auto path = std::filesystem::path(data_dir) / ".searchkernel-schema";
        const auto expected = schema_identity(manifest, config_dir);
        if (std::filesystem::exists(path)) {
            if (!std::filesystem::is_regular_file(path)) {
                throw std::invalid_argument("index schema identity is not a regular file");
            }
            const auto actual = read_identity_file(path);
            if (actual != expected) {
                throw std::invalid_argument(
                    "index data was created with a different generated schema");
            }
            return expected;
        }

        // M5 persisted indexes predate Search Kernel's private schema marker.
        // They remain valid legacy Vespa state; once a marker exists, however,
        // it is always compared above before any engine initialization.
        if (had_existing_state) {
            const auto data_directory = std::filesystem::path(data_dir);
            std::error_code filesystem_error;
            for (std::filesystem::directory_iterator entry(data_directory, filesystem_error), end;
                 entry != end; entry.increment(filesystem_error)) {
                if (filesystem_error) {
                    throw std::runtime_error("could not inspect index data path '" +
                                             data_directory.string() + "': " +
                                             filesystem_error.message());
                }
                if (!entry->is_directory(filesystem_error)) {
                    if (filesystem_error) {
                        throw std::runtime_error("could not inspect index data path '" +
                                                 data_directory.string() + "': " +
                                                 filesystem_error.message());
                    }
                    continue;
                }
                const auto name = entry->path().filename().string();
                if (name != "tmp" && name != manifest.document_type) {
                    throw std::invalid_argument(
                        "index data was created with a different generated schema");
                }
            }
            return expected;
        }
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        TimingProbe::Scope persistence_timing("schema_persistence");
#endif
        auto temporary_path = path.string() + ".tmp.XXXXXX";
        FileDescriptor fd(::mkstemp(temporary_path.data()));
        if (fd.get() < 0) {
            throw std::runtime_error("could not create index schema identity: " + temporary_path);
        }
        bool installed = false;
        try {
            size_t offset = 0;
            while (offset < expected.size()) {
                const auto written = ::write(fd.get(), expected.data() + offset, expected.size() - offset);
                if (written < 0 && errno == EINTR) continue;
                if (written <= 0) {
                    throw std::runtime_error("could not write index schema identity: " + path.string());
                }
                offset += static_cast<size_t>(written);
            }
            if (::fsync(fd.get()) != 0) {
                throw std::runtime_error("could not flush index schema identity: " + path.string());
            }
            const int close_result = fd.close();
            if (close_result != 0) {
                throw std::runtime_error("could not close index schema identity: " + path.string());
            }
            if (::rename(temporary_path.c_str(), path.c_str()) != 0) {
                throw std::runtime_error("could not install index schema identity: " + path.string());
            }
            installed = true;
            const auto directory = std::filesystem::path(data_dir);
            FileDescriptor directory_fd(::open(directory.c_str(), O_RDONLY | O_DIRECTORY));
            if (directory_fd.get() < 0) {
                throw std::runtime_error("could not open index directory for flushing: " + directory.string());
            }
            const int sync_result = ::fsync(directory_fd.get());
            const int directory_close_result = directory_fd.close();
            if (sync_result != 0 || directory_close_result != 0) {
                throw std::runtime_error("could not flush index directory: " + directory.string());
            }
        } catch (...) {
            (void)fd.close();
            if (!installed) {
                ::unlink(temporary_path.c_str());
            }
            throw;
        }
        return expected;
    }

    void require_open() const {
        if (_state.load(std::memory_order_acquire) != State::Open) {
            throw std::invalid_argument("engine is not open");
        }
    }

    const ManifestField &require_manifest_field(const std::string &name) const {
        const auto field = std::find_if(
            _manifest.fields.begin(), _manifest.fields.end(),
            [&name](const auto &candidate) { return candidate.name == name; });
        if (field == _manifest.fields.end()) {
            throw std::invalid_argument("unknown generated schema field: " + name);
        }
        return *field;
    }

    void require_integer_field(const std::string &name) const {
        const auto &field = require_manifest_field(name);
        if (field.type != "integer" || !field.attribute) {
            throw std::invalid_argument("integer operation requires an integer attribute field: " + name);
        }
    }

    void require_text_field(const std::string &name) const {
        const auto &field = require_manifest_field(name);
        if (field.type != "text" || !field.index) {
            throw std::invalid_argument("text operation requires an indexed text field: " + name);
        }
    }

    const ManifestField &require_attribute_field(const std::string &name,
                                                 std::initializer_list<std::string_view> types = {}) const {
        const auto &field = require_manifest_field(name);
        if (!field.attribute) {
            throw std::invalid_argument("field is not an attribute: " + name);
        }
        if (types.size() != 0 && std::find(types.begin(), types.end(), field.type) == types.end()) {
            throw std::invalid_argument("unsupported attribute field type: " + name);
        }
        return field;
    }

    std::unique_ptr<document::FieldValue> make_attribute_value(
        const std::string &field_name, const OwnedValue &value) const {
        const auto &field = require_attribute_field(field_name);
        const auto &vespa_field = _doc_type->getField(field_name);
        if (field.type == "string") {
            if (value.kind != SK_VALUE_STRING) throw std::invalid_argument("wrong value type for attribute: " + field_name);
            return std::make_unique<document::StringFieldValue>(value.string);
        }
        if (field.type == "integer") {
            if (value.kind != SK_VALUE_INTEGER) {
                throw std::invalid_argument("wrong value type for attribute: " + field_name);
            }
            return std::make_unique<document::LongFieldValue>(value.integer);
        }
        if (field.type == "float") {
            if (value.kind != SK_VALUE_FLOAT || !std::isfinite(value.floating)) throw std::invalid_argument("wrong value type for attribute: " + field_name);
            return std::make_unique<document::DoubleFieldValue>(value.floating);
        }
        if (field.type == "boolean") {
            if (value.kind != SK_VALUE_BOOLEAN) throw std::invalid_argument("wrong value type for attribute: " + field_name);
            return std::make_unique<document::BoolFieldValue>(value.boolean);
        }
        (void)vespa_field;
        throw std::invalid_argument("unsupported attribute field type: " + field_name);
    }

    std::shared_ptr<Document> make_attribute_document(const AttributeDocumentState &source) const {
        if (source.fields.empty()) throw std::invalid_argument("attribute document must not be empty");
        const DocumentId id(internal_document_id(source.id, _document_type));
        auto document = std::make_shared<Document>(*_repo, *_doc_type, id);
        for (const auto &[name, value] : source.fields) {
            require_manifest_field(name);
            document->setFieldValue(_doc_type->getField(name), make_attribute_value(name, value));
        }
        return document;
    }

    std::unique_ptr<document::FieldValue> make_document_value(
        const std::string &field_name, const OwnedValue &value,
        const std::vector<TextOccurrence> *occurrences = nullptr) const {
        const auto &field = require_manifest_field(field_name);
        if (field.type == "text" || field.type == "string") {
            if (value.kind != SK_VALUE_STRING) {
                throw std::invalid_argument("wrong string value type for field: " + field_name);
            }
            if (field.type == "text" && occurrences) {
                TextDocument analyzed;
                analyzed.text = value.string;
                analyzed.occurrences = *occurrences;
                return make_text_value(analyzed);
            }
            return std::make_unique<document::StringFieldValue>(value.string);
        }
        if (field.type == "integer") {
            if (value.kind != SK_VALUE_INTEGER) {
                throw std::invalid_argument("wrong integer value type for field: " + field_name);
            }
            return std::make_unique<document::LongFieldValue>(value.integer);
        }
        if (field.type == "float") {
            if (value.kind != SK_VALUE_FLOAT || !std::isfinite(value.floating)) {
                throw std::invalid_argument("wrong float value type for field: " + field_name);
            }
            return std::make_unique<document::DoubleFieldValue>(value.floating);
        }
        if (field.type == "boolean") {
            if (value.kind != SK_VALUE_BOOLEAN) {
                throw std::invalid_argument("wrong boolean value type for field: " + field_name);
            }
            return std::make_unique<document::BoolFieldValue>(value.boolean);
        }
        throw std::invalid_argument("unsupported document field type: " + field_name);
    }

    std::shared_ptr<Document> make_value_document(const ValueDocument &source) const {
        if (source.fields.empty()) throw std::invalid_argument("document must not be empty");
        const DocumentId id(internal_document_id(source.id, _document_type));
        auto document = std::make_shared<Document>(*_repo, *_doc_type, id);
        for (const auto &entry : source.fields) {
            const auto *occurrences = entry.occurrences.empty() ? nullptr : &entry.occurrences;
            document->setFieldValue(_doc_type->getField(entry.name),
                                    make_document_value(entry.name, entry.value, occurrences));
        }
        return document;
    }

    std::shared_ptr<document::DocumentUpdate>
    make_value_update(const ValueUpdate &source) const {
        if (source.fields.empty()) throw std::invalid_argument("document update must not be empty");
        const DocumentId id(internal_document_id(source.id, _document_type));
        auto update = std::make_shared<document::DocumentUpdate>(*_repo, *_doc_type, id);
        for (const auto &[name, value] : source.fields) {
            const auto &field = _doc_type->getField(name);
            document::FieldUpdate field_update(field);
            if (value) {
                const auto annotated = std::find_if(
                    source.annotated_fields.begin(), source.annotated_fields.end(),
                    [&name](const auto &entry) { return entry.first == name; });
                const auto *occurrences = annotated == source.annotated_fields.end()
                                               ? nullptr
                                               : &annotated->second;
                field_update.addUpdate(std::make_unique<document::AssignValueUpdate>(
                    make_document_value(name, *value, occurrences)));
            } else {
                field_update.addUpdate(std::make_unique<document::ClearValueUpdate>());
            }
            update->addUpdate(std::move(field_update));
        }
        return update;
    }

    ValueDocument get_value_document(const std::string &external_id) const {
        std::shared_lock<std::shared_mutex> lifecycle_lock(_lifecycle_mutex);
        require_open();
        validate_document_id(external_id.c_str());
        const DocumentId id(internal_document_id(external_id, _document_type));
        auto ready_sub_db = _db->getReadySubDB();
        if (!ready_sub_db) throw std::runtime_error("Vespa DocumentDB has no ready sub database");
        auto meta_store_guard = ready_sub_db->getDocumentMetaStoreContext().getReadGuard();
        search::IDocumentMetaStore::DocId lid = 0;
        if (!meta_store_guard->get().getLid(id.getGlobalId(), lid)) {
            return {external_id, {}, 0, {}};
        }
        auto retrievers = _db->getDocumentRetrievers(storage::spi::ReadConsistency::STRONG);
        if (!retrievers || retrievers->empty()) {
            throw std::runtime_error("Vespa DocumentDB has no document retriever");
        }
        auto document = (*retrievers)[0]->getDocument(lid, id);
        if (!document) return {external_id, {}, 0, {}};
        ValueDocument result;
        result.id = external_id;
        for (auto iterator = document->begin(); iterator != document->end(); ++iterator) {
            const auto &field = *iterator;
            auto value = document->getValue(field);
            if (!value) continue;
            OwnedValue owned;
            switch (value->type()) {
            case document::FieldValue::Type::STRING:
                owned.kind = SK_VALUE_STRING;
                owned.string = value->getAsString();
                break;
            case document::FieldValue::Type::LONG:
                owned.kind = SK_VALUE_INTEGER;
                owned.integer = value->getAsLong();
                break;
            case document::FieldValue::Type::DOUBLE:
                owned.kind = SK_VALUE_FLOAT;
                owned.floating = value->getAsDouble();
                break;
            case document::FieldValue::Type::BOOL:
                owned.kind = SK_VALUE_BOOLEAN;
                owned.boolean = value->getAsLong() != 0;
                break;
            default:
                throw std::invalid_argument("unsupported retrieved document field type: " + field.getName());
            }
            result.fields.push_back({field.getName(), std::move(owned), {}});
        }
        return result;
    }

    std::shared_ptr<document::DocumentUpdate> make_attribute_update(const AttributeUpdateState &source) const {
        if (source.assignments.empty()) throw std::invalid_argument("attribute update must not be empty");
        const DocumentId id(internal_document_id(source.id, _document_type));
        auto update = std::make_shared<document::DocumentUpdate>(*_repo, *_doc_type, id);
        for (const auto &[name, value] : source.assignments) {
            require_manifest_field(name);
            const auto &field = _doc_type->getField(name);
            document::FieldUpdate field_update(field);
            field_update.addUpdate(std::make_unique<document::AssignValueUpdate>(make_attribute_value(name, value)));
            update->addUpdate(std::move(field_update));
        }
        return update;
    }

    std::vector<TextHit> match_tree(std::unique_ptr<search::query::Node> tree,
                                    const std::string &rank_profile,
                                    const std::string &source) const {
        if (!tree) {
            throw std::invalid_argument("could not build query tree");
        }
        if (source != "*" && source != _document_type) {
            throw std::invalid_argument("unsupported query source: " + source);
        }
        validate_rank_profile(rank_profile.c_str());
        if (rank_profile.empty()) {
            throw std::invalid_argument("query rank profile must not be empty");
        }
        auto request = std::make_shared<SearchRequest>();
        request->setTimeout(60s);
        request->setSerializedQueryTree(SerializedQueryTree::fromStackDump(
            StackDumpCreator::create(*tree)));
        request->ranking = rank_profile;
        request->maxhits = 100;
        failpoint("search_before_match");
        auto reply = _db->match(*request, vespalib::ThreadBundle::trivial());
        failpoint("search_after_match");
        if (!reply) {
            throw std::runtime_error("Vespa matcher returned no reply");
        }

        auto *ready_sub_db = _db->getReadySubDB();
        if (!ready_sub_db) {
            throw std::runtime_error("Vespa DocumentDB has no ready sub database");
        }
        auto meta_store_guard = ready_sub_db->getDocumentMetaStoreContext().getReadGuard();
        const auto *document_id_provider =
            dynamic_cast<const search::IDocumentIdProvider *>(&meta_store_guard->get());
        if (!document_id_provider) {
            throw std::runtime_error("Vespa DocumentDB does not provide document IDs");
        }
        std::vector<TextHit> hits;
        hits.reserve(reply->hits.size());
        for (const auto &hit : reply->hits) {
            search::IDocumentMetaStore::DocId lid = 0;
            if (meta_store_guard->get().getLid(hit.gid, lid)) {
                auto id = document_id_provider->get_document_id_string_view(lid);
                if (!id.empty()) {
                    hits.push_back({external_document_id(id, _document_type), hit.metric});
                }
            }
        }
        return hits;
    }

    uint64_t next_timestamp() {
        auto timestamp = _next_timestamp.load(std::memory_order_relaxed);
        for (;;) {
            if (timestamp == std::numeric_limits<uint64_t>::max()) {
                throw std::runtime_error("mutation timestamp space exhausted");
            }
            if (_next_timestamp.compare_exchange_weak(
                    timestamp, timestamp + 1, std::memory_order_relaxed,
                    std::memory_order_relaxed)) {
                return timestamp;
            }
        }
    }

    static std::map<std::string, std::string> read_config(const std::filesystem::path &path) {
        std::ifstream input(path);
        if (!input) {
            return {};
        }
        std::map<std::string, std::string> values;
        std::string line;
        while (std::getline(input, line)) {
            const auto first = line.find_first_not_of(" \t");
            if (first == std::string::npos || line[first] == '#') {
                continue;
            }
            const auto separator = line.find_first_of(" \t", first);
            if (separator == std::string::npos) {
                values[line.substr(first)] = "";
                continue;
            }
            const auto key = line.substr(first, separator - first);
            const auto value_start = line.find_first_not_of(" \t", separator);
            if (value_start == std::string::npos) {
                continue;
            }
            auto value = line.substr(value_start);
            if (value.size() >= 2 && value.front() == '"' && value.back() == '"') {
                value = value.substr(1, value.size() - 2);
            }
            if (!values.emplace(key, std::move(value)).second) {
                throw std::invalid_argument("inconsistent generated configuration");
            }
        }
        return values;
    }

    static size_t config_section_count(const std::map<std::string, std::string> &values,
                                       const char *section) {
        const std::string prefix = std::string(section) + "[";
        const auto key = std::find_if(values.begin(), values.end(), [&](const auto &entry) {
            if (!entry.first.starts_with(prefix) || !entry.first.ends_with("]")) {
                return false;
            }
            return entry.first.find(']', prefix.size()) == entry.first.size() - 1;
        });
        if (key == values.end()) {
            throw std::invalid_argument("inconsistent generated configuration");
        }
        const auto digits = key->first.substr(prefix.size(), key->first.size() - prefix.size() - 1);
        try {
            size_t consumed = 0;
            const auto count = std::stoull(digits, &consumed);
            if (consumed != digits.size()) {
                throw std::invalid_argument("invalid count");
            }
            return static_cast<size_t>(count);
        } catch (const std::exception &) {
            throw std::invalid_argument("inconsistent generated configuration");
        }
    }

    static std::string config_value(const std::map<std::string, std::string> &values,
                                    const std::string &key) {
        const auto value = values.find(key);
        if (value == values.end()) {
            throw std::invalid_argument("inconsistent generated configuration");
        }
        return value->second;
    }

    static void require_config_value(const std::map<std::string, std::string> &values,
                                     const std::string &key, std::string_view expected) {
        if (config_value(values, key) != expected) {
            throw std::invalid_argument("inconsistent generated configuration");
        }
    }

    static std::string manifest_attribute_type(const std::string &type) {
        if (type == "text" || type == "string") return "STRING";
        if (type == "integer") return "INT64";
        if (type == "float") return "DOUBLE";
        if (type == "boolean") return "BOOL";
        throw std::invalid_argument("inconsistent generated configuration");
    }

    static bool attribute_type_matches(const std::string &manifest_type,
                                       const std::string &config_type) {
        // Older checked-in embedded CRUD fixtures use Vespa's INT32
        // attribute spelling for the integer field.  The generated M6+
        // configuration uses INT64; both are valid Vespa representations of
        // the legacy integer fixture and must remain readable.
        return config_type == manifest_attribute_type(manifest_type) ||
               (manifest_type == "integer" && config_type == "INT32");
    }

    static std::string manifest_summary_type(const std::string &type) {
        if (type == "text" || type == "string") return "longstring";
        if (type == "integer") return "integer";
        if (type == "float") return "double";
        if (type == "boolean") return "bool";
        throw std::invalid_argument("inconsistent generated configuration");
    }

    static void validate_index_configuration(const SchemaManifest &manifest,
                                             const std::filesystem::path &config_path) {
        const auto values = read_config(config_path / "indexschema.cfg");
        const auto expected = std::count_if(manifest.fields.begin(), manifest.fields.end(),
                                            [](const auto &field) { return field.index; });
        if (config_section_count(values, "indexfield") != static_cast<size_t>(expected)) {
            throw std::invalid_argument("inconsistent generated configuration");
        }
        size_t index = 0;
        for (const auto &field : manifest.fields) {
            if (!field.index) continue;
            const auto prefix = "indexfield[" + std::to_string(index) + "].";
            if (field.type != "text") {
                throw std::invalid_argument("inconsistent generated configuration");
            }
            require_config_value(values, prefix + "name", field.name);
            require_config_value(values, prefix + "datatype", "STRING");
            require_config_value(values, prefix + "collectiontype", "SINGLE");
            require_config_value(values, prefix + "prefix", "false");
            require_config_value(values, prefix + "phrases", "false");
            require_config_value(values, prefix + "positions", "true");
            require_config_value(values, prefix + "averageelementlen", "512");
            const auto interleaved = config_value(values, prefix + "interleavedfeatures");
            const bool legacy_text_only = std::none_of(
                manifest.fields.begin(), manifest.fields.end(),
                [](const auto &candidate) { return candidate.attribute; });
            if (interleaved != "false" && !(legacy_text_only && interleaved == "true")) {
                throw std::invalid_argument("inconsistent generated configuration");
            }
            ++index;
        }
    }

    static void validate_attribute_configuration(const SchemaManifest &manifest,
                                                 const std::filesystem::path &config_path) {
        const auto values = read_config(config_path / "attributes.cfg");
        const auto expected = std::count_if(manifest.fields.begin(), manifest.fields.end(),
                                            [](const auto &field) { return field.attribute; });
        if (config_section_count(values, "attribute") != static_cast<size_t>(expected)) {
            throw std::invalid_argument("inconsistent generated configuration");
        }
        size_t index = 0;
        for (const auto &field : manifest.fields) {
            if (!field.attribute) continue;
            const auto prefix = "attribute[" + std::to_string(index) + "].";
            require_config_value(values, prefix + "name", field.name);
            if (!attribute_type_matches(field.type, config_value(values, prefix + "datatype"))) {
                throw std::invalid_argument("inconsistent generated configuration");
            }
            require_config_value(values, prefix + "collectiontype", "SINGLE");
            ++index;
        }
    }

    static void validate_summary_configuration(const SchemaManifest &manifest,
                                               const std::filesystem::path &config_path) {
        const auto values = read_config(config_path / "summary.cfg");
        require_config_value(values, "defaultsummaryid", "1");
        if (config_section_count(values, "classes") != 1) {
            throw std::invalid_argument("inconsistent generated configuration");
        }
        require_config_value(values, "classes[0].id", "1");
        require_config_value(values, "classes[0].name", "default");
        const auto count = config_section_count(values, "classes[0].fields");
        const std::vector<std::tuple<std::string, std::string, std::string, std::string>> implicit = {
            {"documentid", "longstring", "documentid", ""},
            {"rankfeatures", "featuredata", "rankfeatures", ""},
            {"summaryfeatures", "featuredata", "summaryfeatures", ""}};
        size_t index = 0;
        for (const auto &[name, type, command, source] : implicit) {
            const auto prefix = "classes[0].fields[" + std::to_string(index++) + "].";
            if (config_value(values, prefix + "name") != name ||
                config_value(values, prefix + "type") != type ||
                config_value(values, prefix + "command") != command ||
                config_value(values, prefix + "source") != source) {
                throw std::invalid_argument("inconsistent generated configuration");
            }
        }
        for (const auto &field : manifest.fields) {
            if (!field.summary) continue;
            const auto prefix = "classes[0].fields[" + std::to_string(index++) + "].";
            const auto command = field.attribute ? "attribute" : "";
            const auto source = field.attribute ? field.name : "";
            if (config_value(values, prefix + "name") != field.name ||
                config_value(values, prefix + "type") != manifest_summary_type(field.type) ||
                config_value(values, prefix + "command") != command ||
                config_value(values, prefix + "source") != source) {
                throw std::invalid_argument("inconsistent generated configuration");
            }
        }
        if (count != index) {
            throw std::invalid_argument("inconsistent generated configuration");
        }
    }

    static void validate_rank_profile_configuration(const SchemaManifest &manifest,
                                                    const std::filesystem::path &config_path) {
        const auto values = read_config(config_path / "rank-profiles.cfg");
        const auto text_field = std::find_if(
            manifest.fields.begin(), manifest.fields.end(),
            [](const auto &field) { return field.type == "text"; });
        const size_t profile_count = config_section_count(values, "rankprofile");
        const size_t expected_profiles = text_field == manifest.fields.end() ? 1 : 2;
        if (profile_count != expected_profiles && !(text_field == manifest.fields.end() && profile_count == 2)) {
            throw std::invalid_argument("inconsistent generated configuration");
        }
        require_config_value(values, "rankprofile[0].name", "default");
        if (text_field != manifest.fields.end()) {
            require_config_value(values, "rankprofile[1].name", "bm25");
            if (config_section_count(values, "rankprofile[1].fef.property") != 1) {
                throw std::invalid_argument("inconsistent generated configuration");
            }
            require_config_value(values, "rankprofile[1].fef.property[0].name",
                                 "vespa.rank.firstphase");
            require_config_value(
                values, "rankprofile[1].fef.property[0].value",
                "bm25(" + text_field->name + ")");
        } else if (profile_count == 2) {
            const auto price = std::find_if(
                manifest.fields.begin(), manifest.fields.end(),
                [](const auto &field) { return field.name == "price" && field.type == "float" && field.attribute; });
            if (price == manifest.fields.end()) {
                throw std::invalid_argument("attribute_price rank profile requires a float price attribute");
            }
            require_config_value(values, "rankprofile[1].name", "attribute_price");
            if (config_section_count(values, "rankprofile[1].fef.property") != 1) {
                throw std::invalid_argument("inconsistent generated configuration");
            }
            require_config_value(values, "rankprofile[1].fef.property[0].name",
                                 "vespa.rank.firstphase");
            require_config_value(values, "rankprofile[1].fef.property[0].value",
                                 "attribute(price)");
        }
    }

    static void validate_generated_configuration(const SchemaManifest &manifest,
                                                 const std::string &config_dir) {
        const auto config_path = std::filesystem::path(config_dir);
        for (const char *name : {"indexschema.cfg", "attributes.cfg", "summary.cfg",
                                 "rank-profiles.cfg"}) {
            if (!std::filesystem::is_regular_file(config_path / name)) {
                throw std::invalid_argument("inconsistent generated configuration");
            }
        }
        try {
            validate_index_configuration(manifest, config_path);
        } catch (const std::exception &error) {
            throw std::invalid_argument(std::string("inconsistent generated index configuration: ") +
                                        error.what());
        }
        try {
            validate_attribute_configuration(manifest, config_path);
        } catch (const std::exception &error) {
            throw std::invalid_argument(std::string("inconsistent generated attribute configuration: ") +
                                        error.what());
        }
        try {
            validate_summary_configuration(manifest, config_path);
        } catch (const std::exception &error) {
            throw std::invalid_argument(std::string("inconsistent generated summary configuration: ") +
                                        error.what());
        }
        try {
            validate_rank_profile_configuration(manifest, config_path);
        } catch (const std::exception &error) {
            throw std::invalid_argument(std::string("inconsistent generated rank-profile configuration: ") +
                                        error.what());
        }
    }

    // The M6 compiler keeps its canonical indexschema output compatible with
    // the frozen M6 contract.  Vespa's BM25 index requires interleaved
    // features, however, so text-only local engines use a private runtime
    // copy with that native setting enabled.  The caller-owned configuration
    // remains untouched and continues to be the schema-identity input.
    static std::filesystem::path make_runtime_config_directory(
        const SchemaManifest &manifest, const std::filesystem::path &config_dir) {
        const bool has_indexed_text = std::any_of(
            manifest.fields.begin(), manifest.fields.end(),
            [](const auto &field) { return field.index && field.type == "text"; });
        const bool has_attributes = std::any_of(
            manifest.fields.begin(), manifest.fields.end(),
            [](const auto &field) { return field.attribute; });
        if (!has_indexed_text || has_attributes) {
            return {};
        }

        const auto indexschema = config_dir / "indexschema.cfg";
        std::ifstream input(indexschema, std::ios::binary);
        if (!input) {
            throw std::runtime_error("could not read generated configuration: " +
                                     indexschema.string());
        }
        const std::string contents((std::istreambuf_iterator<char>(input)),
                                   std::istreambuf_iterator<char>());
        if (!input.good() && !input.eof()) {
            throw std::runtime_error("could not read generated configuration: " +
                                     indexschema.string());
        }
        constexpr std::string_view old_value = "interleavedfeatures false";
        if (contents.find(old_value) == std::string::npos) {
            return {};
        }

        static std::atomic<uint64_t> next_runtime_config{1};
        const auto runtime_dir = std::filesystem::temp_directory_path() /
                                 ("searchkernel-vespa-config-" +
                                  std::to_string(static_cast<unsigned long long>(getpid())) +
                                  "-" + std::to_string(next_runtime_config.fetch_add(
                                      1, std::memory_order_relaxed)));
        std::error_code filesystem_error;
        try {
            std::filesystem::create_directories(runtime_dir);
            for (std::filesystem::directory_iterator entry(config_dir, filesystem_error), end;
                 entry != end; entry.increment(filesystem_error)) {
                if (filesystem_error) {
                    throw std::runtime_error("could not inspect generated configuration: " +
                                             filesystem_error.message());
                }
                if (!entry->is_regular_file(filesystem_error) || filesystem_error) {
                    throw std::invalid_argument("generated configuration contains a non-file entry");
                }
                std::filesystem::copy_file(entry->path(), runtime_dir / entry->path().filename(),
                                           std::filesystem::copy_options::none, filesystem_error);
                if (filesystem_error) {
                    throw std::runtime_error("could not copy generated configuration: " +
                                             filesystem_error.message());
                }
            }

            std::string runtime_indexschema = contents;
            size_t position = 0;
            constexpr std::string_view new_value = "interleavedfeatures true";
            while ((position = runtime_indexschema.find(old_value, position)) !=
                   std::string::npos) {
                runtime_indexschema.replace(position, old_value.size(), new_value);
                position += new_value.size();
            }
            std::ofstream output(runtime_dir / "indexschema.cfg",
                                 std::ios::binary | std::ios::trunc);
            if (!output) {
                throw std::runtime_error("could not write runtime Vespa configuration");
            }
            output << runtime_indexschema;
            if (!output) {
                throw std::runtime_error("could not write runtime Vespa configuration");
            }
        } catch (...) {
            std::filesystem::remove_all(runtime_dir, filesystem_error);
            throw;
        }
        return runtime_dir;
    }

    static SchemaManifest read_schema_manifest(const std::string &config_dir) {
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        TimingProbe::Scope timing("schema_validation");
#endif
        const auto path = std::filesystem::path(config_dir) / "searchkernel-schema.json";
        if (!std::filesystem::is_regular_file(path)) {
            throw std::invalid_argument("generated schema manifest is missing");
        }
        std::ifstream input(path);
        if (!input) {
            throw std::runtime_error("could not read generated schema manifest: " + path.string());
        }
        const std::string json((std::istreambuf_iterator<char>(input)), std::istreambuf_iterator<char>());
        vespalib::Slime slime;
        if (vespalib::slime::JsonFormat::decode(vespalib::Memory(json), slime) == 0) {
            throw std::invalid_argument("generated schema manifest is not valid JSON");
        }
        const auto &root = slime.get();
        if (!is_slime_type(root, vespalib::slime::OBJECT::ID) ||
            !is_slime_type(root["version"], vespalib::slime::LONG::ID) ||
            root["version"].asLong() != 1 ||
            !is_slime_type(root["document_type"], vespalib::slime::STRING::ID) ||
            !is_slime_type(root["fields"], vespalib::slime::ARRAY::ID)) {
            throw std::invalid_argument("generated schema manifest is incomplete");
        }
        SchemaManifest manifest{root["document_type"].asString().make_string(), {}};
        validate_manifest_identifier(manifest.document_type, "document type");
        const auto &fields = root["fields"];
        manifest.fields.reserve(fields.children());
        std::vector<std::string> field_names;
        field_names.reserve(fields.children());
        for (size_t index = 0; index < fields.children(); ++index) {
            const auto &field = fields[index];
            if (!is_slime_type(field, vespalib::slime::OBJECT::ID) ||
                !is_slime_type(field["name"], vespalib::slime::STRING::ID) ||
                !is_slime_type(field["type"], vespalib::slime::STRING::ID) ||
                !is_slime_type(field["index"], vespalib::slime::BOOL::ID) ||
                !is_slime_type(field["attribute"], vespalib::slime::BOOL::ID) ||
                !is_slime_type(field["summary"], vespalib::slime::BOOL::ID)) {
                throw std::invalid_argument("generated schema manifest field is incomplete");
            }
            auto name = field["name"].asString().make_string();
            validate_manifest_field_name(name);
            if (std::find(field_names.begin(), field_names.end(), name) != field_names.end()) {
                throw std::invalid_argument("generated schema contains duplicate field name");
            }
            field_names.push_back(name);
            auto type = field["type"].asString().make_string();
            if (type != "text" && type != "string" && type != "integer" &&
                type != "float" && type != "boolean") {
                throw std::invalid_argument("unsupported generated schema field type: " + type);
            }
            const bool indexed = field["index"].asBool();
            const bool attribute = field["attribute"].asBool();
            const bool summary = field["summary"].asBool();
            if ((type == "text" && !indexed) ||
                (type != "text" && (indexed || !attribute))) {
                throw std::invalid_argument("generated schema field capabilities are invalid");
            }
            manifest.fields.push_back({std::move(name), std::move(type), indexed, attribute, summary});
        }
        validate_generated_configuration(manifest, config_dir);
        return manifest;
    }

    static std::shared_ptr<const DocumenttypesConfig>
    make_documenttypes_config(const SchemaManifest &manifest, bool legacy) {
        if (legacy) {
            return std::make_shared<DocumenttypesConfig>();
        }
        document::new_config_builder::NewConfigBuilder builder;
        auto &document_type = builder.document(manifest.document_type);
        for (const auto &field : manifest.fields) {
            if (field.type == "text" || field.type == "string") {
                document_type.addField(field.name, builder.stringTypeRef());
            } else if (field.type == "integer") {
                document_type.addField(field.name, builder.longTypeRef());
            } else if (field.type == "float") {
                document_type.addField(field.name, builder.doubleTypeRef());
            } else if (field.type == "boolean") {
                document_type.addField(field.name, builder.boolTypeRef());
            } else {
                throw std::invalid_argument("unsupported generated schema field type: " + field.type);
            }
        }
        return std::make_shared<DocumenttypesConfig>(builder.config());
    }

    static const document::DataType *manifest_type(const std::string &type) {
        if (type == "text" || type == "string") return document::DataType::STRING;
        if (type == "integer") return document::DataType::LONG;
        if (type == "float") return document::DataType::DOUBLE;
        if (type == "boolean") return document::DataType::BOOL;
        throw std::invalid_argument("unsupported generated schema field type: " + type);
    }

    static DocumentType make_document_type(const SchemaManifest &manifest) {
        DocumentType type(manifest.document_type, 0);
        int32_t field_id = 1;
        for (const auto &field : manifest.fields) {
            type.addField(document::Field(field.name, field_id++, *manifest_type(field.type)));
        }
        return type;
    }

    static bool is_legacy_state(const std::string &data_dir, bool had_existing_state) {
        return had_existing_state &&
               !std::filesystem::exists(std::filesystem::path(data_dir) / ".searchkernel-schema");
    }


    void prepare_bucket(const Bucket &bucket) {
        const uint64_t raw_id = bucket.getBucketId().getId();
        if (std::find(_prepared_buckets.begin(), _prepared_buckets.end(), raw_id) ==
            _prepared_buckets.end()) {
            auto active = _persistence.setActiveState(bucket, BucketInfo::ACTIVE);
            if (active.hasError()) {
                throw std::runtime_error(active.getErrorMessage());
            }
            _prepared_buckets.push_back(raw_id);
        }
    }

    std::shared_ptr<Document> make_document(const DocumentId &id, const std::string &field_name,
                                            int64_t value) const {
        require_integer_field(field_name);
        auto document = std::make_shared<Document>(*_repo, *_doc_type, id);
        const auto &field = _doc_type->getField(field_name);
        if (field.getDataType().getId() != document::DataType::T_LONG) {
            throw std::invalid_argument("integer operation requires a LONG field: " + field_name);
        }
        document->setFieldValue(field, std::make_unique<document::LongFieldValue>(value));
        return document;
    }

    std::shared_ptr<document::DocumentUpdate>
    make_update(const DocumentId &id, const std::string &field_name, int64_t value) const {
        require_integer_field(field_name);
        auto update = std::make_shared<document::DocumentUpdate>(*_repo, *_doc_type, id);
        const auto &field = _doc_type->getField(field_name);
        if (field.getDataType().getId() != document::DataType::T_LONG) {
            throw std::invalid_argument("integer operation requires a LONG field: " + field_name);
        }
        document::FieldUpdate field_update(field);
        field_update.addUpdate(std::make_unique<document::AssignValueUpdate>(
            std::make_unique<document::LongFieldValue>(value)));
        update->addUpdate(std::move(field_update));
        return update;
    }

public:
    ValueDocument get_value_document_for_abi(const std::string &external_id) const {
        return get_value_document(external_id);
    }

    EmbeddedEngine(const std::string &data_dir, const std::string &config_dir)
        : _manifest(read_schema_manifest(config_dir)),
          _data_lock(data_dir),
          _schema_identity(validate_or_persist_schema_identity(
              data_dir, config_dir, _manifest, _data_lock.had_existing_state())),
          _document_type(_manifest.document_type),
          _document_types(make_documenttypes_config(
              _manifest, is_legacy_state(data_dir, _data_lock.had_existing_state()))),
          _repo(is_legacy_state(data_dir, _data_lock.had_existing_state())
                    ? std::make_shared<DocumentTypeRepo>(make_document_type(_manifest))
                    : std::make_shared<DocumentTypeRepo>(*_document_types)),
          _doc_type(_repo->getDocumentType(_document_type)),
          _wire_service(),
          _db_owner(),
          _summary_executor(8),
          _shared_service(_summary_executor),
          _hw_info(),
          _file_header_context(),
          _tls_dir("tmp"),
          _tls_port_reservation(),
          _tls_port(_tls_port_reservation.port()),
          _tls(_shared_service.transport(), _tls_dir,
               static_cast<int>(_tls_port), data_dir, _file_header_context),
          _query_limiter(),
          _db(),
          _persistence_owner(),
          _write_filter(),
          _resource_usage_notifier(),
          _persistence(_persistence_owner, _write_filter,
                       _resource_usage_notifier, -1, false),
          _handler(),
          _prepared_buckets() {}

    void failpoint(const char *name) const {
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        _failpoints.hit(name);
#else
        (void)name;
#endif
    }

#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    const FailpointController &failpoints() const noexcept { return _failpoints; }

    void set_failpoint(const char *name, uint32_t hit, uint32_t action) {
        _failpoints.set(name, hit, action);
    }

    void clear_failpoint(const char *name) { _failpoints.clear(name); }

    void release_failpoint(const char *name) { _failpoints.release(name); }

    bool failpoint_is_blocked(const char *name) const { return _failpoints.is_blocked(name); }
#endif

    void initialize(const std::string &data_dir, const std::string &config_dir) {
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        TimingProbe::Scope initialize_timing("initialize_total");
#endif
        std::filesystem::create_directories(data_dir);
        if (_doc_type == nullptr) {
            throw std::runtime_error("generated document type was not added to DocumentTypeRepo: " +
                                     _document_type);
        }
        std::filesystem::create_directories(data_dir + "/" + _document_type);

        _runtime_config_dir = make_runtime_config_directory(_manifest, config_dir);
        const auto vespa_config_dir = _runtime_config_dir.empty()
                                          ? std::filesystem::path(config_dir)
                                          : _runtime_config_dir;
        config::DirSpec spec(vespa_config_dir.string());
        DocumentDBConfigHelper config(spec, _document_type);
        auto proton_config = std::make_shared<ProtonConfigBuilder>();
        proton_config->documentdb.emplace_back();
        proton_config->documentdb.back().inputdoctypename = _document_type;
        proton_config->documentdb.back().documentIdAttribute = true;
        auto file_distribution = std::make_shared<FiledistributorrpcConfig>();
        auto bucket_spaces = std::make_shared<BucketspacesConfig>();
        auto tune = std::make_shared<search::TuneFileDocumentDB>();
        auto bootstrap = std::make_shared<proton::BootstrapConfig>(
            1, _document_types, _repo, proton_config, file_distribution,
            bucket_spaces, tune, _hw_info);
        config.forwardConfig(bootstrap);
        config.nextGeneration(_shared_service.transport(), 0ms);

        proton::ConfigStore::UP config_store;
        if (std::filesystem::exists(std::filesystem::path(config_dir) / "proton.cfg")) {
            auto file_config_store = std::make_unique<proton::FileConfigManager>(
                _shared_service.transport(), data_dir + "/" + _document_type + "/config", "", _document_type);
            file_config_store->setProtonConfig(proton_config);
            config_store = std::move(file_config_store);
        } else {
            config_store = std::make_unique<proton::MemoryConfigStore>();
        }

        auto config_store_ptr = config_store.get();
        std::shared_ptr<vespalib::ThreadStackExecutor> write_executor;
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        {
            TimingProbe::Scope write_executor_timing("write_executor");
            write_executor = std::make_shared<vespalib::ThreadStackExecutor>(16);
        }
        {
            TimingProbe::Scope create_timing("documentdb_create");
#endif
            if (!write_executor) {
                write_executor = std::make_shared<vespalib::ThreadStackExecutor>(16);
            }
            _db = DocumentDB::create(
                data_dir, config.getConfig(),
                vespalib::SocketSpec::from_host_port("localhost", static_cast<int>(_tls_port)).spec(),
                _query_limiter, DocTypeName(_document_type), bucket_space(),
                *proton_config, _db_owner, _shared_service, _tls, _wire_service,
                _file_header_context,
                std::make_shared<search::attribute::Interlock>(),
                std::move(config_store),
                write_executor,
                _hw_info, std::shared_ptr<search::diskindex::IPostingListCache>());
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        }
#endif
        _config_store = config_store_ptr;
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        {
            TimingProbe::Scope start_timing("documentdb_start");
#endif
            _db->start();
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        }
        {
            TimingProbe::Scope online_timing("documentdb_wait_online");
#endif
            _db->waitForOnlineState();
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        }
#endif
        const auto replay_serial = _db->getFeedHandler().getSerialNum();
        if (replay_serial == std::numeric_limits<uint64_t>::max()) {
            throw std::runtime_error("native feed serial space exhausted");
        }
        // Mutation timestamps are owned by this adapter. Start after the
        // recovered feed serial so a new engine lifetime cannot submit an
        // older update than one already persisted for the same document.
        _next_timestamp = replay_serial + 1;

        _handler = std::make_shared<PersistenceHandlerProxy>(_db);
        _persistence.putHandler(_persistence.getWLock(), bucket_space(),
                                DocTypeName(_document_type), _handler);
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        {
            TimingProbe::Scope persistence_timing("persistence_initialize");
            auto initialized = _persistence.initialize();
            if (initialized.hasError()) {
                throw std::runtime_error(initialized.getErrorMessage());
            }
        }
#else
        auto initialized = _persistence.initialize();
        if (initialized.hasError()) {
            throw std::runtime_error(initialized.getErrorMessage());
        }
#endif
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        TimingProbe::Scope cluster_timing("cluster_bucket_init");
#endif
        auto state = single_node_cluster_state();
        auto state_result = _persistence.setClusterState(bucket_space(), state);
        if (state_result.hasError()) {
            throw std::runtime_error(state_result.getErrorMessage());
        }
        auto buckets = _persistence.listBuckets(bucket_space());
        if (buckets.hasError()) {
            throw std::runtime_error(buckets.getErrorMessage());
        }
        for (const auto &bucket_id : buckets.getList()) {
            const Bucket bucket(document::Bucket(bucket_space(), bucket_id));
            auto active = _persistence.setActiveState(bucket, BucketInfo::ACTIVE);
            if (active.hasError()) {
                throw std::runtime_error(active.getErrorMessage());
            }
            _prepared_buckets.push_back(bucket_id.getId());
        }
        _initialized = true;
    }

    void close() {
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        TimingProbe::Scope timing("close");
#endif
        std::unique_lock<std::shared_mutex> lifecycle_lock(_lifecycle_mutex);
        const auto current = _state.load(std::memory_order_acquire);
        if (current == State::Closed || current == State::Failed) {
            throw std::invalid_argument("engine is already closed");
        }
        auto expected = State::Open;
        if (!_state.compare_exchange_strong(expected, State::Closing,
                                            std::memory_order_acq_rel)) {
            if (expected == State::Closed || expected == State::Failed) {
                throw std::invalid_argument("engine is already closed");
            }
            throw std::runtime_error("engine is already closing");
        }
        try {
            {
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
                TimingProbe::Scope remove_handler_timing("close_remove_handler");
#endif
                _persistence.removeHandler(_persistence.getWLock(), bucket_space(),
                                           DocTypeName(_document_type));
            }
            _handler.reset();
            if (_db) {
                {
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
                    TimingProbe::Scope db_timing("close_db");
#endif
                    _db->close();
                }
                if (_config_store) {
                    _config_store = nullptr;
                }
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
                {
                    TimingProbe::Scope db_reset_timing("close_db_reset");
#endif
                _db.reset();
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
                }
#endif
            }
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
            {
                TimingProbe::Scope shutdown_timing("close_shared_service_shutdown");
#endif
            _shared_service.shutdown();
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
            }
#endif
            if (!_runtime_config_dir.empty()) {
                std::error_code filesystem_error;
                std::filesystem::remove_all(_runtime_config_dir, filesystem_error);
                _runtime_config_dir.clear();
            }
            _state.store(State::Closed, std::memory_order_release);
        } catch (...) {
            _state.store(State::Failed, std::memory_order_release);
            throw;
        }
    }

    void cleanup_noexcept() noexcept {
        try {
            // Mutex acquisition may itself report a system error. Keep it
            // inside this boundary because this function is called from a
            // noexcept destructor after a failed graceful close.
            std::unique_lock<std::shared_mutex> lifecycle_lock(_lifecycle_mutex);
            if (_handler) {
                _persistence.removeHandler(_persistence.getWLock(), bucket_space(),
                                           DocTypeName(_document_type));
            }
            _handler.reset();
            if (_db) {
                _db->close();
                _config_store = nullptr;
                _db.reset();
            }
            _shared_service.shutdown_noexcept();
        } catch (...) {
            _config_store = nullptr;
            _db.reset();
            _handler.reset();
            try {
                _shared_service.shutdown_noexcept();
            } catch (...) {
            }
        }
        if (!_runtime_config_dir.empty()) {
            std::error_code filesystem_error;
            std::filesystem::remove_all(_runtime_config_dir, filesystem_error);
            _runtime_config_dir.clear();
        }
    }

    ~EmbeddedEngine() noexcept {
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        TimingProbe::Scope timing("destruction");
#endif
        if (_initialized && _state.load(std::memory_order_acquire) == State::Open) {
            try {
                close();
            } catch (...) {
            }
        }
        if (_state.load(std::memory_order_acquire) != State::Closed) {
            cleanup_noexcept();
        }
    }

    void put(const std::string &id, const std::string &field, int64_t value) {
        std::shared_lock<std::shared_mutex> lifecycle_lock(_lifecycle_mutex);
        require_open();
        DocumentId document_id(internal_document_id(id, _document_type));
        Bucket bucket = bucket_for(document_id);
        {
            std::lock_guard<std::mutex> bucket_lock(_bucket_mutex);
            prepare_bucket(bucket);
        }
        failpoint("mutation_before_native_call");
        auto result = _persistence.put(bucket, Timestamp(next_timestamp()),
                                       make_document(document_id, field, value));
        failpoint("mutation_after_native_call");
        if (result.hasError()) {
            throw std::runtime_error(result.getErrorMessage());
        }
    }

    void update(const std::string &id, const std::string &field, int64_t value) {
        std::shared_lock<std::shared_mutex> lifecycle_lock(_lifecycle_mutex);
        require_open();
        DocumentId document_id(internal_document_id(id, _document_type));
        failpoint("mutation_before_native_call");
        auto result = _persistence.update(
            bucket_for(document_id), Timestamp(next_timestamp()),
            make_update(document_id, field, value));
        failpoint("mutation_after_native_call");
        if (result.hasError()) {
            throw std::runtime_error(result.getErrorMessage());
        }
    }

    bool remove(const std::string &id) {
        std::shared_lock<std::shared_mutex> lifecycle_lock(_lifecycle_mutex);
        require_open();
        DocumentId document_id(internal_document_id(id, _document_type));
        failpoint("mutation_before_native_call");
        auto result = _persistence.remove(
            bucket_for(document_id), Timestamp(next_timestamp()),
            document_id);
        failpoint("mutation_after_native_call");
        if (result.hasError()) {
            throw std::runtime_error(result.getErrorMessage());
        }
        return result.wasFound();
    }

    std::vector<std::string> search(const std::string &field, int64_t value,
                                    uint32_t limit) const {
        std::shared_lock<std::shared_mutex> lifecycle_lock(_lifecycle_mutex);
        require_open();
        require_integer_field(field);
        auto request = std::make_shared<SearchRequest>();
        request->setTimeout(60s);
        request->setSerializedQueryTree(SerializedQueryTree::fromStackDump(
            query_stack_dump(value, field)));
        request->ranking = "default";
        request->maxhits = limit;
        auto reply = _db->match(*request, vespalib::ThreadBundle::trivial());
        if (!reply) {
            throw std::runtime_error("Vespa matcher returned no reply");
        }
        auto *ready_sub_db = _db->getReadySubDB();
        if (!ready_sub_db) {
            throw std::runtime_error("Vespa DocumentDB has no ready sub database");
        }
        auto meta_store_guard = ready_sub_db->getDocumentMetaStoreContext().getReadGuard();
        const auto *document_id_provider =
            dynamic_cast<const search::IDocumentIdProvider *>(&meta_store_guard->get());
        if (!document_id_provider) {
            throw std::runtime_error("Vespa DocumentDB does not provide document IDs");
        }
        std::vector<std::string> ids;
        for (const auto &hit : reply->hits) {
            search::IDocumentMetaStore::DocId lid = 0;
            if (meta_store_guard->get().getLid(hit.gid, lid)) {
                auto document_id = document_id_provider->get_document_id_string_view(lid);
                if (!document_id.empty()) {
                    ids.emplace_back(external_document_id(document_id, _document_type));
                }
            }
        }
        return ids;
    }

    std::unique_ptr<document::StringFieldValue>
    make_text_value(const TextDocument &document) const {
        const document::FixedTypeRepo fixed_repo(*_repo, *_doc_type);
        auto value = std::make_unique<document::StringFieldValue>(document.text);
        auto spans = std::make_unique<document::SpanList>();
        auto *spans_ptr = spans.get();
        spans_ptr->reserve(document.occurrences.size());
        auto tree = std::make_unique<document::SpanTree>("linguistics", std::move(spans));

        for (const auto &occurrence : document.occurrences) {
            if (occurrence.byte_start > document.text.size() ||
                occurrence.byte_length > document.text.size() - occurrence.byte_start) {
                throw std::invalid_argument("text analysis span is outside the original text");
            }
            if (occurrence.byte_start > std::numeric_limits<int32_t>::max() ||
                occurrence.byte_length > std::numeric_limits<int32_t>::max()) {
                throw std::invalid_argument("text analysis span is too large");
            }
            const auto original = std::string_view(document.text).substr(
                occurrence.byte_start, occurrence.byte_length);
            auto &span = spans_ptr->add(std::make_unique<document::Span>(
                static_cast<int32_t>(occurrence.byte_start),
                static_cast<int32_t>(occurrence.byte_length)));
            if (original == occurrence.term) {
                tree->annotate(span, *document::AnnotationType::TERM);
            } else {
                tree->annotate(span, document::Annotation(
                    *document::AnnotationType::TERM,
                    std::make_unique<document::StringFieldValue>(occurrence.term)));
            }
        }

        document::StringFieldValue::SpanTrees trees;
        trees.emplace_back(std::move(tree));
        value->setSpanTrees(trees, fixed_repo);
        return value;
    }

    std::shared_ptr<Document> make_text_document(const TextDocument &text) const {
        require_text_field(text.field);
        const auto &field = _doc_type->getField(text.field);
        if (field.getDataType().getId() != document::DataType::T_STRING) {
            throw std::invalid_argument("text field is not a string field");
        }
        auto document = std::make_shared<Document>(*_repo, *_doc_type,
                                                   DocumentId(internal_document_id(text.id, _document_type)));
        document->setFieldValue(field, make_text_value(text));
        return document;
    }

    std::shared_ptr<document::DocumentUpdate> make_text_update(const TextUpdate &text) const {
        require_text_field(text.field);
        const auto &field = _doc_type->getField(text.field);
        if (field.getDataType().getId() != document::DataType::T_STRING) {
            throw std::invalid_argument("text field is not a string field");
        }
        auto update = std::make_shared<document::DocumentUpdate>(
            *_repo, *_doc_type, DocumentId(internal_document_id(text.id, _document_type)));
        document::FieldUpdate field_update(field);
        field_update.addUpdate(std::make_unique<document::AssignValueUpdate>(
            make_text_value(text)));
        update->addUpdate(std::move(field_update));
        return update;
    }

    void put_text(const TextDocument &text) {
        std::shared_lock<std::shared_mutex> lifecycle_lock(_lifecycle_mutex);
        require_open();
        const DocumentId id(internal_document_id(text.id, _document_type));
        const Bucket bucket = bucket_for(id);
        {
            std::lock_guard<std::mutex> bucket_lock(_bucket_mutex);
            prepare_bucket(bucket);
        }
        failpoint("mutation_before_native_call");
        auto result = _persistence.put(bucket, Timestamp(next_timestamp()),
                                       make_text_document(text));
        failpoint("mutation_after_native_call");
        if (result.hasError()) {
            throw std::runtime_error(result.getErrorMessage());
        }
    }

    void update_text(const TextUpdate &text) {
        std::shared_lock<std::shared_mutex> lifecycle_lock(_lifecycle_mutex);
        require_open();
        const DocumentId id(internal_document_id(text.id, _document_type));
        failpoint("mutation_before_native_call");
        auto result = _persistence.update(bucket_for(id), Timestamp(next_timestamp()),
                                          make_text_update(text));
        failpoint("mutation_after_native_call");
        if (result.hasError()) {
            throw std::runtime_error(result.getErrorMessage());
        }
    }

    std::vector<TextHit> search_text(const TextRequest &text) const {
        std::shared_lock<std::shared_mutex> lifecycle_lock(_lifecycle_mutex);
        require_open();
        require_text_field(text.field);
        if (text.terms.size() >
            static_cast<size_t>(std::numeric_limits<int>::max() - 2)) {
            throw std::invalid_argument("text term count cannot be represented by Vespa");
        }
        VespaQueryBuilder builder;
        if (text.kind == SK_TEXT_QUERY_TERM) {
            if (text.terms.size() != 1) {
                throw std::invalid_argument("term query must contain exactly one term");
            }
            builder.addStringTerm(text.terms[0], text.field, 1, Weight(1));
        } else if (text.kind == SK_TEXT_QUERY_PHRASE) {
            if (text.terms.empty()) {
                throw std::invalid_argument("phrase query must contain a term");
            }
            builder.addPhrase(static_cast<int>(text.terms.size()), text.field, 1, Weight(1));
            int32_t id = 2;
            for (const auto &term : text.terms) {
                builder.addStringTerm(term, text.field, id++, Weight(1));
            }
        } else {
            throw std::invalid_argument("unsupported text query kind");
        }
        return match_tree(builder.build(), text.rank_profile, "*");
    }

    std::string id_for_gid(const document::GlobalId &gid) const {
        auto *ready_sub_db = _db->getReadySubDB();
        if (!ready_sub_db) throw std::runtime_error("Vespa DocumentDB has no ready sub database");
        auto guard = ready_sub_db->getDocumentMetaStoreContext().getReadGuard();
        const auto *provider = dynamic_cast<const search::IDocumentIdProvider *>(&guard->get());
        if (!provider) throw std::runtime_error("Vespa DocumentDB does not provide document IDs");
        search::IDocumentMetaStore::DocId lid = 0;
        if (!guard->get().getLid(gid, lid)) throw std::runtime_error("Vespa result GID has no document ID");
        const auto id = provider->get_document_id_string_view(lid);
        if (id.empty()) throw std::runtime_error("Vespa result document ID is empty");
        return external_document_id(id, _document_type);
    }

    OwnedValue summary_value(const std::string &field_name,
                             const vespalib::slime::Inspector &value) const {
        const auto &field = require_manifest_field(field_name);
        OwnedValue result;
        if (field.type == "text" || field.type == "string") {
            if (value.type() != vespalib::slime::STRING::instance) throw std::runtime_error("unexpected string summary type");
            result.kind = SK_VALUE_STRING;
            result.string = value.asString().make_string();
        } else if (field.type == "integer") {
            if (value.type() != vespalib::slime::LONG::instance) throw std::runtime_error("unexpected integer summary type");
            result.kind = SK_VALUE_INTEGER;
            result.integer = value.asLong();
        } else if (field.type == "float") {
            if (value.type() != vespalib::slime::DOUBLE::instance) throw std::runtime_error("unexpected float summary type");
            result.kind = SK_VALUE_FLOAT;
            result.floating = value.asDouble();
        } else if (field.type == "boolean") {
            if (value.type() != vespalib::slime::BOOL::instance) throw std::runtime_error("unexpected boolean summary type");
            result.kind = SK_VALUE_BOOLEAN;
            result.boolean = value.asBool();
        } else {
            throw std::runtime_error("unsupported summary field type");
        }
        return result;
    }

    LocalSearchResultState search_local(const NativeQueryState &query,
                                        const LocalSearchRequestState &options) const {
        if (options.rank_profile.empty()) throw std::invalid_argument("rank profile must not be empty");
        auto tree = query.builder.build();
        if (!tree) throw std::invalid_argument("could not build query tree");
        validate_rank_profile(options.rank_profile.c_str());
        SearchRequest request;
        request.setTimeout(60s);
        request.ranking = options.rank_profile;
        request.propertiesMap.lookupCreate(search::MapNames::RANK)
            .add("vespa.matching.numthreadspersearch", "1");
        request.offset = options.offset;
        request.maxhits = options.limit;
        for (const auto &[field, order] : options.sort) {
            (void)require_attribute_field(field, {"string", "integer", "float"});
            if (!request.sortSpec.empty()) request.sortSpec.push_back(' ');
            request.sortSpec.push_back(order == SK_SORT_ASCENDING ? '+' : '-');
            request.sortSpec += field;
        }
        if (options.summary_kind == SK_SUMMARY_FIELDS) {
            if (options.summary_fields.empty()) {
                throw std::invalid_argument("explicit summary field list must not be empty");
            }
            for (const auto &field_name : options.summary_fields) {
                const auto &field = require_manifest_field(field_name);
                if (!field.summary) {
                    throw std::invalid_argument("field is not a summary field: " + field_name);
                }
            }
        }
        request.setSerializedQueryTree(SerializedQueryTree::fromStackDump(StackDumpCreator::create(*tree)));
        auto reply = _db->match(request, vespalib::ThreadBundle::trivial());
        if (!reply) throw std::runtime_error("Vespa matcher returned no reply");
        LocalSearchResultState result;
        result.total_hit_count = reply->totalHitCount;
        result.hits.reserve(reply->hits.size());
        for (const auto &hit : reply->hits) {
            result.hits.push_back({id_for_gid(hit.gid), hit.metric, {}});
        }
        if (options.summary_kind != SK_SUMMARY_NONE && !result.hits.empty()) {
            DocsumRequest docsum_request;
            docsum_request.setTimeout(60s);
            docsum_request.resultClassName = "default";
            docsum_request.ranking = options.rank_profile;
            for (const auto &hit : reply->hits) docsum_request.hits.emplace_back(hit.gid);
            if (options.summary_kind == SK_SUMMARY_FIELDS) {
                docsum_request.setFields(options.summary_fields);
            }
            auto docsum = _db->getDocsums(docsum_request);
            if (!docsum || !docsum->hasResult()) throw std::runtime_error("Vespa docsum returned no result");
            const auto &root = docsum->root();
            for (size_t index = 0; index < result.hits.size(); ++index) {
                const auto &doc = root["docsums"][index]["docsum"];
                if (options.summary_kind == SK_SUMMARY_FIELDS) {
                    for (const auto &name : options.summary_fields) {
                        const auto &value = doc[vespalib::Memory(name)];
                        if (value.valid()) result.hits[index].summary.emplace_back(name, summary_value(name, value));
                    }
                } else {
                    for (const auto &field : _manifest.fields) {
                        if (!field.summary) continue;
                        const auto &value = doc[vespalib::Memory(field.name)];
                        if (value.valid()) result.hits[index].summary.emplace_back(field.name, summary_value(field.name, value));
                    }
                }
            }
        }
        return result;
    }

    LocalGroupingResultState group_local(const NativeQueryState &query,
                                         const LocalGroupingRequestState &options) const {
        const auto &group_field = require_attribute_field(options.group_field, {"string", "integer"});
        auto tree = query.builder.build();
        if (!tree) throw std::invalid_argument("could not build query tree");
        SearchRequest request;
        request.setTimeout(60s);
        request.ranking = "default";
        request.maxhits = 0;
        request.setSerializedQueryTree(SerializedQueryTree::fromStackDump(StackDumpCreator::create(*tree)));
        auto level = GroupingLevel().setExpression(std::make_unique<AttributeNode>(group_field.name));
        level.addResult(CountAggregationResult().setExpression(std::make_unique<AttributeNode>(group_field.name)));
        for (const auto &spec : options.aggregations) {
            const auto &field = require_attribute_field(spec.field, {"integer", "float"});
            std::unique_ptr<AggregationResult> aggregation;
            if (spec.kind == SK_AGGREGATION_SUM) aggregation = std::make_unique<SumAggregationResult>();
            else if (spec.kind == SK_AGGREGATION_MIN) aggregation = std::make_unique<MinAggregationResult>();
            else if (spec.kind == SK_AGGREGATION_MAX) aggregation = std::make_unique<MaxAggregationResult>();
            else if (spec.kind == SK_AGGREGATION_AVERAGE) aggregation = std::make_unique<AverageAggregationResult>();
            else throw std::invalid_argument("unsupported aggregation kind");
            aggregation->setExpression(std::make_unique<AttributeNode>(field.name));
            level.addResult(std::move(aggregation));
        }
        Grouping grouping;
        grouping.setFirstLevel(0).setLastLevel(1).addLevel(std::move(level));
        vespalib::nbostream buffer;
        vespalib::NBOSerializer output(buffer);
        uint32_t grouping_count = 1;
        output << grouping_count;
        grouping.serialize(output);
        request.groupSpec.assign(buffer.data(), buffer.data() + buffer.size());
        auto reply = _db->match(request, vespalib::ThreadBundle::trivial());
        if (!reply) throw std::runtime_error("Vespa matcher returned no reply");
        if (!reply->hits.empty()) throw std::runtime_error("native grouping returned ordinary hits");
        if (reply->groupResult.empty()) throw std::runtime_error("native grouping result is empty");
        vespalib::nbostream result_buffer(reply->groupResult.data(), reply->groupResult.size());
        vespalib::NBOSerializer input(result_buffer);
        uint32_t result_count = 0;
        input >> result_count;
        if (result_count != 1) throw std::runtime_error("unexpected native grouping result count");
        Grouping result_grouping;
        result_grouping.deserialize(input);
        LocalGroupingResultState result;
        const auto &root = result_grouping.getRoot();
        result.groups.reserve(root.getChildrenSize());
        for (uint32_t index = 0; index < root.getChildrenSize(); ++index) {
            const auto &group = root.getChild(index);
            LocalGroupState output_group;
            const auto &key = group.getId();
            if (key.getClass().inherits(StringResultNode::classId)) {
                HoldString value(key);
                output_group.key.kind = SK_VALUE_STRING;
                output_group.key.string = std::string(value);
            } else if (key.getClass().inherits(Int64ResultNode::classId)) {
                output_group.key.kind = SK_VALUE_INTEGER;
                output_group.key.integer = key.getInteger();
            } else {
                throw std::runtime_error("unsupported native grouping key type");
            }
            const auto &count = group.getAggregationResult(0);
            if (!count.getClass().inherits(CountAggregationResult::classId)) throw std::runtime_error("native grouping count is missing");
            output_group.count = static_cast<const CountAggregationResult &>(count).getCount();
            for (size_t aggregation_index = 0; aggregation_index < options.aggregations.size(); ++aggregation_index) {
                const auto &aggregation = group.getAggregationResult(aggregation_index + 1);
                const auto emit = [&](const auto &node) {
                    OwnedValue value;
                    if (node.getClass().inherits(Int64ResultNode::classId)) {
                        value.kind = SK_VALUE_INTEGER;
                        value.integer = node.getInteger();
                    } else if (node.getClass().inherits(FloatResultNode::classId)) {
                        value.kind = SK_VALUE_FLOAT;
                        value.floating = node.getFloat();
                    } else {
                        throw std::runtime_error("unsupported native aggregate result type");
                    }
                    output_group.aggregations.push_back(std::move(value));
                };
                if (aggregation.getClass().inherits(SumAggregationResult::classId)) emit(static_cast<const SumAggregationResult &>(aggregation).getSum());
                else if (aggregation.getClass().inherits(MinAggregationResult::classId)) emit(static_cast<const MinAggregationResult &>(aggregation).getMin());
                else if (aggregation.getClass().inherits(MaxAggregationResult::classId)) emit(static_cast<const MaxAggregationResult &>(aggregation).getMax());
                else if (aggregation.getClass().inherits(AverageAggregationResult::classId)) emit(static_cast<const AverageAggregationResult &>(aggregation).getAverage());
                else throw std::runtime_error("unsupported native aggregation result");
            }
            result.groups.push_back(std::move(output_group));
        }
        return result;
    }

public:
    std::vector<TextHit> search_query(VespaQueryBuilder &builder,
                                      const std::string &rank_profile,
                                      const std::string &source) const {
        std::shared_lock<std::shared_mutex> lifecycle_lock(_lifecycle_mutex);
        require_open();
        return match_tree(builder.build(), rank_profile, source);
    }

    void put_attributes(const AttributeDocumentState &document) {
        std::shared_lock<std::shared_mutex> lifecycle_lock(_lifecycle_mutex);
        require_open();
        const DocumentId id(internal_document_id(document.id, _document_type));
        const Bucket bucket = bucket_for(id);
        {
            std::lock_guard<std::mutex> bucket_lock(_bucket_mutex);
            prepare_bucket(bucket);
        }
        auto result = _persistence.put(bucket, Timestamp(next_timestamp()), make_attribute_document(document));
        if (result.hasError()) throw std::runtime_error(result.getErrorMessage());
    }

    void update_attributes(const AttributeUpdateState &update) {
        std::shared_lock<std::shared_mutex> lifecycle_lock(_lifecycle_mutex);
        require_open();
        const DocumentId id(internal_document_id(update.id, _document_type));
        auto result = _persistence.update(bucket_for(id), Timestamp(next_timestamp()), make_attribute_update(update));
        if (result.hasError()) throw std::runtime_error(result.getErrorMessage());
    }

    void put_document(const ValueDocument &document) {
        std::shared_lock<std::shared_mutex> lifecycle_lock(_lifecycle_mutex);
        require_open();
        const DocumentId id(internal_document_id(document.id, _document_type));
        const Bucket bucket = bucket_for(id);
        {
            std::lock_guard<std::mutex> bucket_lock(_bucket_mutex);
            prepare_bucket(bucket);
        }
        auto result = _persistence.put(bucket, Timestamp(next_timestamp()),
                                       make_value_document(document));
        if (result.hasError()) throw std::runtime_error(result.getErrorMessage());
    }

    void update_document(const ValueUpdate &update) {
        std::shared_lock<std::shared_mutex> lifecycle_lock(_lifecycle_mutex);
        require_open();
        const DocumentId id(internal_document_id(update.id, _document_type));
        auto result = _persistence.update(bucket_for(id), Timestamp(next_timestamp()),
                                          make_value_update(update));
        if (result.hasError()) throw std::runtime_error(result.getErrorMessage());
    }

    LocalSearchResultState search_local_result(const NativeQueryState &query,
                                                const LocalSearchRequestState &request) const {
        std::shared_lock<std::shared_mutex> lifecycle_lock(_lifecycle_mutex);
        require_open();
        return search_local(query, request);
    }

    LocalGroupingResultState group_local_result(const NativeQueryState &query,
                                                 const LocalGroupingRequestState &request) const {
        std::shared_lock<std::shared_mutex> lifecycle_lock(_lifecycle_mutex);
        require_open();
        return group_local(query, request);
    }

    std::vector<QueryFieldMetadata> query_field_metadata() const {
        std::shared_lock<std::shared_mutex> lifecycle_lock(_lifecycle_mutex);
        require_open();
        std::vector<QueryFieldMetadata> metadata;
        metadata.reserve(_manifest.fields.size());
        for (const auto &field : _manifest.fields) {
            metadata.push_back({field.name, field.type, field.index, field.attribute});
        }
        return metadata;
    }
};

struct ValueRequest {
    std::string field;
    int64_t value;
    uint32_t limit;
};


struct ErrorResult {
    std::vector<std::string> ids;
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    const FailpointController *failpoints = nullptr;
#endif
};

void clear_error(uint8_t *error, uint32_t error_capacity) noexcept {
    if (!error || error_capacity == 0) {
        return;
    }
    error[0] = '\0';
}

int32_t set_error(uint8_t *error, uint32_t error_capacity, int32_t code,
                 const char *message) noexcept {
    if (!error || error_capacity == 0 || !message) {
        return code;
    }
    const size_t length = std::strlen(message);
    const size_t copied = std::min(length, static_cast<size_t>(error_capacity - 1));
    std::memcpy(error, message, copied);
    error[copied] = '\0';
    return code;
}

template <typename Function>
int32_t invoke(uint8_t *error, uint32_t error_capacity, Function &&function) noexcept {
    clear_error(error, error_capacity);
    try {
        function();
        return SK_NATIVE_STATUS_OK;
    } catch (const std::bad_alloc &) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_OUT_OF_MEMORY, nullptr);
    } catch (const std::invalid_argument &exception) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, exception.what());
    } catch (const std::exception &exception) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INTERNAL, exception.what());
    } catch (...) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INTERNAL, "unknown native exception");
    }
}

template <typename Handle>
void destroy_noexcept(Handle *handle) noexcept {
    try {
        delete handle;
    } catch (...) {
    }
}

std::string copy_text_bytes(const uint8_t *bytes, uint32_t length,
                            size_t maximum, const char *what);

void validate_query_field(const NativeQueryState &query, uint32_t kind,
                          const char *field) {
    if (!field) {
        throw std::invalid_argument("query field must not be null");
    }
    validate_field_name(field);
    if (*field == '\0') {
        throw std::invalid_argument("query field must not be empty");
    }
    const auto metadata = std::find_if(
        query.fields.begin(), query.fields.end(),
        [field](const auto &candidate) { return candidate.name == field; });
    if (metadata == query.fields.end()) {
        throw std::invalid_argument("unknown generated schema field: " + std::string(field));
    }
    switch (kind) {
    case SK_QUERY_FIELD_TEXT:
        if (metadata->type != "text" || !metadata->index) {
            throw std::invalid_argument(
                "text operation requires an indexed text field: " + std::string(field));
        }
        break;
    case SK_QUERY_FIELD_INTEGER:
        if (!metadata->attribute) {
            throw std::invalid_argument(
                "integer operation requires an integer attribute field: " + std::string(field));
        }
        if (metadata->type != "integer") {
            throw std::invalid_argument("wrong value type for query field: " + std::string(field));
        }
        break;
    case SK_QUERY_FIELD_STRING:
        if (!metadata->attribute) {
            throw std::invalid_argument("string operation requires a string attribute field: " + std::string(field));
        }
        if (metadata->type != "string") {
            throw std::invalid_argument("wrong value type for query field: " + std::string(field));
        }
        break;
    case SK_QUERY_FIELD_FLOAT:
        if (!metadata->attribute) {
            throw std::invalid_argument("float operation requires a float attribute field: " + std::string(field));
        }
        if (metadata->type != "float") {
            throw std::invalid_argument("wrong value type for query field: " + std::string(field));
        }
        break;
    case SK_QUERY_FIELD_BOOLEAN:
        if (!metadata->attribute) {
            throw std::invalid_argument("boolean operation requires a boolean attribute field: " + std::string(field));
        }
        if (metadata->type != "boolean") {
            throw std::invalid_argument("wrong value type for query field: " + std::string(field));
        }
        break;
    case SK_QUERY_FIELD_ATTRIBUTE:
        if (!metadata->attribute || metadata->type == "text") {
            throw std::invalid_argument("attribute operation requires an attribute field: " + std::string(field));
        }
        break;
    default:
        throw std::invalid_argument("unsupported query field kind");
    }
}

void add_query_operator(NativeQueryState &query, uint32_t kind, uint32_t child_count) {
    if (child_count > static_cast<uint32_t>(std::numeric_limits<int>::max())) {
        throw std::invalid_argument("query child count cannot be represented by Vespa");
    }
    const auto count = static_cast<int>(child_count);
    switch (kind) {
    case SK_QUERY_NODE_AND:
        query.builder.addAnd(count);
        break;
    case SK_QUERY_NODE_OR:
        query.builder.addOr(count);
        break;
    case SK_QUERY_NODE_NOT:
        if (child_count != 1) {
            throw std::invalid_argument("NOT query must contain exactly one child");
        }
        query.builder.addAndNot(2);
        query.builder.add_true_node();
        break;
    default:
        throw std::invalid_argument("unsupported query operator");
    }
}

void add_query_text(NativeQueryState &query, uint32_t kind, const char *field,
                    const uint8_t *term_bytes, uint32_t term_bytes_length,
                    const uint32_t *term_lengths, uint32_t term_count) {
    if (!field) {
        throw std::invalid_argument("query field must not be null");
    }
    validate_field_name(field);
    if (*field == '\0') {
        throw std::invalid_argument("query field must not be empty");
    }
    validate_query_field(query, SK_QUERY_FIELD_TEXT, field);
    if (kind != SK_QUERY_TEXT_TERM && kind != SK_QUERY_TEXT_PHRASE) {
        throw std::invalid_argument("unsupported query text kind");
    }
    if ((kind == SK_QUERY_TEXT_TERM && term_count != 1) ||
        (kind == SK_QUERY_TEXT_PHRASE && term_count == 0)) {
        throw std::invalid_argument("invalid number of terms for query text kind");
    }
    if (term_count > MAX_TEXT_TERMS) {
        throw std::invalid_argument("query text term count is too large");
    }
    if (term_count > 0 && !term_lengths) {
        throw std::invalid_argument("query text term lengths must not be null");
    }
    if (term_count > 0 && !term_bytes) {
        throw std::invalid_argument("query text term bytes must not be null");
    }
    size_t total_bytes = 0;
    for (uint32_t index = 0; index < term_count; ++index) {
        const auto length = term_lengths[index];
        if (length == 0 || length > MAX_TEXT_TERM_BYTES ||
            length > MAX_TOTAL_TEXT_TERM_BYTES - total_bytes) {
            throw std::invalid_argument("query text term bytes are too large or empty");
        }
        total_bytes += length;
    }
    if (total_bytes != term_bytes_length) {
        throw std::invalid_argument("query text term lengths do not match term bytes");
    }
    if (term_bytes_length > 0 && !term_bytes) {
        throw std::invalid_argument("query text term bytes must not be null");
    }
    std::vector<std::string> terms;
    terms.reserve(term_count);
    size_t term_offset = 0;
    for (uint32_t index = 0; index < term_count; ++index) {
        const auto length = term_lengths[index];
        terms.push_back(copy_text_bytes(term_bytes + term_offset, length,
                                        MAX_TEXT_TERM_BYTES, "query text term"));
        term_offset += length;
    }
    if (kind == SK_QUERY_TEXT_TERM) {
        query.builder.addStringTerm(terms[0], field, query.allocate_term_id(), Weight(1));
    } else {
        query.builder.addPhrase(static_cast<int>(terms.size()), field,
                                query.allocate_term_id(), Weight(1));
        for (const auto &term : terms) {
            query.builder.addStringTerm(term, field, query.allocate_term_id(), Weight(1));
        }
    }
}

std::unique_ptr<search::NumericRangeSpec> make_query_range(
    uint32_t lower_kind, int64_t lower_value,
    uint32_t upper_kind, int64_t upper_value) {
    auto spec = std::make_unique<search::NumericRangeSpec>();
    spec->valid = true;
    spec->valid_integers = true;
    const auto set_bound = [](uint32_t kind, int64_t value, bool lower,
                              search::NumericRangeSpec &range) {
        auto &integer_limit = lower ? range.int64_lower_limit : range.int64_upper_limit;
        auto &fp_limit = lower ? range.fp_lower_limit : range.fp_upper_limit;
        auto &inclusive = lower ? range.lower_inclusive : range.upper_inclusive;
        if (kind == SK_QUERY_BOUND_UNBOUNDED) {
            integer_limit = lower ? std::numeric_limits<int64_t>::min()
                                   : std::numeric_limits<int64_t>::max();
            fp_limit = lower ? -std::numeric_limits<double>::infinity()
                              : std::numeric_limits<double>::infinity();
            inclusive = true;
        } else if (kind == SK_QUERY_BOUND_INCLUDED || kind == SK_QUERY_BOUND_EXCLUDED) {
            integer_limit = value;
            fp_limit = value;
            inclusive = (kind == SK_QUERY_BOUND_INCLUDED);
        } else {
            throw std::invalid_argument("unsupported query range bound");
        }
    };
    set_bound(lower_kind, lower_value, true, *spec);
    set_bound(upper_kind, upper_value, false, *spec);
    return spec;
}

} // namespace

struct sk_engine {
    std::unique_ptr<EmbeddedEngine> impl;
};
struct sk_document : ValueDocument {};
struct sk_document_update : ValueUpdate {};
struct sk_search_request : ValueRequest {};
struct sk_search_result : ErrorResult {};
struct sk_text_document : TextDocument {};
struct sk_text_update : TextUpdate {};
struct sk_text_request : TextRequest {};
struct sk_text_result : TextResult {};
struct sk_query : NativeQueryState {};
struct sk_local_search_request : LocalSearchRequestState {};
struct sk_local_search_result : LocalSearchResultState {};
struct sk_attribute_document : AttributeDocumentState {};
struct sk_attribute_update : AttributeUpdateState {};
struct sk_local_grouping_request : LocalGroupingRequestState {};
struct sk_local_grouping_result : LocalGroupingResultState {};

extern "C" uint32_t sk_native_abi_version(void) SK_NOEXCEPT {
    return SK_NATIVE_ABI_VERSION;
}

extern "C" int32_t sk_engine_open(const char *data_dir, const char *config_dir,
                                   sk_engine **out_engine, uint8_t *error,
                                   uint32_t error_capacity) SK_NOEXCEPT {
    if (out_engine) {
        *out_engine = nullptr;
    }
    if (!out_engine || !data_dir || !config_dir) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "engine open arguments must not be null");
    }
    return invoke(error, error_capacity, [&] {
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        TimingProbe::Scope open_timing("open_total");
#endif
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        OPEN_FAILPOINTS.hit("open_before_native_create");
#endif
        std::error_code filesystem_error;
        const std::filesystem::path data_path(data_dir);
        if (std::filesystem::exists(data_path, filesystem_error) &&
            !std::filesystem::is_directory(data_path, filesystem_error)) {
            throw std::invalid_argument("engine data path is not a directory: " + data_path.string());
        }
        if (filesystem_error) {
            throw std::runtime_error("could not inspect engine data path '" + data_path.string() + "': " +
                                     filesystem_error.message());
        }
        const std::filesystem::path config_path(config_dir);
        if (!std::filesystem::is_directory(config_path, filesystem_error)) {
            if (filesystem_error) {
                throw std::runtime_error("could not inspect engine config path '" + config_path.string() + "': " +
                                         filesystem_error.message());
            }
            throw std::invalid_argument("engine config path is not a directory: " + config_path.string());
        }
        auto engine = std::make_unique<sk_engine>();
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        {
            TimingProbe::Scope constructor_timing("constructor");
#endif
        engine->impl = std::make_unique<EmbeddedEngine>(data_dir,
                                                        config_path.string());
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        }
#endif
        engine->impl->initialize(data_dir, config_path.string());
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        OPEN_FAILPOINTS.hit("open_after_native_create");
#endif
        *out_engine = engine.release();
    });
}

extern "C" int32_t sk_engine_close(sk_engine *engine, uint8_t *error,
                                    uint32_t error_capacity) SK_NOEXCEPT {
    if (!engine || !engine->impl) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "engine close arguments are invalid");
    }
    return invoke(error, error_capacity, [&] { engine->impl->close(); });
}

extern "C" void sk_engine_destroy(sk_engine *engine) SK_NOEXCEPT {
    try {
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        if (engine && engine->impl) {
            try {
                engine->impl->failpoint("destroy_before_native_release");
            } catch (...) {
            }
        }
#endif
        delete engine;
    } catch (...) {
    }
}

extern "C" int32_t sk_document_create(const char *id, const char *field,
                                        int64_t value, sk_document **out,
                                        uint8_t *error,
                                        uint32_t error_capacity) SK_NOEXCEPT {
    if (out) {
        *out = nullptr;
    }
    if (!out || !id || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "document arguments must not be null");
    }
    return invoke(error, error_capacity, [&] {
        validate_document_id(id);
        validate_field_name(field);
        if (*id == '\0' || *field == '\0') {
            throw std::invalid_argument("document ID and field must not be empty");
        }
        auto document = std::make_unique<sk_document>();
        document->id = id;
        document->field = field;
        document->value = value;
        *out = document.release();
    });
}

extern "C" void sk_document_destroy(sk_document *document) SK_NOEXCEPT {
    destroy_noexcept(document);
}

extern "C" SK_API int32_t sk_document_create_empty(const char *id, sk_document **out,
                                              uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out) *out = nullptr;
    if (!out || !id) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                                      "document arguments must not be null");
    return invoke(error, error_capacity, [&] {
        validate_document_id(id);
        if (*id == '\0') throw std::invalid_argument("document ID must not be empty");
        auto document = std::make_unique<sk_document>();
        document->id = id;
        *out = document.release();
    });
}

template <typename Add>
int32_t add_document_value(sk_document *document, const char *field,
                           uint8_t *error, uint32_t error_capacity, Add &&add) noexcept {
    if (!document || !field) return set_error(error, error_capacity,
                                              SK_NATIVE_STATUS_INVALID_ARGUMENT,
                                              "document value arguments are invalid");
    return invoke(error, error_capacity, [&] {
        validate_field_name(field);
        if (*field == '\0') throw std::invalid_argument("document field must not be empty");
        OwnedValue value = add();
        auto existing = std::find_if(document->fields.begin(), document->fields.end(),
                                     [field](const auto &entry) { return entry.name == field; });
        if (existing != document->fields.end()) {
            existing->value = std::move(value);
        } else {
            document->fields.push_back({field, std::move(value), {}});
        }
    });
}

extern "C" SK_API int32_t sk_document_add_string(sk_document *document, const char *field,
                                            const char *value, uint8_t *error,
                                            uint32_t error_capacity) SK_NOEXCEPT {
    if (!value) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                                 "document string value must not be null");
    return add_document_value(document, field, error, error_capacity, [&] {
        OwnedValue result; result.kind = SK_VALUE_STRING; result.string = value; return result;
    });
}

extern "C" SK_API int32_t sk_document_add_integer(sk_document *document, const char *field,
                                             int64_t value, uint8_t *error,
                                             uint32_t error_capacity) SK_NOEXCEPT {
    return add_document_value(document, field, error, error_capacity, [&] {
        OwnedValue result; result.kind = SK_VALUE_INTEGER; result.integer = value; return result;
    });
}

extern "C" SK_API int32_t sk_document_add_float(sk_document *document, const char *field,
                                           double value, uint8_t *error,
                                           uint32_t error_capacity) SK_NOEXCEPT {
    return add_document_value(document, field, error, error_capacity, [&] {
        OwnedValue result; result.kind = SK_VALUE_FLOAT; result.floating = value; return result;
    });
}

extern "C" SK_API int32_t sk_document_add_boolean(sk_document *document, const char *field,
                                             uint8_t value, uint8_t *error,
                                             uint32_t error_capacity) SK_NOEXCEPT {
    return add_document_value(document, field, error, error_capacity, [&] {
        OwnedValue result; result.kind = SK_VALUE_BOOLEAN; result.boolean = value != 0; return result;
    });
}

extern "C" SK_API int32_t sk_document_annotate_text(
    sk_document *document, const char *field, const uint8_t *term_bytes,
    uint32_t term_bytes_length, const uint32_t *term_lengths,
    const uint32_t *byte_starts, const uint32_t *byte_lengths,
    uint32_t occurrence_count, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!document || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "document annotation arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        auto existing = std::find_if(document->fields.begin(), document->fields.end(),
                                     [field](const auto &entry) { return entry.name == field; });
        if (existing == document->fields.end()) {
            throw std::invalid_argument("document text field must be added before annotation");
        }
        if (existing->value.kind != SK_VALUE_STRING) {
            throw std::invalid_argument("document annotation requires a string field");
        }
        TextDocument analyzed;
        analyzed.text = existing->value.string;
        append_text_occurrences(analyzed, term_bytes, term_bytes_length, term_lengths,
                                byte_starts, byte_lengths, occurrence_count);
        existing->occurrences = std::move(analyzed.occurrences);
    });
}

extern "C" int32_t sk_document_update_create(const char *id, const char *field,
                                               int64_t value,
                                               sk_document_update **out,
                                               uint8_t *error,
                                               uint32_t error_capacity) SK_NOEXCEPT {
    if (out) {
        *out = nullptr;
    }
    if (!out || !id || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "update arguments must not be null");
    }
    return invoke(error, error_capacity, [&] {
        validate_document_id(id);
        validate_field_name(field);
        if (*id == '\0' || *field == '\0') {
            throw std::invalid_argument("document ID and field must not be empty");
        }
        auto update = std::make_unique<sk_document_update>();
        update->id = id;
        update->field = field;
        update->value = value;
        *out = update.release();
    });
}

extern "C" void sk_document_update_destroy(sk_document_update *update) SK_NOEXCEPT {
    destroy_noexcept(update);
}

extern "C" SK_API int32_t sk_document_update_create_empty(
    const char *id, sk_document_update **out, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out) *out = nullptr;
    if (!out || !id) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                                      "update arguments must not be null");
    return invoke(error, error_capacity, [&] {
        validate_document_id(id);
        if (*id == '\0') throw std::invalid_argument("document ID must not be empty");
        auto update = std::make_unique<sk_document_update>();
        update->id = id;
        *out = update.release();
    });
}

template <typename Add>
int32_t add_update_value(sk_document_update *update, const char *field,
                         uint8_t *error, uint32_t error_capacity, Add &&add) noexcept {
    if (!update || !field) return set_error(error, error_capacity,
                                            SK_NATIVE_STATUS_INVALID_ARGUMENT,
                                            "update value arguments are invalid");
    return invoke(error, error_capacity, [&] {
        validate_field_name(field);
        if (*field == '\0') throw std::invalid_argument("update field must not be empty");
        auto value = add();
        auto existing = std::find_if(update->fields.begin(), update->fields.end(),
                                     [field](const auto &entry) { return entry.first == field; });
        if (existing != update->fields.end()) existing->second = std::move(value);
        else update->fields.emplace_back(field, std::move(value));
    });
}

extern "C" SK_API int32_t sk_document_update_assign_string(
    sk_document_update *update, const char *field, const char *value,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!value) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                                 "update string value must not be null");
    return add_update_value(update, field, error, error_capacity, [&] {
        OwnedValue result; result.kind = SK_VALUE_STRING; result.string = value; return std::optional<OwnedValue>(std::move(result));
    });
}

extern "C" SK_API int32_t sk_document_update_assign_integer(
    sk_document_update *update, const char *field, int64_t value,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    return add_update_value(update, field, error, error_capacity, [&] {
        OwnedValue result; result.kind = SK_VALUE_INTEGER; result.integer = value; return std::optional<OwnedValue>(std::move(result));
    });
}

extern "C" SK_API int32_t sk_document_update_assign_float(
    sk_document_update *update, const char *field, double value,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    return add_update_value(update, field, error, error_capacity, [&] {
        OwnedValue result; result.kind = SK_VALUE_FLOAT; result.floating = value; return std::optional<OwnedValue>(std::move(result));
    });
}

extern "C" SK_API int32_t sk_document_update_assign_boolean(
    sk_document_update *update, const char *field, uint8_t value,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    return add_update_value(update, field, error, error_capacity, [&] {
        OwnedValue result; result.kind = SK_VALUE_BOOLEAN; result.boolean = value != 0; return std::optional<OwnedValue>(std::move(result));
    });
}

extern "C" SK_API int32_t sk_document_update_clear(sk_document_update *update, const char *field,
                                              uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    return add_update_value(update, field, error, error_capacity,
                            [] { return std::optional<OwnedValue>(); });
}

extern "C" SK_API int32_t sk_document_update_annotate_text(
    sk_document_update *update, const char *field, const uint8_t *term_bytes,
    uint32_t term_bytes_length, const uint32_t *term_lengths,
    const uint32_t *byte_starts, const uint32_t *byte_lengths,
    uint32_t occurrence_count, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!update || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "update annotation arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        auto existing = std::find_if(update->fields.begin(), update->fields.end(),
                                     [field](const auto &entry) { return entry.first == field; });
        if (existing == update->fields.end() || !existing->second ||
            existing->second->kind != SK_VALUE_STRING) {
            throw std::invalid_argument("a string assignment is required before annotation");
        }
        TextDocument analyzed;
        analyzed.text = existing->second->string;
        append_text_occurrences(analyzed, term_bytes, term_bytes_length, term_lengths,
                                byte_starts, byte_lengths, occurrence_count);
        auto annotated = std::find_if(update->annotated_fields.begin(),
                                      update->annotated_fields.end(),
                                      [field](const auto &entry) { return entry.first == field; });
        if (annotated == update->annotated_fields.end()) {
            update->annotated_fields.emplace_back(field, std::move(analyzed.occurrences));
        } else {
            annotated->second = std::move(analyzed.occurrences);
        }
    });
}

extern "C" int32_t sk_search_request_create(const char *field, int64_t value,
                                              uint32_t limit,
                                              sk_search_request **out,
                                              uint8_t *error,
                                              uint32_t error_capacity) SK_NOEXCEPT {
    if (out) {
        *out = nullptr;
    }
    if (!out || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "search arguments must not be null");
    }
    return invoke(error, error_capacity, [&] {
        validate_field_name(field);
        if (*field == '\0') {
            throw std::invalid_argument("search field must not be empty");
        }
        if (limit > MAX_SEARCH_LIMIT) {
            throw std::invalid_argument("search limit is too large");
        }
        auto request = std::make_unique<sk_search_request>();
        request->field = field;
        request->value = value;
        request->limit = limit;
        *out = request.release();
    });
}

extern "C" void sk_search_request_destroy(sk_search_request *request) SK_NOEXCEPT {
    destroy_noexcept(request);
}

extern "C" int32_t sk_engine_put(sk_engine *engine, const sk_document *document,
                                  uint8_t *error,
                                  uint32_t error_capacity) SK_NOEXCEPT {
    if (!engine || !engine->impl || !document) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "put arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        if (!document->fields.empty()) {
            engine->impl->put_document(*document);
        } else {
            engine->impl->put(document->id, document->field, document->value);
        }
    });
}

extern "C" int32_t sk_engine_update(sk_engine *engine,
                                     const sk_document_update *update,
                                     uint8_t *error,
                                     uint32_t error_capacity) SK_NOEXCEPT {
    if (!engine || !engine->impl || !update) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "update arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        if (!update->fields.empty()) {
            engine->impl->update_document(*update);
        } else {
            engine->impl->update(update->id, update->field, update->value);
        }
    });
}

extern "C" SK_API int32_t sk_engine_get(const sk_engine *engine, const char *id,
                                  sk_document **out_document, uint8_t *error,
                                  uint32_t error_capacity) SK_NOEXCEPT {
    if (out_document) *out_document = nullptr;
    if (!engine || !engine->impl || !id || !out_document) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "get arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        auto document = engine->impl->get_value_document_for_abi(id);
        if (!document.fields.empty()) {
            *out_document = std::make_unique<sk_document>(std::move(document)).release();
        }
    });
}

extern "C" int32_t sk_document_field_count(const sk_document *document,
                                             uint32_t *out_count, uint8_t *error,
                                             uint32_t error_capacity) SK_NOEXCEPT {
    if (!document || !out_count) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "document field count arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        if (document->fields.size() > std::numeric_limits<uint32_t>::max()) {
            throw std::runtime_error("document field count exceeds native capacity");
        }
        *out_count = static_cast<uint32_t>(document->fields.size());
    });
}

extern "C" int32_t sk_document_field(const sk_document *document, uint32_t index,
                                      const char **out_name, uint32_t *out_name_length,
                                      uint32_t *out_kind, const char **out_string,
                                      uint32_t *out_string_length, int64_t *out_integer,
                                      double *out_float, uint8_t *out_boolean,
                                      uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!document || !out_name || !out_name_length || !out_kind || !out_string ||
        !out_string_length || !out_integer || !out_float || !out_boolean) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "document field arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        if (index >= document->fields.size()) {
            throw std::invalid_argument("document field index is out of range");
        }
        const auto &field = document->fields[index];
        if (field.name.size() > std::numeric_limits<uint32_t>::max() ||
            field.value.string.size() > std::numeric_limits<uint32_t>::max()) {
            throw std::runtime_error("document field value exceeds native capacity");
        }
        *out_name = field.name.data();
        *out_name_length = static_cast<uint32_t>(field.name.size());
        *out_kind = field.value.kind;
        *out_string = field.value.string.data();
        *out_string_length = static_cast<uint32_t>(field.value.string.size());
        *out_integer = field.value.integer;
        *out_float = field.value.floating;
        *out_boolean = field.value.boolean ? 1 : 0;
    });
}

extern "C" int32_t sk_engine_remove(sk_engine *engine, const char *id,
                                     int32_t *out_was_found, uint8_t *error,
                                     uint32_t error_capacity) SK_NOEXCEPT {
    if (out_was_found) {
        *out_was_found = 0;
    }
    if (!engine || !engine->impl || !id || !out_was_found) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "remove arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        validate_document_id(id);
        *out_was_found = engine->impl->remove(id) ? 1 : 0;
    });
}

extern "C" int32_t sk_engine_search(const sk_engine *engine,
                                     const sk_search_request *request,
                                     sk_search_result **out_result,
                                     uint8_t *error,
                                     uint32_t error_capacity) SK_NOEXCEPT {
    if (out_result) {
        *out_result = nullptr;
    }
    if (!engine || !engine->impl || !request || !out_result) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "search arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        if (request->limit == 0) {
            throw std::invalid_argument("search limit must be greater than zero");
        }
        auto result = std::make_unique<sk_search_result>();
        result->ids = engine->impl->search(request->field, request->value,
                                           request->limit);
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        engine->impl->failpoint("result_after_allocation");
        result->failpoints = &engine->impl->failpoints();
#endif
        *out_result = result.release();
    });
}

extern "C" uint32_t sk_search_result_size(const sk_search_result *result) SK_NOEXCEPT {
    return result ? static_cast<uint32_t>(result->ids.size()) : 0;
}

extern "C" int32_t sk_search_result_id(const sk_search_result *result,
                                        uint32_t index, const char **out_id,
                                        uint32_t *out_length, uint8_t *error,
                                        uint32_t error_capacity) SK_NOEXCEPT {
    if (out_id) {
        *out_id = nullptr;
    }
    if (out_length) {
        *out_length = 0;
    }
    if (!result || !out_id || !out_length) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "result arguments are invalid");
    }
    if (index >= result->ids.size()) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "result index is out of bounds");
    }
    return invoke(error, error_capacity, [&] {
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        if (result->failpoints) {
            result->failpoints->hit("result_before_copy");
        }
#endif
        *out_id = result->ids[index].data();
        *out_length = static_cast<uint32_t>(result->ids[index].size());
    });
}

extern "C" void sk_search_result_destroy(sk_search_result *result) SK_NOEXCEPT {
    destroy_noexcept(result);
}

template <typename Container>
void append_owned_value(Container &container, const char *field, OwnedValue value) {
    validate_field_name(field);
    if (*field == '\0') throw std::invalid_argument("attribute field must not be empty");
    container.emplace_back(field, std::move(value));
}

extern "C" int32_t sk_local_search_request_create(
    const char *rank_profile, uint32_t limit, uint32_t offset,
    uint32_t summary_kind, sk_local_search_request **out_request,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_request) *out_request = nullptr;
    if (!rank_profile || !out_request) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "local search request arguments are invalid");
    return invoke(error, error_capacity, [&] {
        validate_rank_profile(rank_profile);
        if (*rank_profile == '\0') throw std::invalid_argument("rank profile must not be empty");
        if (limit > MAX_SEARCH_LIMIT) throw std::invalid_argument("search limit is too large");
        if (summary_kind != SK_SUMMARY_NONE && summary_kind != SK_SUMMARY_DEFAULT && summary_kind != SK_SUMMARY_FIELDS) throw std::invalid_argument("unsupported summary kind");
        auto request = std::make_unique<sk_local_search_request>();
        request->rank_profile = rank_profile;
        request->limit = limit;
        request->offset = offset;
        request->summary_kind = summary_kind;
        *out_request = request.release();
    });
}

extern "C" int32_t sk_local_search_request_add_sort(
    sk_local_search_request *request, const char *field, uint32_t order,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!request || !field) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "local sort arguments are invalid");
    return invoke(error, error_capacity, [&] {
        validate_field_name(field);
        if (order != SK_SORT_ASCENDING && order != SK_SORT_DESCENDING) throw std::invalid_argument("unsupported sort order");
        request->sort.emplace_back(field, order);
    });
}

extern "C" int32_t sk_local_search_request_add_summary_field(
    sk_local_search_request *request, const char *field,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!request || !field) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "local summary arguments are invalid");
    return invoke(error, error_capacity, [&] {
        validate_field_name(field);
        if (request->summary_kind != SK_SUMMARY_FIELDS) throw std::invalid_argument("summary fields require Fields summary mode");
        if (std::find(request->summary_fields.begin(), request->summary_fields.end(), field) != request->summary_fields.end()) throw std::invalid_argument("summary fields must not contain duplicates");
        request->summary_fields.emplace_back(field);
    });
}

extern "C" void sk_local_search_request_destroy(sk_local_search_request *request) SK_NOEXCEPT { destroy_noexcept(request); }

extern "C" int32_t sk_engine_search_local(
    const sk_engine *engine, const sk_query *query,
    const sk_local_search_request *request, sk_local_search_result **out_result,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_result) *out_result = nullptr;
    if (!engine || !engine->impl || !query || !request || !out_result) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "local search arguments are invalid");
    return invoke(error, error_capacity, [&] {
        auto result = std::make_unique<sk_local_search_result>();
        static_cast<LocalSearchResultState &>(*result) = engine->impl->search_local_result(*query, *request);
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        engine->impl->failpoint("result_after_allocation");
        result->failpoints = &engine->impl->failpoints();
#endif
        *out_result = result.release();
    });
}

extern "C" uint64_t sk_local_search_result_total_hit_count(const sk_local_search_result *result) SK_NOEXCEPT { return result ? result->total_hit_count : 0; }
extern "C" uint32_t sk_local_search_result_hit_count(const sk_local_search_result *result) SK_NOEXCEPT { return result ? static_cast<uint32_t>(result->hits.size()) : 0; }

extern "C" int32_t sk_local_search_result_hit(
    const sk_local_search_result *result, uint32_t index, const char **out_id,
    uint32_t *out_id_length, double *out_score, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out_id) *out_id = nullptr;
    if (out_id_length) *out_id_length = 0;
    if (out_score) *out_score = 0.0;
    if (!result || !out_id || !out_id_length || !out_score) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "local hit arguments are invalid");
    if (index >= result->hits.size()) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "local hit index is out of bounds");
    return invoke(error, error_capacity, [&] {
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        if (result->failpoints) result->failpoints->hit("result_before_copy");
#endif
        *out_id = result->hits[index].id.data();
        *out_id_length = static_cast<uint32_t>(result->hits[index].id.size());
        *out_score = result->hits[index].score;
    });
}

extern "C" uint32_t sk_local_search_result_summary_count(const sk_local_search_result *result, uint32_t hit_index) SK_NOEXCEPT {
    return result && hit_index < result->hits.size() ? static_cast<uint32_t>(result->hits[hit_index].summary.size()) : 0;
}

extern "C" int32_t sk_local_search_result_summary_name(
    const sk_local_search_result *result, uint32_t hit_index, uint32_t summary_index,
    const char **out_name, uint32_t *out_length, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out_name) *out_name = nullptr;
    if (out_length) *out_length = 0;
    if (!result || !out_name || !out_length || hit_index >= result->hits.size() || summary_index >= result->hits[hit_index].summary.size()) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "summary name arguments are invalid");
    return invoke(error, error_capacity, [&] {
        const auto &name = result->hits[hit_index].summary[summary_index].first;
        *out_name = name.data();
        *out_length = static_cast<uint32_t>(name.size());
    });
}

extern "C" int32_t sk_local_search_result_summary_value(
    const sk_local_search_result *result, uint32_t hit_index, uint32_t summary_index,
    uint32_t *out_kind, const char **out_string, uint32_t *out_string_length,
    int64_t *out_integer, double *out_float, uint8_t *out_boolean,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_kind) *out_kind = 0;
    if (out_string) *out_string = nullptr;
    if (out_string_length) *out_string_length = 0;
    if (out_integer) *out_integer = 0;
    if (out_float) *out_float = 0.0;
    if (out_boolean) *out_boolean = 0;
    if (!result || !out_kind || !out_string || !out_string_length || !out_integer || !out_float || !out_boolean || hit_index >= result->hits.size() || summary_index >= result->hits[hit_index].summary.size()) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "summary value arguments are invalid");
    return invoke(error, error_capacity, [&] {
        const auto &value = result->hits[hit_index].summary[summary_index].second;
        *out_kind = value.kind;
        *out_string = value.string.data();
        *out_string_length = static_cast<uint32_t>(value.string.size());
        *out_integer = value.integer;
        *out_float = value.floating;
        *out_boolean = value.boolean;
    });
}

extern "C" void sk_local_search_result_destroy(sk_local_search_result *result) SK_NOEXCEPT { destroy_noexcept(result); }

extern "C" int32_t sk_attribute_document_create(
    const char *id, sk_attribute_document **out, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out) *out = nullptr;
    if (!id || !out) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "attribute document arguments are invalid");
    return invoke(error, error_capacity, [&] { validate_document_id(id); auto document = std::make_unique<sk_attribute_document>(); document->id = id; *out = document.release(); });
}

extern "C" int32_t sk_attribute_document_add_string(sk_attribute_document *document, const char *field, const char *value, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!document || !field || !value) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "attribute string arguments are invalid");
    return invoke(error, error_capacity, [&] { OwnedValue v; v.kind = SK_VALUE_STRING; v.string = value; append_owned_value(document->fields, field, std::move(v)); });
}
extern "C" int32_t sk_attribute_document_add_integer(sk_attribute_document *document, const char *field, int64_t value, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!document || !field) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "attribute integer arguments are invalid");
    return invoke(error, error_capacity, [&] { OwnedValue v; v.kind = SK_VALUE_INTEGER; v.integer = value; append_owned_value(document->fields, field, std::move(v)); });
}
extern "C" int32_t sk_attribute_document_add_float(sk_attribute_document *document, const char *field, double value, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!document || !field) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "attribute float arguments are invalid");
    return invoke(error, error_capacity, [&] { if (!std::isfinite(value)) throw std::invalid_argument("attribute float value must be finite"); OwnedValue v; v.kind = SK_VALUE_FLOAT; v.floating = value; append_owned_value(document->fields, field, std::move(v)); });
}
extern "C" int32_t sk_attribute_document_add_boolean(sk_attribute_document *document, const char *field, uint8_t value, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!document || !field) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "attribute boolean arguments are invalid");
    return invoke(error, error_capacity, [&] { OwnedValue v; v.kind = SK_VALUE_BOOLEAN; v.boolean = value != 0; append_owned_value(document->fields, field, std::move(v)); });
}
extern "C" void sk_attribute_document_destroy(sk_attribute_document *document) SK_NOEXCEPT { destroy_noexcept(document); }

extern "C" int32_t sk_attribute_update_create(const char *id, sk_attribute_update **out, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out) *out = nullptr;
    if (!id || !out) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "attribute update arguments are invalid");
    return invoke(error, error_capacity, [&] { validate_document_id(id); auto update = std::make_unique<sk_attribute_update>(); update->id = id; *out = update.release(); });
}
extern "C" int32_t sk_attribute_update_add_string(sk_attribute_update *update, const char *field, const char *value, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!update || !field || !value) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "attribute string update arguments are invalid");
    return invoke(error, error_capacity, [&] { OwnedValue v; v.kind = SK_VALUE_STRING; v.string = value; append_owned_value(update->assignments, field, std::move(v)); });
}
extern "C" int32_t sk_attribute_update_add_integer(sk_attribute_update *update, const char *field, int64_t value, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!update || !field) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "attribute integer update arguments are invalid");
    return invoke(error, error_capacity, [&] { OwnedValue v; v.kind = SK_VALUE_INTEGER; v.integer = value; append_owned_value(update->assignments, field, std::move(v)); });
}
extern "C" int32_t sk_attribute_update_add_float(sk_attribute_update *update, const char *field, double value, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!update || !field) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "attribute float update arguments are invalid");
    return invoke(error, error_capacity, [&] { if (!std::isfinite(value)) throw std::invalid_argument("attribute float value must be finite"); OwnedValue v; v.kind = SK_VALUE_FLOAT; v.floating = value; append_owned_value(update->assignments, field, std::move(v)); });
}
extern "C" int32_t sk_attribute_update_add_boolean(sk_attribute_update *update, const char *field, uint8_t value, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!update || !field) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "attribute boolean update arguments are invalid");
    return invoke(error, error_capacity, [&] { OwnedValue v; v.kind = SK_VALUE_BOOLEAN; v.boolean = value != 0; append_owned_value(update->assignments, field, std::move(v)); });
}
extern "C" void sk_attribute_update_destroy(sk_attribute_update *update) SK_NOEXCEPT { destroy_noexcept(update); }

extern "C" int32_t sk_engine_put_attributes(sk_engine *engine, const sk_attribute_document *document, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!engine || !engine->impl || !document) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "attribute put arguments are invalid");
    return invoke(error, error_capacity, [&] { engine->impl->put_attributes(*document); });
}
extern "C" int32_t sk_engine_update_attributes(sk_engine *engine, const sk_attribute_update *update, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!engine || !engine->impl || !update) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "attribute update arguments are invalid");
    return invoke(error, error_capacity, [&] { engine->impl->update_attributes(*update); });
}

extern "C" int32_t sk_local_grouping_request_create(const char *field, sk_local_grouping_request **out, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out) *out = nullptr;
    if (!field || !out) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "grouping request arguments are invalid");
    return invoke(error, error_capacity, [&] { validate_field_name(field); auto request = std::make_unique<sk_local_grouping_request>(); request->group_field = field; *out = request.release(); });
}
extern "C" int32_t sk_local_grouping_request_add_aggregation(sk_local_grouping_request *request, uint32_t kind, const char *field, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!request || !field) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "grouping aggregation arguments are invalid");
    return invoke(error, error_capacity, [&] { validate_field_name(field); if (kind < SK_AGGREGATION_SUM || kind > SK_AGGREGATION_AVERAGE) throw std::invalid_argument("unsupported aggregation kind"); request->aggregations.push_back({kind, field}); });
}
extern "C" void sk_local_grouping_request_destroy(sk_local_grouping_request *request) SK_NOEXCEPT { destroy_noexcept(request); }
extern "C" int32_t sk_engine_group_local(const sk_engine *engine, const sk_query *query, const sk_local_grouping_request *request, sk_local_grouping_result **out, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out) *out = nullptr;
    if (!engine || !engine->impl || !query || !request || !out) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "local grouping arguments are invalid");
    return invoke(error, error_capacity, [&] { auto result = std::make_unique<sk_local_grouping_result>(); static_cast<LocalGroupingResultState &>(*result) = engine->impl->group_local_result(*query, *request); *out = result.release(); });
}
extern "C" uint32_t sk_local_grouping_result_group_count(const sk_local_grouping_result *result) SK_NOEXCEPT { return result ? static_cast<uint32_t>(result->groups.size()) : 0; }
extern "C" int32_t sk_local_grouping_result_group(const sk_local_grouping_result *result, uint32_t index, uint32_t *out_key_kind, const char **out_key_string, uint32_t *out_key_string_length, int64_t *out_key_integer, uint64_t *out_count, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_key_kind) *out_key_kind = 0;
    if (out_key_string) *out_key_string = nullptr;
    if (out_key_string_length) *out_key_string_length = 0;
    if (out_key_integer) *out_key_integer = 0;
    if (out_count) *out_count = 0;
    if (!result || !out_key_kind || !out_key_string || !out_key_string_length || !out_key_integer || !out_count || index >= result->groups.size()) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "group result arguments are invalid");
    return invoke(error, error_capacity, [&] { const auto &group = result->groups[index]; *out_key_kind = group.key.kind; *out_key_string = group.key.string.data(); *out_key_string_length = static_cast<uint32_t>(group.key.string.size()); *out_key_integer = group.key.integer; *out_count = group.count; });
}
extern "C" uint32_t sk_local_grouping_result_aggregation_count(const sk_local_grouping_result *result, uint32_t index) SK_NOEXCEPT { return result && index < result->groups.size() ? static_cast<uint32_t>(result->groups[index].aggregations.size()) : 0; }
extern "C" int32_t sk_local_grouping_result_aggregation(const sk_local_grouping_result *result, uint32_t group_index, uint32_t aggregation_index, uint32_t *out_kind, int64_t *out_integer, double *out_float, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_kind) *out_kind = 0;
    if (out_integer) *out_integer = 0;
    if (out_float) *out_float = 0.0;
    if (!result || !out_kind || !out_integer || !out_float || group_index >= result->groups.size() || aggregation_index >= result->groups[group_index].aggregations.size()) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "aggregation result arguments are invalid");
    return invoke(error, error_capacity, [&] { const auto &value = result->groups[group_index].aggregations[aggregation_index]; *out_kind = value.kind; *out_integer = value.integer; *out_float = value.floating; });
}
extern "C" void sk_local_grouping_result_destroy(sk_local_grouping_result *result) SK_NOEXCEPT { destroy_noexcept(result); }

namespace {

std::string copy_text_bytes(const uint8_t *bytes, uint32_t length,
                            size_t maximum, const char *what) {
    if (length > maximum) {
        throw std::invalid_argument(std::string(what) + " is too long");
    }
    if (length == 0) {
        return {};
    }
    if (bytes == nullptr) {
        throw std::invalid_argument(std::string(what) + " must not be null");
    }
    std::string result(reinterpret_cast<const char *>(bytes), length);
    if (!is_valid_utf8(result)) {
        throw std::invalid_argument(std::string(what) + " must be valid UTF-8");
    }
    return result;
}

void initialize_text_document(TextDocument &document, const char *id, const char *field,
                              const uint8_t *text, uint32_t text_length) {
    validate_document_id(id);
    validate_field_name(field);
    if (*id == '\0' || *field == '\0') {
        throw std::invalid_argument("text document ID and field must not be empty");
    }
    document.id = id;
    document.field = field;
    document.text = copy_text_bytes(text, text_length, MAX_TEXT_BYTES, "text");
}

void append_text_occurrences(TextDocument &document, const uint8_t *term_bytes,
                             uint32_t term_bytes_length, const uint32_t *term_lengths,
                             const uint32_t *byte_starts, const uint32_t *byte_lengths,
                             uint32_t occurrence_count) {
    if (document.occurrences_added) {
        throw std::invalid_argument("text occurrences may only be added once");
    }
    if (document.occurrences.size() > MAX_TEXT_OCCURRENCES ||
        occurrence_count > MAX_TEXT_OCCURRENCES - document.occurrences.size()) {
        throw std::invalid_argument("text occurrence count is too large");
    }
    if (occurrence_count == 0) {
        if (term_bytes_length != 0) {
            throw std::invalid_argument("text occurrence term bytes are not fully described");
        }
        document.occurrences_added = true;
        return;
    }
    if (!term_lengths || !byte_starts || !byte_lengths) {
        throw std::invalid_argument("text occurrence arrays must not be null");
    }
    if (term_bytes_length != 0 && !term_bytes) {
        throw std::invalid_argument("text occurrence term bytes must not be null");
    }
    size_t total_term_bytes = 0;
    for (uint32_t index = 0; index < occurrence_count; ++index) {
        if (term_lengths[index] == 0) {
            throw std::invalid_argument("text occurrence term must not be empty");
        }
        if (term_lengths[index] > MAX_TEXT_TERM_BYTES ||
            term_lengths[index] > MAX_TOTAL_TEXT_TERM_BYTES - total_term_bytes) {
            throw std::invalid_argument("text occurrence term bytes are too large");
        }
        total_term_bytes += term_lengths[index];
        if (byte_starts[index] > document.text.size() ||
            byte_lengths[index] > document.text.size() - byte_starts[index] ||
            !is_utf8_boundary(document.text, byte_starts[index]) ||
            !is_utf8_boundary(document.text, byte_starts[index] + byte_lengths[index])) {
            throw std::invalid_argument("text occurrence span is not on UTF-8 boundaries");
        }
    }
    if (total_term_bytes != term_bytes_length) {
        throw std::invalid_argument("text occurrence term lengths do not match term bytes");
    }

    std::vector<TextOccurrence> additions;
    additions.reserve(occurrence_count);
    size_t term_offset = 0;
    for (uint32_t index = 0; index < occurrence_count; ++index) {
        const auto term_length = term_lengths[index];
        additions.push_back({
            copy_text_bytes(term_bytes + term_offset, term_length,
                            MAX_TEXT_TERM_BYTES, "text occurrence term"),
            byte_starts[index],
            byte_lengths[index],
            static_cast<uint32_t>(document.occurrences.size() + index)});
        term_offset += term_length;
    }
    document.occurrences.insert(document.occurrences.end(),
                                std::make_move_iterator(additions.begin()),
                                std::make_move_iterator(additions.end()));
    document.occurrences_added = true;
}

void initialize_text_request(TextRequest &request, uint32_t kind, const char *field,
                             const char *rank_profile) {
    validate_field_name(field);
    validate_rank_profile(rank_profile);
    if (*field == '\0' || *rank_profile == '\0') {
        throw std::invalid_argument("text query field and rank profile must not be empty");
    }
    if (kind != SK_TEXT_QUERY_TERM && kind != SK_TEXT_QUERY_PHRASE) {
        throw std::invalid_argument("unsupported text query kind");
    }
    request.kind = kind;
    request.field = field;
    request.rank_profile = rank_profile;
}

void append_text_terms(TextRequest &request, const uint8_t *term_bytes,
                       uint32_t term_bytes_length, const uint32_t *term_lengths,
                       uint32_t term_count) {
    if (request.terms_added) {
        throw std::invalid_argument("text query terms may only be added once");
    }
    if (request.terms.size() > MAX_TEXT_TERMS ||
        term_count > MAX_TEXT_TERMS - request.terms.size()) {
        throw std::invalid_argument("text term count is too large");
    }
    const auto resulting_count = request.terms.size() + term_count;
    if ((request.kind == SK_TEXT_QUERY_TERM && resulting_count != 1) ||
        (request.kind == SK_TEXT_QUERY_PHRASE && resulting_count == 0)) {
        throw std::invalid_argument("invalid number of terms for text query kind");
    }
    if (term_count == 0) {
        if (term_bytes_length != 0) {
            throw std::invalid_argument("text query term bytes are not fully described");
        }
        request.terms_added = true;
        return;
    }
    if (!term_lengths) {
        throw std::invalid_argument("text query term lengths must not be null");
    }
    if (term_bytes_length != 0 && !term_bytes) {
        throw std::invalid_argument("text query term bytes must not be null");
    }
    size_t total_term_bytes = 0;
    for (uint32_t index = 0; index < term_count; ++index) {
        if (term_lengths[index] == 0) {
            throw std::invalid_argument("text query term must not be empty");
        }
        if (term_lengths[index] > MAX_TEXT_TERM_BYTES ||
            term_lengths[index] > MAX_TOTAL_TEXT_TERM_BYTES - total_term_bytes) {
            throw std::invalid_argument("text query term bytes are too large");
        }
        total_term_bytes += term_lengths[index];
    }
    if (total_term_bytes != term_bytes_length) {
        throw std::invalid_argument("text query term lengths do not match term bytes");
    }
    std::vector<std::string> additions;
    additions.reserve(term_count);
    size_t term_offset = 0;
    for (uint32_t index = 0; index < term_count; ++index) {
        const auto term_length = term_lengths[index];
        additions.push_back(copy_text_bytes(term_bytes + term_offset, term_length,
                                            MAX_TEXT_TERM_BYTES, "text query term"));
        term_offset += term_length;
    }
    request.terms.insert(request.terms.end(),
                         std::make_move_iterator(additions.begin()),
                         std::make_move_iterator(additions.end()));
    request.positions.reserve(request.terms.size());
    for (size_t index = request.positions.size(); index < request.terms.size(); ++index) {
        request.positions.push_back(static_cast<uint32_t>(index));
    }
    request.terms_added = true;
}

} // namespace

extern "C" int32_t sk_text_document_create(
    const char *id, const char *field, const uint8_t *text, uint32_t text_length,
    sk_text_document **out, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out) {
        *out = nullptr;
    }
    if (!out || !id || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "text document arguments must not be null");
    }
    return invoke(error, error_capacity, [&] {
        auto document = std::make_unique<sk_text_document>();
        initialize_text_document(*document, id, field, text, text_length);
        *out = document.release();
    });
}

extern "C" int32_t sk_text_document_add_occurrences(
    sk_text_document *document, const uint8_t *term_bytes,
    uint32_t term_bytes_length, const uint32_t *term_lengths,
    const uint32_t *byte_starts, const uint32_t *byte_lengths,
    uint32_t occurrence_count, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!document) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "text document is null");
    }
    return invoke(error, error_capacity, [&] {
        append_text_occurrences(*document, term_bytes, term_bytes_length, term_lengths,
                                byte_starts, byte_lengths, occurrence_count);
    });
}

extern "C" void sk_text_document_destroy(sk_text_document *document) SK_NOEXCEPT {
    destroy_noexcept(document);
}

extern "C" int32_t sk_text_update_create(
    const char *id, const char *field, const uint8_t *text, uint32_t text_length,
    sk_text_update **out, uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out) {
        *out = nullptr;
    }
    if (!out || !id || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "text update arguments must not be null");
    }
    return invoke(error, error_capacity, [&] {
        auto update = std::make_unique<sk_text_update>();
        initialize_text_document(*update, id, field, text, text_length);
        *out = update.release();
    });
}

extern "C" int32_t sk_text_update_add_occurrences(
    sk_text_update *update, const uint8_t *term_bytes,
    uint32_t term_bytes_length, const uint32_t *term_lengths,
    const uint32_t *byte_starts, const uint32_t *byte_lengths,
    uint32_t occurrence_count, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!update) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "text update is null");
    }
    return invoke(error, error_capacity, [&] {
        append_text_occurrences(*update, term_bytes, term_bytes_length, term_lengths,
                                byte_starts, byte_lengths, occurrence_count);
    });
}

extern "C" void sk_text_update_destroy(sk_text_update *update) SK_NOEXCEPT {
    destroy_noexcept(update);
}

extern "C" int32_t sk_text_request_create(
    uint32_t kind, const char *field, const char *rank_profile,
    sk_text_request **out, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (out) {
        *out = nullptr;
    }
    if (!out || !field || !rank_profile) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "text request arguments must not be null");
    }
    return invoke(error, error_capacity, [&] {
        auto request = std::make_unique<sk_text_request>();
        initialize_text_request(*request, kind, field, rank_profile);
        *out = request.release();
    });
}

extern "C" int32_t sk_text_request_add_terms(
    sk_text_request *request, const uint8_t *term_bytes,
    uint32_t term_bytes_length, const uint32_t *term_lengths,
    uint32_t term_count, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!request) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "text request is null");
    }
    return invoke(error, error_capacity, [&] {
        append_text_terms(*request, term_bytes, term_bytes_length, term_lengths, term_count);
    });
}

extern "C" void sk_text_request_destroy(sk_text_request *request) SK_NOEXCEPT {
    destroy_noexcept(request);
}

extern "C" int32_t sk_engine_put_text(sk_engine *engine,
                                        const sk_text_document *document,
                                        uint8_t *error,
                                        uint32_t error_capacity) SK_NOEXCEPT {
    if (!engine || !engine->impl || !document) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "text put arguments are invalid");
    }
    return invoke(error, error_capacity, [&] { engine->impl->put_text(*document); });
}

extern "C" int32_t sk_engine_update_text(sk_engine *engine,
                                           const sk_text_update *update,
                                           uint8_t *error,
                                           uint32_t error_capacity) SK_NOEXCEPT {
    if (!engine || !engine->impl || !update) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "text update arguments are invalid");
    }
    return invoke(error, error_capacity, [&] { engine->impl->update_text(*update); });
}

extern "C" int32_t sk_engine_search_text(const sk_engine *engine,
                                           const sk_text_request *request,
                                           sk_text_result **out_result,
                                           uint8_t *error,
                                           uint32_t error_capacity) SK_NOEXCEPT {
    if (out_result) {
        *out_result = nullptr;
    }
    if (!engine || !engine->impl || !request || !out_result) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "text search arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        auto result = std::make_unique<sk_text_result>();
        result->hits = engine->impl->search_text(*request);
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        engine->impl->failpoint("result_after_allocation");
        result->failpoints = &engine->impl->failpoints();
#endif
        *out_result = result.release();
    });
}

extern "C" uint32_t sk_text_result_size(const sk_text_result *result) SK_NOEXCEPT {
    return result ? static_cast<uint32_t>(result->hits.size()) : 0;
}

extern "C" int32_t sk_text_result_hit(const sk_text_result *result,
                                       uint32_t index, const char **out_id,
                                       uint32_t *out_length, double *out_score,
                                       uint8_t *error,
                                       uint32_t error_capacity) SK_NOEXCEPT {
    if (out_id) {
        *out_id = nullptr;
    }
    if (out_length) {
        *out_length = 0;
    }
    if (out_score) {
        *out_score = 0.0;
    }
    if (!result || !out_id || !out_length || !out_score) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "text result arguments are invalid");
    }
    if (index >= result->hits.size()) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "text result index is out of bounds");
    }
    return invoke(error, error_capacity, [&] {
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        if (result->failpoints) {
            result->failpoints->hit("result_before_copy");
        }
#endif
        *out_id = result->hits[index].id.data();
        *out_length = static_cast<uint32_t>(result->hits[index].id.size());
        *out_score = result->hits[index].score;
    });
}

extern "C" void sk_text_result_destroy(sk_text_result *result) SK_NOEXCEPT {
    destroy_noexcept(result);
}

extern "C" int32_t sk_query_create(const sk_engine *engine, sk_query **out_query,
                                    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (out_query) {
        *out_query = nullptr;
    }
    if (!engine || !engine->impl || !out_query) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "query create arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        auto query = std::make_unique<sk_query>();
        query->fields = engine->impl->query_field_metadata();
        *out_query = query.release();
    });
}

extern "C" int32_t sk_query_add_operator(sk_query *query, uint32_t kind,
                                           uint32_t child_count, uint8_t *error,
                                           uint32_t error_capacity) SK_NOEXCEPT {
    if (!query) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "query operator arguments are invalid");
    }
    return invoke(error, error_capacity, [&] { add_query_operator(*query, kind, child_count); });
}

extern "C" int32_t sk_query_validate_field(const sk_query *query, uint32_t kind,
                                              const char *field, uint8_t *error,
                                              uint32_t error_capacity) SK_NOEXCEPT {
    if (!query) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "query field arguments are invalid");
    }
    return invoke(error, error_capacity,
                  [&] { validate_query_field(*query, kind, field); });
}

extern "C" int32_t sk_query_add_true(sk_query *query, uint8_t *error,
                                      uint32_t error_capacity) SK_NOEXCEPT {
    if (!query) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "query true arguments are invalid");
    }
    return invoke(error, error_capacity, [&] { query->builder.add_true_node(); });
}

extern "C" int32_t sk_query_add_text(sk_query *query, uint32_t kind,
                                      const char *field, const uint8_t *term_bytes,
                                      uint32_t term_bytes_length,
                                      const uint32_t *term_lengths,
                                      uint32_t term_count, uint8_t *error,
                                      uint32_t error_capacity) SK_NOEXCEPT {
    if (!query) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "query text arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        add_query_text(*query, kind, field, term_bytes, term_bytes_length,
                       term_lengths, term_count);
    });
}

extern "C" int32_t sk_query_add_equals(sk_query *query, const char *field,
                                        int64_t value, uint8_t *error,
                                        uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "query equality arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        validate_field_name(field);
        validate_query_field(*query, SK_QUERY_FIELD_INTEGER, field);
        query->builder.addNumberTerm(std::to_string(value), field,
                                     query->allocate_term_id(), Weight(1));
    });
}

extern "C" int32_t sk_query_add_range(sk_query *query, const char *field,
                                       uint32_t lower_kind, int64_t lower_value,
                                       uint32_t upper_kind, int64_t upper_value,
                                       uint8_t *error,
                                       uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "query range arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        validate_field_name(field);
        validate_query_field(*query, SK_QUERY_FIELD_INTEGER, field);
        auto range = search::query::Range(make_query_range(
            lower_kind, lower_value, upper_kind, upper_value));
        query->builder.addRangeTerm(range, field, query->allocate_term_id(), Weight(1));
    });
}

extern "C" int32_t sk_query_add_attribute_string_equals(
    sk_query *query, const char *field, const char *value,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field || !value) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "string equality arguments are invalid");
    return invoke(error, error_capacity, [&] {
        validate_query_field(*query, SK_QUERY_FIELD_STRING, field);
        query->builder.addStringTerm(value, field, query->allocate_term_id(), Weight(1));
    });
}

extern "C" int32_t sk_query_add_attribute_float_equals(
    sk_query *query, const char *field, double value,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "float equality arguments are invalid");
    return invoke(error, error_capacity, [&] {
        if (!std::isfinite(value)) throw std::invalid_argument("attribute float value must be finite");
        validate_query_field(*query, SK_QUERY_FIELD_FLOAT, field);
        std::ostringstream text;
        text.imbue(std::locale::classic());
        text << std::setprecision(std::numeric_limits<double>::max_digits10) << value;
        query->builder.addNumberTerm(text.str(), field, query->allocate_term_id(), Weight(1));
    });
}

extern "C" int32_t sk_query_add_attribute_boolean_equals(
    sk_query *query, const char *field, uint8_t value,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "boolean equality arguments are invalid");
    return invoke(error, error_capacity, [&] {
        validate_query_field(*query, SK_QUERY_FIELD_BOOLEAN, field);
        query->builder.addNumberTerm(value ? "1" : "0", field, query->allocate_term_id(), Weight(1));
    });
}

extern "C" int32_t sk_query_add_attribute_exists(
    sk_query *query, const char *field, uint8_t *error,
    uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "attribute exists arguments are invalid");
    return invoke(error, error_capacity, [&] {
        validate_query_field(*query, SK_QUERY_FIELD_ATTRIBUTE, field);
        query->builder.addRangeTerm(search::query::Range("[;]"), field, query->allocate_term_id(), Weight(1));
    });
}

extern "C" int32_t sk_query_add_attribute_integer_range(
    sk_query *query, const char *field, uint32_t lower_kind,
    int64_t lower_value, uint32_t upper_kind, int64_t upper_value,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "integer range arguments are invalid");
    return invoke(error, error_capacity, [&] {
        validate_query_field(*query, SK_QUERY_FIELD_INTEGER, field);
        query->builder.addRangeTerm(search::query::Range(make_query_range(lower_kind, lower_value, upper_kind, upper_value)), field, query->allocate_term_id(), Weight(1));
    });
}

extern "C" int32_t sk_query_add_attribute_float_range(
    sk_query *query, const char *field, uint32_t lower_kind,
    double lower_value, uint32_t upper_kind, double upper_value,
    uint8_t *error, uint32_t error_capacity) SK_NOEXCEPT {
    if (!query || !field) return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT, "float range arguments are invalid");
    return invoke(error, error_capacity, [&] {
        if ((lower_kind != SK_QUERY_BOUND_UNBOUNDED && !std::isfinite(lower_value)) ||
            (upper_kind != SK_QUERY_BOUND_UNBOUNDED && !std::isfinite(upper_value))) {
            throw std::invalid_argument("attribute float bound must be finite");
        }
        validate_query_field(*query, SK_QUERY_FIELD_FLOAT, field);
        const auto delimiter = [](uint32_t kind, bool lower) {
            if (kind == SK_QUERY_BOUND_UNBOUNDED) return lower ? '[' : ']';
            if (kind == SK_QUERY_BOUND_INCLUDED) return lower ? '[' : ']';
            if (kind == SK_QUERY_BOUND_EXCLUDED) return lower ? '<' : '>';
            throw std::invalid_argument("unsupported attribute range bound kind");
        };
        std::ostringstream text;
        text.imbue(std::locale::classic());
        text << delimiter(lower_kind, true);
        if (lower_kind != SK_QUERY_BOUND_UNBOUNDED) text << std::setprecision(std::numeric_limits<double>::max_digits10) << lower_value;
        text << ';';
        if (upper_kind != SK_QUERY_BOUND_UNBOUNDED) text << std::setprecision(std::numeric_limits<double>::max_digits10) << upper_value;
        text << delimiter(upper_kind, false);
        query->builder.addRangeTerm(search::query::Range(text.str()), field, query->allocate_term_id(), Weight(1));
    });
}

extern "C" void sk_query_destroy(sk_query *query) SK_NOEXCEPT {
    destroy_noexcept(query);
}

extern "C" int32_t sk_engine_search_query(const sk_engine *engine,
                                           const sk_query *query,
                                           const char *rank_profile,
                                           const char *source,
                                           sk_text_result **out_result,
                                           uint8_t *error,
                                           uint32_t error_capacity) SK_NOEXCEPT {
    if (out_result) {
        *out_result = nullptr;
    }
    if (!engine || !engine->impl || !query || !rank_profile || !source || !out_result) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "query search arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        auto result = std::make_unique<sk_text_result>();
        result->hits = engine->impl->search_query(query->builder, rank_profile, source);
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        engine->impl->failpoint("result_after_allocation");
        result->failpoints = &engine->impl->failpoints();
#endif
        *out_result = result.release();
    });
}

#ifdef SK_ENABLE_HARDENING_FAILPOINTS
extern "C" int32_t sk_test_failpoint_set(sk_engine *engine, const char *name,
                                            uint32_t hit, uint32_t action,
                                            uint8_t *error,
                                            uint32_t error_capacity) SK_NOEXCEPT {
    if (!engine || !engine->impl || !name) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "failpoint set arguments are invalid");
    }
    return invoke(error, error_capacity,
                  [&] { engine->impl->set_failpoint(name, hit, action); });
}

extern "C" int32_t sk_test_failpoint_set_open(const char *name, uint32_t hit,
                                                 uint32_t action, uint8_t *error,
                                                 uint32_t error_capacity) SK_NOEXCEPT {
    if (!name) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "open failpoint arguments are invalid");
    }
    return invoke(error, error_capacity,
                  [&] { OPEN_FAILPOINTS.set(name, hit, action); });
}

extern "C" int32_t sk_test_failpoint_clear(sk_engine *engine, const char *name,
                                              uint8_t *error,
                                              uint32_t error_capacity) SK_NOEXCEPT {
    if (!engine || !engine->impl || !name) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "failpoint clear arguments are invalid");
    }
    return invoke(error, error_capacity,
                  [&] { engine->impl->clear_failpoint(name); });
}

extern "C" int32_t sk_test_failpoint_clear_open(const char *name, uint8_t *error,
                                                   uint32_t error_capacity) SK_NOEXCEPT {
    if (!name) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "open failpoint arguments are invalid");
    }
    return invoke(error, error_capacity, [&] { OPEN_FAILPOINTS.clear(name); });
}

extern "C" int32_t sk_test_failpoint_release(sk_engine *engine, const char *name,
                                                uint8_t *error,
                                                uint32_t error_capacity) SK_NOEXCEPT {
    if (!engine || !engine->impl || !name) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "failpoint release arguments are invalid");
    }
    return invoke(error, error_capacity,
                  [&] { engine->impl->release_failpoint(name); });
}

extern "C" int32_t sk_test_failpoint_is_blocked(const sk_engine *engine,
                                                   const char *name,
                                                   uint8_t *out_blocked,
                                                   uint8_t *error,
                                                   uint32_t error_capacity) SK_NOEXCEPT {
    if (out_blocked) {
        *out_blocked = 0;
    }
    if (!engine || !engine->impl || !name || !out_blocked) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "failpoint status arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        *out_blocked = engine->impl->failpoint_is_blocked(name) ? 1 : 0;
    });
}
#endif
