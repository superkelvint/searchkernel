// Copyright Vespa.ai. Licensed under the terms of the Apache 2.0 license.
//
// Phase-0 embedded Vespa spike.
//
// Deliberately exercises a real Proton DocumentDB + PersistenceEngine + Matcher
// entirely in-process. The searchable field is an INT32 attribute so this test
// does not depend on Vespa's Java linguistic/indexing pipeline. A follow-up
// spike should add a tokenized text index once this lifecycle test passes.

#include <vespa/config-bucketspaces.h>
#include <vespa/config-stor-distribution.h>
#include <vespa/config/subscription/sourcespec.h>
#include <vespa/document/config/documenttypes_config_fwd.h>
#include <vespa/document/datatype/datatype.h>
#include <vespa/document/datatype/documenttype.h>
#include <vespa/document/fieldvalue/document.h>
#include <vespa/document/fieldvalue/intfieldvalue.h>
#include <vespa/document/repo/documenttyperepo.h>
#include <vespa/document/test/make_bucket_space.h>
#include <vespa/document/update/assignvalueupdate.h>
#include <vespa/document/update/documentupdate.h>
#include <vespa/document/update/fieldupdate.h>
#include <vespa/searchcore/proton/matching/querylimiter.h>
#include <vespa/searchcore/proton/matching/querynodes.h>
#include <vespa/searchcore/proton/metrics/dummy_wire_service.h>
#include <vespa/searchcore/proton/persistenceengine/i_resource_write_filter.h>
#include <vespa/searchcore/proton/persistenceengine/ipersistenceengineowner.h>
#include <vespa/searchcore/proton/persistenceengine/persistenceengine.h>
#include <vespa/searchcore/proton/server/bootstrapconfig.h>
#include <vespa/searchcore/proton/server/documentdb.h>
#include <vespa/searchcore/proton/server/documentdbconfigmanager.h>
#include <vespa/searchcore/proton/server/memoryconfigstore.h>
#include <vespa/searchcore/proton/server/persistencehandlerproxy.h>
#include <vespa/searchcore/proton/test/bucketfactory.h>
#include <vespa/searchcore/proton/test/dummydbowner.h>
#include <vespa/searchcore/proton/test/mock_shared_threading_service.h>
#include <vespa/searchcore/proton/test/port_numbers.h>
#include <vespa/searchcore/proton/test/resource_usage_notifier.h>
#include <vespa/searchlib/attribute/interlock.h>
#include <vespa/searchlib/engine/searchreply.h>
#include <vespa/searchlib/engine/searchrequest.h>
#include <vespa/searchlib/index/dummyfileheadercontext.h>
#include <vespa/searchlib/query/tree/querybuilder.h>
#include <vespa/searchlib/query/tree/stackdumpcreator.h>
#include <vespa/searchlib/transactionlog/translogserver.h>
#include <vespa/vdslib/distribution/distribution.h>
#include <vespa/vdslib/state/clusterstate.h>
#include <vespa/vespalib/gtest/gtest.h>
#include <vespa/vespalib/net/socket_spec.h>
#include <vespa/vespalib/test/test_path.h>
#include <vespa/vespalib/util/hw_info.h>
#include <vespa/vespalib/util/threadstackexecutor.h>

#include <algorithm>
#include <chrono>
#include <filesystem>
#include <memory>
#include <string>
#include <unordered_set>

using namespace std::chrono_literals;
using document::Document;
using document::DocumentId;
using document::DocumentType;
using document::DocumentTypeRepo;
using document::test::makeBucketSpace;
using proton::DocTypeName;
using proton::DocumentDB;
using proton::DocumentDBConfigHelper;
using proton::MemoryConfigStore;
using proton::PersistenceEngine;
using proton::PersistenceHandlerProxy;
using search::engine::SearchReply;
using search::engine::SearchRequest;
using search::engine::SerializedQueryTree;
using search::index::DummyFileHeaderContext;
using search::query::QueryBuilder;
using search::query::StackDumpCreator;
using search::query::Weight;
using search::transactionlog::TransLogServer;
using storage::spi::Bucket;
using storage::spi::BucketInfo;
using storage::spi::Timestamp;
using vespalib::HwInfo;
using vespa::config::content::core::BucketspacesConfig;
using vespa::config::search::core::ProtonConfig;
using cloud::config::filedistribution::FiledistributorrpcConfig;

namespace {

constexpr int TLS_PORT = proton::test::port_numbers::documentdb_tls_port;
constexpr const char* DOC_TYPE = "typea";
constexpr const char* ATTR = "attr1";

std::string tls_spec() {
    return vespalib::SocketSpec::from_host_port("localhost", TLS_PORT).spec();
}

void cleanup() {
    std::filesystem::remove_all(std::filesystem::path(DOC_TYPE));
    std::filesystem::remove_all(std::filesystem::path("tmp"));
}

DocumentType make_document_type() {
    DocumentType type(DOC_TYPE, 0);
    type.addField(document::Field(ATTR, 1, *document::DataType::INT));
    return type;
}

class PersistenceOwner final : public proton::IPersistenceEngineOwner {
public:
    void setClusterState(document::BucketSpace, const storage::spi::ClusterState&) override {}
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
    using Nodes = Group::Nodes;

    storage::lib::ClusterState cluster_state;
    cluster_state.setNodeState(Node(NodeType::STORAGE, 0),
                               NodeState(NodeType::STORAGE, State::UP, "embedded", 1.0));
    cluster_state.setClusterState(State::UP);

    StorDistributionConfigBuilder cfg;
    cfg.redundancy = 1;
    cfg.readyCopies = 1;
    cfg.group.emplace_back();
    auto& group = cfg.group.back();
    group.index = "invalid";
    group.name = "embedded";
    group.capacity = 1.0;
    group.partitions = "";
    group.nodes.emplace_back();
    group.nodes.back().index = 0;

    return storage::spi::ClusterState(cluster_state, 0, Distribution(cfg));
}

std::string number_query_stack_dump(int value) {
    QueryBuilder<proton::matching::ProtonNodeTypes> builder;
    builder.addNumberTerm(std::to_string(value), ATTR, 1, Weight(1));
    auto tree = builder.build();
    EXPECT_TRUE(tree);
    return StackDumpCreator::create(*tree);
}

SearchRequest::SP make_request(int value) {
    auto request = std::make_shared<SearchRequest>();
    request->setTimeout(60s);
    request->setSerializedQueryTree(SerializedQueryTree::fromStackDump(number_query_stack_dump(value)));
    request->ranking = "default";
    request->maxhits = 10;
    return request;
}

bool contains(const SearchReply& reply, const DocumentId& id) {
    const auto gid = id.getGlobalId();
    return std::any_of(reply.hits.begin(), reply.hits.end(),
                       [&](const auto& hit) { return hit.gid == gid; });
}

class EmbeddedFixture {
    proton::DummyWireService _wire_service;
    proton::DummyDBOwner _db_owner;
    vespalib::ThreadStackExecutor _summary_executor;
    proton::MockSharedThreadingService _shared_service;
    HwInfo _hw_info;
    DummyFileHeaderContext _file_header_context;
    TransLogServer _tls;
    proton::matching::QueryLimiter _query_limiter;

    DocumentType _doc_type;
    std::shared_ptr<DocumentTypeRepo> _repo;
    DocumentDB::SP _db;

    PersistenceOwner _persistence_owner;
    AcceptAllWrites _write_filter;
    proton::test::ResourceUsageNotifier _resource_usage_notifier;
    PersistenceEngine _persistence;
    std::shared_ptr<PersistenceHandlerProxy> _handler;
    std::unordered_set<uint64_t> _prepared_buckets;

public:
    EmbeddedFixture()
        : _wire_service(),
          _db_owner(),
          _summary_executor(8),
          _shared_service(_summary_executor),
          _hw_info(),
          _file_header_context(),
          _tls(_shared_service.transport(), "tmp", TLS_PORT, ".", _file_header_context),
          _query_limiter(),
          _doc_type(make_document_type()),
          _repo(std::make_shared<DocumentTypeRepo>(_doc_type)),
          _db(),
          _persistence_owner(),
          _write_filter(),
          _resource_usage_notifier(),
          _persistence(_persistence_owner, _write_filter, _resource_usage_notifier, -1, false),
          _handler(),
          _prepared_buckets()
    {
        // Reuse a clone of Vespa's existing documentdb unit-test config. It defines
        // attr1 as INT32 and apply.sh adds an empty default rank profile so the
        // matcher is constructed against the real schema.
        config::DirSpec spec(TEST_PATH("embedded_crud_cfg"));
        DocumentDBConfigHelper cfg(spec, DOC_TYPE);

        auto document_types = std::make_shared<DocumenttypesConfig>();
        auto proton_config = std::make_shared<ProtonConfig>();
        auto file_distribution = std::make_shared<FiledistributorrpcConfig>();
        auto bucket_spaces = std::make_shared<BucketspacesConfig>();
        auto tune = std::make_shared<search::TuneFileDocumentDB>();

        auto bootstrap = std::make_shared<proton::BootstrapConfig>(
            1, document_types, _repo, proton_config, file_distribution,
            bucket_spaces, tune, _hw_info);

        cfg.forwardConfig(bootstrap);
        cfg.nextGeneration(_shared_service.transport(), 0ms);

        _db = DocumentDB::create(
            ".", cfg.getConfig(), tls_spec(), _query_limiter, DocTypeName(DOC_TYPE),
            makeBucketSpace(), *proton_config, _db_owner, _shared_service, _tls,
            _wire_service, _file_header_context,
            std::make_shared<search::attribute::Interlock>(),
            std::make_unique<MemoryConfigStore>(),
            std::make_shared<vespalib::ThreadStackExecutor>(16),
            _hw_info, std::shared_ptr<search::diskindex::IPostingListCache>());

        _db->start();
        _db->waitForOnlineState();

        _handler = std::make_shared<PersistenceHandlerProxy>(_db);
        _persistence.putHandler(_persistence.getWLock(), makeBucketSpace(),
                                DocTypeName(DOC_TYPE), _handler);
        ASSERT_FALSE(_persistence.initialize().hasError());

        auto state = single_node_cluster_state();
        ASSERT_FALSE(_persistence.setClusterState(makeBucketSpace(), state).hasError());
    }

    ~EmbeddedFixture() {
        _handler.reset();
        if (_db) {
            _db->close();
            _db.reset();
        }
        _shared_service.transport().ShutDown(true);
    }

    const DocumentType& doc_type() const { return _doc_type; }
    const DocumentTypeRepo& repo() const { return *_repo; }

    Bucket bucket_for(const DocumentId& id) const {
        return proton::BucketFactory::getBucket(id);
    }

    std::shared_ptr<Document> make_doc(const DocumentId& id, int attr_value) const {
        auto doc = std::make_shared<Document>(repo(), doc_type(), id);
        const auto& field = doc_type().getField(ATTR);
        doc->setFieldValue(field, std::make_unique<document::IntFieldValue>(attr_value));
        return doc;
    }

    std::shared_ptr<document::DocumentUpdate> make_update(const DocumentId& id, int attr_value) const {
        auto update = std::make_shared<document::DocumentUpdate>(repo(), doc_type(), id);
        const auto& field = doc_type().getField(ATTR);
        document::FieldUpdate field_update(field);
        field_update.addUpdate(std::make_unique<document::AssignValueUpdate>(
            std::make_unique<document::IntFieldValue>(attr_value)));
        update->addUpdate(std::move(field_update));
        return update;
    }

    void prepare_bucket(const Bucket& bucket) {
        const auto raw_id = bucket.getBucketId().getId();
        if (_prepared_buckets.insert(raw_id).second) {
            ASSERT_FALSE(_persistence.createBucket(bucket).hasError());
            ASSERT_FALSE(_persistence.setActiveState(bucket, BucketInfo::ACTIVE).hasError());
        }
    }

    void put(const DocumentId& id, int value, uint64_t ts) {
        auto bucket = bucket_for(id);
        prepare_bucket(bucket);
        auto result = _persistence.put(bucket, Timestamp(ts), make_doc(id, value));
        ASSERT_FALSE(result.hasError());
    }

    void update(const DocumentId& id, int value, uint64_t ts) {
        auto result = _persistence.update(bucket_for(id), Timestamp(ts), make_update(id, value));
        ASSERT_FALSE(result.hasError());
    }

    void remove(const DocumentId& id, uint64_t ts) {
        auto result = _persistence.remove(bucket_for(id), Timestamp(ts), id);
        ASSERT_FALSE(result.hasError());
        ASSERT_TRUE(result.wasFound());
    }

    SearchReply::UP search(int value) const {
        auto request = make_request(value);
        return _db->match(*request, vespalib::ThreadBundle::trivial());
    }
};

} // namespace

class EmbeddedCrudSpikeTest : public ::testing::Test {
protected:
    static void SetUpTestSuite() {
        cleanup();
        std::filesystem::create_directory(std::filesystem::path(DOC_TYPE));
        DummyFileHeaderContext::setCreator("embedded_crud_spike");
    }

    static void TearDownTestSuite() {
        cleanup();
    }
};

TEST_F(EmbeddedCrudSpikeTest, put_search_update_search_delete_search) {
    EmbeddedFixture db;

    const DocumentId doc1("id:test:typea::1");
    const DocumentId doc2("id:test:typea::2");

    // 1. Write two documents.
    db.put(doc1, 10, 1);
    db.put(doc2, 20, 2);

    // 2. Search. attr1=10 returns only doc1.
    {
        auto result = db.search(10);
        ASSERT_TRUE(result);
        ASSERT_EQ(1u, result->hits.size());
        EXPECT_TRUE(contains(*result, doc1));
        EXPECT_FALSE(contains(*result, doc2));
    }

    // 3. Update doc2 so it also matches attr1=10.
    db.update(doc2, 10, 3);

    // 4. Search again. Both documents must be immediately visible.
    {
        auto result = db.search(10);
        ASSERT_TRUE(result);
        ASSERT_EQ(2u, result->hits.size());
        EXPECT_TRUE(contains(*result, doc1));
        EXPECT_TRUE(contains(*result, doc2));
    }

    // 5. Delete doc1.
    db.remove(doc1, 4);

    // 6. Search again. Only doc2 remains.
    {
        auto result = db.search(10);
        ASSERT_TRUE(result);
        ASSERT_EQ(1u, result->hits.size());
        EXPECT_FALSE(contains(*result, doc1));
        EXPECT_TRUE(contains(*result, doc2));
    }

    // Also verify the old doc2 value is gone after the update.
    {
        auto result = db.search(20);
        ASSERT_TRUE(result);
        EXPECT_EQ(0u, result->hits.size());
    }
}

GTEST_MAIN_RUN_ALL_TESTS()
