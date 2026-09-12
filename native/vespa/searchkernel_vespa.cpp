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
#include <vespa/vespalib/util/host_name.h>
#include <vespa/vespalib/util/invokeserviceimpl.h>
#include <vespa/vespalib/util/sequencedtaskexecutor.h>
#include <vespa/vespalib/util/shared_operation_throttler.h>
#include <vespa/persistence/dummyimpl/dummy_bucket_executor.h>

#include <algorithm>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <limits>
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

    uint64_t next_timestamp() {
        if (_next_timestamp == std::numeric_limits<uint64_t>::max()) {
            throw std::runtime_error("mutation timestamp space exhausted");
        }
        return _next_timestamp++;
    }

    static DocumentType make_document_type() {
        DocumentType type(DOC_TYPE, 0);
        type.addField(document::Field("attr1", 1, *document::DataType::INT));
        type.addField(document::Field("title", 2, *document::DataType::STRING));
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

        proton::ConfigStore::UP config_store;
        if (std::filesystem::exists(std::filesystem::path(config_dir) / "proton.cfg")) {
            auto file_config_store = std::make_unique<proton::FileConfigManager>(
                _shared_service.transport(), data_dir + "/" + DOC_TYPE + "/config", "", DOC_TYPE);
            file_config_store->setProtonConfig(proton_config);
            config_store = std::move(file_config_store);
        } else {
            config_store = std::make_unique<proton::MemoryConfigStore>();
        }

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
        auto result = _persistence.put(bucket, Timestamp(next_timestamp()),
                                       make_document(document_id, value));
        if (result.hasError()) {
            throw std::runtime_error(result.getErrorMessage());
        }
    }

    void update(const std::string &id, int32_t value) {
        DocumentId document_id(id);
        auto result = _persistence.update(
            bucket_for(document_id), Timestamp(next_timestamp()),
            make_update(document_id, value));
        if (result.hasError()) {
            throw std::runtime_error(result.getErrorMessage());
        }
    }

    bool remove(const std::string &id) {
        DocumentId document_id(id);
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

    std::unique_ptr<document::StringFieldValue>
    make_text_value(const TextDocument &document) const {
        const document::FixedTypeRepo fixed_repo(*_repo, _doc_type);
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
        const auto &field = _doc_type.getField(text.field);
        if (field.getDataType().getId() != document::DataType::T_STRING) {
            throw std::invalid_argument("text field is not a string field");
        }
        auto document = std::make_shared<Document>(*_repo, _doc_type, DocumentId(text.id));
        document->setFieldValue(field, make_text_value(text));
        return document;
    }

    std::shared_ptr<document::DocumentUpdate> make_text_update(const TextUpdate &text) const {
        const auto &field = _doc_type.getField(text.field);
        if (field.getDataType().getId() != document::DataType::T_STRING) {
            throw std::invalid_argument("text field is not a string field");
        }
        auto update = std::make_shared<document::DocumentUpdate>(
            *_repo, _doc_type, DocumentId(text.id));
        document::FieldUpdate field_update(field);
        field_update.addUpdate(std::make_unique<document::AssignValueUpdate>(
            make_text_value(text)));
        update->addUpdate(std::move(field_update));
        return update;
    }

    void put_text(const TextDocument &text) {
        const DocumentId id(text.id);
        const Bucket bucket = bucket_for(id);
        prepare_bucket(bucket);
        auto result = _persistence.put(bucket, Timestamp(next_timestamp()),
                                       make_text_document(text));
        if (result.hasError()) {
            throw std::runtime_error(result.getErrorMessage());
        }
    }

    void update_text(const TextUpdate &text) {
        const DocumentId id(text.id);
        auto result = _persistence.update(bucket_for(id), Timestamp(next_timestamp()),
                                          make_text_update(text));
        if (result.hasError()) {
            throw std::runtime_error(result.getErrorMessage());
        }
    }

    std::vector<TextHit> search_text(const TextRequest &text) const {
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
                    hits.push_back({std::string(id), hit.metric});
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

std::filesystem::path resolve_config_directory(const std::filesystem::path &path) {
    if (std::filesystem::is_regular_file(path / "indexschema.cfg")) {
        return path;
    }
    const auto nested_config = path / "embedded_text_cfg";
    if (std::filesystem::is_regular_file(nested_config / "indexschema.cfg")) {
        return nested_config;
    }
    for (const auto &entry : std::filesystem::directory_iterator(path)) {
        if (entry.is_directory() &&
            std::filesystem::is_regular_file(entry.path() / "indexschema.cfg")) {
            return entry.path();
        }
    }
    return path;
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
struct sk_text_document : TextDocument {};
struct sk_text_update : TextUpdate {};
struct sk_text_request : TextRequest {};
struct sk_text_result : TextResult {};

extern "C" uint32_t sk_native_abi_version(void) { return 1; }

extern "C" int32_t sk_engine_open(const char *data_dir, const char *config_dir,
                                   sk_engine **out_engine, sk_error *error) {
    if (!out_engine || !data_dir || !config_dir) {
        set_error(error, 1, "engine open arguments must not be null");
        return 1;
    }
    *out_engine = nullptr;
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

namespace {

std::string copy_text_bytes(const uint8_t *bytes, uint32_t length,
                            const char *what) {
    if (length != 0 && bytes == nullptr) {
        throw std::invalid_argument(std::string(what) + " must not be null");
    }
    return std::string(reinterpret_cast<const char *>(bytes), length);
}

void fill_text_document(TextDocument &document, const char *id, const char *field,
                        const uint8_t *text, uint32_t text_length,
                        const sk_text_occurrence *occurrences,
                        uint32_t occurrence_count) {
    if (*id == '\0' || *field == '\0') {
        throw std::invalid_argument("text document ID and field must not be empty");
    }
    if (occurrence_count != 0 && occurrences == nullptr) {
        throw std::invalid_argument("text occurrences must not be null");
    }
    document.id = id;
    document.field = field;
    document.text = copy_text_bytes(text, text_length, "text");
    document.occurrences.reserve(occurrence_count);
    for (uint32_t index = 0; index < occurrence_count; ++index) {
        const auto &input = occurrences[index];
        if (input.term_length == 0) {
            throw std::invalid_argument("text occurrence term must not be empty");
        }
        if (input.position != index) {
            throw std::invalid_argument("text occurrence positions must be contiguous");
        }
        document.occurrences.push_back({
            copy_text_bytes(input.term, input.term_length, "text occurrence term"),
            input.byte_start,
            input.byte_length,
            input.position
        });
    }
}

void fill_text_request(TextRequest &request, uint32_t kind, const char *field,
                       const sk_text_query_term *terms, uint32_t term_count,
                       const char *rank_profile) {
    if (*field == '\0' || *rank_profile == '\0') {
        throw std::invalid_argument("text query field and rank profile must not be empty");
    }
    if (kind != 1 && kind != 2) {
        throw std::invalid_argument("unsupported text query kind");
    }
    if ((kind == 1 && term_count != 1) || (kind == 2 && term_count == 0)) {
        throw std::invalid_argument("invalid number of terms for text query kind");
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
                                                 "text query term"));
        request.positions.push_back(input.position);
    }
}

} // namespace

extern "C" int32_t sk_text_document_create(
    const char *id, const char *field, const uint8_t *text, uint32_t text_length,
    const sk_text_occurrence *occurrences, uint32_t occurrence_count,
    sk_text_document **out, sk_error *error) {
    if (!out || !id || !field) {
        set_error(error, 1, "text document arguments must not be null");
        return 1;
    }
    *out = nullptr;
    return invoke(error, [&] {
        auto document = std::make_unique<sk_text_document>();
        fill_text_document(*document, id, field, text, text_length,
                           occurrences, occurrence_count);
        *out = document.release();
    });
}

extern "C" void sk_text_document_destroy(sk_text_document *document) {
    delete document;
}

extern "C" int32_t sk_text_update_create(
    const char *id, const char *field, const uint8_t *text, uint32_t text_length,
    const sk_text_occurrence *occurrences, uint32_t occurrence_count,
    sk_text_update **out, sk_error *error) {
    if (!out || !id || !field) {
        set_error(error, 1, "text update arguments must not be null");
        return 1;
    }
    *out = nullptr;
    return invoke(error, [&] {
        auto update = std::make_unique<sk_text_update>();
        fill_text_document(*update, id, field, text, text_length,
                           occurrences, occurrence_count);
        *out = update.release();
    });
}

extern "C" void sk_text_update_destroy(sk_text_update *update) {
    delete update;
}

extern "C" int32_t sk_text_request_create(
    uint32_t kind, const char *field, const sk_text_query_term *terms,
    uint32_t term_count, const char *rank_profile, sk_text_request **out,
    sk_error *error) {
    if (!out || !field || !rank_profile) {
        set_error(error, 1, "text request arguments must not be null");
        return 1;
    }
    *out = nullptr;
    return invoke(error, [&] {
        auto request = std::make_unique<sk_text_request>();
        fill_text_request(*request, kind, field, terms, term_count, rank_profile);
        *out = request.release();
    });
}

extern "C" void sk_text_request_destroy(sk_text_request *request) {
    delete request;
}

extern "C" int32_t sk_engine_put_text(sk_engine *engine,
                                        const sk_text_document *document,
                                        sk_error *error) {
    if (!engine || !engine->impl || !document) {
        set_error(error, 1, "text put arguments are invalid");
        return 1;
    }
    return invoke(error, [&] { engine->impl->put_text(*document); });
}

extern "C" int32_t sk_engine_update_text(sk_engine *engine,
                                           const sk_text_update *update,
                                           sk_error *error) {
    if (!engine || !engine->impl || !update) {
        set_error(error, 1, "text update arguments are invalid");
        return 1;
    }
    return invoke(error, [&] { engine->impl->update_text(*update); });
}

extern "C" int32_t sk_engine_search_text(const sk_engine *engine,
                                           const sk_text_request *request,
                                           sk_text_result **out_result,
                                           sk_error *error) {
    if (!engine || !engine->impl || !request || !out_result) {
        set_error(error, 1, "text search arguments are invalid");
        return 1;
    }
    *out_result = nullptr;
    return invoke(error, [&] {
        auto result = std::make_unique<sk_text_result>();
        result->hits = engine->impl->search_text(*request);
        *out_result = result.release();
    });
}

extern "C" uint32_t sk_text_result_size(const sk_text_result *result) {
    return result ? static_cast<uint32_t>(result->hits.size()) : 0;
}

extern "C" int32_t sk_text_result_hit(const sk_text_result *result,
                                       uint32_t index, const char **out_id,
                                       uint32_t *out_length, double *out_score,
                                       sk_error *error) {
    if (!result || !out_id || !out_length || !out_score) {
        set_error(error, 1, "text result arguments are invalid");
        return 1;
    }
    if (index >= result->hits.size()) {
        set_error(error, 1, "text result index is out of bounds");
        return 1;
    }
    clear_error(error);
    *out_id = result->hits[index].id.data();
    *out_length = static_cast<uint32_t>(result->hits[index].id.size());
    *out_score = result->hits[index].score;
    return 0;
}

extern "C" void sk_text_result_destroy(sk_text_result *result) {
    delete result;
}
