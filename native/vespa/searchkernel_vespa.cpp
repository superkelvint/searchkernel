#include "searchkernel_vespa.h"
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
#include <vespa/document/fieldvalue/stringfieldvalue.h>
#include <vespa/document/repo/documenttyperepo.h>
#include <vespa/document/repo/fixedtyperepo.h>
#include <vespa/document/repo/newconfigbuilder.h>
#include <vespa/document/update/assignvalueupdate.h>
#include <vespa/document/update/documentupdate.h>
#include <vespa/document/update/fieldupdate.h>
#include <vespa/fnet/transport.h>
#include <vespa/searchcore/proton/matching/querylimiter.h>
#include <vespa/searchcore/proton/matching/querynodes.h>
#include <vespa/searchcore/proton/metrics/metricswireservice.h>
#include <vespa/searchcore/proton/persistenceengine/i_resource_write_filter.h>
#include <vespa/searchcore/proton/persistenceengine/ipersistenceengineowner.h>
#include <vespa/searchcore/proton/persistenceengine/persistenceengine.h>
#include <vespa/searchcore/proton/server/bootstrapconfig.h>
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
#include <vespa/vespalib/util/host_name.h>
#include <vespa/vespalib/util/invokeserviceimpl.h>
#include <vespa/vespalib/util/sequencedtaskexecutor.h>
#include <vespa/vespalib/util/shared_operation_throttler.h>
#include <vespa/persistence/dummyimpl/dummy_bucket_executor.h>

#include <algorithm>
#include <array>
#include <atomic>
#include <cerrno>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <fcntl.h>
#include <fstream>
#include <iterator>
#include <limits>
#include <map>
#include <memory>
#include <mutex>
#include <new>
#include <set>
#include <sstream>
#include <stdexcept>
#include <string>
#include <string_view>
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
    auto bucket_id = id.getGlobalId().convertToBucketId();
    bucket_id.setUsedBits(8);
    return bucket_id;
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
};

std::string query_stack_dump(int32_t value, const std::string &field) {
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
    PersistenceOwner _persistence_owner;
    AcceptAllWrites _write_filter;
    EmbeddedResourceUsageNotifier _resource_usage_notifier;
    PersistenceEngine _persistence;
    std::shared_ptr<PersistenceHandlerProxy> _handler;
    // A bucket can be created successfully while activation fails. Keep the
    // successful creation separate so a retry only re-attempts activation.
    std::vector<uint64_t> _created_buckets;
    std::vector<uint64_t> _prepared_buckets;
    uint64_t _next_timestamp = 1;
    bool _initialized = false;
    enum class State { Open, Closing, Closed, Failed };
    std::atomic<State> _state{State::Open};
    mutable std::mutex _operation_mutex;

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
        if (_next_timestamp == std::numeric_limits<uint64_t>::max()) {
            throw std::runtime_error("mutation timestamp space exhausted");
        }
        return _next_timestamp++;
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
        if (type == "integer") return "INT32";
        if (type == "float") return "DOUBLE";
        if (type == "boolean") return "BOOL";
        throw std::invalid_argument("inconsistent generated configuration");
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
            require_config_value(values, prefix + "datatype", manifest_attribute_type(field.type));
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
        const size_t expected_profiles = text_field == manifest.fields.end() ? 1 : 2;
        if (config_section_count(values, "rankprofile") != expected_profiles) {
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

    static SchemaManifest read_schema_manifest(const std::string &config_dir) {
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
                document_type.addField(field.name, builder.intTypeRef());
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
        if (type == "integer") return document::DataType::INT;
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
            if (std::find(_created_buckets.begin(), _created_buckets.end(), raw_id) ==
                _created_buckets.end()) {
                // PersistenceHandlerProxy::handleCreateBucket in the pinned
                // Vespa source checks BucketHandler::hasBucket before
                // creating. We still remember success locally so an
                // activation failure does not depend on that idempotence on
                // the retry path.
                auto created = _persistence.createBucket(bucket);
                if (created.hasError()) {
                    throw std::runtime_error(created.getErrorMessage());
                }
                _created_buckets.push_back(raw_id);
            }
            auto active = _persistence.setActiveState(bucket, BucketInfo::ACTIVE);
            if (active.hasError()) {
                throw std::runtime_error(active.getErrorMessage());
            }
            _prepared_buckets.push_back(raw_id);
        }
    }

    std::shared_ptr<Document> make_document(const DocumentId &id, const std::string &field_name,
                                            int32_t value) const {
        require_integer_field(field_name);
        auto document = std::make_shared<Document>(*_repo, *_doc_type, id);
        const auto &field = _doc_type->getField(field_name);
        if (field.getDataType().getId() != document::DataType::T_INT) {
            throw std::invalid_argument("integer operation requires an INT32 field: " + field_name);
        }
        document->setFieldValue(field, std::make_unique<document::IntFieldValue>(value));
        return document;
    }

    std::shared_ptr<document::DocumentUpdate>
    make_update(const DocumentId &id, const std::string &field_name, int32_t value) const {
        require_integer_field(field_name);
        auto update = std::make_shared<document::DocumentUpdate>(*_repo, *_doc_type, id);
        const auto &field = _doc_type->getField(field_name);
        if (field.getDataType().getId() != document::DataType::T_INT) {
            throw std::invalid_argument("integer operation requires an INT32 field: " + field_name);
        }
        document::FieldUpdate field_update(field);
        field_update.addUpdate(std::make_unique<document::AssignValueUpdate>(
            std::make_unique<document::IntFieldValue>(value)));
        update->addUpdate(std::move(field_update));
        return update;
    }

public:
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
          _created_buckets(),
          _prepared_buckets() {}

    void initialize(const std::string &data_dir, const std::string &config_dir) {
        std::filesystem::create_directories(data_dir);
        if (_doc_type == nullptr) {
            throw std::runtime_error("generated document type was not added to DocumentTypeRepo: " +
                                     _document_type);
        }
        std::filesystem::create_directories(data_dir + "/" + _document_type);

        config::DirSpec spec(config_dir);
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

        _db = DocumentDB::create(
            data_dir, config.getConfig(),
            vespalib::SocketSpec::from_host_port("localhost", static_cast<int>(_tls_port)).spec(),
            _query_limiter, DocTypeName(_document_type), bucket_space(),
            *proton_config, _db_owner, _shared_service, _tls, _wire_service,
            _file_header_context,
            std::make_shared<search::attribute::Interlock>(),
            std::move(config_store),
            std::make_shared<vespalib::ThreadStackExecutor>(16),
            _hw_info, std::shared_ptr<search::diskindex::IPostingListCache>());
        _db->start();
        _db->waitForOnlineState();
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
        auto initialized = _persistence.initialize();
        if (initialized.hasError()) {
            throw std::runtime_error(initialized.getErrorMessage());
        }
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
            _created_buckets.push_back(bucket_id.getId());
            _prepared_buckets.push_back(bucket_id.getId());
        }
        _initialized = true;
    }

    void close() {
        std::lock_guard<std::mutex> operation_lock(_operation_mutex);
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
            _persistence.removeHandler(_persistence.getWLock(), bucket_space(),
                                       DocTypeName(_document_type));
            _handler.reset();
            if (_db) {
                _db->close();
                _db.reset();
            }
            _shared_service.shutdown();
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
            std::lock_guard<std::mutex> operation_lock(_operation_mutex);
            if (_handler) {
                _persistence.removeHandler(_persistence.getWLock(), bucket_space(),
                                           DocTypeName(_document_type));
            }
        } catch (...) {
        }
        _handler.reset();
        try {
            if (_db) {
                _db->close();
                _db.reset();
            }
        } catch (...) {
            _db.reset();
        }
        try {
            _shared_service.shutdown_noexcept();
        } catch (...) {
        }
    }

    ~EmbeddedEngine() noexcept {
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

    void put(const std::string &id, const std::string &field, int32_t value) {
        std::lock_guard<std::mutex> operation_lock(_operation_mutex);
        require_open();
        DocumentId document_id(internal_document_id(id, _document_type));
        Bucket bucket = bucket_for(document_id);
        prepare_bucket(bucket);
        auto result = _persistence.put(bucket, Timestamp(next_timestamp()),
                                       make_document(document_id, field, value));
        if (result.hasError()) {
            throw std::runtime_error(result.getErrorMessage());
        }
    }

    void update(const std::string &id, const std::string &field, int32_t value) {
        std::lock_guard<std::mutex> operation_lock(_operation_mutex);
        require_open();
        DocumentId document_id(internal_document_id(id, _document_type));
        auto result = _persistence.update(
            bucket_for(document_id), Timestamp(next_timestamp()),
            make_update(document_id, field, value));
        if (result.hasError()) {
            throw std::runtime_error(result.getErrorMessage());
        }
    }

    bool remove(const std::string &id) {
        std::lock_guard<std::mutex> operation_lock(_operation_mutex);
        require_open();
        DocumentId document_id(internal_document_id(id, _document_type));
        auto result = _persistence.remove(
            bucket_for(document_id), Timestamp(next_timestamp()),
            document_id);
        if (result.hasError()) {
            throw std::runtime_error(result.getErrorMessage());
        }
        return result.wasFound();
    }

    std::vector<std::string> search(const std::string &field, int32_t value,
                                    uint32_t limit) const {
        std::lock_guard<std::mutex> operation_lock(_operation_mutex);
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
        std::lock_guard<std::mutex> operation_lock(_operation_mutex);
        require_open();
        const DocumentId id(internal_document_id(text.id, _document_type));
        const Bucket bucket = bucket_for(id);
        prepare_bucket(bucket);
        auto result = _persistence.put(bucket, Timestamp(next_timestamp()),
                                       make_text_document(text));
        if (result.hasError()) {
            throw std::runtime_error(result.getErrorMessage());
        }
    }

    void update_text(const TextUpdate &text) {
        std::lock_guard<std::mutex> operation_lock(_operation_mutex);
        require_open();
        const DocumentId id(internal_document_id(text.id, _document_type));
        auto result = _persistence.update(bucket_for(id), Timestamp(next_timestamp()),
                                          make_text_update(text));
        if (result.hasError()) {
            throw std::runtime_error(result.getErrorMessage());
        }
    }

    std::vector<TextHit> search_text(const TextRequest &text) const {
        std::lock_guard<std::mutex> operation_lock(_operation_mutex);
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

public:
    std::vector<TextHit> search_query(VespaQueryBuilder &builder,
                                      const std::string &rank_profile,
                                      const std::string &source) const {
        std::lock_guard<std::mutex> operation_lock(_operation_mutex);
        require_open();
        return match_tree(builder.build(), rank_profile, source);
    }

    std::vector<QueryFieldMetadata> query_field_metadata() const {
        std::vector<QueryFieldMetadata> metadata;
        metadata.reserve(_manifest.fields.size());
        for (const auto &field : _manifest.fields) {
            metadata.push_back({field.name, field.type, field.index, field.attribute});
        }
        return metadata;
    }
};

struct ValueDocument {
    std::string id;
    std::string field;
    int32_t value;
};

struct ValueUpdate {
    std::string id;
    std::string field;
    int32_t value;
};

struct ValueRequest {
    std::string field;
    int32_t value;
    uint32_t limit;
};

struct ErrorResult {
    std::vector<std::string> ids;
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

std::string copy_text_bytes(const uint8_t *bytes, uint32_t length,
                            size_t maximum, const char *what);

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
        if (metadata->type != "integer" || !metadata->attribute) {
            throw std::invalid_argument(
                "integer operation requires an integer attribute field: " + std::string(field));
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
    uint32_t lower_kind, int32_t lower_value,
    uint32_t upper_kind, int32_t upper_value) {
    auto spec = std::make_unique<search::NumericRangeSpec>();
    spec->valid = true;
    spec->valid_integers = true;
    const auto set_bound = [](uint32_t kind, int32_t value, bool lower,
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
        engine->impl = std::make_unique<EmbeddedEngine>(data_dir,
                                                        config_path.string());
        engine->impl->initialize(data_dir, config_path.string());
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
        delete engine;
    } catch (...) {
    }
}

extern "C" int32_t sk_document_create(const char *id, const char *field,
                                        int32_t value, sk_document **out,
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

extern "C" void sk_document_destroy(sk_document *document) SK_NOEXCEPT { delete document; }

extern "C" int32_t sk_document_update_create(const char *id, const char *field,
                                               int32_t value,
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
    delete update;
}

extern "C" int32_t sk_search_request_create(const char *field, int32_t value,
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
    delete request;
}

extern "C" int32_t sk_engine_put(sk_engine *engine, const sk_document *document,
                                  uint8_t *error,
                                  uint32_t error_capacity) SK_NOEXCEPT {
    if (!engine || !engine->impl || !document) {
        return set_error(error, error_capacity, SK_NATIVE_STATUS_INVALID_ARGUMENT,
                         "put arguments are invalid");
    }
    return invoke(error, error_capacity, [&] {
        engine->impl->put(document->id, document->field, document->value);
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
        engine->impl->update(update->id, update->field, update->value);
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
    clear_error(error, error_capacity);
    *out_id = result->ids[index].data();
    *out_length = static_cast<uint32_t>(result->ids[index].size());
    return SK_NATIVE_STATUS_OK;
}

extern "C" void sk_search_result_destroy(sk_search_result *result) SK_NOEXCEPT {
    delete result;
}

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

template <typename TextDocumentType>
void append_text_occurrences(TextDocumentType &document, const uint8_t *term_bytes,
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
    delete document;
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
    delete update;
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
    delete request;
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
    clear_error(error, error_capacity);
    *out_id = result->hits[index].id.data();
    *out_length = static_cast<uint32_t>(result->hits[index].id.size());
    *out_score = result->hits[index].score;
    return SK_NATIVE_STATUS_OK;
}

extern "C" void sk_text_result_destroy(sk_text_result *result) SK_NOEXCEPT {
    delete result;
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
                                        int32_t value, uint8_t *error,
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
                                       uint32_t lower_kind, int32_t lower_value,
                                       uint32_t upper_kind, int32_t upper_value,
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

extern "C" void sk_query_destroy(sk_query *query) SK_NOEXCEPT {
    delete query;
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
        *out_result = result.release();
    });
}
