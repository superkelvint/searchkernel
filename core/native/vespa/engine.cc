// Search Kernel native adapter: engine lifecycle and Vespa ownership.
// This file is included by abi.cc so the private adapter remains one native
// boundary while responsibilities stay physically searchable by subsystem.

#include "highlighting.h"


constexpr size_t MAX_DOCUMENT_ID_BYTES = 4096;
constexpr size_t MAX_SCHEMA_IDENTIFIER_BYTES = 256;
constexpr size_t MAX_RANK_PROFILE_BYTES = 256;
constexpr size_t MAX_TEXT_BYTES = 16 * 1024 * 1024;
constexpr size_t MAX_TEXT_TERM_BYTES = 1024 * 1024;
constexpr size_t MAX_TOTAL_TEXT_TERM_BYTES = 64 * 1024 * 1024;
constexpr size_t MAX_TEXT_TERMS = 65536;
constexpr size_t MAX_TEXT_OCCURRENCES = 65536;
constexpr size_t MAX_PACKED_DOCUMENT_ITEMS = MAX_TEXT_TERMS;
constexpr size_t MAX_PACKED_DOCUMENT_ELEMENT_BYTES = MAX_TEXT_TERM_BYTES;
constexpr size_t MAX_PACKED_DOCUMENT_TOTAL_BYTES = MAX_TEXT_BYTES;
constexpr uint32_t MAX_SEARCH_LIMIT = 1'000'000;
constexpr std::string_view HIGHLIGHT_SUMMARY_PREFIX = "__searchkernel_highlight_";
constexpr std::array<std::string_view, 18> RESERVED_FIELD_NAMES = {
    "index",          "index_url",      "summary",       "attribute", "select_input",
    "host",           "documentid",     "position",      "split_foreach", "tokenize",
    "if",             "else",           "switch",        "case",      "sddocname",
    "relevancy",      "rankfeatures",  "summaryfeatures"};
constexpr std::string_view RESERVED_RANK_PROPERTY = "vespa.matching.numthreadspersearch";

std::optional<uint32_t> native_trace_level() {
    const char *value = std::getenv("SEARCHKERNEL_NATIVE_TRACE_LEVEL");
    if (!value || *value == '\0') return std::nullopt;
    char *end = nullptr;
    const unsigned long parsed = std::strtoul(value, &end, 10);
    if (*end != '\0' || parsed > std::numeric_limits<uint32_t>::max()) {
        throw std::invalid_argument("SEARCHKERNEL_NATIVE_TRACE_LEVEL must be an unsigned integer");
    }
    return static_cast<uint32_t>(parsed);
}

void write_native_trace(const search::engine::Trace &trace) {
    const char *path = std::getenv("SEARCHKERNEL_NATIVE_TRACE_FILE");
    if (!path || *path == '\0') return;
    if (!trace.hasTrace()) {
        throw std::runtime_error("native trace observation was not available");
    }
    vespalib::SmartBuffer output(4096);
    vespalib::slime::JsonFormat::encode(trace.getSlime(), output, true);
    const auto json = output.obtain();
    std::ofstream output_file(path, std::ios::app);
    if (!output_file) throw std::runtime_error("could not open native trace observation file");
    output_file.write(json.data, static_cast<std::streamsize>(json.size));
    output_file.put('\n');
}

std::string highlight_summary_name(std::string_view field) {
    uint64_t hash = 0xcbf29ce484222325ULL;
    for (const auto byte : field) {
        hash ^= static_cast<uint8_t>(byte);
        hash *= 0x100000001b3ULL;
    }
    char suffix[17];
    std::snprintf(suffix, sizeof(suffix), "%016llx",
                  static_cast<unsigned long long>(hash));
    return std::string(HIGHLIGHT_SUMMARY_PREFIX) + suffix;
}

std::optional<std::vector<LocalHighlightFragmentState>> decode_dynamic_teaser(std::string_view teaser) {
    const auto decoded = searchkernel::native_highlighting::decode(teaser);
    if (!decoded) return std::nullopt;
    std::vector<LocalHighlightFragmentState> fragments;
    fragments.reserve(decoded->size());
    for (const auto &source : *decoded) {
        LocalHighlightFragmentState fragment;
        fragment.text = source.text;
        for (const auto span : source.matches) {
            fragment.matches.push_back({span.start_byte, span.end_byte});
        }
        fragments.push_back(std::move(fragment));
    }
    return fragments;
}


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
    if (value.starts_with(HIGHLIGHT_SUMMARY_PREFIX)) {
        throw std::invalid_argument("generated schema field name uses a reserved highlight prefix");
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


class EmbeddedEngine {
    class EmptySummaryGenerator final : public HitsAggregationResult::SummaryGenerator {
    public:
        vespalib::ConstBufferRef fillSummary(search::expression::DocId, std::string_view) override {
            static const uint8_t empty = 0;
            return {&empty, 0};
        }
    };

    struct ManifestStructField {
        std::string name;
        std::string type;
    };
    struct ManifestField {
        std::string name;
        std::string type;
        std::string tensor_type;
        std::vector<ManifestStructField> struct_fields;
        bool index = false;
        bool attribute = false;
        bool summary = false;
        bool virtual_field = false;
        uint32_t predicate_arity = 0;
    };
    struct SchemaManifest {
        std::string document_type;
        std::vector<ManifestField> fields;
    };

    SchemaManifest _manifest;
    DataDirectoryLock _data_lock;
    std::string _schema_identity;
    std::string _document_type;
    bool _legacy_state;
    std::shared_ptr<const DocumenttypesConfig> _document_types;
    std::shared_ptr<DocumentTypeRepo> _repo;
    const DocumentType *_doc_type;
    EmbeddedWireService _wire_service;
    EmbeddedDBOwner _db_owner;
    vespalib::ThreadStackExecutor _summary_executor;
    mutable EmptySummaryGenerator _empty_summary_generator;
    EmbeddedThreadingService _shared_service;
    HwInfo _hw_info;
    EmbeddedFileHeaderContext _file_header_context;
    std::string _tls_dir;
    PortReservation _tls_port_reservation;
    const unsigned int _tls_port;
    TransLogServer _tls;
    proton::matching::QueryLimiter _query_limiter;
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    // Declared before _db so the DocumentDB is destroyed before its active
    // hardening-only forwarding view.
    std::shared_ptr<PublicationGateFeedView> _hardening_feed_view;
#endif
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
    std::shared_ptr<FailpointController> _failpoints = std::make_shared<FailpointController>();
    std::shared_ptr<ResultFaultController> _result_faults = std::make_shared<ResultFaultController>();
    std::shared_ptr<std::atomic<uint64_t>> _result_destroyed =
        std::make_shared<std::atomic<uint64_t>>(0);
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

    // Rank configuration affects query execution, not persisted document/index
    // layout. Preserve the v2 marker format and compare every storage-relevant
    // byte; parse length-framed blocks so expression text cannot spoof a section.
    static std::optional<std::string> storage_identity(const std::string &identity) {
        if (!identity.starts_with("version=2\ndocument_type=")) return std::nullopt;
        const auto first_file = identity.find("file=indexschema.cfg\nlength=");
        if (first_file == std::string::npos) return std::nullopt;
        std::string result = identity.substr(0, first_file);
        size_t cursor = first_file;
        for (const char *name : {"indexschema.cfg", "attributes.cfg", "summary.cfg",
                                 "rank-profiles.cfg", "juniperrc.cfg"}) {
            const auto prefix = "file=" + std::string(name) + "\nlength=";
            if (identity.compare(cursor, prefix.size(), prefix) != 0) return std::nullopt;
            const auto start = cursor;
            cursor += prefix.size();
            const auto newline = identity.find('\n', cursor);
            if (newline == std::string::npos) return std::nullopt;
            size_t length = 0;
            const auto parsed = std::from_chars(identity.data() + cursor, identity.data() + newline, length);
            if (parsed.ec != std::errc() || parsed.ptr != identity.data() + newline) return std::nullopt;
            cursor = newline + 1;
            if (length >= identity.size() - cursor || identity[cursor + length] != '\n') return std::nullopt;
            cursor += length + 1;
            if (std::string_view(name) != "rank-profiles.cfg") result.append(identity, start, cursor - start);
        }
        if (cursor != identity.size()) return std::nullopt;
        return result;
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
            const auto actual_storage = storage_identity(actual);
            const auto expected_storage = storage_identity(expected);
            if (actual != expected && (!actual_storage || !expected_storage || *actual_storage != *expected_storage)) {
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

    void reject_long_undefined_sentinel(const std::string &field_name, int64_t value) const {
        if (!_legacy_state && value == std::numeric_limits<int64_t>::min()) {
            throw std::invalid_argument(
                "integer value INT64_MIN is reserved as Vespa's undefined attribute sentinel: " +
                field_name);
        }
    }

    void require_text_field(const std::string &name) const {
        const auto &field = require_manifest_field(name);
        if (field.type != "text" || !field.index) {
            throw std::invalid_argument("text operation requires an indexed text field: " + name);
        }
    }

    const ManifestField &require_highlight_field(const std::string &name) const {
        const auto &field = require_manifest_field(name);
        if (field.type != "text" || !field.index || !field.summary) {
            throw std::invalid_argument("field is not a stored searchable text field: " + name);
        }
        return field;
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

    std::unique_ptr<document::ArrayFieldValue>
    make_string_array_value(
        const document::DataType &type, const OwnedValue &value,
        const std::vector<std::vector<TextOccurrence>> *array_occurrences = nullptr) const {
        if (value.kind != SK_VALUE_STRING_ARRAY) {
            throw std::invalid_argument("wrong array value type");
        }
        if (array_occurrences && array_occurrences->size() != value.strings.size()) {
            throw std::invalid_argument("string array annotations do not match array elements");
        }
        auto result = std::make_unique<document::ArrayFieldValue>(type);
        for (size_t index = 0; index < value.strings.size(); ++index) {
            if (array_occurrences) {
                TextDocument analyzed;
                analyzed.text = value.strings[index];
                analyzed.occurrences = (*array_occurrences)[index];
                result->append(make_text_value(analyzed));
            } else {
                result->append(std::make_unique<document::StringFieldValue>(value.strings[index]));
            }
        }
        return result;
    }

    static std::unique_ptr<document::ArrayFieldValue>
    make_struct_array_value(const document::DataType &type, const OwnedValue &value) {
        if (value.kind != SK_VALUE_STRUCT_ARRAY) {
            throw std::invalid_argument("wrong struct array value type");
        }
        const auto *array_type = type.cast_collection();
        const auto *struct_type = array_type
                                      ? dynamic_cast<const document::StructDataType *>(
                                            &array_type->getNestedType())
                                      : nullptr;
        if (!struct_type) throw std::invalid_argument("document field is not an array of structs");
        auto result = std::make_unique<document::ArrayFieldValue>(type);
        for (const auto &source : value.struct_elements) {
            auto element = std::make_unique<document::StructFieldValue>(*struct_type);
            for (const auto &[name, member] : source) {
                if (!struct_type->hasField(name)) {
                    throw std::invalid_argument("unknown struct field: " + name);
                }
                const auto &field = struct_type->getField(name);
                std::unique_ptr<document::FieldValue> field_value;
                switch (member.kind) {
                case SK_VALUE_STRING:
                    field_value = std::make_unique<document::StringFieldValue>(member.string);
                    break;
                case SK_VALUE_INTEGER:
                    field_value = std::make_unique<document::LongFieldValue>(member.integer);
                    break;
                case SK_VALUE_FLOAT:
                    field_value = std::make_unique<document::DoubleFieldValue>(member.floating);
                    break;
                case SK_VALUE_BOOLEAN:
                    field_value = std::make_unique<document::BoolFieldValue>(member.boolean);
                    break;
                default:
                    throw std::invalid_argument("unsupported struct member value: " + name);
                }
                if (field.getDataType().getId() == document::DataType::T_INT &&
                    member.kind == SK_VALUE_INTEGER) {
                    field_value = std::make_unique<document::IntFieldValue>(
                        static_cast<int32_t>(member.integer));
                }
                element->setValue(field, std::move(field_value));
            }
            result->append(std::move(element));
        }
        return result;
    }

    static std::unique_ptr<document::StructFieldValue>
    make_position_value(const document::DataType &type, double latitude, double longitude) {
        if (!std::isfinite(latitude) || !std::isfinite(longitude) ||
            latitude < -90.0 || latitude > 90.0 || longitude < -180.0 || longitude > 180.0) {
            throw std::invalid_argument("position coordinates are outside the valid range");
        }
        const auto scaled = [](double value) {
            const auto result = std::llround(value * 1'000'000.0);
            if (result < std::numeric_limits<int32_t>::min() ||
                result > std::numeric_limits<int32_t>::max()) {
                throw std::invalid_argument("position coordinate is outside Vespa's range");
            }
            return static_cast<int32_t>(result);
        };
        auto value = type.createFieldValue();
        auto *result = dynamic_cast<document::StructFieldValue *>(value.get());
        if (!result) throw std::runtime_error("could not create Vespa position value");
        result->setValue(document::PositionDataType::FIELD_X,
                         std::make_unique<document::IntFieldValue>(scaled(longitude)));
        result->setValue(document::PositionDataType::FIELD_Y,
                         std::make_unique<document::IntFieldValue>(scaled(latitude)));
        return std::unique_ptr<document::StructFieldValue>(
            static_cast<document::StructFieldValue *>(value.release()));
    }

    static void expand_predicate_ranges(vespalib::slime::Cursor &node, uint32_t arity) {
        const auto type = node[document::Predicate::NODE_TYPE].asLong();
        if (type == document::Predicate::TYPE_CONJUNCTION ||
            type == document::Predicate::TYPE_DISJUNCTION ||
            type == document::Predicate::TYPE_NEGATION) {
            auto &children = node[document::Predicate::CHILDREN];
            for (size_t index = 0; index < children.children(); ++index) {
                expand_predicate_ranges(children[index], arity);
            }
            return;
        }
        if (type != document::Predicate::TYPE_FEATURE_RANGE) return;
        const auto hashed_partitions = node[document::Predicate::HASHED_PARTITIONS].valid();
        const auto hashed_edges = node[document::Predicate::HASHED_EDGE_PARTITIONS].valid();
        if (hashed_partitions || hashed_edges) {
            if (hashed_partitions && hashed_edges) return;
            throw std::invalid_argument("predicate range has incomplete native annotations");
        }
        const auto key = node[document::Predicate::KEY].asString().make_string();
        const auto lower = node[document::Predicate::RANGE_MIN].valid()
                               ? node[document::Predicate::RANGE_MIN].asLong()
                               : std::numeric_limits<int64_t>::min();
        const auto upper = node[document::Predicate::RANGE_MAX].valid()
                               ? node[document::Predicate::RANGE_MAX].asLong()
                               : std::numeric_limits<int64_t>::max();
        auto &partitions = node.setArray(document::Predicate::HASHED_PARTITIONS);
        auto &edges = node.setArray(document::Predicate::HASHED_EDGE_PARTITIONS);
        auto add_partition = [&partitions](const std::string &label) {
            partitions.addLong(static_cast<int64_t>(search::predicate::PredicateHash::hash64(label)));
        };
        auto add_edge = [&edges](const std::string &label, int64_t value, int32_t lower_bound,
                                 int32_t upper_bound) {
            uint64_t payload;
            if (lower_bound > 0) {
                payload = upper_bound >= 0
                              ? (static_cast<uint64_t>(lower_bound) << 16) |
                                    static_cast<uint32_t>(upper_bound + 1)
                              : static_cast<uint32_t>(lower_bound) | 0x80000000ULL;
            } else {
                payload = upper_bound >= 0 ? static_cast<uint32_t>(upper_bound + 1) | 0x40000000ULL
                                           : 0x80000000ULL;
            }
            auto &edge = edges.addObject();
            edge.setLong(document::Predicate::HASH,
                         static_cast<int64_t>(search::predicate::PredicateHash::hash64(label)));
            edge.setLong(document::Predicate::VALUE, value);
            edge.setLong(document::Predicate::PAYLOAD, static_cast<int64_t>(payload));
        };
        auto add_edge_for = [&add_edge, &key](int64_t value, int32_t lower_bound, int32_t upper_bound,
                                               bool negative) {
            std::string label = key + "=";
            if (negative) label += "-";
            label += std::to_string(value);
            add_edge(label, value, lower_bound, upper_bound);
        };
        auto add_partition_for = [&add_partition, &key](uint64_t from, uint64_t to, bool negative) {
            std::string label = key + "=";
            if (negative) {
                label += "-" + std::to_string(to) + "-" + std::to_string(from);
            } else {
                label += std::to_string(from) + "-" + std::to_string(to);
            }
            add_partition(label);
        };
        std::function<void(uint64_t, uint64_t, uint64_t, uint64_t, bool)> add_partitions;
        add_partitions = [&add_partition_for](uint64_t part, uint64_t part_size, uint64_t first,
                                               uint64_t last, bool negative) {
            for (uint64_t index = first; index < last; ++index) {
                const auto from = (part + index) * part_size;
                add_partition_for(from, from + part_size - 1, negative);
            }
        };
        std::function<void(uint64_t, uint64_t, uint64_t, uint32_t, bool)> make_partitions;
        make_partitions = [&make_partitions, &add_partitions](uint64_t from, uint64_t to, uint64_t step_size,
                                                               uint32_t partition_arity, bool negative) {
            const auto from_remainder = from % partition_arity;
            const auto to_remainder = to % partition_arity;
            const auto next_from = from - from_remainder;
            const auto next_to = to - to_remainder;
            if (next_from == next_to) {
                add_partitions(next_from, step_size, from_remainder, to_remainder, negative);
            } else {
                if (from_remainder > 0) {
                    add_partitions(next_from, step_size, from_remainder, partition_arity, negative);
                    from += partition_arity - from_remainder;
                }
                add_partitions(next_to, step_size, 0, to_remainder, negative);
                make_partitions(from / partition_arity, to / partition_arity, step_size * partition_arity,
                                partition_arity, negative);
            }
        };
        std::function<void(uint64_t, uint64_t, bool)> partition_range;
        partition_range = [&partition_range, &make_partitions, &add_edge_for, arity](uint64_t from, uint64_t to,
                                                                                       bool negative) {
            const auto from_remainder = from % arity;
            const auto to_remainder = (to % arity + 1) % arity;
            const auto from_value = from - from_remainder;
            const auto to_value = to - to_remainder;
            if (from_value == to_value + 1) {
                add_edge_for(static_cast<int64_t>(from_value), static_cast<int32_t>(from_remainder),
                             static_cast<int32_t>(to_remainder) - 1, negative);
                return;
            }
            if (from_remainder != 0) {
                add_edge_for(static_cast<int64_t>(from_value), static_cast<int32_t>(from_remainder), -1, negative);
                from = from_value + arity;
            }
            if (to_remainder != 0) {
                add_edge_for(static_cast<int64_t>(to_value + 1), -1, static_cast<int32_t>(to_remainder) - 1,
                             negative);
            }
            make_partitions(from / arity, (to_value - (arity - 1)) / arity + 1, arity, arity, negative);
        };
        if (lower < 0 && upper < 0) {
            if (lower == std::numeric_limits<int64_t>::min()) {
                throw std::invalid_argument("predicate range at INT64_MIN is unsupported");
            }
            partition_range(static_cast<uint64_t>(-upper), static_cast<uint64_t>(-lower), true);
        } else if (lower < 0) {
            if (lower == std::numeric_limits<int64_t>::min()) {
                throw std::invalid_argument("predicate range at INT64_MIN is unsupported");
            }
            partition_range(0, static_cast<uint64_t>(-lower), true);
            partition_range(0, static_cast<uint64_t>(upper), false);
        } else {
            partition_range(static_cast<uint64_t>(lower), static_cast<uint64_t>(upper), false);
        }
    }

    struct PredicateNumberValidator final
        : vespalib::slime::ArrayTraverser,
          vespalib::slime::ObjectTraverser {
        void validate(const vespalib::slime::Inspector &value) {
            switch (value.type().getId()) {
            case vespalib::slime::DOUBLE::ID:
                throw std::invalid_argument(
                    "predicate expression numbers must be signed 64-bit integers");
            case vespalib::slime::ARRAY::ID:
                value.traverse(static_cast<vespalib::slime::ArrayTraverser &>(*this));
                break;
            case vespalib::slime::OBJECT::ID:
                value.traverse(static_cast<vespalib::slime::ObjectTraverser &>(*this));
                break;
            default:
                break;
            }
        }

        void entry(size_t, const vespalib::slime::Inspector &value) override {
            validate(value);
        }

        void field(const vespalib::Memory &,
                   const vespalib::slime::Inspector &value) override {
            validate(value);
        }
    };

    static std::unique_ptr<document::PredicateFieldValue>
    make_predicate_value(const OwnedValue &value, uint32_t arity = 8) {
        if (value.kind != SK_VALUE_PREDICATE ||
            (value.predicate_features.empty() && value.predicate_json.empty())) {
            throw std::invalid_argument("predicate value requires at least one feature");
        }
        if (!value.predicate_json.empty()) {
            auto slime = std::make_unique<vespalib::Slime>();
            if (vespalib::slime::JsonFormat::decode(
                    vespalib::Memory(value.predicate_json), *slime) == 0 ||
                !is_slime_type(slime->get(), vespalib::slime::OBJECT::ID)) {
                throw std::invalid_argument("predicate expression is not a JSON object");
            }
            PredicateNumberValidator number_validator;
            number_validator.validate(slime->get());
            expand_predicate_ranges(slime->get(), arity);
            return std::make_unique<document::PredicateFieldValue>(std::move(slime));
        }
        document::PredicateSlimeBuilder builder;
        for (const auto &[key, feature] : value.predicate_features) {
            builder.feature(key).value(feature);
        }
        return std::make_unique<document::PredicateFieldValue>(builder.build());
    }

    std::unique_ptr<document::FieldValue> make_attribute_value(
        const std::string &field_name, const OwnedValue &value) const {
        const auto &field = require_attribute_field(field_name);
        if (field.type == "string") {
            if (value.kind != SK_VALUE_STRING) throw std::invalid_argument("wrong value type for attribute: " + field_name);
            return std::make_unique<document::StringFieldValue>(value.string);
        }
        if (field.type == "integer") {
            if (value.kind != SK_VALUE_INTEGER) {
                throw std::invalid_argument("wrong value type for attribute: " + field_name);
            }
            if (_legacy_state) {
                if (value.integer < std::numeric_limits<int32_t>::min() ||
                    value.integer > std::numeric_limits<int32_t>::max()) {
                    throw std::invalid_argument("integer value is outside the legacy INT32 range: " + field_name);
                }
                return std::make_unique<document::IntFieldValue>(static_cast<int32_t>(value.integer));
            }
            reject_long_undefined_sentinel(field_name, value.integer);
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
        if (field.type == "array<string>") {
            return make_string_array_value(_doc_type->getField(field_name).getDataType(), value);
        }
        if (field.type == "array<struct>") {
            return make_struct_array_value(_doc_type->getField(field_name).getDataType(), value);
        }
        if (field.type == "position") {
            if (value.kind != SK_VALUE_POSITION) {
                throw std::invalid_argument("wrong position value type for attribute: " + field_name);
            }
            return make_position_value(_doc_type->getField(field_name).getDataType(),
                                       value.latitude, value.longitude);
        }
        if (field.type == "predicate") {
            return make_predicate_value(value, field.predicate_arity);
        }
        throw std::invalid_argument("unsupported attribute field type: " + field_name);
    }

    std::shared_ptr<Document> make_attribute_document(const AttributeDocumentState &source) const {
        if (source.fields.empty()) throw std::invalid_argument("attribute document must not be empty");
        const DocumentId id(internal_document_id(source.id, _document_type));
        auto document = std::make_shared<Document>(*_repo, *_doc_type, id);
        for (const auto &[name, value] : source.fields) {
            const auto &manifest_field = require_manifest_field(name);
            auto field_value = make_attribute_value(name, value);
            if (manifest_field.type == "position" && manifest_field.attribute) {
                const auto *position = dynamic_cast<const document::StructFieldValue *>(field_value.get());
                auto x = position->getValue(document::PositionDataType::FIELD_X);
                auto y = position->getValue(document::PositionDataType::FIELD_Y);
                const auto zcurve = vespalib::geo::ZCurve::encode(x->getAsInt(), y->getAsInt());
                document->setFieldValue(
                    _doc_type->getField(document::PositionDataType::getZCurveFieldName(name)),
                    std::make_unique<document::LongFieldValue>(zcurve));
            }
            document->setFieldValue(_doc_type->getField(name), std::move(field_value));
        }
        return document;
    }

    std::unique_ptr<document::FieldValue> make_document_value(
        const std::string &field_name, const OwnedValue &value,
        const std::vector<TextOccurrence> *occurrences = nullptr,
        const std::vector<std::vector<TextOccurrence>> *array_occurrences = nullptr) const {
        const auto &field = require_manifest_field(field_name);
        if (field.type == "text" || field.type == "string") {
            if (value.kind != SK_VALUE_STRING) {
                if (value.kind == SK_VALUE_TENSOR) {
                    throw TensorValidationError("tensor_field_type_mismatch",
                                                "tensor value supplied to non-tensor field: " + field_name);
                }
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
                if (value.kind == SK_VALUE_TENSOR) {
                    throw TensorValidationError("tensor_field_type_mismatch",
                                                "tensor value supplied to non-tensor field: " + field_name);
                }
                throw std::invalid_argument("wrong integer value type for field: " + field_name);
            }
            if (_legacy_state) {
                if (value.integer < std::numeric_limits<int32_t>::min() ||
                    value.integer > std::numeric_limits<int32_t>::max()) {
                    throw std::invalid_argument("integer value is outside the legacy INT32 range: " + field_name);
                }
                return std::make_unique<document::IntFieldValue>(static_cast<int32_t>(value.integer));
            }
            reject_long_undefined_sentinel(field_name, value.integer);
            return std::make_unique<document::LongFieldValue>(value.integer);
        }
        if (field.type == "float") {
            if (value.kind != SK_VALUE_FLOAT || !std::isfinite(value.floating)) {
                if (value.kind == SK_VALUE_TENSOR) {
                    throw TensorValidationError("tensor_field_type_mismatch",
                                                "tensor value supplied to non-tensor field: " + field_name);
                }
                throw std::invalid_argument("wrong float value type for field: " + field_name);
            }
            return std::make_unique<document::DoubleFieldValue>(value.floating);
        }
        if (field.type == "boolean") {
            if (value.kind != SK_VALUE_BOOLEAN) {
                if (value.kind == SK_VALUE_TENSOR) {
                    throw TensorValidationError("tensor_field_type_mismatch",
                                                "tensor value supplied to non-tensor field: " + field_name);
                }
                throw std::invalid_argument("wrong boolean value type for field: " + field_name);
            }
            return std::make_unique<document::BoolFieldValue>(value.boolean);
        }
        if (field.type == "array<string>") {
            return make_string_array_value(_doc_type->getField(field_name).getDataType(), value,
                                           array_occurrences);
        }
        if (field.type == "array<struct>") {
            return make_struct_array_value(_doc_type->getField(field_name).getDataType(), value);
        }
        if (field.type == "position") {
            if (value.kind != SK_VALUE_POSITION) {
                throw std::invalid_argument("wrong position value type for field: " + field_name);
            }
            return make_position_value(_doc_type->getField(field_name).getDataType(),
                                       value.latitude, value.longitude);
        }
        if (field.type == "predicate") {
            return make_predicate_value(value, field.predicate_arity);
        }
        if (field.type == "tensor") {
            if (value.kind != SK_VALUE_TENSOR) {
                throw TensorValidationError("tensor_field_type_mismatch",
                                            "wrong tensor value type for field: " + field_name);
            }
            if (value.tensor_type != field.tensor_type) {
                throw TensorValidationError("tensor_field_type_mismatch",
                                            "tensor value does not match field type: " + field_name);
            }
            vespalib::nbostream stream(value.string.data(), value.string.size());
            auto tensor = vespalib::eval::decode_value(
                stream, vespalib::eval::FastValueBuilderFactory::get());
            auto data_type = _doc_type->getField(field_name).getDataType().cast_tensor();
            if (!data_type || !data_type->isAssignableType(tensor->type())) {
                throw TensorValidationError("tensor_field_type_mismatch",
                                            "tensor value does not match field type: " + field_name);
            }
            auto result = std::make_unique<document::TensorFieldValue>(*data_type);
            *result = std::move(tensor);
            return result;
        }
        if (value.kind == SK_VALUE_TENSOR) {
            throw TensorValidationError("tensor_field_type_mismatch",
                                        "tensor value supplied to non-tensor field: " + field_name);
        }
        throw std::invalid_argument("unsupported document field type: " + field_name);
    }

    std::shared_ptr<Document> make_value_document(const ValueDocument &source) const {
        if (source.fields.empty()) throw std::invalid_argument("document must not be empty");
        const DocumentId id(internal_document_id(source.id, _document_type));
        auto document = std::make_shared<Document>(*_repo, *_doc_type, id);
        for (const auto &entry : source.fields) {
            const auto &manifest_field = require_manifest_field(entry.name);
            const auto *occurrences = entry.occurrences.empty() ? nullptr : &entry.occurrences;
            const auto *array_occurrences = entry.array_occurrences.empty()
                                                ? nullptr
                                                : &entry.array_occurrences;
            auto field_value = make_document_value(entry.name, entry.value, occurrences,
                                                   array_occurrences);
            if (manifest_field.type == "position" && manifest_field.attribute) {
                const auto *position = dynamic_cast<const document::StructFieldValue *>(field_value.get());
                auto x = position->getValue(document::PositionDataType::FIELD_X);
                auto y = position->getValue(document::PositionDataType::FIELD_Y);
                const auto zcurve = vespalib::geo::ZCurve::encode(x->getAsInt(), y->getAsInt());
                document->setFieldValue(
                    _doc_type->getField(document::PositionDataType::getZCurveFieldName(entry.name)),
                    std::make_unique<document::LongFieldValue>(zcurve));
            }
            document->setFieldValue(_doc_type->getField(entry.name), std::move(field_value));
        }
        return document;
    }

    std::shared_ptr<document::DocumentUpdate>
    make_value_update(const ValueUpdate &source) const {
        if (source.fields.empty() && source.arithmetic.empty()) throw std::invalid_argument("document update must not be empty");
        const DocumentId id(internal_document_id(source.id, _document_type));
        auto update = std::make_shared<document::DocumentUpdate>(*_repo, *_doc_type, id);
        update->setCreateIfNonExistent(source.create_if_nonexistent);
        for (const auto &[name, value] : source.fields) {
            const auto &field = _doc_type->getField(name);
            const auto &manifest_field = require_manifest_field(name);
            document::FieldUpdate field_update(field);
            if (value) {
                const auto annotated = std::find_if(
                    source.annotated_fields.begin(), source.annotated_fields.end(),
                    [&name](const auto &entry) { return entry.first == name; });
                const auto *occurrences = annotated == source.annotated_fields.end()
                                               ? nullptr
                                               : &annotated->second;
                const auto annotated_array = std::find_if(
                    source.annotated_array_fields.begin(), source.annotated_array_fields.end(),
                    [&name](const auto &entry) { return entry.first == name; });
                const auto *array_occurrences = annotated_array == source.annotated_array_fields.end()
                                                    ? nullptr
                                                    : &annotated_array->second;
                auto field_value = make_document_value(name, *value, occurrences, array_occurrences);
                std::optional<int64_t> zcurve;
                if (manifest_field.type == "position" && manifest_field.attribute) {
                    const auto *position = dynamic_cast<const document::StructFieldValue *>(field_value.get());
                    if (!position) {
                        throw std::invalid_argument("position field value is not a struct: " + name);
                    }
                    auto x = position->getValue(document::PositionDataType::FIELD_X);
                    auto y = position->getValue(document::PositionDataType::FIELD_Y);
                    zcurve = vespalib::geo::ZCurve::encode(x->getAsInt(), y->getAsInt());
                }
                field_update.addUpdate(std::make_unique<document::AssignValueUpdate>(
                    std::move(field_value)));
                update->addUpdate(std::move(field_update));
                if (zcurve) {
                    // Geo queries read the generated *_zcurve attribute. The
                    // embedded DocumentUpdate path does not derive it from a
                    // position assignment, so keep it in lockstep with the
                    // source field just as make_value_document does for PUT.
                    document::FieldUpdate zcurve_update(_doc_type->getField(
                        document::PositionDataType::getZCurveFieldName(name)));
                    zcurve_update.addUpdate(std::make_unique<document::AssignValueUpdate>(
                        std::make_unique<document::LongFieldValue>(*zcurve)));
                    update->addUpdate(std::move(zcurve_update));
                }
            } else {
                field_update.addUpdate(std::make_unique<document::ClearValueUpdate>());
                update->addUpdate(std::move(field_update));
                if (manifest_field.type == "position" && manifest_field.attribute) {
                    document::FieldUpdate zcurve_update(_doc_type->getField(
                        document::PositionDataType::getZCurveFieldName(name)));
                    zcurve_update.addUpdate(std::make_unique<document::ClearValueUpdate>());
                    update->addUpdate(std::move(zcurve_update));
                }
            }
        }
        for (const auto &arithmetic : source.arithmetic) {
            const auto &field = _doc_type->getField(arithmetic.field);
            if (field.getDataType().getId() != document::DataType::T_DOUBLE &&
                field.getDataType().getId() != document::DataType::T_FLOAT &&
                field.getDataType().getId() != document::DataType::T_LONG) {
                throw std::invalid_argument("arithmetic update requires a numeric field: " + arithmetic.field);
            }
            document::ArithmeticValueUpdate::Operator operation;
            switch (arithmetic.operation) {
            case SK_ARITHMETIC_ADD: operation = document::ArithmeticValueUpdate::Add; break;
            case SK_ARITHMETIC_DIV: operation = document::ArithmeticValueUpdate::Div; break;
            case SK_ARITHMETIC_MUL: operation = document::ArithmeticValueUpdate::Mul; break;
            case SK_ARITHMETIC_SUB: operation = document::ArithmeticValueUpdate::Sub; break;
            default: throw std::invalid_argument("unknown arithmetic update operator");
            }
            document::FieldUpdate field_update(field);
            field_update.addUpdate(std::make_unique<document::ArithmeticValueUpdate>(operation, arithmetic.operand));
            update->addUpdate(std::move(field_update));
        }
        return update;
    }

    bool find_lid(const DocumentId &id, search::IDocumentMetaStore::DocId &lid) const {
        auto ready_sub_db = _db->getReadySubDB();
        if (!ready_sub_db) throw std::runtime_error("Vespa DocumentDB has no ready sub database");
        auto meta_store_guard = ready_sub_db->getDocumentMetaStoreContext().getReadGuard();
        return meta_store_guard->get().getLid(id.getGlobalId(), lid);
    }

    std::unique_ptr<Document> read_source_document(const DocumentId &id) const {
        search::IDocumentMetaStore::DocId lid = 0;
        if (!find_lid(id, lid)) return {};
        auto ready_sub_db = _db->getReadySubDB();
        if (!ready_sub_db) throw std::runtime_error("Vespa DocumentDB has no ready sub database");
        const auto &summary_manager = ready_sub_db->getSummaryManager();
        if (!summary_manager) throw std::runtime_error("Vespa DocumentDB has no summary manager");
        // DocumentRetriever::getFullDocument() patches attribute values into the
        // stored document and deliberately removes zero-valued multivalue
        // attributes. Read the backing document store directly when source
        // presence matters so an explicitly stored empty collection remains
        // distinguishable from an absent field.
        return summary_manager->getBackingStore().read(lid, *_repo);
    }

    std::unique_ptr<Document> read_retrieved_document(const DocumentId &id) const {
        search::IDocumentMetaStore::DocId lid = 0;
        if (!find_lid(id, lid)) return {};
        auto retrievers = _db->getDocumentRetrievers(storage::spi::ReadConsistency::STRONG);
        if (!retrievers || retrievers->empty()) {
            throw std::runtime_error("Vespa DocumentDB has no document retriever");
        }
        return (*retrievers)[0]->getFullDocument(lid);
    }

    bool update_requires_document_store_sync(const ValueUpdate &update) const {
        return std::any_of(update.fields.begin(), update.fields.end(), [this](const auto &entry) {
            const auto &field = require_manifest_field(entry.first);
            return field.type == "array<string>" || field.type == "array<struct>";
        });
    }

    std::unique_ptr<Document> read_document_for_rewrite(const DocumentId &id) const {
        auto source = read_source_document(id);
        if (!source) return {};
        auto observed = read_retrieved_document(id);
        if (!observed) return source;

        // The backing document is authoritative for explicit presence, while
        // the retriever is authoritative for current attribute values. Merge
        // the two before a read-modify-write update so memory-only attribute
        // updates are not overwritten by a later collection rewrite.
        for (const auto &manifest_field : _manifest.fields) {
            if (!manifest_field.attribute || manifest_field.virtual_field) continue;
            const auto &field = _doc_type->getField(manifest_field.name);
            auto observed_value = observed->getValue(field);
            const bool observed_has_value =
                observed_value &&
                (observed->hasValue(field) || manifest_field.type == "tensor");
            if (observed_has_value) {
                if (!source->hasValue(field) &&
                    observed_value->type() == document::FieldValue::Type::BOOL &&
                    observed_value->getAsLong() == 0) {
                    continue;
                }
                source->setFieldValue(field, std::move(observed_value));
                continue;
            }

            auto source_value = source->getValue(field);
            if (!source_value) continue;
            bool preserve_source_presence = false;
            if (source_value->type() == document::FieldValue::Type::STRING) {
                preserve_source_presence = source_value->getAsString().empty();
            } else if (source_value->type() == document::FieldValue::Type::LONG ||
                       source_value->type() == document::FieldValue::Type::INT) {
                preserve_source_presence =
                    source_value->getAsLong() ==
                    (_legacy_state ? std::numeric_limits<int32_t>::min()
                                   : std::numeric_limits<int64_t>::min());
            } else if (source_value->type() == document::FieldValue::Type::ARRAY) {
                const auto *array =
                    dynamic_cast<const document::ArrayFieldValue *>(source_value.get());
                preserve_source_presence = array && array->isEmpty();
            }
            if (!preserve_source_presence) {
                source->remove(field);
            }
        }
        return source;
    }

    static std::string slime_json(const vespalib::Slime &slime) {
        vespalib::SmartBuffer output(1024);
        vespalib::slime::JsonFormat::encode(slime, output, true);
        const auto json = output.obtain();
        return std::string(json.data, json.size);
    }

    static std::string json_string(const std::string &value) {
        vespalib::Slime encoded;
        encoded.setString(value);
        return slime_json(encoded);
    }

    static std::string json_double(double value) {
        if (!std::isfinite(value)) {
            throw std::invalid_argument("non-finite composite float cannot be encoded");
        }
        std::ostringstream output;
        output.imbue(std::locale::classic());
        output << std::setprecision(std::numeric_limits<double>::max_digits10) << value;
        if (!output) throw std::runtime_error("could not encode composite float");
        return output.str();
    }

    static const char *struct_scalar_type(const OwnedValue &value) {
        switch (value.kind) {
        case SK_VALUE_STRING: return "string";
        case SK_VALUE_INTEGER: return "integer";
        case SK_VALUE_FLOAT: return "float";
        case SK_VALUE_BOOLEAN: return "boolean";
        default: throw std::invalid_argument("unsupported struct member value kind");
        }
    }

    static std::string struct_scalar_json(const OwnedValue &value) {
        switch (value.kind) {
        case SK_VALUE_STRING:
            return json_string(value.string);
        case SK_VALUE_INTEGER:
            return std::to_string(value.integer);
        case SK_VALUE_FLOAT:
            return json_double(value.floating);
        case SK_VALUE_BOOLEAN:
            return value.boolean ? "true" : "false";
        default:
            throw std::invalid_argument("unsupported struct member value kind");
        }
    }

    static std::string struct_array_json(
        const std::vector<std::vector<std::pair<std::string, OwnedValue>>> &elements) {
        std::string result = "[";
        bool first_element = true;
        for (const auto &element : elements) {
            if (!first_element) result += ',';
            first_element = false;
            result += '{';
            bool first_field = true;
            for (const auto &[name, value] : element) {
                if (!first_field) result += ',';
                first_field = false;
                result += json_string(name);
                result += ":{\"type\":\"";
                result += struct_scalar_type(value);
                result += "\",\"value\":";
                result += struct_scalar_json(value);
                result += '}';
            }
            result += '}';
        }
        result += ']';
        return result;
    }

    OwnedValue owned_value(const document::FieldValue &value, const std::string &field_name) const {
        OwnedValue owned;
        switch (value.type()) {
        case document::FieldValue::Type::STRING:
            owned.kind = SK_VALUE_STRING;
            owned.string = value.getAsString();
            break;
        case document::FieldValue::Type::LONG:
            owned.kind = SK_VALUE_INTEGER;
            owned.integer = value.getAsLong();
            break;
        case document::FieldValue::Type::INT:
            owned.kind = SK_VALUE_INTEGER;
            owned.integer = value.getAsInt();
            break;
        case document::FieldValue::Type::DOUBLE:
            owned.kind = SK_VALUE_FLOAT;
            owned.floating = value.getAsDouble();
            break;
        case document::FieldValue::Type::BOOL:
            owned.kind = SK_VALUE_BOOLEAN;
            owned.boolean = value.getAsLong() != 0;
            break;
        case document::FieldValue::Type::ARRAY: {
            const auto *array = dynamic_cast<const document::ArrayFieldValue *>(&value);
            if (!array) throw std::invalid_argument("array field has no value: " + field_name);
            const bool struct_array = require_manifest_field(field_name).type == "array<struct>";
            if (struct_array) {
                owned.kind = SK_VALUE_STRUCT_ARRAY;
                for (const auto &entry : *array) {
                    const auto *struct_value = dynamic_cast<const document::StructFieldValue *>(&entry);
                    if (!struct_value) {
                        throw std::invalid_argument("retrieved struct array field has a non-struct element: " + field_name);
                    }
                    std::vector<std::pair<std::string, OwnedValue>> fields;
                    for (auto iterator = struct_value->begin(); iterator != struct_value->end(); ++iterator) {
                        const auto &member = iterator.field();
                        const auto full_name = member.getName();
                        const auto separator = full_name.rfind('.');
                        const auto member_name = separator == std::string::npos
                                                     ? full_name
                                                     : full_name.substr(separator + 1);
                        auto member_value = struct_value->getValue(member_name);
                        if (!member_value) continue;
                        fields.emplace_back(member_name, owned_value(*member_value, field_name));
                    }
                    owned.struct_elements.push_back(std::move(fields));
                }
                owned.string = struct_array_json(owned.struct_elements);
            } else {
                owned.kind = SK_VALUE_STRING_ARRAY;
                vespalib::Slime encoded;
                auto &encoded_array = encoded.setArray();
                for (const auto &entry : *array) {
                    if (entry.type() != document::FieldValue::Type::STRING) {
                        throw std::invalid_argument("retrieved array field is not string-valued: " + field_name);
                    }
                    owned.strings.push_back(entry.getAsString());
                    encoded_array.addString(entry.getAsString());
                }
                owned.string = slime_json(encoded);
            }
            break;
        }
        case document::FieldValue::Type::STRUCT: {
            const auto *position = dynamic_cast<const document::StructFieldValue *>(&value);
            if (!position) {
                throw std::invalid_argument("retrieved struct field is not a position: " + field_name);
            }
            auto x = position->getValue(document::PositionDataType::FIELD_X);
            auto y = position->getValue(document::PositionDataType::FIELD_Y);
            if (!x || !y) throw std::invalid_argument("position field has no coordinates: " + field_name);
            owned.kind = SK_VALUE_POSITION;
            owned.longitude = static_cast<double>(x->getAsInt()) / 1'000'000.0;
            owned.latitude = static_cast<double>(y->getAsInt()) / 1'000'000.0;
            owned.string = "{\"latitude\":" + json_double(owned.latitude) +
                           ",\"longitude\":" + json_double(owned.longitude) + "}";
            break;
        }
        case document::FieldValue::Type::PREDICATE:
            {
            const auto *predicate = dynamic_cast<const document::PredicateFieldValue *>(&value);
            if (!predicate) throw std::invalid_argument("predicate field has no value: " + field_name);
            owned.kind = SK_VALUE_PREDICATE;
            owned.predicate_json = slime_json(predicate->getSlime());
            owned.string = owned.predicate_json;
            break;
            }
        case document::FieldValue::Type::TENSOR: {
            const auto *tensor_value = dynamic_cast<const document::TensorFieldValue *>(&value);
            const auto *tensor = tensor_value ? tensor_value->getAsTensorPtr() : nullptr;
            if (!tensor) throw std::invalid_argument("tensor field has no value: " + field_name);
            owned.kind = SK_VALUE_TENSOR;
            owned.tensor_type = tensor->type().to_spec();
            owned.tensor_cells = dense_tensor_cells(*tensor);
            vespalib::nbostream stream;
            vespalib::eval::encode_value(*tensor, stream);
            owned.string.assign(stream.data(), stream.size());
            break;
        }
        default:
            throw std::invalid_argument("unsupported retrieved document field type: " + field_name);
        }
        return owned;
    }

    static bool value_matches(const document::FieldValue &actual, const OwnedValue &expected,
                              uint32_t predicate_arity = 8) {
        switch (expected.kind) {
        case SK_VALUE_STRING:
            return actual.type() == document::FieldValue::Type::STRING && actual.getAsString() == expected.string;
        case SK_VALUE_INTEGER:
            return (actual.type() == document::FieldValue::Type::LONG ||
                    actual.type() == document::FieldValue::Type::INT) &&
                   actual.getAsLong() == expected.integer;
        case SK_VALUE_FLOAT:
            return actual.type() == document::FieldValue::Type::DOUBLE && actual.getAsDouble() == expected.floating;
        case SK_VALUE_BOOLEAN:
            return actual.type() == document::FieldValue::Type::BOOL && (actual.getAsLong() != 0) == expected.boolean;
        case SK_VALUE_STRING_ARRAY: {
            const auto *array = dynamic_cast<const document::ArrayFieldValue *>(&actual);
            if (!array || array->size() != expected.strings.size()) return false;
            return std::equal(array->begin(), array->end(), expected.strings.begin(),
                              [](const auto &entry, const auto &value) {
                                  return entry.type() == document::FieldValue::Type::STRING &&
                                         entry.getAsString() == value;
                              });
        }
        case SK_VALUE_STRUCT_ARRAY: {
            const auto *array = dynamic_cast<const document::ArrayFieldValue *>(&actual);
            if (!array || array->size() != expected.struct_elements.size()) return false;
            auto actual_entry = array->begin();
            for (const auto &expected_entry : expected.struct_elements) {
                if (actual_entry == array->end()) return false;
                const auto *actual_struct = dynamic_cast<const document::StructFieldValue *>(&*actual_entry);
                if (!actual_struct) return false;
                for (const auto &[name, expected_value] : expected_entry) {
                    if (!actual_struct->hasValue(name)) return false;
                    auto actual_value = actual_struct->getValue(name);
                    if (!actual_value || !value_matches(*actual_value, expected_value)) return false;
                }
                ++actual_entry;
            }
            return actual_entry == array->end();
        }
        case SK_VALUE_POSITION: {
            const auto *position = dynamic_cast<const document::StructFieldValue *>(&actual);
            if (!position) return false;
            auto x = position->getValue(document::PositionDataType::FIELD_X);
            auto y = position->getValue(document::PositionDataType::FIELD_Y);
            if (!x || !y) return false;
            return x->getAsLong() == std::llround(expected.longitude * 1'000'000.0) &&
                   y->getAsLong() == std::llround(expected.latitude * 1'000'000.0);
        }
        case SK_VALUE_PREDICATE: {
            if (actual.type() != document::FieldValue::Type::PREDICATE) return false;
            auto expected_value = make_predicate_value(expected, predicate_arity);
            return actual.compare(*expected_value) == 0;
        }
        case SK_VALUE_TENSOR: {
            // The document store may expose a lazy TensorFieldValue whose
            // evaluator-backed payload is not available during the immediate
            // post-feed visibility probe. The field type is already validated
            // while constructing the document; presence of a tensor value is
            // the stable visibility signal here.
            return actual.type() == document::FieldValue::Type::TENSOR;
        }
        default:
            return false;
        }
    }

    static OwnedValue string_value(std::string value) {
        OwnedValue result;
        result.kind = SK_VALUE_STRING;
        result.string = std::move(value);
        return result;
    }

    static OwnedValue integer_value(int64_t value) {
        OwnedValue result;
        result.kind = SK_VALUE_INTEGER;
        result.integer = value;
        return result;
    }

    static std::vector<std::pair<std::string, std::optional<OwnedValue>>>
    expected_fields(const ValueDocument &document) {
        std::vector<std::pair<std::string, std::optional<OwnedValue>>> result;
        result.reserve(document.fields.size());
        for (const auto &field : document.fields) {
            result.emplace_back(field.name, field.value);
        }
        return result;
    }

    static std::vector<std::pair<std::string, std::optional<OwnedValue>>>
    expected_fields(const AttributeDocumentState &document) {
        std::vector<std::pair<std::string, std::optional<OwnedValue>>> result;
        result.reserve(document.fields.size());
        for (const auto &field : document.fields) {
            result.emplace_back(field.first, field.second);
        }
        return result;
    }

    static std::vector<std::pair<std::string, std::optional<OwnedValue>>>
    expected_fields(const AttributeUpdateState &update) {
        std::vector<std::pair<std::string, std::optional<OwnedValue>>> result;
        result.reserve(update.assignments.size());
        for (const auto &assignment : update.assignments) {
            result.emplace_back(assignment.first, assignment.second);
        }
        return result;
    }

    bool source_fields_match_impl(
        const DocumentId &id, bool expected_present,
        const std::vector<std::pair<std::string, std::optional<OwnedValue>>> &expected_fields,
        bool exact) const {
        auto document = read_source_document(id);
        auto observed = exact ? nullptr : read_retrieved_document(id);
        const Document *value_document = exact ? document.get() : observed.get();
        if (static_cast<bool>(document) != expected_present || static_cast<bool>(value_document) != expected_present) {
            return false;
        }
        if (!document) return true;
        if (expected_fields.empty()) return true;
        search::IDocumentMetaStore::DocId lid = 0;
        if (!find_lid(id, lid)) return false;
        for (const auto &[name, value] : expected_fields) {
            const auto &field = _doc_type->getField(name);
            const auto manifest = std::find_if(_manifest.fields.begin(), _manifest.fields.end(),
                                              [&name](const auto &entry) { return entry.name == name; });
            const bool is_attribute = manifest != _manifest.fields.end() && manifest->attribute;
            // Exact source checks deliberately skip the retrieved-document
            // path.  In that mode the source document is also the only valid
            // view for attributes; never dereference the absent observation.
            const Document *field_document = is_attribute && !exact ? observed.get() : document.get();
            if (!value) {
                if (is_attribute) {
                    auto actual = field_document->getValue(field);
                    const bool boolean_default = actual && actual->type() == document::FieldValue::Type::BOOL &&
                                                 actual->getAsLong() == 0;
                    if (field_document->hasValue(field) && !boolean_default) {
                        return false;
                    }
                } else if (static_cast<bool>(field_document->getValue(field))) {
                    return false;
                }
                continue;
            }
            auto actual = field_document->getValue(field);
            if (!exact && is_attribute &&
                ((value->kind == SK_VALUE_STRING_ARRAY && value->strings.empty()) ||
                 (value->kind == SK_VALUE_STRUCT_ARRAY && value->struct_elements.empty()))) {
                // Vespa's attribute-patched retriever removes zero-valued
                // multivalue fields. Confirm the assignment against the raw
                // backing document instead of waiting forever for a presence
                // bit that intentionally remains false.
                auto source_actual = document->getValue(field);
                if (source_actual && value_matches(
                        *source_actual, *value,
                        manifest != _manifest.fields.end() ? manifest->predicate_arity : 8)) {
                    continue;
                }
            }
            if (value->kind == SK_VALUE_STRUCT_ARRAY) {
                // Vespa may normalize array-of-struct members while publishing
                // the source document. Presence is the stable visibility
                // signal here; the same native document is still used by the
                // matcher and attribute views below.
                if (!actual || actual->type() != document::FieldValue::Type::ARRAY) return false;
                continue;
            }
            if (!actual || (!exact && !field_document->hasValue(field)) ||
                !value_matches(*actual, *value, manifest != _manifest.fields.end() ? manifest->predicate_arity : 8)) {
                return false;
            }
        }
        if (exact) {
            for (const auto &field : _manifest.fields) {
                // Struct member paths are represented in the manifest for
                // projection, but are not top-level document fields.
                if (field.name.find('.') != std::string::npos) continue;
                const bool expected = std::any_of(
                    expected_fields.begin(), expected_fields.end(),
                    [&field](const auto &entry) { return entry.first == field.name && entry.second.has_value(); });
                // getValue() can materialize an empty default for an absent
                // array. The source document's presence bit distinguishes
                // that from an explicitly stored empty collection.
                const bool actual_present = value_document->hasValue(_doc_type->getField(field.name));
                if (actual_present != expected) {
                    return false;
                }
            }
        }
        return true;
    }

    bool source_fields_match(
        const DocumentId &id, bool expected_present,
        const std::vector<std::pair<std::string, std::optional<OwnedValue>>> &expected_fields,
        bool exact) const {
        try {
            return source_fields_match_impl(id, expected_present, expected_fields, exact);
        } catch (const std::exception &) {
            // A summary write can publish the LID before its serialized
            // document is readable. Treat that transient state as not yet
            // visible and let the bounded retry complete the operation.
            return false;
        }
    }

    void wait_for_search_publication() {
        // Source/document-retriever visibility is not the same thing as the
        // generation consumed by matching. Initiate the canonical feed-view
        // force-commit on Vespa's master write thread, but never wait for its
        // completion there. The caller waits on the completion callback so the
        // master executor remains available to concurrent feed operations.
        //
        // Sample the serial on the master thread immediately before initiating
        // the commit. This defines the fence by master ordering: it covers this
        // mutation and any feed operations already processed ahead of the fence,
        // but does not make the caller wait for operations still queued behind it.
        failpoint("mutation_before_search_publication_wait");
        auto *ready_sub_db = _db->getReadySubDB();
        if (!ready_sub_db) {
            throw std::runtime_error("Vespa DocumentDB has no ready sub database");
        }
        auto feed_view = ready_sub_db->getFeedView();
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        // The hardening view forwards to this same real Vespa view and also
        // gates FeedHandler's independently scheduled automatic forceCommit.
        // Using it here makes the regression control both publication paths.
        if (_hardening_feed_view) feed_view = _hardening_feed_view;
#endif
        if (!feed_view) {
            throw std::runtime_error("Vespa DocumentDB has no ready feed view");
        }

        vespalib::Gate gate;
        std::exception_ptr failure;
        auto on_done = std::make_shared<vespalib::GateCallback>(gate);
        _db->getWriteService().blocking_master_execute(
            vespalib::makeLambdaTask(
                [this, feed_view = std::move(feed_view), on_done, &failure]() mutable {
                    try {
                        const auto serial = _db->getFeedHandler().getSerialNum();
                        feed_view->forceCommit(
                            search::CommitParam(serial, search::CommitParam::UpdateStats::SKIP),
                            on_done);
                    } catch (...) {
                        failure = std::current_exception();
                    }
                }));
        // The lambda and this local reference are the only non-Vespa owners.
        // Dropping our copy makes GateCallback completion track only commit work.
        on_done.reset();
        if (failure) {
            gate.await();
            std::rethrow_exception(failure);
        }
        failpoint("mutation_after_search_publication_start");
        gate.await();
    }

    void wait_for_source_state(
        const DocumentId &id, bool expected_present,
        const std::vector<std::pair<std::string, std::optional<OwnedValue>>> &expected_fields,
        bool exact) const {
        constexpr int attempts = 1000;
        std::string last_error;
        for (int attempt = 0; attempt < attempts; ++attempt) {
            try {
                if (source_fields_match(id, expected_present, expected_fields, exact)) return;
            } catch (const std::exception &error) {
                // A feed operation can complete before the document store has
                // published a readable blob. Retry the read until the native
                // state becomes visible instead of exposing Vespa's transient
                // stream-read exception to the caller.
                last_error = error.what();
            }
            std::this_thread::sleep_for(std::chrono::milliseconds(1));
        }
        if (!last_error.empty()) {
            throw std::runtime_error("Vespa document operation did not become visible: " + last_error);
        }
        throw std::runtime_error("Vespa document operation did not become visible");
    }

    bool source_document_present(const DocumentId &id) const {
        return static_cast<bool>(read_source_document(id));
    }

    ValueDocument get_value_document(const std::string &external_id) const {
        std::shared_lock<std::shared_mutex> lifecycle_lock(_lifecycle_mutex);
        require_open();
        validate_document_id(external_id.c_str());
        const DocumentId id(internal_document_id(external_id, _document_type));
        auto document = read_source_document(id);
        if (!document) {
            ValueDocument result;
            result.id = external_id;
            return result;
        }
        ValueDocument result;
        result.id = external_id;
        result.exists = true;
        for (auto iterator = document->begin(); iterator != document->end(); ++iterator) {
            const auto &field = *iterator;
            const auto manifest_field = std::find_if(
                _manifest.fields.begin(), _manifest.fields.end(),
                [&field](const auto &candidate) { return candidate.name == field.getName(); });
            if (manifest_field == _manifest.fields.end() || manifest_field->virtual_field) continue;
            auto value = document->getValue(field);
            if (!value) continue;
            OwnedValue owned = owned_value(*value, field.getName());
            result.fields.push_back({field.getName(), std::move(owned), {}, {}});
        }
        search::IDocumentMetaStore::DocId lid = 0;
        auto observed = find_lid(id, lid) ? read_retrieved_document(id) : nullptr;
        if (observed) {
            for (const auto &manifest_field : _manifest.fields) {
                if (!manifest_field.attribute || manifest_field.virtual_field) continue;
                const auto &schema_field = _doc_type->getField(manifest_field.name);
                const auto existing = std::find_if(
                    result.fields.begin(), result.fields.end(),
                    [&manifest_field](const auto &entry) { return entry.name == manifest_field.name; });
                auto observed_value = observed->getValue(schema_field);
                // Tensor attributes are backed by a tensor attribute store and
                // can have a value even when the document retriever's generic
                // presence bit is false. Preserve their value for the public
                // GET path instead of treating it as an absent scalar field.
                const bool observed_has_value = observed_value &&
                    (observed->hasValue(schema_field) || manifest_field.type == "tensor");
                if (!observed_has_value) {
                    // Boolean attributes have no undefined sentinel in Vespa:
                    // an unset field reads as false. GET exposes that default
                    // so callers can distinguish a known boolean field from
                    // a missing schema field.
                    if (observed_value &&
                        observed_value->type() == document::FieldValue::Type::BOOL &&
                        observed_value->getAsLong() == 0) {
                        DocumentFieldState state{
                            manifest_field.name,
                            owned_value(*observed_value, manifest_field.name),
                            {},
                            {}};
                        if (existing != result.fields.end()) {
                            *existing = std::move(state);
                        } else {
                            result.fields.push_back(std::move(state));
                        }
                        continue;
                    }
                    if (manifest_field.attribute && existing != result.fields.end() &&
                        existing->value.kind == SK_VALUE_STRING &&
                        existing->value.string.empty()) {
                        continue;
                    }
                    if (manifest_field.attribute && existing != result.fields.end() &&
                        ((existing->value.kind == SK_VALUE_STRING_ARRAY &&
                          existing->value.strings.empty()) ||
                         (existing->value.kind == SK_VALUE_STRUCT_ARRAY &&
                          existing->value.struct_elements.empty()))) {
                        // The attribute-patched retriever intentionally removes
                        // zero-valued multivalue fields. The backing document
                        // remains authoritative for explicit collection
                        // presence so [] does not collapse to absence.
                        continue;
                    }
                    // Signed integer attributes use their minimum value as the
                    // undefined sentinel, so an explicitly stored boundary value is
                    // absent from the attribute view but remains in the source document.
                    if (manifest_field.attribute && existing != result.fields.end() &&
                        existing->value.kind == SK_VALUE_INTEGER &&
                        existing->value.integer == (_legacy_state
                                                        ? std::numeric_limits<int32_t>::min()
                                                        : std::numeric_limits<int64_t>::min())) {
                        continue;
                    }
                    if (existing != result.fields.end()) result.fields.erase(existing);
                    continue;
                }
                DocumentFieldState state{manifest_field.name, owned_value(*observed_value, manifest_field.name), {}, {}};
                if (existing != result.fields.end()) {
                    *existing = std::move(state);
                } else {
                    result.fields.push_back(std::move(state));
                }
            }
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
        if (type == "array<string>") return "STRING";
        if (type == "integer") return "INT64";
        if (type == "float") return "DOUBLE";
        if (type == "boolean") return "BOOL";
        if (type == "position") return "INT64";
        if (type == "predicate") return "PREDICATE";
        if (type == "tensor") return "TENSOR";
        throw std::invalid_argument("inconsistent generated configuration");
    }

    static bool attribute_type_matches(const std::string &manifest_type,
                                       const std::string &config_type) {
        // Existing generated configurations may use the legacy INT32
        // spelling. New configurations use the portable Int64 contract.
        return config_type == manifest_attribute_type(manifest_type) ||
               (manifest_type == "integer" && config_type == "INT32");
    }

    static std::string manifest_summary_type(const std::string &type) {
        if (type == "text" || type == "string") return "longstring";
        if (type == "array<string>") return "longstring";
        if (type == "integer") return "integer";
        if (type == "float") return "double";
        if (type == "boolean") return "bool";
        if (type == "tensor") return "tensor";
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
            if (field.type != "text" && field.type != "array<string>") {
                throw std::invalid_argument("inconsistent generated configuration");
            }
            auto expected_index_name = field.name;
            if (field.virtual_field) {
                expected_index_name.clear();
                for (const auto character : field.name) {
                    if (character == '.') expected_index_name += "__";
                    else expected_index_name += character;
                }
            }
            require_config_value(values, prefix + "name", expected_index_name);
            require_config_value(values, prefix + "datatype", "STRING");
            require_config_value(values, prefix + "collectiontype",
                                 (field.type == "array<string>" || field.virtual_field) ? "ARRAY" : "SINGLE");
            require_config_value(values, prefix + "prefix", "false");
            require_config_value(values, prefix + "phrases", "false");
            if (field.type == "text") {
                require_config_value(values, prefix + "positions", "true");
            }
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
            require_config_value(values, prefix + "name",
                                 field.type == "position" ? field.name + "_zcurve" : field.name);
            if (!attribute_type_matches(field.type, config_value(values, prefix + "datatype"))) {
                throw std::invalid_argument("inconsistent generated configuration");
            }
            if (field.type == "tensor") {
                require_config_value(values, prefix + "tensortype", field.tensor_type);
            }
            if (field.type == "predicate") {
                require_config_value(values, prefix + "arity", std::to_string(field.predicate_arity));
            }
            require_config_value(values, prefix + "collectiontype",
                                 (field.type == "array<string>" || field.virtual_field) ? "ARRAY" : "SINGLE");
            ++index;
        }
    }

    static void validate_summary_configuration(const SchemaManifest &manifest,
                                               const std::filesystem::path &config_path) {
        const auto values = read_config(config_path / "summary.cfg");
        require_config_value(values, "defaultsummaryid", "1");
        const auto highlight_count = std::count_if(
            manifest.fields.begin(), manifest.fields.end(),
            [](const auto &field) {
                return field.type == "text" && field.index && field.summary;
            });
        const size_t expected_class_count = highlight_count == 0 ? 1 : 2;
        if (config_section_count(values, "classes") != expected_class_count) {
            throw std::invalid_argument("inconsistent generated configuration");
        }

        const std::vector<std::tuple<std::string, std::string, std::string, std::string>> implicit = {
            {"documentid", "longstring", "documentid", ""},
            {"rankfeatures", "featuredata", "rankfeatures", ""},
            {"summaryfeatures", "featuredata", "summaryfeatures", ""}};
        const auto validate_common_fields = [&](size_t class_index) {
            size_t index = 0;
            for (const auto &[name, type, command, source] : implicit) {
                const auto prefix = "classes[" + std::to_string(class_index) + "].fields[" +
                                    std::to_string(index++) + "].";
                if (config_value(values, prefix + "name") != name ||
                    config_value(values, prefix + "type") != type ||
                    config_value(values, prefix + "command") != command ||
                    config_value(values, prefix + "source") != source) {
                    throw std::invalid_argument("inconsistent generated configuration");
                }
            }
            for (const auto &field : manifest.fields) {
                if (!field.summary) continue;
                const auto prefix = "classes[" + std::to_string(class_index) + "].fields[" +
                                    std::to_string(index++) + "].";
                const auto command = field.attribute ? "attribute" : "";
                const auto source = field.attribute ? field.name : "";
                if (config_value(values, prefix + "name") != field.name ||
                    config_value(values, prefix + "type") != manifest_summary_type(field.type) ||
                    config_value(values, prefix + "command") != command ||
                    config_value(values, prefix + "source") != source) {
                    throw std::invalid_argument("inconsistent generated configuration");
                }
            }
            return index;
        };

        require_config_value(values, "classes[0].id", "1");
        require_config_value(values, "classes[0].name", "default");
        const auto default_count = config_section_count(values, "classes[0].fields");
        const auto common_count = validate_common_fields(0);
        if (default_count != common_count) {
            throw std::invalid_argument("inconsistent generated configuration");
        }

        if (highlight_count != 0) {
            require_config_value(values, "classes[1].id", "2");
            require_config_value(values, "classes[1].name", "searchkernel_highlight");
            const auto highlight_class_count = config_section_count(values, "classes[1].fields");
            size_t index = validate_common_fields(1);
            for (const auto &field : manifest.fields) {
                if (field.type != "text" || !field.index || !field.summary) continue;
                const auto prefix = "classes[1].fields[" + std::to_string(index++) + "].";
                if (config_value(values, prefix + "name") != highlight_summary_name(field.name) ||
                    config_value(values, prefix + "type") != "longstring" ||
                    config_value(values, prefix + "command") != "dynamicteaser" ||
                    config_value(values, prefix + "source") != field.name) {
                    throw std::invalid_argument("inconsistent generated configuration");
                }
            }
            if (highlight_class_count != index) {
                throw std::invalid_argument("inconsistent generated configuration");
            }
        }
    }

    static void validate_rank_profile_configuration(const SchemaManifest &manifest,
                                                    const std::filesystem::path &config_path) {
        const auto values = read_config(config_path / "rank-profiles.cfg");
        const auto text_field = std::find_if(
            manifest.fields.begin(), manifest.fields.end(),
            [](const auto &field) { return field.type == "text" && !field.virtual_field; });
        const size_t profile_count = config_section_count(values, "rankprofile");
        const auto legacy_text_expression = text_field == manifest.fields.end()
                                                ? std::string()
                                                : "bm25(" + text_field->name + ")";
        const auto legacy_property_count =
            profile_count >= 2
                ? config_section_count(values, "rankprofile[1].fef.property")
                : 0;
        const bool legacy_text_profile =
            profile_count >= 2 && text_field != manifest.fields.end() &&
            config_value(values, "rankprofile[1].name") == "bm25" &&
            legacy_property_count == 1 &&
            config_value(values, "rankprofile[1].fef.property[0].name") ==
                "vespa.rank.firstphase" &&
            config_value(values, "rankprofile[1].fef.property[0].value") ==
                legacy_text_expression;
        const bool legacy_attribute_profile =
            profile_count >= 2 && text_field == manifest.fields.end() &&
            config_value(values, "rankprofile[1].name") == "attribute_price" &&
            legacy_property_count == 1 &&
            config_value(values, "rankprofile[1].fef.property[0].name") ==
                "vespa.rank.firstphase" &&
            config_value(values, "rankprofile[1].fef.property[0].value") ==
                "attribute(price)";
        require_config_value(values, "rankprofile[0].name", "default");
        size_t first_explicit_profile = 1;
        if (legacy_text_profile) {
            require_config_value(values, "rankprofile[1].name", "bm25");
            if (config_section_count(values, "rankprofile[1].fef.property") != 1) {
                throw std::invalid_argument("inconsistent generated configuration");
            }
            require_config_value(values, "rankprofile[1].fef.property[0].name",
                                 "vespa.rank.firstphase");
            require_config_value(
                values, "rankprofile[1].fef.property[0].value",
                "bm25(" + text_field->name + ")");
            first_explicit_profile = 2;
        } else if (legacy_attribute_profile) {
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
            first_explicit_profile = 2;
        }

        if (profile_count == 0) {
            throw std::invalid_argument("inconsistent generated configuration");
        }
        if (text_field != manifest.fields.end() && profile_count < 2) {
            throw std::invalid_argument("inconsistent generated configuration");
        }
        for (size_t index = first_explicit_profile; index < profile_count; ++index) {
            const auto prefix = "rankprofile[" + std::to_string(index) + "].";
            const auto name = config_value(values, prefix + "name");
            validate_manifest_identifier(name, "rank profile name");
            if (name == "default") {
                throw std::invalid_argument("default rank profile is reserved");
            }
            const auto property_section = prefix + "fef.property";
            const auto property_count = config_section_count(values, property_section.c_str());
            if (property_count < 2) {
                throw std::invalid_argument("rank profile must have a first-phase expression");
            }
            require_config_value(values, prefix + "fef.property[0].name",
                                 "vespa.rank.firstphase");
            const auto first_phase_value =
                config_value(values, prefix + "fef.property[0].value");
            if (first_phase_value.empty()) {
                throw std::invalid_argument("rank profile first-phase expression is empty");
            }
            const auto named_first_phase =
                first_phase_value == "rankingExpression(" + name + ")";
            bool has_first_phase_script = false;
            bool has_second_phase = false;
            bool has_second_phase_script = false;
            bool has_rerank_count = false;
            for (size_t property = 1; property < property_count; ++property) {
                const auto property_prefix =
                    prefix + "fef.property[" + std::to_string(property) + "].";
                const auto property_name = config_value(values, property_prefix + "name");
                const auto property_value = config_value(values, property_prefix + "value");
                if (property_name == "rankingExpression(" + name + ").rankingScript") {
                    if (property_value.empty()) {
                        throw std::invalid_argument("rank profile ranking script is empty");
                    }
                    has_first_phase_script = true;
                } else if (property_name == "vespa.rank.secondphase") {
                    if (property_value != "rankingExpression(secondphase)") {
                        throw std::invalid_argument("rank profile second-phase expression reference is invalid");
                    }
                    has_second_phase = true;
                } else if (property_name == "rankingExpression(secondphase).rankingScript") {
                    if (property_value.empty()) {
                        throw std::invalid_argument("rank profile second-phase ranking script is empty");
                    }
                    has_second_phase_script = true;
                } else if (property_name == "vespa.hitcollector.heapsize") {
                    if (property_value.empty() ||
                        !std::all_of(property_value.begin(), property_value.end(), [](char value) {
                            return value >= '0' && value <= '9';
                        })) {
                        throw std::invalid_argument("rank profile rerank count is invalid");
                    }
                    has_rerank_count = true;
                } else if (property_name.rfind("vespa.type.query.", 0) == 0) {
                    if (property_value.empty()) {
                        throw std::invalid_argument("rank profile query input type is empty");
                    }
                } else if (property_name == "vespa.match.feature") {
                    if (property_value.empty()) {
                        throw std::invalid_argument("rank profile match feature expression is empty");
                    }
                } else {
                    throw std::invalid_argument("unsupported rank profile property: " + property_name);
                }
            }
            if (has_first_phase_script != named_first_phase ||
                has_second_phase != has_second_phase_script) {
                throw std::invalid_argument("rank profile phase configuration is incomplete");
            }
            if (has_rerank_count && !has_second_phase) {
                throw std::invalid_argument("rank profile rerank count requires second-phase ranking");
            }
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
        constexpr std::array<std::string_view, 4> optional_config_names = {
            "imported-fields.cfg",
            "onnx-models.cfg",
            "ranking-constants.cfg",
            "ranking-expressions.cfg",
        };
        const bool needs_optional_config_placeholders = std::any_of(
            optional_config_names.begin(), optional_config_names.end(),
            [&](std::string_view name) {
                return !std::filesystem::is_regular_file(config_dir / std::string(name));
            });

        bool rewrite_indexschema = false;
        std::string indexschema_contents;
        constexpr std::string_view old_interleaved_value = "interleavedfeatures false";

        const bool has_indexed_field = std::any_of(
            manifest.fields.begin(), manifest.fields.end(),
            [](const auto &field) { return field.index; });
        if (has_indexed_field) {
            // The generated fallback BM25 profile needs interleaved features for
            // the local Vespa index implementation. Explicit rank profiles are
            // authoritative, however: changing their index settings here changes
            // the semantics of the caller's Vespa expressions and breaks direct
            // profile parity. Only normalize the compiler-generated fallback.
            const auto rank_profiles = read_config(config_dir / "rank-profiles.cfg");
            const auto profile_count = config_section_count(rank_profiles, "rankprofile");
            const auto fallback_property_count =
                profile_count >= 2
                    ? config_section_count(rank_profiles, "rankprofile[1].fef.property")
                    : 0;
            const bool generated_direct_bm25_fallback =
                profile_count == 2 &&
                config_value(rank_profiles, "rankprofile[1].name") == "bm25" &&
                fallback_property_count == 1 &&
                config_value(rank_profiles, "rankprofile[1].fef.property[0].name") ==
                    "vespa.rank.firstphase" &&
                config_value(rank_profiles, "rankprofile[1].fef.property[0].value").starts_with(
                    "bm25(");
            const bool generated_script_bm25_fallback =
                profile_count == 2 &&
                config_value(rank_profiles, "rankprofile[1].name") == "bm25" &&
                fallback_property_count == 2 &&
                config_value(rank_profiles, "rankprofile[1].fef.property[1].name") ==
                    "rankingExpression(bm25).rankingScript";
            const bool uses_only_generated_fallback =
                generated_direct_bm25_fallback || generated_script_bm25_fallback;

            if (uses_only_generated_fallback) {
                const auto indexschema = config_dir / "indexschema.cfg";
                std::ifstream input(indexschema, std::ios::binary);
                if (!input) {
                    throw std::runtime_error("could not read generated configuration: " +
                                             indexschema.string());
                }
                indexschema_contents.assign(std::istreambuf_iterator<char>(input),
                                            std::istreambuf_iterator<char>());
                if (!input.good() && !input.eof()) {
                    throw std::runtime_error("could not read generated configuration: " +
                                             indexschema.string());
                }
                rewrite_indexschema =
                    indexschema_contents.find(old_interleaved_value) != std::string::npos;
            }
        }

        // Vespa's directory config source requests these optional files even when
        // the corresponding feature is unused. Complete the private runtime view
        // here instead of requiring every Search Kernel config producer or fixture
        // to carry empty placeholder files.
        if (!needs_optional_config_placeholders && !rewrite_indexschema) {
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
                const bool is_regular_file = entry->is_regular_file(filesystem_error);
                if (filesystem_error) {
                    throw std::runtime_error(
                        "could not inspect generated configuration entry '" +
                        entry->path().string() + "': " + filesystem_error.message());
                }
                if (!is_regular_file) {
                    throw std::invalid_argument(
                        "generated configuration contains a non-file entry: " +
                        entry->path().string());
                }
                std::filesystem::copy_file(entry->path(), runtime_dir / entry->path().filename(),
                                           std::filesystem::copy_options::none, filesystem_error);
                if (filesystem_error) {
                    throw std::runtime_error("could not copy generated configuration: " +
                                             filesystem_error.message());
                }
            }

            for (const auto name : optional_config_names) {
                const auto placeholder = runtime_dir / std::string(name);
                if (std::filesystem::is_regular_file(placeholder)) {
                    continue;
                }
                std::ofstream output(placeholder, std::ios::binary | std::ios::trunc);
                if (!output) {
                    throw std::runtime_error("could not write runtime Vespa config placeholder: " +
                                             placeholder.string());
                }
                output.close();
                if (!output) {
                    throw std::runtime_error("could not write runtime Vespa config placeholder: " +
                                             placeholder.string());
                }
            }

            if (rewrite_indexschema) {
                std::string runtime_indexschema = indexschema_contents;
                size_t position = 0;
                constexpr std::string_view new_value = "interleavedfeatures true";
                while ((position = runtime_indexschema.find(old_interleaved_value, position)) !=
                       std::string::npos) {
                    runtime_indexschema.replace(position, old_interleaved_value.size(), new_value);
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
        bool legacy_manifest_shape = false;
        for (size_t index = 0; index < fields.children(); ++index) {
            const auto &field = fields[index];
            if (!is_slime_type(field, vespalib::slime::OBJECT::ID) ||
                !is_slime_type(field["name"], vespalib::slime::STRING::ID) ||
                !is_slime_type(field["type"], vespalib::slime::STRING::ID) ||
                !is_slime_type(field["attribute"], vespalib::slime::BOOL::ID) ||
                !is_slime_type(field["summary"], vespalib::slime::BOOL::ID)) {
                throw std::invalid_argument("generated schema manifest field is incomplete");
            }
            if (field["index"].valid() &&
                !is_slime_type(field["index"], vespalib::slime::BOOL::ID)) {
                throw std::invalid_argument("generated schema manifest field index is invalid");
            }
            legacy_manifest_shape |= !field["index"].valid();
            auto name = field["name"].asString().make_string();
            validate_manifest_field_name(name);
            if (std::find(field_names.begin(), field_names.end(), name) != field_names.end()) {
                throw std::invalid_argument("generated schema contains duplicate field name");
            }
            field_names.push_back(name);
            auto type = field["type"].asString().make_string();
            const bool is_struct_array = type == "array<struct>";
            if (type != "text" && type != "array<string>" && type != "array<struct>" && type != "string" &&
                type != "integer" && type != "float" && type != "boolean" &&
                type != "position" && type != "predicate" && type != "tensor") {
                throw std::invalid_argument("unsupported generated schema field type: " + type);
            }
            const bool indexed = is_slime_type(field["index"], vespalib::slime::BOOL::ID)
                                     ? field["index"].asBool()
                                     : false;
            const bool attribute = field["attribute"].asBool();
            const bool summary = field["summary"].asBool();
            uint32_t predicate_arity = 0;
            if (is_slime_type(field["predicate_arity"], vespalib::slime::LONG::ID)) {
                const auto arity = field["predicate_arity"].asLong();
                if (arity <= 0 || arity > std::numeric_limits<uint32_t>::max()) {
                    throw std::invalid_argument("predicate field arity is invalid");
                }
                predicate_arity = static_cast<uint32_t>(arity);
            }
            const auto tensor_type = is_slime_type(field["tensor_type"], vespalib::slime::STRING::ID)
                                         ? field["tensor_type"].asString().make_string()
                                         : std::string();
            std::vector<ManifestStructField> struct_fields;
            if (type == "array<struct>") {
                if (!is_slime_type(field["struct_fields"], vespalib::slime::ARRAY::ID) ||
                    field["struct_fields"].children() == 0) {
                    throw std::invalid_argument("array<struct> field has no struct fields");
                }
                for (size_t struct_index = 0; struct_index < field["struct_fields"].children(); ++struct_index) {
                    const auto &struct_field = field["struct_fields"][struct_index];
                    if (!is_slime_type(struct_field["name"], vespalib::slime::STRING::ID) ||
                        !is_slime_type(struct_field["type"], vespalib::slime::STRING::ID)) {
                        throw std::invalid_argument("array<struct> member is incomplete");
                    }
                    auto struct_name = struct_field["name"].asString().make_string();
                    auto struct_type = struct_field["type"].asString().make_string();
                    validate_manifest_field_name(struct_name);
                    if (struct_type != "string" && struct_type != "integer" &&
                        struct_type != "float" && struct_type != "boolean") {
                        throw std::invalid_argument("unsupported array<struct> member type: " + struct_type);
                    }
                    struct_fields.push_back({std::move(struct_name), std::move(struct_type)});
                }
            }
            if ((type == "tensor" && (tensor_type.empty() || !attribute)) ||
                (type == "text" && !indexed && name.find('.') == std::string::npos) ||
                (type != "text" && type != "array<string>" && type != "array<struct>" && (indexed || !attribute)) ||
                (type == "array<struct>" && indexed) ||
                ((type == "position" || type == "predicate") && summary)) {
                throw std::invalid_argument("generated schema field capabilities are invalid");
            }
            if (type == "predicate" && predicate_arity == 0) {
                throw std::invalid_argument("predicate field arity is missing");
            }
            if (type != "predicate" && predicate_arity != 0) {
                throw std::invalid_argument("predicate arity is only valid for predicate fields");
            }
            manifest.fields.push_back({std::move(name), std::move(type), std::move(tensor_type),
                                       std::move(struct_fields), indexed,
                                       is_struct_array ? false : attribute, summary, false,
                                       predicate_arity});
            if (is_struct_array) {
                const auto parent_name = manifest.fields.back().name;
                const auto parent_struct_fields = manifest.fields.back().struct_fields;
                for (const auto &member : parent_struct_fields) {
                    manifest.fields.push_back({parent_name + "." + member.name,
                                               member.type == "string" ? "text" : member.type,
                                               {}, {}, member.type == "string", true, false, true, 0});
                }
            }
        }
        if (!legacy_manifest_shape) {
            validate_generated_configuration(manifest, config_dir);
        }
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
            if (field.virtual_field) continue;
            if (field.type == "text" || field.type == "string") {
                document_type.addField(field.name, builder.stringTypeRef());
            } else if (field.type == "array<string>") {
                document_type.addField(field.name,
                                       document_type.createArray(builder.stringTypeRef()).ref());
            } else if (field.type == "array<struct>") {
                auto struct_type = document_type.createStruct(field.name + "_struct");
                for (const auto &member : field.struct_fields) {
                    if (member.type == "string") struct_type.addField(member.name, builder.stringTypeRef());
                    else if (member.type == "integer") struct_type.addField(member.name, builder.longTypeRef());
                    else if (member.type == "float") struct_type.addField(member.name, builder.doubleTypeRef());
                    else if (member.type == "boolean") struct_type.addField(member.name, builder.boolTypeRef());
                    else throw std::invalid_argument("unsupported array<struct> member type");
                }
                auto struct_ref = struct_type.ref();
                document_type.addField(field.name, document_type.createArray(struct_ref).ref());
                for (const auto &member : field.struct_fields) {
                    if (member.type == "string") {
                        document_type.addField(field.name + "__" + member.name,
                                               document_type.createArray(builder.stringTypeRef()).ref());
                    }
                }
            } else if (field.type == "integer") {
                document_type.addField(field.name, builder.longTypeRef());
            } else if (field.type == "float") {
                document_type.addField(field.name, builder.doubleTypeRef());
            } else if (field.type == "boolean") {
                document_type.addField(field.name, builder.boolTypeRef());
            } else if (field.type == "position") {
                document_type.addField(field.name, builder.positionType());
                if (field.attribute) {
                    document_type.addField(document::PositionDataType::getZCurveFieldName(field.name),
                                           builder.longTypeRef());
                }
            } else if (field.type == "predicate") {
                document_type.addField(field.name, builder.predicateTypeRef());
            } else if (field.type == "tensor") {
                document_type.addTensorField(field.name, field.tensor_type);
            } else {
                throw std::invalid_argument("unsupported generated schema field type: " + field.type);
            }
        }
        return std::make_shared<DocumenttypesConfig>(builder.config());
    }

    static const document::DataType *manifest_type(const std::string &type, bool legacy) {
        if (type == "text" || type == "string") return document::DataType::STRING;
        if (type == "integer") return legacy ? document::DataType::INT : document::DataType::LONG;
        if (type == "float") return document::DataType::DOUBLE;
        if (type == "boolean") return document::DataType::BOOL;
        if (type == "position") return &document::PositionDataType::getInstance();
        if (type == "predicate") return document::DataType::PREDICATE;
        if (type == "array<string>") {
            throw std::invalid_argument("array fields are not supported in legacy state");
        }
        if (type == "array<struct>") {
            throw std::invalid_argument("array fields are not supported in legacy state");
        }
        if (type == "tensor") {
            throw std::invalid_argument("tensor fields are not supported in legacy state");
        }
        throw std::invalid_argument("unsupported generated schema field type: " + type);
    }

    static DocumentType make_document_type(const SchemaManifest &manifest, bool legacy) {
        DocumentType type(manifest.document_type, 0);
        int32_t field_id = 1;
        for (const auto &field : manifest.fields) {
            type.addField(document::Field(field.name, field_id++, *manifest_type(field.type, legacy)));
        }
        return type;
    }

    static bool is_legacy_state(const std::string &data_dir, const std::string &config_dir,
                                bool had_existing_state) {
        // Tensor-capable schemas are emitted in the current document-type
        // representation even though the scalar integer attribute spelling
        // remains compatible with the older generated configuration. Do not
        // classify them as legacy: legacy document types cannot contain
        // tensor fields. The data directory may already exist because the
        // lifecycle code creates it before opening the backend, so this check
        // must precede the existing-state test below.
        std::ifstream manifest(std::filesystem::path(config_dir) / "searchkernel-schema.json");
        if (manifest) {
            const std::string contents((std::istreambuf_iterator<char>(manifest)),
                                       std::istreambuf_iterator<char>());
            if (contents.find("\"type\":\"tensor\"") != std::string::npos) {
                return false;
            }
        }
        if (had_existing_state &&
            !std::filesystem::exists(std::filesystem::path(data_dir) / ".searchkernel-schema")) {
            return true;
        }
        // The frozen embedded CRUD fixture predates the schema marker and
        // uses Vespa's INT32 spelling even when its data directory is new.
        // Keep the in-memory document type aligned with that native config.
        std::ifstream attributes(std::filesystem::path(config_dir) / "attributes.cfg");
        if (!attributes) return false;
        std::string line;
        while (std::getline(attributes, line)) {
            if (line.find("datatype INT32") != std::string::npos) return true;
        }
        return false;
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
        if (_legacy_state) {
            if (field.getDataType().getId() != document::DataType::T_INT) {
                throw std::invalid_argument("integer operation requires an INT32 field: " + field_name);
            }
            if (value < std::numeric_limits<int32_t>::min() ||
                value > std::numeric_limits<int32_t>::max()) {
                throw std::invalid_argument("integer value is outside the legacy INT32 range: " + field_name);
            }
            document->setFieldValue(field, std::make_unique<document::IntFieldValue>(static_cast<int32_t>(value)));
            return document;
        }
        if (field.getDataType().getId() != document::DataType::T_LONG) {
            throw std::invalid_argument("integer operation requires a LONG field: " + field_name);
        }
        reject_long_undefined_sentinel(field_name, value);
        document->setFieldValue(field, std::make_unique<document::LongFieldValue>(value));
        return document;
    }

    std::shared_ptr<document::DocumentUpdate>
    make_update(const DocumentId &id, const std::string &field_name, int64_t value) const {
        require_integer_field(field_name);
        auto update = std::make_shared<document::DocumentUpdate>(*_repo, *_doc_type, id);
        const auto &field = _doc_type->getField(field_name);
        std::unique_ptr<document::FieldValue> assigned;
        if (_legacy_state) {
            if (field.getDataType().getId() != document::DataType::T_INT) {
                throw std::invalid_argument("integer operation requires an INT32 field: " + field_name);
            }
            if (value < std::numeric_limits<int32_t>::min() ||
                value > std::numeric_limits<int32_t>::max()) {
                throw std::invalid_argument("integer value is outside the legacy INT32 range: " + field_name);
            }
            assigned = std::make_unique<document::IntFieldValue>(static_cast<int32_t>(value));
        } else {
            if (field.getDataType().getId() != document::DataType::T_LONG) {
                throw std::invalid_argument("integer operation requires a LONG field: " + field_name);
            }
            reject_long_undefined_sentinel(field_name, value);
            assigned = std::make_unique<document::LongFieldValue>(value);
        }
        document::FieldUpdate field_update(field);
        field_update.addUpdate(std::make_unique<document::AssignValueUpdate>(
            std::move(assigned)));
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
          _legacy_state(is_legacy_state(data_dir, config_dir, _data_lock.had_existing_state())),
          _document_types(make_documenttypes_config(_manifest, _legacy_state)),
          _repo(_legacy_state
                    ? std::make_shared<DocumentTypeRepo>(make_document_type(_manifest, true))
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
        _failpoints->hit(name);
#else
        (void)name;
#endif
    }

#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    std::shared_ptr<const FailpointController> failpoints() const noexcept { return _failpoints; }
    std::shared_ptr<const ResultFaultController> result_faults() const noexcept { return _result_faults; }
    void set_result_fault(uint32_t fault) { _result_faults->set(fault); }
    void clear_result_fault() noexcept { _result_faults->clear(); }
    uint64_t result_destroyed() const noexcept { return _result_destroyed->load(); }
    std::shared_ptr<std::atomic<uint64_t>> result_destroyed_counter() const noexcept {
        return _result_destroyed;
    }

    void set_failpoint(const char *name, uint32_t hit, uint32_t action) {
        _failpoints->set(name, hit, action);
    }

    void clear_failpoint(const char *name) { _failpoints->clear(name); }

    void release_failpoint(const char *name) { _failpoints->release(name); }

    bool failpoint_is_blocked(const char *name) const { return _failpoints->is_blocked(name); }

    void install_hardening_feed_view() {
        // Preserve DocumentDB::syncFeedView() topology exactly: its active view is
        // DocumentSubDBCollection::getFeedView(), which is a CombiningFeedView
        // for the normal ready/removed/not-ready sub-databases.  Hardening may
        // gate forceCommit(), but must not replace production feed routing with
        // the ready sub-DB view. getDocumentSubDBs() is exposed as const even
        // though getFeedView() only constructs a forwarding view, so remove
        // constness narrowly for that pinned-Vespa factory call.
        auto &sub_dbs = const_cast<proton::DocumentSubDBCollection &>(
            _db->getDocumentSubDBs());
        auto feed_view = sub_dbs.getFeedView();
        if (!feed_view) {
            throw std::runtime_error("Vespa DocumentDB has no production feed view");
        }
        _hardening_feed_view = std::make_shared<PublicationGateFeedView>(
            std::move(feed_view), _failpoints);

        // FeedHandler owns a non-owning active-view pointer and updates its
        // document repo from that view, so install on Vespa's master executor
        // and retain the forwarding view for the DocumentDB's full lifetime.
        vespalib::Gate gate;
        _db->getWriteService().blocking_master_execute(
            vespalib::makeLambdaTask([this, &gate]() {
                _db->getFeedHandler().setActiveFeedView(_hardening_feed_view.get());
                gate.countDown();
            }));
        gate.await();
    }
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

        // DocumentDB builds its matchers on a worker where an invalid expression
        // can escape as a fatal exception. Compile the exact same Vespa matcher
        // synchronously first so malformed administration input becomes an ABI
        // error and cannot terminate the daemon.
        const auto snapshot = config.getConfig();
        vespalib::eval::ConstantTensorLoader constant_factory(vespalib::eval::FastValueBuilderFactory::get());
        search::fef::RankingAssetsRepo ranking_assets(constant_factory,
            snapshot->getRankingConstantsSP(), snapshot->getRankingExpressionsSP(), snapshot->getOnnxModelsSP());
        for (const auto &profile : snapshot->getRankProfilesConfig().rankprofile) {
            search::fef::Properties properties;
            for (const auto &property : profile.fef.property) properties.add(property.name, property.value);
            proton::matching::Matcher preflight(*snapshot->getSchemaSP(), std::move(properties),
                _shared_service.nowRef(), _query_limiter, ranking_assets, -1);
        }

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
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
        install_hardening_feed_view();
#endif
        const auto replay_serial = _db->getFeedHandler().getSerialNum();
        if (replay_serial == std::numeric_limits<uint64_t>::max()) {
            throw std::runtime_error("native feed serial space exhausted");
        }
        // Mutation timestamps are owned by this adapter. Feed serials and
        // document timestamps are separate Vespa domains, so a replay serial
        // is not a safe lower bound for a timestamp after reopen. Use a
        // wall-clock value to move across engine lifetimes, while retaining
        // the replay serial as a useful lower bound for old test data.
        const auto now = std::chrono::duration_cast<std::chrono::microseconds>(
                             std::chrono::system_clock::now().time_since_epoch())
                             .count();
        const auto timestamp_floor = static_cast<uint64_t>(std::max<int64_t>(0, now));
        _next_timestamp = std::max(replay_serial + 1, timestamp_floor);

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
        wait_for_source_state(document_id, true, {{field, integer_value(value)}}, true);
        wait_for_search_publication();
    }

    void update(const std::string &id, const std::string &field, int64_t value) {
        std::shared_lock<std::shared_mutex> lifecycle_lock(_lifecycle_mutex);
        require_open();
        DocumentId document_id(internal_document_id(id, _document_type));
        const bool was_present = source_document_present(document_id);
        failpoint("mutation_before_native_call");
        auto result = _persistence.update(
            bucket_for(document_id), Timestamp(next_timestamp()),
            make_update(document_id, field, value));
        failpoint("mutation_after_native_call");
        if (result.hasError()) {
            throw std::runtime_error(result.getErrorMessage());
        }
        wait_for_source_state(document_id, was_present, {{field, integer_value(value)}}, false);
        wait_for_search_publication();
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
        wait_for_source_state(document_id, false, {}, false);
        wait_for_search_publication();
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
        wait_for_source_state(id, true, {{text.field, string_value(text.text)}}, true);
        wait_for_search_publication();
    }

    void update_text(const TextUpdate &text) {
        std::shared_lock<std::shared_mutex> lifecycle_lock(_lifecycle_mutex);
        require_open();
        const DocumentId id(internal_document_id(text.id, _document_type));
        const bool was_present = source_document_present(id);
        failpoint("mutation_before_native_call");
        auto result = _persistence.update(bucket_for(id), Timestamp(next_timestamp()),
                                          make_text_update(text));
        failpoint("mutation_after_native_call");
        if (result.hasError()) {
            throw std::runtime_error(result.getErrorMessage());
        }
        wait_for_source_state(id, was_present, {{text.field, string_value(text.text)}}, false);
        wait_for_search_publication();
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

    std::optional<std::string> id_for_lid_unlocked(
        const search::IDocumentMetaStore &meta_store,
        search::IDocumentMetaStore::DocId lid) const {
        const auto *provider = dynamic_cast<const search::IDocumentIdProvider *>(&meta_store);
        if (!provider) throw std::runtime_error("Vespa DocumentDB does not provide document IDs");
        const auto id = provider->get_document_id_string_view(lid);
        if (id.empty()) return std::nullopt;
        return external_document_id(id, _document_type);
    }

    std::optional<std::string> id_for_lid_unlocked(search::IDocumentMetaStore::DocId lid) const {
        auto *ready_sub_db = _db->getReadySubDB();
        if (!ready_sub_db) throw std::runtime_error("Vespa DocumentDB has no ready sub database");
        auto guard = ready_sub_db->getDocumentMetaStoreContext().getReadGuard();
        return id_for_lid_unlocked(guard->get(), lid);
    }

    std::optional<std::string> id_for_gid_unlocked(const document::GlobalId &gid) const {
        auto *ready_sub_db = _db->getReadySubDB();
        if (!ready_sub_db) throw std::runtime_error("Vespa DocumentDB has no ready sub database");
        auto guard = ready_sub_db->getDocumentMetaStoreContext().getReadGuard();
        search::IDocumentMetaStore::DocId lid = 0;
        // A concurrent remove may invalidate a hit between matching and
        // result conversion. Treat that stale hit as absent.
        if (!guard->get().getLid(gid, lid)) return std::nullopt;
        return id_for_lid_unlocked(guard->get(), lid);
    }

    std::optional<std::string> id_for_gid(const document::GlobalId &gid) const {
        // Keep the mutation lock through the copy because the provider returns
        // a string_view into mutable document metadata.
        return id_for_gid_unlocked(gid);
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

    static sk_grouping_tree_output::HitField grouping_hit_field(
        const std::string &name, const OwnedValue &value) {
        sk_grouping_tree_output::HitField result;
        result.name = name;
        result.kind = value.kind;
        result.string = value.string;
        result.integer = value.integer;
        result.floating = value.floating;
        result.boolean = value.boolean;
        return result;
    }

    void resolve_grouping_output(sk_grouping_tree_output &output) const {
        if (output.kind != SK_GROUPING_TREE_OUTPUT_HIT_LIST) return;
        std::vector<sk_grouping_tree_output::Hit> resolved;
        resolved.reserve(output.hits.size());
        std::vector<size_t> summary_hits;
        std::vector<document::GlobalId> gids;
        for (auto hit : output.hits) {
            std::optional<document::GlobalId> gid;
            if (!hit.global_id.empty()) {
                gid = document::GlobalId::parse(hit.global_id);
                hit.id = id_for_gid_unlocked(*gid).value_or("");
            } else if (hit.has_local_id) {
                hit.id = id_for_lid_unlocked(hit.local_id).value_or("");
            } else if (!hit.external_id.empty()) {
                hit.id = external_document_id(hit.external_id, _document_type);
                gid = document::DocumentId(internal_document_id(hit.id, _document_type)).getGlobalId();
            }
            if (hit.id.empty()) continue;
            if (!gid && !hit.external_id.empty()) {
                gid = document::DocumentId(internal_document_id(hit.id, _document_type)).getGlobalId();
            }
            if (gid) {
                summary_hits.push_back(resolved.size());
                gids.push_back(*gid);
            }
            resolved.push_back(std::move(hit));
        }
        output.hits = std::move(resolved);
        if (output.summary_class.empty() || gids.empty()) return;

        DocsumRequest request;
        request.resultClassName = output.summary_class;
        for (const auto &gid : gids) request.hits.emplace_back(gid);
        auto docsum = _db->getDocsums(request);
        if (!docsum || !docsum->hasResult()) {
            throw std::runtime_error("Vespa grouped docsum returned no result");
        }
        const auto &root = docsum->root();
        for (size_t index = 0; index < summary_hits.size(); ++index) {
            const auto &doc = root["docsums"][index]["docsum"];
            auto &hit = output.hits[summary_hits[index]];
            for (const auto &field : _manifest.fields) {
                if (!field.summary) continue;
                const auto &value = doc[vespalib::Memory(field.name)];
                if (value.valid()) {
                    hit.summary.push_back(
                        grouping_hit_field(field.name, summary_value(field.name, value)));
                }
            }
        }
    }

    void resolve_grouping_node(sk_grouping_tree_node &node) const {
        for (auto &output : node.outputs) resolve_grouping_output(*output);
        for (auto &group : node.groups) resolve_grouping_node(*group);
    }

    Grouping make_grouping(const sk_grouping_plan &plan) const {
        if (plan.levels.empty() && plan.root_aggregations.empty()) {
            throw std::invalid_argument("grouping plan must contain a level or root aggregation");
        }
        Grouping grouping;
        grouping.setAll(plan.all);
        grouping.setFirstLevel(0)
                .setLastLevel(static_cast<uint32_t>(plan.levels.size()));
        for (const auto &level_state : plan.levels) {
            if (!level_state || !level_state->classifier) {
                throw std::invalid_argument("grouping level classifier is missing");
            }
            auto level = GroupingLevel().setExpression(
                std::unique_ptr<search::expression::ExpressionNode>(
                    level_state->classifier->clone()));
            if (level_state->max_groups >= -1) level.setMaxGroups(level_state->max_groups);
            if (level_state->precision >= -1) level.setPresicion(level_state->precision);
            for (const auto &order : level_state->order_by) {
                if (!order.expression) throw std::invalid_argument("grouping order expression is missing");
                level.addOrderBy(std::unique_ptr<search::expression::ExpressionNode>(order.expression->clone()),
                                 order.ascending);
            }
            if (level_state->aggregations.empty()) {
                throw std::invalid_argument("grouping level must contain an aggregation");
            }
            for (const auto &aggregation_state : level_state->aggregations) {
                std::unique_ptr<AggregationResult> aggregation;
                if (aggregation_state.kind == GroupingAggregationKind::COUNT) {
                    auto count = std::make_unique<CountAggregationResult>();
                    count->setExpression(std::unique_ptr<search::expression::ExpressionNode>(
                        level_state->classifier->clone()));
                    aggregation = std::move(count);
                } else if (aggregation_state.kind == GroupingAggregationKind::AVERAGE) {
                    if (!aggregation_state.expression) {
                        throw std::invalid_argument(
                            "average grouping aggregation expression is missing");
                    }
                    auto average = std::make_unique<AverageAggregationResult>();
                    average->setExpression(std::unique_ptr<search::expression::ExpressionNode>(
                        aggregation_state.expression->clone()));
                    aggregation = std::move(average);
                } else if (aggregation_state.kind == GroupingAggregationKind::SUM) {
                    if (!aggregation_state.expression) throw std::invalid_argument("sum grouping aggregation expression is missing");
                    auto sum = std::make_unique<SumAggregationResult>();
                    sum->setExpression(std::unique_ptr<search::expression::ExpressionNode>(aggregation_state.expression->clone()));
                    aggregation = std::move(sum);
                } else if (aggregation_state.kind == GroupingAggregationKind::MIN) {
                    if (!aggregation_state.expression) throw std::invalid_argument("min grouping aggregation expression is missing");
                    auto min = std::make_unique<MinAggregationResult>();
                    min->setExpression(std::unique_ptr<search::expression::ExpressionNode>(aggregation_state.expression->clone()));
                    aggregation = std::move(min);
                } else if (aggregation_state.kind == GroupingAggregationKind::MAX) {
                    if (!aggregation_state.expression) throw std::invalid_argument("max grouping aggregation expression is missing");
                    auto max = std::make_unique<MaxAggregationResult>();
                    max->setExpression(std::unique_ptr<search::expression::ExpressionNode>(aggregation_state.expression->clone()));
                    aggregation = std::move(max);
                } else if (aggregation_state.kind == GroupingAggregationKind::STDDEV) {
                    if (!aggregation_state.expression) throw std::invalid_argument("stddev grouping aggregation expression is missing");
                    auto stddev = std::make_unique<StandardDeviationAggregationResult>();
                    stddev->setExpression(std::unique_ptr<search::expression::ExpressionNode>(aggregation_state.expression->clone()));
                    aggregation = std::move(stddev);
                } else if (aggregation_state.kind == GroupingAggregationKind::XOR) {
                    if (!aggregation_state.expression) throw std::invalid_argument("xor grouping aggregation expression is missing");
                    auto xor_result = std::make_unique<XorAggregationResult>();
                    xor_result->setExpression(std::unique_ptr<search::expression::ExpressionNode>(aggregation_state.expression->clone()));
                    aggregation = std::move(xor_result);
                } else if (aggregation_state.kind == GroupingAggregationKind::QUANTILES) {
                    if (!aggregation_state.expression || aggregation_state.quantiles.empty()) throw std::invalid_argument("quantile grouping aggregation arguments are invalid");
                    auto quantiles = std::make_unique<QuantileAggregationResult>();
                    quantiles->setExpression(std::unique_ptr<search::expression::ExpressionNode>(aggregation_state.expression->clone()));
                    quantiles->set_quantiles(aggregation_state.quantiles);
                    aggregation = std::move(quantiles);
                } else if (aggregation_state.kind == GroupingAggregationKind::SUMMARY) {
                    auto hits = std::make_unique<HitsAggregationResult>();
                    hits->setSummaryGenerator(_empty_summary_generator);
                    hits->setSummaryClass(aggregation_state.summary_class);
                    hits->setMaxHits(aggregation_state.max_hits);
                    hits->setExpression(std::make_unique<ConstantNode>(
                        std::make_unique<Int64ResultNode>(0)));
                    aggregation = std::move(hits);
                } else {
                    throw UnsupportedNativeError(
                        "grouping_aggregation_kind",
                        "unsupported grouping aggregation kind");
                }
                level.addAggregationResult(std::move(aggregation));
            }
            if (level_state->filter) {
                // GroupingLevel exposes V2 filter support in its wire format but
                // the pinned C++ API has no public setter. Round-tripping the
                // level through that exact pinned representation keeps the
                // filter inside Vespa's grouping implementation.
                using Filter = search::expression::FilterPredicateNode;
                if (!level_state->filter->predicate) {
                    throw std::invalid_argument("grouping filter predicate is missing");
                }
                auto predicate = std::unique_ptr<Filter>(
                    level_state->filter->predicate->clone());
                vespalib::IdentifiablePtr<Filter> filter(predicate.release());
                vespalib::nbostream stream;
                vespalib::NBOSerializer serializer(stream);
                serializer << static_cast<uint32_t>(CID_search_aggregation_GroupingLevelV2)
                           << level.getMaxGroups() << level.getPrecision()
                           << level.getExpression() << filter << level.getGroupPrototype();
                GroupingLevel filtered;
                vespalib::NBOSerializer deserializer(stream);
                filtered.deserializeVariant(deserializer, true);
                level = std::move(filtered);
            }
            grouping.addLevel(std::move(level));
        }
        for (const auto &aggregation_state : plan.root_aggregations) {
            std::unique_ptr<AggregationResult> aggregation;
            if (aggregation_state.kind == GroupingAggregationKind::COUNT) {
                auto count = std::make_unique<CountAggregationResult>();
                count->setExpression(std::make_unique<search::expression::ConstantNode>(
                    std::make_unique<Int64ResultNode>(1)));
                aggregation = std::move(count);
            } else if (aggregation_state.kind == GroupingAggregationKind::AVERAGE) {
                auto average = std::make_unique<AverageAggregationResult>();
                average->setExpression(std::unique_ptr<search::expression::ExpressionNode>(
                    aggregation_state.expression->clone()));
                aggregation = std::move(average);
            } else if (aggregation_state.kind == GroupingAggregationKind::SUM) {
                auto sum = std::make_unique<SumAggregationResult>();
                sum->setExpression(std::unique_ptr<search::expression::ExpressionNode>(
                    aggregation_state.expression->clone()));
                aggregation = std::move(sum);
            } else if (aggregation_state.kind == GroupingAggregationKind::MIN) {
                auto min = std::make_unique<MinAggregationResult>();
                min->setExpression(std::unique_ptr<search::expression::ExpressionNode>(
                    aggregation_state.expression->clone()));
                aggregation = std::move(min);
            } else if (aggregation_state.kind == GroupingAggregationKind::MAX) {
                auto max = std::make_unique<MaxAggregationResult>();
                max->setExpression(std::unique_ptr<search::expression::ExpressionNode>(
                    aggregation_state.expression->clone()));
                aggregation = std::move(max);
            } else if (aggregation_state.kind == GroupingAggregationKind::STDDEV) {
                auto stddev = std::make_unique<StandardDeviationAggregationResult>();
                stddev->setExpression(std::unique_ptr<search::expression::ExpressionNode>(
                    aggregation_state.expression->clone()));
                aggregation = std::move(stddev);
            } else if (aggregation_state.kind == GroupingAggregationKind::XOR) {
                auto xor_result = std::make_unique<XorAggregationResult>();
                xor_result->setExpression(std::unique_ptr<search::expression::ExpressionNode>(
                    aggregation_state.expression->clone()));
                aggregation = std::move(xor_result);
            } else if (aggregation_state.kind == GroupingAggregationKind::QUANTILES) {
                auto quantiles = std::make_unique<QuantileAggregationResult>();
                quantiles->setExpression(std::unique_ptr<search::expression::ExpressionNode>(
                    aggregation_state.expression->clone()));
                quantiles->set_quantiles(aggregation_state.quantiles);
                aggregation = std::move(quantiles);
            } else if (aggregation_state.kind == GroupingAggregationKind::SUMMARY) {
                auto hits = std::make_unique<HitsAggregationResult>();
                hits->setSummaryGenerator(_empty_summary_generator);
                hits->setSummaryClass(aggregation_state.summary_class);
                hits->setMaxHits(aggregation_state.max_hits);
                hits->setExpression(std::make_unique<ConstantNode>(
                    std::make_unique<Int64ResultNode>(0)));
                aggregation = std::move(hits);
            } else {
                throw UnsupportedNativeError("grouping_aggregation_kind",
                                             "unsupported root grouping aggregation kind");
            }
            grouping.root().addAggregationResult(std::move(aggregation));
        }
        return grouping;
    }

    Grouping make_grouping(const LocalGroupingRequestState &options) const {
        const auto &group_field = require_manifest_field(options.group_field);
        if (!group_field.attribute) {
            throw std::invalid_argument("field is not an attribute: " + options.group_field);
        }
        if (group_field.type != "string" && group_field.type != "integer" &&
            group_field.type != "boolean") {
            throw UnsupportedNativeError(
                "facet_key_type_not_supported",
                "grouping key type is not supported by the M grouping path: " + options.group_field);
        }
        auto level = GroupingLevel().setExpression(std::make_unique<AttributeNode>(group_field.name));
        auto count = std::make_unique<CountAggregationResult>();
        count->setExpression(std::make_unique<AttributeNode>(group_field.name));
        level.addAggregationResult(std::move(count));
        // Vespa's distributed grouping path may prune to maxGroups before its
        // final resort. That is observable for value-ordered facets: a low
        // value can be discarded based on relevance and never reach the
        // caller. Collect the complete set for ordered requests and apply the
        // public limit after the result has been sorted below.
        const bool ordered = options.order != SK_GROUP_ORDER_NONE;
        level.setMaxGroups(ordered || options.max_groups == std::numeric_limits<uint32_t>::max()
                               ? -1
                               : static_cast<int64_t>(options.max_groups));
        switch (options.order) {
        case SK_GROUP_ORDER_COUNT_ASC:
            level.addOrderBy(std::make_unique<search::expression::AggregationRefNode>(0), true);
            break;
        case SK_GROUP_ORDER_COUNT_DESC:
            level.addOrderBy(std::make_unique<search::expression::AggregationRefNode>(0), false);
            break;
        case SK_GROUP_ORDER_VALUE_ASC:
            level.addOrderBy(std::make_unique<AttributeNode>(group_field.name), true);
            break;
        case SK_GROUP_ORDER_VALUE_DESC:
            level.addOrderBy(std::make_unique<AttributeNode>(group_field.name), false);
            break;
        case SK_GROUP_ORDER_NONE:
            break;
        default:
            throw std::invalid_argument("unsupported grouping order");
        }
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
        return Grouping().setFirstLevel(0).setLastLevel(1).addLevel(std::move(level));
    }

    void set_grouping_spec(SearchRequest &request,
                           const std::vector<LocalGroupingRequestState> &groupings) const {
        vespalib::nbostream buffer;
        vespalib::NBOSerializer output(buffer);
        output << static_cast<uint32_t>(groupings.size());
        for (const auto &options : groupings) {
            auto grouping = make_grouping(options);
            grouping.serialize(output);
        }
        request.groupSpec.assign(buffer.data(), buffer.data() + buffer.size());
    }

    void set_grouping_spec(SearchRequest &request,
                           const LocalGroupingRequestState &grouping) const {
        set_grouping_spec(request, std::vector<LocalGroupingRequestState>{grouping});
    }

    void set_grouping_spec(SearchRequest &request,
                           const LocalSearchRequestState &options) const {
        // The pinned Vespa Matcher::match path deserializes this count and all
        // Grouping objects into one GroupingContext before matching, then
        // returns their results in one SearchReply::groupResult payload.
        vespalib::nbostream buffer;
        vespalib::NBOSerializer output(buffer);
        output << static_cast<uint32_t>(options.groupings.size() + options.grouping_plans.size());
        for (const auto &grouping : options.groupings) {
            auto grouping_spec = make_grouping(grouping);
            grouping_spec.serialize(output);
        }
        for (const auto &plan : options.grouping_plans) {
            if (!plan) throw std::invalid_argument("structured grouping plan is null");
            auto grouping_spec = make_grouping(*plan);
            grouping_spec.serialize(output);
        }
        request.groupSpec.assign(buffer.data(), buffer.data() + buffer.size());
    }

    LocalGroupingResultState decode_grouping(const Grouping &result_grouping,
                                             const LocalGroupingRequestState &options) const {
        LocalGroupingResultState result;
        const auto &root = result_grouping.getRoot();
        result.groups.reserve(root.getChildrenSize());
        for (uint32_t index = 0; index < root.getChildrenSize(); ++index) {
            const auto &group = root.getChild(index);
            LocalGroupState output_group;
            const auto &key = group.getId();
            if (key.getClass().inherits(BoolResultNode::classId)) {
                const auto &value = static_cast<const BoolResultNode &>(key);
                output_group.key.kind = SK_VALUE_BOOLEAN;
                output_group.key.boolean = value.getBool();
            } else if (key.getClass().inherits(StringResultNode::classId)) {
                HoldString value(key);
                output_group.key.kind = SK_VALUE_STRING;
                output_group.key.string = std::string(value);
            } else if (key.getClass().inherits(Int64ResultNode::classId)) {
                // Vespa uses INT64_MIN as the undefined sentinel for signed
                // integer attributes. It is not a real facet value: explicit
                // writes of that value are rejected at the boundary.
                if (key.getInteger() == std::numeric_limits<int64_t>::min() ||
                    key.getInteger() == std::numeric_limits<int32_t>::min()) {
                    continue;
                }
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
        if (options.order != SK_GROUP_ORDER_NONE) {
            const auto value_less = [](const OwnedValue &lhs, const OwnedValue &rhs) {
                if (lhs.kind != rhs.kind) return lhs.kind < rhs.kind;
                switch (lhs.kind) {
                case SK_VALUE_STRING: return lhs.string < rhs.string;
                case SK_VALUE_INTEGER: return lhs.integer < rhs.integer;
                case SK_VALUE_FLOAT: return lhs.floating < rhs.floating;
                case SK_VALUE_BOOLEAN: return lhs.boolean < rhs.boolean;
                default: return false;
                }
            };
            const auto ordered_less = [&](const LocalGroupState &lhs, const LocalGroupState &rhs) {
                switch (options.order) {
                case SK_GROUP_ORDER_COUNT_ASC:
                    return lhs.count != rhs.count ? lhs.count < rhs.count
                                                   : value_less(lhs.key, rhs.key);
                case SK_GROUP_ORDER_COUNT_DESC:
                    return lhs.count != rhs.count ? lhs.count > rhs.count
                                                   : value_less(lhs.key, rhs.key);
                case SK_GROUP_ORDER_VALUE_ASC:
                    return value_less(lhs.key, rhs.key);
                case SK_GROUP_ORDER_VALUE_DESC:
                    return value_less(rhs.key, lhs.key);
                case SK_GROUP_ORDER_NONE:
                    return false;
                default:
                    return false;
                }
            };
            std::stable_sort(result.groups.begin(), result.groups.end(), ordered_less);
            if (options.max_groups != std::numeric_limits<uint32_t>::max() &&
                result.groups.size() > options.max_groups) {
                result.groups.resize(options.max_groups);
            }
        }
        return result;
    }

    LocalSearchResultState search_local(const NativeQueryState &query,
                                        const LocalSearchRequestState &options) const {
        if (options.rank_profile.empty()) throw std::invalid_argument("rank profile must not be empty");
        validate_rank_profile(options.rank_profile.c_str());
        const auto search_start = std::chrono::steady_clock::now();
        const auto timeout = std::chrono::milliseconds(options.timeout_millis);
        SearchRequest request;
        request.setTimeout(timeout);
        request.ranking = options.rank_profile;
        request.propertiesMap.lookupCreate(search::MapNames::RANK)
            .add("vespa.matching.numthreadspersearch", "1");
        // This environment-gated hook is deliberately outside the public ABI.
        // M14 uses it to capture Vespa's own blueprint and query statistics;
        // normal callers never request or receive native execution traces.
        if (const auto level = native_trace_level()) request.trace().setLevel(*level);
        auto &rank_properties = request.propertiesMap.lookupCreate(search::MapNames::RANK);
        for (const auto &[name, value] : options.query_features) {
            std::ostringstream encoded;
            encoded.imbue(std::locale::classic());
            encoded << std::setprecision(std::numeric_limits<double>::max_digits10) << value;
            rank_properties.add(name, encoded.str());
        }
        for (const auto &[name, value] : options.query_tensors) {
            rank_properties.add("query(" + name + ")", value);
        }
        for (const auto &[name, value] : options.rank_properties) {
            rank_properties.add(name, value);
        }
        request.offset = options.offset;
        request.maxhits = options.limit;
        if (options.raw_sort_spec) {
            request.sortSpec = *options.raw_sort_spec;
        }
        for (const auto &sort : options.sort) {
            const auto &field = require_manifest_field(sort.field);
            if (!field.attribute) {
                throw std::invalid_argument("field is not an attribute: " + sort.field);
            }
            if (field.type != "string" && field.type != "integer" &&
                field.type != "float" && field.type != "boolean") {
                throw std::invalid_argument("unsupported sort field type: " + sort.field);
            }
            if (!request.sortSpec.empty()) request.sortSpec.push_back(' ');
            request.sortSpec.push_back(sort.order == SK_SORT_ASCENDING ? '+' : '-');
            if (sort.missing == SK_SORT_MISSING_FIRST || sort.missing == SK_SORT_MISSING_LAST) {
                request.sortSpec += "missing(" + sort.field + "," +
                                    (sort.missing == SK_SORT_MISSING_FIRST ? "first" : "last") + ")";
            } else if (sort.missing != SK_SORT_MISSING_DEFAULT) {
                throw std::invalid_argument("unsupported sort missing placement");
            } else {
                request.sortSpec += sort.field;
            }
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
        for (const auto &field_name : options.highlight_fields) {
            require_highlight_field(field_name);
        }
        if (!options.groupings.empty() || !options.grouping_plans.empty()) {
            set_grouping_spec(request, options);
        }
        request.setSerializedQueryTree(query.serialized_query_tree());
        failpoint("search_before_match");
        auto reply = _db->match(request, vespalib::ThreadBundle::trivial());
        failpoint("search_after_match");
        if (!reply) throw std::runtime_error("Vespa matcher returned no reply");
        write_native_trace(request.trace());
        LocalSearchResultState result;
        copy_search_reply_metadata(*reply, result);
        const auto &features = reply->match_features;
        if (features.values.size() != features.names.size() * reply->hits.size()) {
            throw std::runtime_error("native match-feature matrix shape mismatch");
        }
        result.match_feature_names = features.names;
        std::vector<size_t> live_hit_rows;
        live_hit_rows.reserve(reply->hits.size());
        result.hits.reserve(reply->hits.size());
        for (size_t row = 0; row < reply->hits.size(); ++row) {
            const auto &hit = reply->hits[row];
            // search_local_result holds the mutation lock for the complete
            // match/reply conversion, including all structured groupings.
            if (auto id = id_for_gid_unlocked(hit.gid)) {
                result.hits.push_back({std::move(*id), hit.metric, {}, {}});
                live_hit_rows.push_back(row);
            }
        }
        failpoint("search_after_id_resolution");
        // The document meta store can invalidate a hit after matching. Keep
        // all per-hit result data aligned with the compacted live-hit list;
        // never use the original reply row as the result accessor row.
        result.match_feature_values.reserve(result.match_feature_names.size() * live_hit_rows.size());
        for (const size_t row : live_hit_rows) {
            for (size_t feature = 0; feature < result.match_feature_names.size(); ++feature) {
                const auto &value = features.values[row * result.match_feature_names.size() + feature];
                result.match_feature_values.push_back({value});
            }
        }
        if ((options.summary_kind != SK_SUMMARY_NONE || !options.highlight_fields.empty()) &&
            !result.hits.empty()) {
            const auto remaining = remaining_search_timeout(
                timeout, std::chrono::steady_clock::now() - search_start);
            if (remaining <= 0ms) {
                throw DeadlineExceededNativeError("search execution deadline exceeded");
            }
            DocsumRequest docsum_request;
            docsum_request.setTimeout(remaining);
            if (options.highlight_fields.empty()) {
                docsum_request.resultClassName = options.summary_class.value_or("default");
            } else {
                docsum_request.resultClassName = "searchkernel_highlight";
            }
            docsum_request.ranking = options.rank_profile;
            // Dynamic teaser fields consume the query tree from the docsum
            // request, not from the preceding match request. Keep this
            // explicit because DocumentDB::getDocsums() does not inherit the
            // match request's serialized query tree.
            docsum_request.setSerializedQueryTree(query.serialized_query_tree());
            for (const size_t row : live_hit_rows) {
                docsum_request.hits.emplace_back(reply->hits[row].gid);
            }
            std::vector<std::string> docsum_fields;
            if (options.summary_kind == SK_SUMMARY_FIELDS) {
                docsum_fields = options.summary_fields;
            } else if (options.summary_kind == SK_SUMMARY_DEFAULT) {
                for (const auto &field : _manifest.fields) {
                    if (field.summary) docsum_fields.push_back(field.name);
                }
            }
            for (const auto &field_name : options.highlight_fields) {
                docsum_fields.push_back(highlight_summary_name(field_name));
            }
            if (!docsum_fields.empty()) {
                docsum_request.setFields(docsum_fields);
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
                for (const auto &field_name : options.highlight_fields) {
                    const auto &value = doc[vespalib::Memory(highlight_summary_name(field_name))];
                    if (!value.valid()) continue;
                    if (value.type() != vespalib::slime::STRING::instance) {
                        throw std::runtime_error("unexpected dynamic teaser summary type");
                    }
                    const auto fragment = decode_dynamic_teaser(value.asString().make_string());
                    if (!fragment) continue;
                    result.hits[index].highlights.push_back({
                        field_name,
                        std::move(*fragment),
                    });
                }
            }
            failpoint("search_after_docsums");
        }
        if (!options.groupings.empty() || !options.grouping_plans.empty()) {
            if (reply->groupResult.empty()) {
                throw std::runtime_error("native search returned no grouping result");
            }
            vespalib::nbostream result_buffer(reply->groupResult.data(), reply->groupResult.size());
            vespalib::NBOSerializer input(result_buffer);
            uint32_t result_count = 0;
            input >> result_count;
            if (result_count != options.groupings.size() + options.grouping_plans.size()) {
                throw std::runtime_error("native search returned an unexpected grouping count");
            }
            result.groupings.reserve(result_count);
            for (uint32_t index = 0; index < options.groupings.size(); ++index) {
                Grouping result_grouping;
                result_grouping.deserialize(input);
                result.groupings.push_back(decode_grouping(
                    result_grouping, options.groupings[index]));
            }
            for (size_t index = 0; index < options.grouping_plans.size(); ++index) {
                Grouping result_grouping;
                result_grouping.deserialize(input);
                auto *ready_sub_db = _db->getReadySubDB();
                if (!ready_sub_db) throw std::runtime_error("Vespa DocumentDB has no ready sub database");
                auto meta_store_guard = ready_sub_db->getDocumentMetaStoreContext().getReadGuard();
                result_grouping.convertToGlobalId(meta_store_guard->get());
                auto tree = std::make_unique<sk_grouping_tree_result>();
                tree->root = grouping_tree_node(result_grouping.getRoot(), false);
                if (tree->root) resolve_grouping_node(*tree->root);
                result.grouping_trees.push_back(std::move(tree));
            }
        }
        return result;
    }

    LocalGroupingResultState group_local(const NativeQueryState &query,
                                         const LocalGroupingRequestState &options) const {
        SearchRequest request;
        request.setTimeout(60s);
        request.ranking = "default";
        // Keep one ordinary hit so Vespa executes the grouping resort path;
        // the public grouping operation discards that hit below.
        request.maxhits = 1;
        request.setSerializedQueryTree(query.serialized_query_tree());
        set_grouping_spec(request, options);
        failpoint("group_before_match");
        auto reply = _db->match(request, vespalib::ThreadBundle::trivial());
        failpoint("group_after_match");
        if (!reply) throw std::runtime_error("Vespa matcher returned no reply");
        if (reply->groupResult.empty()) throw std::runtime_error("native grouping result is empty");
        vespalib::nbostream result_buffer(reply->groupResult.data(), reply->groupResult.size());
        vespalib::NBOSerializer input(result_buffer);
        uint32_t result_count = 0;
        input >> result_count;
        if (result_count != 1) throw std::runtime_error("unexpected native grouping result count");
        Grouping result_grouping;
        result_grouping.deserialize(input);
        return decode_grouping(result_grouping, options);
    }

    sk_grouping_tree_result group_tree(const NativeQueryState &query,
                                       const sk_grouping_plan &plan) const {
        std::shared_lock<std::shared_mutex> lifecycle_lock(_lifecycle_mutex);
        require_open();

        SearchRequest request;
        request.setTimeout(60s);
        request.ranking = "default";
        request.maxhits = 0;
        request.propertiesMap.lookupCreate(search::MapNames::RANK)
            .add("vespa.matching.numthreadspersearch", "1");
        request.setSerializedQueryTree(query.serialized_query_tree());

        const auto grouping = make_grouping(plan);
        vespalib::nbostream request_buffer;
        vespalib::NBOSerializer request_output(request_buffer);
        uint32_t grouping_count = 1;
        request_output << grouping_count;
        grouping.serialize(request_output);
        request.groupSpec.assign(request_buffer.data(),
                                 request_buffer.data() + request_buffer.size());

        failpoint("group_before_match");
        auto reply = _db->match(request, vespalib::ThreadBundle::trivial());
        failpoint("group_after_match");
        if (!reply) {
            throw std::runtime_error("Vespa matcher returned no reply");
        }
        if (!reply->hits.empty()) {
            throw std::runtime_error("general grouping returned ordinary hits");
        }
        if (reply->groupResult.empty()) {
            throw std::runtime_error("general grouping result is empty");
        }

        vespalib::nbostream result_buffer(reply->groupResult.data(),
                                          reply->groupResult.size());
        vespalib::NBOSerializer input(result_buffer);
        uint32_t result_count = 0;
        input >> result_count;
        if (result_count != 1) {
            throw std::runtime_error("unexpected general grouping result count");
        }
        Grouping result_grouping;
        result_grouping.deserialize(input);
        auto *ready_sub_db = _db->getReadySubDB();
        if (!ready_sub_db) throw std::runtime_error("Vespa DocumentDB has no ready sub database");
        auto meta_store_guard = ready_sub_db->getDocumentMetaStoreContext().getReadGuard();
        result_grouping.convertToGlobalId(meta_store_guard->get());
        sk_grouping_tree_result result;
        result.root = grouping_tree_node(result_grouping.getRoot(), false);
        if (result.root) resolve_grouping_node(*result.root);
        return result;
    }

    sk_grouping_tree_result group_tree(const NativeQueryState &query,
                                       const sk_grouping_plan &plan,
                                       const LocalSearchRequestState &options) const {
        std::shared_lock<std::shared_mutex> lifecycle_lock(_lifecycle_mutex);
        require_open();

        SearchRequest request;
        request.setTimeout(std::chrono::milliseconds(options.timeout_millis));
        validate_rank_profile(options.rank_profile.c_str());
        request.ranking = options.rank_profile;
        request.maxhits = 0;
        auto &rank_properties = request.propertiesMap.lookupCreate(search::MapNames::RANK);
        rank_properties.add("vespa.matching.numthreadspersearch", "1");
        for (const auto &[name, value] : options.query_features) {
            std::ostringstream encoded;
            encoded.imbue(std::locale::classic());
            encoded << std::setprecision(std::numeric_limits<double>::max_digits10) << value;
            rank_properties.add(name, encoded.str());
        }
        for (const auto &[name, value] : options.query_tensors) {
            rank_properties.add("query(" + name + ")", value);
        }
        for (const auto &[name, value] : options.rank_properties) {
            rank_properties.add(name, value);
        }
        request.setSerializedQueryTree(query.serialized_query_tree());

        const auto grouping = make_grouping(plan);
        vespalib::nbostream request_buffer;
        vespalib::NBOSerializer request_output(request_buffer);
        uint32_t grouping_count = 1;
        request_output << grouping_count;
        grouping.serialize(request_output);
        request.groupSpec.assign(request_buffer.data(),
                                 request_buffer.data() + request_buffer.size());

        failpoint("group_before_match");
        auto reply = _db->match(request, vespalib::ThreadBundle::trivial());
        failpoint("group_after_match");
        if (!reply) {
            throw std::runtime_error("Vespa matcher returned no reply");
        }
        if (!reply->hits.empty()) {
            throw std::runtime_error("general grouping returned ordinary hits");
        }
        if (reply->groupResult.empty()) {
            throw std::runtime_error("general grouping result is empty");
        }

        vespalib::nbostream result_buffer(reply->groupResult.data(),
                                          reply->groupResult.size());
        vespalib::NBOSerializer input(result_buffer);
        uint32_t result_count = 0;
        input >> result_count;
        if (result_count != 1) {
            throw std::runtime_error("unexpected general grouping result count");
        }
        Grouping result_grouping;
        result_grouping.deserialize(input);
        auto *ready_sub_db = _db->getReadySubDB();
        if (!ready_sub_db) throw std::runtime_error("Vespa DocumentDB has no ready sub database");
        auto meta_store_guard = ready_sub_db->getDocumentMetaStoreContext().getReadGuard();
        result_grouping.convertToGlobalId(meta_store_guard->get());
        sk_grouping_tree_result result;
        result.root = grouping_tree_node(result_grouping.getRoot(), false);
        if (result.root) resolve_grouping_node(*result.root);
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
        failpoint("attribute_mutation_before_native_call");
        auto result = _persistence.put(bucket, Timestamp(next_timestamp()), make_attribute_document(document));
        failpoint("attribute_mutation_after_native_call");
        if (result.hasError()) throw std::runtime_error(result.getErrorMessage());
        wait_for_source_state(id, true, expected_fields(document), true);
        wait_for_search_publication();
    }

    void update_attributes(const AttributeUpdateState &update) {
        std::shared_lock<std::shared_mutex> lifecycle_lock(_lifecycle_mutex);
        require_open();
        const DocumentId id(internal_document_id(update.id, _document_type));
        const bool was_present = source_document_present(id);
        failpoint("attribute_mutation_before_native_call");
        auto result = _persistence.update(bucket_for(id), Timestamp(next_timestamp()), make_attribute_update(update));
        failpoint("attribute_mutation_after_native_call");
        if (result.hasError()) throw std::runtime_error(result.getErrorMessage());
        wait_for_source_state(id, was_present, expected_fields(update), false);
        wait_for_search_publication();
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
        failpoint("mutation_before_native_call");
        auto result = _persistence.put(bucket, Timestamp(next_timestamp()),
                                       make_value_document(document));
        failpoint("mutation_after_native_call");
        if (result.hasError()) throw std::runtime_error(result.getErrorMessage());
        // Hardening can hold the completed feed at the exact boundary the
        // visibility barrier is required to close. This makes the regression
        // deterministic instead of relying on Vespa publication timing.
        failpoint("mutation_before_visibility_wait");
        const bool has_struct_array = std::any_of(
            document.fields.begin(), document.fields.end(),
            [](const auto &field) { return field.value.kind == SK_VALUE_STRUCT_ARRAY; });
        const bool has_position = std::any_of(
            document.fields.begin(), document.fields.end(),
            [](const auto &field) { return field.value.kind == SK_VALUE_POSITION; });
        // This source-state wait closes the document-store/retriever window.
        // Search publication is fenced separately below through Vespa's
        // force-commit path. TensorFieldValue presence remains a valid source
        // visibility signal; it is not treated as proof of matcher publication.
        if (has_struct_array || has_position) {
            wait_for_source_state(id, true, {}, true);
        } else {
            wait_for_source_state(id, true, expected_fields(document), true);
        }
        wait_for_search_publication();
    }

    void update_document(const ValueUpdate &update) {
        std::shared_lock<std::shared_mutex> lifecycle_lock(_lifecycle_mutex);
        require_open();
        const DocumentId id(internal_document_id(update.id, _document_type));
        const bool was_present = source_document_present(id);
        auto native_update = make_value_update(update);

        if (update_requires_document_store_sync(update)) {
            auto document = read_document_for_rewrite(id);
            if (document || update.create_if_nonexistent) {
                if (!document) {
                    document = std::make_unique<Document>(*_repo, *_doc_type, id);
                }
                native_update->applyTo(*document);
                const Bucket bucket = bucket_for(id);
                {
                    std::lock_guard<std::mutex> bucket_lock(_bucket_mutex);
                    prepare_bucket(bucket);
                }
                failpoint("mutation_before_native_call");
                failpoint("attribute_mutation_before_native_call");
                auto result = _persistence.put(
                    bucket, Timestamp(next_timestamp()),
                    std::shared_ptr<Document>(std::move(document)));
                failpoint("attribute_mutation_after_native_call");
                failpoint("mutation_after_native_call");
                if (result.hasError()) throw std::runtime_error(result.getErrorMessage());
                wait_for_source_state(id, true, update.fields, false);
                wait_for_search_publication();
                return;
            }
        }

        failpoint("mutation_before_native_call");
        failpoint("attribute_mutation_before_native_call");
        auto result = _persistence.update(bucket_for(id), Timestamp(next_timestamp()),
                                          std::move(native_update));
        failpoint("attribute_mutation_after_native_call");
        failpoint("mutation_after_native_call");
        if (result.hasError()) throw std::runtime_error(result.getErrorMessage());
        wait_for_source_state(id, was_present || update.create_if_nonexistent, update.fields, false);
        wait_for_search_publication();
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
            metadata.push_back({field.name, field.type, field.index, field.attribute,
                                field.virtual_field});
        }
        return metadata;
    }
};
