#include "searchkernel_vespa.h"

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
#include <vespa/searchlib/query/tree/querybuilder.h>
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
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <limits>
#include <map>
#include <memory>
#include <mutex>
#include <new>
#include <sstream>
#include <stdexcept>
#include <string>
#include <string_view>
#include <tuple>
#include <utility>
#include <vector>

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

constexpr unsigned int FIRST_TLS_PORT = 9014;
constexpr unsigned int LAST_TLS_PORT = 65535;
std::atomic<unsigned int> NEXT_TLS_PORT{FIRST_TLS_PORT};
constexpr char LEGACY_DOC_TYPE[] = {'t', 'y', 'p', 'e', 'a', '\0'};
constexpr size_t MAX_DOCUMENT_ID_BYTES = 4096;
constexpr size_t MAX_FIELD_NAME_BYTES = 256;
constexpr size_t MAX_RANK_PROFILE_BYTES = 256;
constexpr size_t MAX_TEXT_BYTES = 16 * 1024 * 1024;
constexpr size_t MAX_TEXT_TERM_BYTES = 1024 * 1024;
constexpr size_t MAX_TEXT_TERMS = 65536;
constexpr size_t MAX_TEXT_OCCURRENCES = 65536;
constexpr std::array<std::string_view, 18> RESERVED_FIELD_NAMES = {
    "index",          "index_url",      "summary",       "attribute", "select_input",
    "host",           "documentid",     "position",      "split_foreach", "tokenize",
    "if",             "else",           "switch",        "case",      "sddocname",
    "relevancy",      "rankfeatures",  "summaryfeatures"};

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
    if (value.size() > MAX_FIELD_NAME_BYTES || !is_identifier(value)) {
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
    validate_string_size(value, MAX_FIELD_NAME_BYTES, "field name");
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

class EmbeddedThreadingService final : public proton::ISharedThreadingService {
    vespalib::ThreadExecutor &_shared;
    std::unique_ptr<vespalib::ISequencedTaskExecutor> _field_writer =
        vespalib::SequencedTaskExecutor::create([](vespalib::Runnable &worker) {
            worker.run();
            return 1;
        }, 1);
    vespalib::InvokeServiceImpl _invoke_service{10ms};
    FNET_Transport _transport;
    storage::spi::dummy::DummyBucketExecutor _bucket_executor{2};

public:
    explicit EmbeddedThreadingService(vespalib::ThreadExecutor &shared) : _shared(shared) { _transport.Start(); }
    ThreadExecutor &shared() override { return _shared; }
    vespalib::ISequencedTaskExecutor &field_writer() override { return *_field_writer; }
    vespalib::InvokeService &invokeService() override { return _invoke_service; }
    FNET_Transport &transport() override { return _transport; }
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

unsigned int next_tls_port() {
    auto port = NEXT_TLS_PORT.load(std::memory_order_relaxed);
    for (;;) {
        if (port > LAST_TLS_PORT) {
            throw std::runtime_error("transaction log port space exhausted");
        }
        if (NEXT_TLS_PORT.compare_exchange_weak(port, port + 1,
                                                std::memory_order_relaxed,
                                                std::memory_order_relaxed)) {
            return port;
        }
    }
}

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
};

struct TextUpdate : TextDocument {};

struct TextRequest {
    uint32_t kind;
    std::string field;
    std::vector<std::string> terms;
    std::vector<uint32_t> positions;
    std::string rank_profile;
};

struct TextHit {
    std::string id;
    double score;
};

struct TextResult {
    std::vector<TextHit> hits;
};

std::string query_stack_dump(int32_t value, const std::string &field) {
    QueryBuilder<proton::matching::ProtonNodeTypes> builder;
    builder.addNumberTerm(std::to_string(value), field, 1, Weight(1));
    auto tree = builder.build();
    if (!tree) {
        throw std::runtime_error("could not build number query");
    }
    return StackDumpCreator::create(*tree);
}

class EmbeddedEngine {
    EmbeddedWireService _wire_service;
    EmbeddedDBOwner _db_owner;
    vespalib::ThreadStackExecutor _summary_executor;
    EmbeddedThreadingService _shared_service;
    HwInfo _hw_info;
    EmbeddedFileHeaderContext _file_header_context;
    std::string _tls_dir;
    const unsigned int _tls_port;
    TransLogServer _tls;
    proton::matching::QueryLimiter _query_limiter;
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
        bool generated = false;
    };

    SchemaManifest _manifest;
    std::string _document_type;
    std::shared_ptr<const DocumenttypesConfig> _document_types;
    std::shared_ptr<DocumentTypeRepo> _repo;
    const DocumentType *_doc_type;
    DocumentDB::SP _db;
    PersistenceOwner _persistence_owner;
    AcceptAllWrites _write_filter;
    EmbeddedResourceUsageNotifier _resource_usage_notifier;
    PersistenceEngine _persistence;
    std::shared_ptr<PersistenceHandlerProxy> _handler;
    std::vector<uint64_t> _prepared_buckets;
    uint64_t _next_timestamp = 1;
    enum class State { Open, Closing, Closed, Failed };
    std::atomic<State> _state{State::Open};
    mutable std::mutex _operation_mutex;

    void require_open() const {
        if (_state.load(std::memory_order_acquire) != State::Open) {
            throw std::invalid_argument("engine is not open");
        }
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
            values[key] = value;
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
            if (config_value(values, prefix + "name") != field.name ||
                config_value(values, prefix + "datatype") != "STRING") {
                throw std::invalid_argument("inconsistent generated configuration");
            }
            if (field.type != "text") {
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
            if (config_value(values, prefix + "name") != field.name ||
                config_value(values, prefix + "datatype") != manifest_attribute_type(field.type)) {
                throw std::invalid_argument("inconsistent generated configuration");
            }
            ++index;
        }
    }

    static void validate_summary_configuration(const SchemaManifest &manifest,
                                               const std::filesystem::path &config_path) {
        const auto values = read_config(config_path / "summary.cfg");
        if (config_value(values, "classes[0].name") != "default") {
            throw std::invalid_argument("inconsistent generated configuration");
        }
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

    static void validate_generated_configuration(const SchemaManifest &manifest,
                                                 const std::string &config_dir) {
        const auto config_path = std::filesystem::path(config_dir);
        for (const char *name : {"indexschema.cfg", "attributes.cfg", "summary.cfg"}) {
            if (!std::filesystem::is_regular_file(config_path / name)) {
                throw std::invalid_argument("inconsistent generated configuration");
            }
        }
        validate_index_configuration(manifest, config_path);
        validate_attribute_configuration(manifest, config_path);
        validate_summary_configuration(manifest, config_path);
    }

    static SchemaManifest read_schema_manifest(const std::string &config_dir) {
        const auto path = std::filesystem::path(config_dir) / "searchkernel-schema.json";
        if (!std::filesystem::is_regular_file(path)) {
            const auto config_path = std::filesystem::path(config_dir);
            const bool legacy_config =
                std::filesystem::is_regular_file(config_path / "proton.cfg") ||
                std::filesystem::is_regular_file(config_path / ".m4-text-fixture");
            if (legacy_config) {
                return {LEGACY_DOC_TYPE, {}, false};
            }
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
        SchemaManifest manifest{root["document_type"].asString().make_string(), {}, true};
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
        return manifest;
    }

    static const document::DataType *manifest_type(const std::string &type) {
        if (type == "text" || type == "string") return document::DataType::STRING;
        if (type == "integer") return document::DataType::INT;
        if (type == "float") return document::DataType::DOUBLE;
        if (type == "boolean") return document::DataType::BOOL;
        throw std::invalid_argument("unsupported generated schema field type: " + type);
    }

    static std::shared_ptr<const DocumenttypesConfig>
    make_documenttypes_config(const SchemaManifest &manifest) {
        if (!manifest.generated) {
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

    static const document::DataType *attribute_type(const std::string &type) {
        if (type == "STRING") return document::DataType::STRING;
        if (type == "INT32") return document::DataType::INT;
        if (type == "DOUBLE") return document::DataType::DOUBLE;
        if (type == "BOOL") return document::DataType::BOOL;
        throw std::invalid_argument("unsupported generated attribute datatype: " + type);
    }

    static void add_document_field(std::map<std::string, const document::DataType *> &fields,
                                   const std::string &name, const document::DataType *type) {
        const auto [it, inserted] = fields.emplace(name, type);
        if (!inserted && it->second->getId() != type->getId()) {
            throw std::invalid_argument("generated configuration assigns conflicting types to field " + name);
        }
    }

    static void add_attribute_fields(std::map<std::string, const document::DataType *> &fields,
                                     const std::filesystem::path &config_dir) {
        const auto values = read_config(config_dir / "attributes.cfg");
        for (size_t index = 0; ; ++index) {
            const auto name_it = values.find("attribute[" + std::to_string(index) + "].name");
            if (name_it == values.end()) {
                break;
            }
            const auto type_it = values.find("attribute[" + std::to_string(index) + "].datatype");
            if (type_it == values.end()) {
                throw std::invalid_argument("generated attribute is missing datatype: " + name_it->second);
            }
            add_document_field(fields, name_it->second, attribute_type(type_it->second));
        }
    }

    static void add_index_fields(std::map<std::string, const document::DataType *> &fields,
                                 const std::filesystem::path &config_dir) {
        const auto values = read_config(config_dir / "indexschema.cfg");
        for (size_t index = 0; ; ++index) {
            const auto name_it = values.find("indexfield[" + std::to_string(index) + "].name");
            if (name_it == values.end()) break;
            add_document_field(fields, name_it->second, document::DataType::STRING);
        }
    }

    static void add_summary_fields(std::map<std::string, const document::DataType *> &fields,
                                   const std::filesystem::path &config_dir) {
        const auto values = read_config(config_dir / "summary.cfg");
        size_t default_class = 0;
        bool found_default = false;
        for (size_t index = 0; ; ++index) {
            const auto name_it = values.find("classes[" + std::to_string(index) + "].name");
            if (name_it == values.end()) break;
            if (name_it->second == "default") {
                default_class = index;
                found_default = true;
                break;
            }
        }
        if (!found_default) return;
        const std::string prefix = "classes[" + std::to_string(default_class) + "].fields[";
        for (size_t index = 0; ; ++index) {
            const auto field_prefix = prefix + std::to_string(index) + "]";
            const auto name_it = values.find(field_prefix + ".name");
            if (name_it == values.end()) break;
            const auto command_it = values.find(field_prefix + ".command");
            if (command_it != values.end() && command_it->second.empty()) {
                if (name_it->second != "documentid" && name_it->second != "rankfeatures" &&
                    name_it->second != "summaryfeatures") {
                    add_document_field(fields, name_it->second, document::DataType::STRING);
                }
            }
        }
    }

    static DocumentType make_document_type(const SchemaManifest &manifest,
                                           const std::string &config_dir) {
        DocumentType type(manifest.document_type, 0);
        std::map<std::string, const document::DataType *> fields;
        const auto config_path = std::filesystem::path(config_dir);
        if (manifest.generated) {
            for (const auto &field : manifest.fields) {
                add_document_field(fields, field.name, manifest_type(field.type));
            }
        } else {
            add_attribute_fields(fields, config_path);
            add_index_fields(fields, config_path);
            add_summary_fields(fields, config_path);
            // Keep the historical M0-M5 fixture fields available when opening
            // an old hand-written fixture.
            if (!fields.contains("attr1")) {
                fields.emplace("attr1", document::DataType::INT);
            }
            if (!fields.contains("title")) {
                fields.emplace("title", document::DataType::STRING);
            }
        }
        int32_t field_id = 1;
        for (const auto &[name, data_type] : fields) {
            type.addField(document::Field(name, field_id++, *data_type));
        }
        return type;
    }

    void prepare_bucket(const Bucket &bucket) {
        const uint64_t raw_id = bucket.getBucketId().getId();
        if (std::find(_prepared_buckets.begin(), _prepared_buckets.end(), raw_id) ==
            _prepared_buckets.end()) {
            _prepared_buckets.push_back(raw_id);
            auto created = _persistence.createBucket(bucket);
            if (created.hasError()) {
                throw std::runtime_error(created.getErrorMessage());
            }
            auto active = _persistence.setActiveState(bucket, BucketInfo::ACTIVE);
            if (active.hasError()) {
                throw std::runtime_error(active.getErrorMessage());
            }
        }
    }

    std::shared_ptr<Document> make_document(const DocumentId &id, const std::string &field_name,
                                            int32_t value) const {
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
        : _wire_service(),
          _db_owner(),
          _summary_executor(8),
          _shared_service(_summary_executor),
          _hw_info(),
          _file_header_context(),
          _tls_dir("tmp"),
          _tls_port(next_tls_port()),
          _tls(_shared_service.transport(), _tls_dir,
               static_cast<int>(_tls_port), data_dir, _file_header_context),
          _query_limiter(),
          _manifest(read_schema_manifest(config_dir)),
          _document_type(_manifest.document_type),
          _document_types(make_documenttypes_config(_manifest)),
          _repo(_manifest.generated
                    ? std::make_shared<DocumentTypeRepo>(*_document_types)
                    : std::make_shared<DocumentTypeRepo>(
                          make_document_type(_manifest, config_dir))),
          _doc_type(_repo->getDocumentType(_document_type)),
          _db(),
          _persistence_owner(),
          _write_filter(),
          _resource_usage_notifier(),
          _persistence(_persistence_owner, _write_filter,
                       _resource_usage_notifier, -1, false),
          _handler(),
          _prepared_buckets() {
        if (_manifest.generated) {
            validate_generated_configuration(_manifest, config_dir);
        }
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
            auto active = _persistence.setActiveState(
                Bucket(document::Bucket(bucket_space(), bucket_id)), BucketInfo::ACTIVE);
            if (active.hasError()) {
                throw std::runtime_error(active.getErrorMessage());
            }
        }
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
            _shared_service.transport().ShutDown(true);
            _state.store(State::Closed, std::memory_order_release);
        } catch (...) {
            _state.store(State::Failed, std::memory_order_release);
            throw;
        }
    }

    void cleanup_noexcept() noexcept {
        std::lock_guard<std::mutex> operation_lock(_operation_mutex);
        try {
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
            _shared_service.transport().ShutDown(true);
        } catch (...) {
        }
    }

    ~EmbeddedEngine() noexcept {
        if (_state.load(std::memory_order_acquire) == State::Open) {
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
        if (text.terms.size() >
            static_cast<size_t>(std::numeric_limits<int>::max() - 2)) {
            throw std::invalid_argument("text term count cannot be represented by Vespa");
        }
        QueryBuilder<proton::matching::ProtonNodeTypes> builder;
        if (text.kind == 1) {
            if (text.terms.size() != 1) {
                throw std::invalid_argument("term query must contain exactly one term");
            }
            builder.addStringTerm(text.terms[0], text.field, 1, Weight(1));
        } else if (text.kind == 2) {
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
        auto tree = builder.build();
        if (!tree) {
            throw std::invalid_argument("could not build text query: " + builder.error());
        }

        auto request = std::make_shared<SearchRequest>();
        request->setTimeout(60s);
        request->setSerializedQueryTree(SerializedQueryTree::fromStackDump(
            StackDumpCreator::create(*tree)));
        request->ranking = text.rank_profile;
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

void destroy_error(sk_error *error) noexcept {
    if (!error) {
        return;
    }
    error->code = 0;
    error->message[0] = '\0';
}

void clear_error(sk_error *error) noexcept {
    destroy_error(error);
}

int32_t set_error(sk_error *error, int32_t code, const char *message) noexcept {
    if (!error) {
        return code;
    }
    destroy_error(error);
    error->code = code;
    if (!message) {
        return code;
    }
    const size_t length = std::strlen(message);
    const size_t copied = std::min(length, sizeof(error->message) - 1);
    std::memcpy(error->message, message, copied);
    error->message[copied] = '\0';
    return error->code;
}

std::filesystem::path resolve_config_directory(const std::filesystem::path &path) {
    // M4's frozen fixture predates ConfigCompiler and passes its fixture root.
    // Keep this one named compatibility rule; all other callers must pass the
    // actual configuration directory.
    const auto nested_config = path / "embedded_text_cfg";
    if (path.filename() == "fixture" &&
        std::filesystem::is_regular_file(nested_config / "indexschema.cfg") &&
        std::filesystem::is_regular_file(nested_config / ".m4-text-fixture")) {
        return nested_config;
    }
    return path;
}

template <typename Function>
int32_t invoke(sk_error *error, Function &&function) noexcept {
    clear_error(error);
    try {
        function();
        return SK_STATUS_OK;
    } catch (const std::bad_alloc &) {
        return set_error(error, SK_STATUS_OUT_OF_MEMORY, nullptr);
    } catch (const std::invalid_argument &exception) {
        return set_error(error, SK_STATUS_INVALID_ARGUMENT, exception.what());
    } catch (const std::exception &exception) {
        return set_error(error, SK_STATUS_INTERNAL, exception.what());
    } catch (...) {
        return set_error(error, SK_STATUS_INTERNAL, "unknown native exception");
    }
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

extern "C" uint32_t sk_native_abi_version(void) SK_NOEXCEPT {
    return SK_NATIVE_ABI_VERSION;
}

extern "C" int32_t sk_engine_open(const char *data_dir, const char *config_dir,
                                   sk_engine **out_engine, sk_error *error) {
    if (out_engine) {
        *out_engine = nullptr;
    }
    if (!out_engine || !data_dir || !config_dir) {
        return set_error(error, SK_STATUS_INVALID_ARGUMENT, "engine open arguments must not be null");
    }
    return invoke(error, [&] {
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
        const auto resolved_config_path = resolve_config_directory(config_path);
        auto engine = std::make_unique<sk_engine>();
        engine->impl = std::make_unique<EmbeddedEngine>(data_dir,
                                                        resolved_config_path.string());
        *out_engine = engine.release();
    });
}

extern "C" int32_t sk_engine_close(sk_engine *engine, sk_error *error) {
    if (!engine || !engine->impl) {
        return set_error(error, SK_STATUS_INVALID_ARGUMENT, "engine close arguments are invalid");
    }
    return invoke(error, [&] { engine->impl->close(); });
}

extern "C" void sk_engine_destroy(sk_engine *engine) SK_NOEXCEPT {
    try {
        delete engine;
    } catch (...) {
    }
}

extern "C" void sk_error_destroy(sk_error *error) SK_NOEXCEPT {
    destroy_error(error);
}

extern "C" int32_t sk_document_create(const char *id, const char *field,
                                        int32_t value, sk_document **out,
                                        sk_error *error) {
    if (out) {
        *out = nullptr;
    }
    if (!out || !id || !field) {
        return set_error(error, SK_STATUS_INVALID_ARGUMENT, "document arguments must not be null");
    }
    return invoke(error, [&] {
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
                                               sk_error *error) {
    if (out) {
        *out = nullptr;
    }
    if (!out || !id || !field) {
        return set_error(error, SK_STATUS_INVALID_ARGUMENT, "update arguments must not be null");
    }
    return invoke(error, [&] {
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
                                              sk_error *error) {
    if (out) {
        *out = nullptr;
    }
    if (!out || !field) {
        return set_error(error, SK_STATUS_INVALID_ARGUMENT, "search arguments must not be null");
    }
    return invoke(error, [&] {
        validate_field_name(field);
        if (*field == '\0') {
            throw std::invalid_argument("search field must not be empty");
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
                                  sk_error *error) {
    if (!engine || !engine->impl || !document) {
        return set_error(error, SK_STATUS_INVALID_ARGUMENT, "put arguments are invalid");
    }
    return invoke(error, [&] {
        engine->impl->put(document->id, document->field, document->value);
    });
}

extern "C" int32_t sk_engine_update(sk_engine *engine,
                                     const sk_document_update *update,
                                     sk_error *error) {
    if (!engine || !engine->impl || !update) {
        return set_error(error, SK_STATUS_INVALID_ARGUMENT, "update arguments are invalid");
    }
    return invoke(error, [&] {
        engine->impl->update(update->id, update->field, update->value);
    });
}

extern "C" int32_t sk_engine_remove(sk_engine *engine, const char *id,
                                     int32_t *out_was_found, sk_error *error) {
    if (out_was_found) {
        *out_was_found = 0;
    }
    if (!engine || !engine->impl || !id || !out_was_found) {
        return set_error(error, SK_STATUS_INVALID_ARGUMENT, "remove arguments are invalid");
    }
    return invoke(error, [&] {
        validate_document_id(id);
        *out_was_found = engine->impl->remove(id) ? 1 : 0;
    });
}

extern "C" int32_t sk_engine_search(const sk_engine *engine,
                                     const sk_search_request *request,
                                     sk_search_result **out_result,
                                     sk_error *error) {
    if (out_result) {
        *out_result = nullptr;
    }
    if (!engine || !engine->impl || !request || !out_result) {
        return set_error(error, SK_STATUS_INVALID_ARGUMENT, "search arguments are invalid");
    }
    return invoke(error, [&] {
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
                                        uint32_t *out_length, sk_error *error) {
    if (out_id) {
        *out_id = nullptr;
    }
    if (out_length) {
        *out_length = 0;
    }
    if (!result || !out_id || !out_length) {
        return set_error(error, SK_STATUS_INVALID_ARGUMENT, "result arguments are invalid");
    }
    if (index >= result->ids.size()) {
        return set_error(error, SK_STATUS_INVALID_ARGUMENT, "result index is out of bounds");
    }
    clear_error(error);
    *out_id = result->ids[index].data();
    *out_length = static_cast<uint32_t>(result->ids[index].size());
    return SK_STATUS_OK;
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

void fill_text_document(TextDocument &document, const char *id, const char *field,
                        const uint8_t *text, uint32_t text_length,
                        const sk_text_occurrence *occurrences,
                        uint32_t occurrence_count) {
    validate_document_id(id);
    validate_field_name(field);
    if (*id == '\0' || *field == '\0') {
        throw std::invalid_argument("text document ID and field must not be empty");
    }
    if (occurrence_count > MAX_TEXT_OCCURRENCES) {
        throw std::invalid_argument("text occurrence count is too large");
    }
    if (occurrence_count != 0 && occurrences == nullptr) {
        throw std::invalid_argument("text occurrences must not be null");
    }
    document.id = id;
    document.field = field;
    document.text = copy_text_bytes(text, text_length, MAX_TEXT_BYTES, "text");
    document.occurrences.reserve(occurrence_count);
    for (uint32_t index = 0; index < occurrence_count; ++index) {
        const auto &input = occurrences[index];
        if (input.term_length == 0) {
            throw std::invalid_argument("text occurrence term must not be empty");
        }
        if (input.position != index) {
            throw std::invalid_argument("text occurrence positions must be contiguous");
        }
        if (input.byte_start > document.text.size() ||
            input.byte_length > document.text.size() - input.byte_start ||
            !is_utf8_boundary(document.text, input.byte_start) ||
            !is_utf8_boundary(document.text, input.byte_start + input.byte_length)) {
            throw std::invalid_argument("text occurrence span is not on UTF-8 boundaries");
        }
        document.occurrences.push_back({
            copy_text_bytes(input.term, input.term_length, MAX_TEXT_TERM_BYTES,
                            "text occurrence term"),
            input.byte_start,
            input.byte_length,
            input.position
        });
    }
}

void fill_text_request(TextRequest &request, uint32_t kind, const char *field,
                       const sk_text_query_term *terms, uint32_t term_count,
                       const char *rank_profile) {
    validate_field_name(field);
    validate_rank_profile(rank_profile);
    if (*field == '\0' || *rank_profile == '\0') {
        throw std::invalid_argument("text query field and rank profile must not be empty");
    }
    if (kind != 1 && kind != 2) {
        throw std::invalid_argument("unsupported text query kind");
    }
    if ((kind == 1 && term_count != 1) || (kind == 2 && term_count == 0)) {
        throw std::invalid_argument("invalid number of terms for text query kind");
    }
    if (term_count > MAX_TEXT_TERMS) {
        throw std::invalid_argument("text term count is too large");
    }
    if (term_count != 0 && terms == nullptr) {
        throw std::invalid_argument("text query terms must not be null");
    }
    request.kind = kind;
    request.field = field;
    request.rank_profile = rank_profile;
    request.terms.reserve(term_count);
    request.positions.reserve(term_count);
    for (uint32_t index = 0; index < term_count; ++index) {
        const auto &input = terms[index];
        if (input.term_length == 0) {
            throw std::invalid_argument("text query term must not be empty");
        }
        if (input.position != index) {
            throw std::invalid_argument("text query positions must be contiguous");
        }
        request.terms.push_back(copy_text_bytes(input.term, input.term_length,
                                                 MAX_TEXT_TERM_BYTES,
                                                 "text query term"));
        request.positions.push_back(input.position);
    }
}

} // namespace

extern "C" int32_t sk_text_document_create(
    const char *id, const char *field, const uint8_t *text, uint32_t text_length,
    const sk_text_occurrence *occurrences, uint32_t occurrence_count,
    sk_text_document **out, sk_error *error) {
    if (out) {
        *out = nullptr;
    }
    if (!out || !id || !field) {
        return set_error(error, SK_STATUS_INVALID_ARGUMENT, "text document arguments must not be null");
    }
    return invoke(error, [&] {
        auto document = std::make_unique<sk_text_document>();
        fill_text_document(*document, id, field, text, text_length,
                           occurrences, occurrence_count);
        *out = document.release();
    });
}

extern "C" void sk_text_document_destroy(sk_text_document *document) SK_NOEXCEPT {
    delete document;
}

extern "C" int32_t sk_text_update_create(
    const char *id, const char *field, const uint8_t *text, uint32_t text_length,
    const sk_text_occurrence *occurrences, uint32_t occurrence_count,
    sk_text_update **out, sk_error *error) {
    if (out) {
        *out = nullptr;
    }
    if (!out || !id || !field) {
        return set_error(error, SK_STATUS_INVALID_ARGUMENT, "text update arguments must not be null");
    }
    return invoke(error, [&] {
        auto update = std::make_unique<sk_text_update>();
        fill_text_document(*update, id, field, text, text_length,
                           occurrences, occurrence_count);
        *out = update.release();
    });
}

extern "C" void sk_text_update_destroy(sk_text_update *update) SK_NOEXCEPT {
    delete update;
}

extern "C" int32_t sk_text_request_create(
    uint32_t kind, const char *field, const sk_text_query_term *terms,
    uint32_t term_count, const char *rank_profile, sk_text_request **out,
    sk_error *error) {
    if (out) {
        *out = nullptr;
    }
    if (!out || !field || !rank_profile) {
        return set_error(error, SK_STATUS_INVALID_ARGUMENT, "text request arguments must not be null");
    }
    return invoke(error, [&] {
        auto request = std::make_unique<sk_text_request>();
        fill_text_request(*request, kind, field, terms, term_count, rank_profile);
        *out = request.release();
    });
}

extern "C" void sk_text_request_destroy(sk_text_request *request) SK_NOEXCEPT {
    delete request;
}

extern "C" int32_t sk_engine_put_text(sk_engine *engine,
                                        const sk_text_document *document,
                                        sk_error *error) {
    if (!engine || !engine->impl || !document) {
        return set_error(error, SK_STATUS_INVALID_ARGUMENT, "text put arguments are invalid");
    }
    return invoke(error, [&] { engine->impl->put_text(*document); });
}

extern "C" int32_t sk_engine_update_text(sk_engine *engine,
                                           const sk_text_update *update,
                                           sk_error *error) {
    if (!engine || !engine->impl || !update) {
        return set_error(error, SK_STATUS_INVALID_ARGUMENT, "text update arguments are invalid");
    }
    return invoke(error, [&] { engine->impl->update_text(*update); });
}

extern "C" int32_t sk_engine_search_text(const sk_engine *engine,
                                           const sk_text_request *request,
                                           sk_text_result **out_result,
                                           sk_error *error) {
    if (out_result) {
        *out_result = nullptr;
    }
    if (!engine || !engine->impl || !request || !out_result) {
        return set_error(error, SK_STATUS_INVALID_ARGUMENT, "text search arguments are invalid");
    }
    return invoke(error, [&] {
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
                                       sk_error *error) {
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
        return set_error(error, SK_STATUS_INVALID_ARGUMENT, "text result arguments are invalid");
    }
    if (index >= result->hits.size()) {
        return set_error(error, SK_STATUS_INVALID_ARGUMENT, "text result index is out of bounds");
    }
    clear_error(error);
    *out_id = result->hits[index].id.data();
    *out_length = static_cast<uint32_t>(result->hits[index].id.size());
    *out_score = result->hits[index].score;
    return SK_STATUS_OK;
}

extern "C" void sk_text_result_destroy(sk_text_result *result) SK_NOEXCEPT {
    delete result;
}
