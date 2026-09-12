#include "searchkernel_vespa.h"

#include <vespa/config-bucketspaces.h>
#include <vespa/config-stor-distribution.h>
#include <vespa/config/subscription/sourcespec.h>
#include <vespa/document/config/documenttypes_config_fwd.h>
#include <vespa/document/datatype/datatype.h>
#include <vespa/document/datatype/documenttype.h>
#include <vespa/document/fieldvalue/document.h>
#include <vespa/document/fieldvalue/intfieldvalue.h>
#include <vespa/document/repo/documenttyperepo.h>
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
#include <vespa/vespalib/util/host_name.h>
#include <vespa/vespalib/util/invokeserviceimpl.h>
#include <vespa/vespalib/util/sequencedtaskexecutor.h>
#include <vespa/vespalib/util/shared_operation_throttler.h>
#include <vespa/persistence/dummyimpl/dummy_bucket_executor.h>

#include <algorithm>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <memory>
#include <stdexcept>
#include <string>
#include <string_view>
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

constexpr int TLS_PORT = 9014;
constexpr const char *DOC_TYPE = "typea";

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
    TransLogServer _tls;
    proton::matching::QueryLimiter _query_limiter;
    DocumentType _doc_type;
    std::shared_ptr<DocumentTypeRepo> _repo;
    DocumentDB::SP _db;
    PersistenceOwner _persistence_owner;
    AcceptAllWrites _write_filter;
    EmbeddedResourceUsageNotifier _resource_usage_notifier;
    PersistenceEngine _persistence;
    std::shared_ptr<PersistenceHandlerProxy> _handler;
    std::vector<uint64_t> _prepared_buckets;
    uint64_t _next_timestamp = 1;

    static DocumentType make_document_type() {
        DocumentType type(DOC_TYPE, 0);
        type.addField(document::Field("attr1", 1, *document::DataType::INT));
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

    std::shared_ptr<Document> make_document(const DocumentId &id, int32_t value) const {
        auto document = std::make_shared<Document>(*_repo, _doc_type, id);
        const auto &field = _doc_type.getField("attr1");
        document->setFieldValue(field, std::make_unique<document::IntFieldValue>(value));
        return document;
    }

    std::shared_ptr<document::DocumentUpdate>
    make_update(const DocumentId &id, int32_t value) const {
        auto update = std::make_shared<document::DocumentUpdate>(*_repo, _doc_type, id);
        const auto &field = _doc_type.getField("attr1");
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
          _tls(_shared_service.transport(), _tls_dir,
               TLS_PORT, data_dir, _file_header_context),
          _query_limiter(),
          _doc_type(make_document_type()),
          _repo(std::make_shared<DocumentTypeRepo>(_doc_type)),
          _db(),
          _persistence_owner(),
          _write_filter(),
          _resource_usage_notifier(),
          _persistence(_persistence_owner, _write_filter,
                       _resource_usage_notifier, -1, false),
          _handler(),
          _prepared_buckets() {
        std::filesystem::create_directories(data_dir);
        std::filesystem::create_directories(data_dir + "/" + DOC_TYPE);

        config::DirSpec spec(config_dir);
        DocumentDBConfigHelper config(spec, DOC_TYPE);
        auto document_types = std::make_shared<DocumenttypesConfig>();
        auto proton_config = std::make_shared<ProtonConfigBuilder>();
        proton_config->documentdb.emplace_back();
        proton_config->documentdb.back().inputdoctypename = DOC_TYPE;
        proton_config->documentdb.back().documentIdAttribute = true;
        auto file_distribution = std::make_shared<FiledistributorrpcConfig>();
        auto bucket_spaces = std::make_shared<BucketspacesConfig>();
        auto tune = std::make_shared<search::TuneFileDocumentDB>();
        auto bootstrap = std::make_shared<proton::BootstrapConfig>(
            1, document_types, _repo, proton_config, file_distribution,
            bucket_spaces, tune, _hw_info);
        config.forwardConfig(bootstrap);
        config.nextGeneration(_shared_service.transport(), 0ms);

        auto config_store = std::make_unique<proton::FileConfigManager>(
            _shared_service.transport(), data_dir + "/" + DOC_TYPE + "/config", "", DOC_TYPE);
        config_store->setProtonConfig(proton_config);

        _db = DocumentDB::create(
            data_dir, config.getConfig(),
            vespalib::SocketSpec::from_host_port("localhost", TLS_PORT).spec(),
            _query_limiter, DocTypeName(DOC_TYPE), bucket_space(),
            *proton_config, _db_owner, _shared_service, _tls, _wire_service,
            _file_header_context,
            std::make_shared<search::attribute::Interlock>(),
            std::move(config_store),
            std::make_shared<vespalib::ThreadStackExecutor>(16),
            _hw_info, std::shared_ptr<search::diskindex::IPostingListCache>());
        _db->start();
        _db->waitForOnlineState();

        _handler = std::make_shared<PersistenceHandlerProxy>(_db);
        _persistence.putHandler(_persistence.getWLock(), bucket_space(),
                                DocTypeName(DOC_TYPE), _handler);
        auto initialized = _persistence.initialize();
        if (initialized.hasError()) {
            throw std::runtime_error(initialized.getErrorMessage());
        }
        auto state = single_node_cluster_state();
        auto state_result = _persistence.setClusterState(bucket_space(), state);
        if (state_result.hasError()) {
            throw std::runtime_error(state_result.getErrorMessage());
        }
    }

    ~EmbeddedEngine() noexcept {
        try {
            _persistence.removeHandler(_persistence.getWLock(), bucket_space(),
                                      DocTypeName(DOC_TYPE));
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

    void put(const std::string &id, int32_t value) {
        DocumentId document_id(id);
        Bucket bucket = bucket_for(document_id);
        prepare_bucket(bucket);
        auto result = _persistence.put(bucket, Timestamp(_next_timestamp++),
                                       make_document(document_id, value));
        if (result.hasError()) {
            throw std::runtime_error(result.getErrorMessage());
        }
    }

    void update(const std::string &id, int32_t value) {
        DocumentId document_id(id);
        auto result = _persistence.update(
            bucket_for(document_id), Timestamp(_next_timestamp++),
            make_update(document_id, value));
        if (result.hasError()) {
            throw std::runtime_error(result.getErrorMessage());
        }
    }

    bool remove(const std::string &id) {
        DocumentId document_id(id);
        auto result = _persistence.remove(
            bucket_for(document_id), Timestamp(_next_timestamp++),
            document_id);
        if (result.hasError()) {
            throw std::runtime_error(result.getErrorMessage());
        }
        return result.wasFound();
    }

    std::vector<std::string> search(const std::string &field, int32_t value,
                                    uint32_t limit) const {
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
                    ids.emplace_back(document_id);
                }
            }
        }
        return ids;
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

void clear_error(sk_error *error) {
    if (error) {
        sk_error_destroy(error);
    }
}

void set_error(sk_error *error, int32_t code, const std::string &message) {
    if (!error) {
        return;
    }
    sk_error_destroy(error);
    error->code = code;
    error->message = static_cast<char *>(std::malloc(message.size() + 1));
    if (error->message) {
        std::memcpy(error->message, message.c_str(), message.size() + 1);
    } else {
        error->code = 3;
    }
}

template <typename Function>
int32_t invoke(sk_error *error, Function &&function) noexcept {
    clear_error(error);
    try {
        function();
        return 0;
    } catch (const std::exception &exception) {
        set_error(error, 2, exception.what());
        return 2;
    } catch (...) {
        set_error(error, 2, "unknown native exception");
        return 2;
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

extern "C" uint32_t sk_native_abi_version(void) { return 1; }

extern "C" int32_t sk_engine_open(const char *data_dir, const char *config_dir,
                                   sk_engine **out_engine, sk_error *error) {
    if (!out_engine || !data_dir || !config_dir) {
        set_error(error, 1, "engine open arguments must not be null");
        return 1;
    }
    *out_engine = nullptr;
    return invoke(error, [&] {
        auto engine = std::make_unique<sk_engine>();
        engine->impl = std::make_unique<EmbeddedEngine>(data_dir, config_dir);
        *out_engine = engine.release();
    });
}

extern "C" void sk_engine_destroy(sk_engine *engine) {
    try {
        delete engine;
    } catch (...) {
    }
}

extern "C" void sk_error_destroy(sk_error *error) {
    if (error) {
        std::free(error->message);
        error->message = nullptr;
        error->code = 0;
    }
}

extern "C" int32_t sk_document_create(const char *id, const char *field,
                                        int32_t value, sk_document **out,
                                        sk_error *error) {
    if (!out || !id || !field) {
        set_error(error, 1, "document arguments must not be null");
        return 1;
    }
    *out = nullptr;
    return invoke(error, [&] {
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

extern "C" void sk_document_destroy(sk_document *document) { delete document; }

extern "C" int32_t sk_document_update_create(const char *id, const char *field,
                                               int32_t value,
                                               sk_document_update **out,
                                               sk_error *error) {
    if (!out || !id || !field) {
        set_error(error, 1, "update arguments must not be null");
        return 1;
    }
    *out = nullptr;
    return invoke(error, [&] {
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

extern "C" void sk_document_update_destroy(sk_document_update *update) {
    delete update;
}

extern "C" int32_t sk_search_request_create(const char *field, int32_t value,
                                              uint32_t limit,
                                              sk_search_request **out,
                                              sk_error *error) {
    if (!out || !field) {
        set_error(error, 1, "search arguments must not be null");
        return 1;
    }
    *out = nullptr;
    return invoke(error, [&] {
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

extern "C" void sk_search_request_destroy(sk_search_request *request) {
    delete request;
}

extern "C" int32_t sk_engine_put(sk_engine *engine, const sk_document *document,
                                  sk_error *error) {
    if (!engine || !engine->impl || !document) {
        set_error(error, 1, "put arguments are invalid");
        return 1;
    }
    return invoke(error, [&] {
        if (document->field != "attr1") {
            throw std::invalid_argument("the configured document type has no such field");
        }
        engine->impl->put(document->id, document->value);
    });
}

extern "C" int32_t sk_engine_update(sk_engine *engine,
                                     const sk_document_update *update,
                                     sk_error *error) {
    if (!engine || !engine->impl || !update) {
        set_error(error, 1, "update arguments are invalid");
        return 1;
    }
    return invoke(error, [&] {
        if (update->field != "attr1") {
            throw std::invalid_argument("the configured document type has no such field");
        }
        engine->impl->update(update->id, update->value);
    });
}

extern "C" int32_t sk_engine_remove(sk_engine *engine, const char *id,
                                     int32_t *out_was_found, sk_error *error) {
    if (!engine || !engine->impl || !id || !out_was_found) {
        set_error(error, 1, "remove arguments are invalid");
        return 1;
    }
    return invoke(error, [&] { *out_was_found = engine->impl->remove(id) ? 1 : 0; });
}

extern "C" int32_t sk_engine_search(const sk_engine *engine,
                                     const sk_search_request *request,
                                     sk_search_result **out_result,
                                     sk_error *error) {
    if (!engine || !engine->impl || !request || !out_result) {
        set_error(error, 1, "search arguments are invalid");
        return 1;
    }
    *out_result = nullptr;
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

extern "C" uint32_t sk_search_result_size(const sk_search_result *result) {
    return result ? static_cast<uint32_t>(result->ids.size()) : 0;
}

extern "C" int32_t sk_search_result_id(const sk_search_result *result,
                                        uint32_t index, const char **out_id,
                                        uint32_t *out_length, sk_error *error) {
    if (!result || !out_id || !out_length) {
        set_error(error, 1, "result arguments are invalid");
        return 1;
    }
    if (index >= result->ids.size()) {
        set_error(error, 1, "result index is out of bounds");
        return 1;
    }
    clear_error(error);
    *out_id = result->ids[index].data();
    *out_length = static_cast<uint32_t>(result->ids[index].size());
    return 0;
}

extern "C" void sk_search_result_destroy(sk_search_result *result) {
    delete result;
}
