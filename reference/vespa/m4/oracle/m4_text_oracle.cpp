// Copyright Vespa.ai. Licensed under the terms of the Apache 2.0 license.
//
// Search Kernel M4 golden oracle.
//
// This executable is intentionally a direct Vespa C++ program. It does not
// link Search Kernel Rust/native adapter code. Document/query linguistic terms
// are supplied by the frozen M3 Java oracle over stdin; this program only
// lowers those terms into Vespa annotations/query trees and exercises the real
// embedded Proton indexing + Matcher path.

#include <vespa/config-bucketspaces.h>
#include <vespa/config-stor-distribution.h>
#include <vespa/config/subscription/sourcespec.h>
#include <vespa/fnet/transport.h>
#include <vespa/document/annotation/annotation.h>
#include <vespa/document/annotation/span.h>
#include <vespa/document/annotation/spanlist.h>
#include <vespa/document/annotation/spantree.h>
#include <vespa/document/config/documenttypes_config_fwd.h>
#include <vespa/document/datatype/annotationtype.h>
#include <vespa/document/datatype/datatype.h>
#include <vespa/document/datatype/documenttype.h>
#include <vespa/document/fieldvalue/document.h>
#include <vespa/document/fieldvalue/stringfieldvalue.h>
#include <vespa/document/repo/documenttyperepo.h>
#include <vespa/document/repo/fixedtyperepo.h>
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
#include <vespa/vespalib/net/socket_spec.h>
#include <vespa/vespalib/util/hw_info.h>
#include <vespa/vespalib/util/thread_bundle.h>
#include <vespa/vespalib/util/threadstackexecutor.h>

#include <algorithm>
#include <charconv>
#include <chrono>
#include <cstdint>
#include <cstdlib>
#include <filesystem>
#include <iomanip>
#include <iostream>
#include <limits>
#include <memory>
#include <sstream>
#include <stdexcept>
#include <string>
#include <string_view>
#include <system_error>
#include <unordered_set>
#include <utility>
#include <vector>

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
using search::SerializedQueryTree;
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
constexpr const char* TEXT_FIELD = "title";
constexpr const char* LINGUISTICS_TREE = "linguistics";

struct AnalyzedTerm {
    uint32_t position;
    int32_t byte_start;
    int32_t byte_len;
    std::string term;
};

struct QueryTerm {
    uint32_t position;
    std::string term;
};

[[noreturn]] void fail(std::string_view message) {
    throw std::runtime_error(std::string(message));
}

void require(bool condition, std::string_view message) {
    if (!condition) {
        fail(message);
    }
}

std::string tls_spec() {
    return vespalib::SocketSpec::from_host_port("localhost", TLS_PORT).spec();
}

void cleanup_workdir() {
    std::filesystem::remove_all(std::filesystem::path(DOC_TYPE));
    std::filesystem::remove_all(std::filesystem::path("tmp"));
}

DocumentType make_document_type() {
    DocumentType type(DOC_TYPE, 0);
    type.addField(document::Field(TEXT_FIELD, 1, *document::DataType::STRING));
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

std::vector<std::string> split_tabs(const std::string& line) {
    std::vector<std::string> out;
    size_t start = 0;
    while (true) {
        const auto pos = line.find('\t', start);
        if (pos == std::string::npos) {
            out.emplace_back(line.substr(start));
            break;
        }
        out.emplace_back(line.substr(start, pos - start));
        start = pos + 1;
    }
    return out;
}

uint8_t hex_nibble(char c) {
    if (c >= '0' && c <= '9') return static_cast<uint8_t>(c - '0');
    if (c >= 'a' && c <= 'f') return static_cast<uint8_t>(c - 'a' + 10);
    if (c >= 'A' && c <= 'F') return static_cast<uint8_t>(c - 'A' + 10);
    fail("invalid hex digit");
}

std::string hex_decode(std::string_view hex) {
    require((hex.size() % 2) == 0, "odd-length hex payload");
    std::string out;
    out.resize(hex.size() / 2);
    for (size_t i = 0; i < out.size(); ++i) {
        out[i] = static_cast<char>((hex_nibble(hex[2 * i]) << 4) | hex_nibble(hex[2 * i + 1]));
    }
    return out;
}

uint32_t parse_u32(std::string_view value, std::string_view what) {
    uint32_t out = 0;
    auto result = std::from_chars(value.data(), value.data() + value.size(), out);
    if (result.ec != std::errc() || result.ptr != value.data() + value.size()) {
        fail(std::string("invalid ") + std::string(what));
    }
    return out;
}

int32_t parse_i32(std::string_view value, std::string_view what) {
    int32_t out = 0;
    auto result = std::from_chars(value.data(), value.data() + value.size(), out);
    if (result.ec != std::errc() || result.ptr != value.data() + value.size()) {
        fail(std::string("invalid ") + std::string(what));
    }
    return out;
}

// The document protocol is:
//   position,byte_start,byte_len,term_hex;position,...
// It is generated mechanically from authoritative M3 document output.
std::vector<AnalyzedTerm> parse_document_analysis(std::string_view encoded_hex) {
    const std::string spec = hex_decode(encoded_hex);
    std::vector<AnalyzedTerm> terms;
    if (spec.empty()) {
        return terms;
    }
    size_t item_start = 0;
    while (item_start <= spec.size()) {
        const auto item_end = spec.find(';', item_start);
        const auto item = std::string_view(spec).substr(
            item_start,
            item_end == std::string::npos ? std::string::npos : item_end - item_start);
        std::vector<std::string_view> cols;
        size_t col_start = 0;
        while (true) {
            const auto comma = item.find(',', col_start);
            if (comma == std::string_view::npos) {
                cols.emplace_back(item.substr(col_start));
                break;
            }
            cols.emplace_back(item.substr(col_start, comma - col_start));
            col_start = comma + 1;
        }
        require(cols.size() == 4, "invalid analysis term record");
        AnalyzedTerm t{
            parse_u32(cols[0], "position"),
            parse_i32(cols[1], "byte_start"),
            parse_i32(cols[2], "byte_len"),
            hex_decode(cols[3])
        };
        require(t.position == terms.size(), "M4 oracle expects contiguous M3 logical positions");
        require(t.byte_start >= 0 && t.byte_len >= 0, "negative source span");
        require(!t.term.empty(), "empty analyzed term");
        terms.emplace_back(std::move(t));
        if (item_end == std::string::npos) {
            break;
        }
        item_start = item_end + 1;
    }
    return terms;
}

// The query protocol is intentionally different from the document protocol:
//   position,term_hex;position,...
// M3 query output exposes StemList values and logical positions, not source
// spans. The M4 corpus is restricted to one effective stem per query position.
std::vector<QueryTerm> parse_query_analysis(std::string_view encoded_hex) {
    const std::string spec = hex_decode(encoded_hex);
    std::vector<QueryTerm> terms;
    if (spec.empty()) {
        return terms;
    }
    size_t item_start = 0;
    while (item_start <= spec.size()) {
        const auto item_end = spec.find(';', item_start);
        const auto item = std::string_view(spec).substr(
            item_start,
            item_end == std::string::npos ? std::string::npos : item_end - item_start);
        const auto comma = item.find(',');
        require(comma != std::string_view::npos && item.find(',', comma + 1) == std::string_view::npos,
                "invalid query analysis term record");
        QueryTerm t{
            parse_u32(item.substr(0, comma), "query position"),
            hex_decode(item.substr(comma + 1))
        };
        require(t.position == terms.size(), "M4 oracle expects contiguous M3 query positions");
        require(!t.term.empty(), "empty analyzed query term");
        terms.emplace_back(std::move(t));
        if (item_end == std::string::npos) {
            break;
        }
        item_start = item_end + 1;
    }
    return terms;
}

std::unique_ptr<document::StringFieldValue>
make_annotated_string(std::string_view raw,
                      const std::vector<AnalyzedTerm>& terms,
                      const document::FixedTypeRepo& fixed_repo) {
    auto value = std::make_unique<document::StringFieldValue>(raw);
    auto spans = std::make_unique<document::SpanList>();
    auto* spans_ptr = spans.get();
    spans_ptr->reserve(terms.size());
    auto tree = std::make_unique<document::SpanTree>(LINGUISTICS_TREE, std::move(spans));

    for (const auto& term : terms) {
        const auto start = static_cast<size_t>(term.byte_start);
        const auto len = static_cast<size_t>(term.byte_len);
        require(start <= raw.size() && len <= raw.size() - start, "analysis span outside raw string");
        const auto original = raw.substr(start, len);
        auto& span = spans_ptr->add(std::make_unique<document::Span>(term.byte_start, term.byte_len));
        if (original == term.term) {
            tree->annotate(span, *document::AnnotationType::TERM);
        } else {
            tree->annotate(span, document::Annotation(
                *document::AnnotationType::TERM,
                std::make_unique<document::StringFieldValue>(term.term)));
        }
    }

    document::StringFieldValue::SpanTrees trees;
    trees.emplace_back(std::move(tree));
    value->setSpanTrees(trees, fixed_repo);
    return value;
}

std::string query_stack_dump(std::string_view kind, const std::vector<QueryTerm>& terms) {
    QueryBuilder<proton::matching::ProtonNodeTypes> builder;
    if (kind == "TERM") {
        require(terms.size() == 1, "TERM query must analyze to exactly one term in M4");
        builder.addStringTerm(terms[0].term, TEXT_FIELD, 1, Weight(1));
    } else if (kind == "PHRASE") {
        require(!terms.empty(), "PHRASE query analyzed to zero terms");
        builder.addPhrase(static_cast<int>(terms.size()), TEXT_FIELD, 1, Weight(1));
        int32_t id = 2;
        for (const auto& term : terms) {
            builder.addStringTerm(term.term, TEXT_FIELD, id++, Weight(1));
        }
    } else {
        fail("unsupported query kind");
    }
    auto tree = builder.build();
    require(static_cast<bool>(tree), std::string("query build failed: ") + builder.error());
    return StackDumpCreator::create(*tree);
}

SearchRequest::SP make_request(std::string_view kind,
                               const std::vector<QueryTerm>& terms,
                               const std::string& rank_profile) {
    auto request = std::make_shared<SearchRequest>();
    request->setTimeout(60s);
    request->setSerializedQueryTree(
        SerializedQueryTree::fromStackDump(query_stack_dump(kind, terms)));
    request->ranking = rank_profile;
    request->maxhits = 100;
    return request;
}

class EmbeddedTextFixture {
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
    std::vector<std::string> _known_ids;
    uint64_t _next_timestamp;

public:
    explicit EmbeddedTextFixture(const std::string& config_dir)
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
          _prepared_buckets(),
          _known_ids(),
          _next_timestamp(1)
    {
        config::DirSpec spec(config_dir);
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

        std::cerr << "M4_ORACLE startup: DocumentDB::start begin\n" << std::flush;
        _db->start();
        std::cerr << "M4_ORACLE startup: DocumentDB::start returned\n" << std::flush;

        std::cerr << "M4_ORACLE startup: waitForOnlineState begin\n" << std::flush;
        _db->waitForOnlineState();
        std::cerr << "M4_ORACLE startup: waitForOnlineState returned\n" << std::flush;

        std::cerr << "M4_ORACLE startup: PersistenceHandlerProxy begin\n" << std::flush;
        _handler = std::make_shared<PersistenceHandlerProxy>(_db);
        _persistence.putHandler(_persistence.getWLock(), makeBucketSpace(),
                                DocTypeName(DOC_TYPE), _handler);
        std::cerr << "M4_ORACLE startup: PersistenceHandlerProxy installed\n" << std::flush;

        std::cerr << "M4_ORACLE startup: PersistenceEngine::initialize begin\n" << std::flush;
        auto init_result = _persistence.initialize();
        require(!init_result.hasError(), "persistence initialization failed");
        std::cerr << "M4_ORACLE startup: PersistenceEngine::initialize returned\n" << std::flush;

        std::cerr << "M4_ORACLE startup: setClusterState begin\n" << std::flush;
        auto state = single_node_cluster_state();
        auto state_result = _persistence.setClusterState(makeBucketSpace(), state);
        require(!state_result.hasError(), "cluster state setup failed");
        std::cerr << "M4_ORACLE startup: setClusterState returned\n" << std::flush;
        std::cerr << "M4_ORACLE startup: READY\n" << std::flush;
    }

    ~EmbeddedTextFixture() {
        std::cerr << "M4_ORACLE shutdown: removeHandler begin\n" << std::flush;
        {
            auto removed = _persistence.removeHandler(
                _persistence.getWLock(), makeBucketSpace(), DocTypeName(DOC_TYPE));
            require(static_cast<bool>(removed),
                    "persistence handler was not registered during shutdown");
        }
        _handler.reset();
        std::cerr << "M4_ORACLE shutdown: removeHandler returned\n" << std::flush;

        if (_db) {
            std::cerr << "M4_ORACLE shutdown: DocumentDB::close begin\n" << std::flush;
            _db->close();
            std::cerr << "M4_ORACLE shutdown: DocumentDB::close returned\n" << std::flush;
            _db.reset();
            std::cerr << "M4_ORACLE shutdown: DocumentDB released\n" << std::flush;
        }

        std::cerr << "M4_ORACLE shutdown: transport ShutDown begin\n" << std::flush;
        _shared_service.transport().ShutDown(true);
        std::cerr << "M4_ORACLE shutdown: transport ShutDown returned\n" << std::flush;
    }

    const DocumentType& doc_type() const { return _doc_type; }
    const DocumentTypeRepo& repo() const { return *_repo; }

    Bucket bucket_for(const DocumentId& id) const {
        return proton::BucketFactory::getBucket(id);
    }

    std::unique_ptr<document::StringFieldValue>
    text_value(std::string_view raw, const std::vector<AnalyzedTerm>& terms) const {
        const document::FixedTypeRepo fixed_repo(repo(), doc_type());
        return make_annotated_string(raw, terms, fixed_repo);
    }

    std::shared_ptr<Document>
    make_doc(const DocumentId& id, std::string_view raw, const std::vector<AnalyzedTerm>& terms) const {
        auto doc = std::make_shared<Document>(repo(), doc_type(), id);
        const auto& field = doc_type().getField(TEXT_FIELD);
        doc->setFieldValue(field, text_value(raw, terms));
        return doc;
    }

    std::shared_ptr<document::DocumentUpdate>
    make_update(const DocumentId& id, std::string_view raw, const std::vector<AnalyzedTerm>& terms) const {
        auto update = std::make_shared<document::DocumentUpdate>(repo(), doc_type(), id);
        const auto& field = doc_type().getField(TEXT_FIELD);
        document::FieldUpdate field_update(field);
        field_update.addUpdate(std::make_unique<document::AssignValueUpdate>(text_value(raw, terms)));
        update->addUpdate(std::move(field_update));
        return update;
    }

    void prepare_bucket(const Bucket& bucket) {
        const auto raw_id = bucket.getBucketId().getId();
        if (_prepared_buckets.insert(raw_id).second) {
            auto create_result = _persistence.createBucket(bucket);
            require(!create_result.hasError(), "createBucket failed");
            auto active_result = _persistence.setActiveState(bucket, BucketInfo::ACTIVE);
            require(!active_result.hasError(), "setActiveState failed");
        }
    }

    void put(const std::string& native_id,
             std::string_view raw,
             const std::vector<AnalyzedTerm>& terms) {
        const DocumentId id(native_id);
        auto bucket = bucket_for(id);
        prepare_bucket(bucket);
        auto result = _persistence.put(bucket, Timestamp(_next_timestamp++), make_doc(id, raw, terms));
        require(!result.hasError(), "put failed");
        if (std::find(_known_ids.begin(), _known_ids.end(), native_id) == _known_ids.end()) {
            _known_ids.push_back(native_id);
        }
    }

    void update(const std::string& native_id,
                std::string_view raw,
                const std::vector<AnalyzedTerm>& terms) {
        const DocumentId id(native_id);
        auto result = _persistence.update(bucket_for(id), Timestamp(_next_timestamp++),
                                          make_update(id, raw, terms));
        require(!result.hasError(), "update failed");
    }

    void remove(const std::string& native_id) {
        const DocumentId id(native_id);
        auto result = _persistence.remove(bucket_for(id), Timestamp(_next_timestamp++), id);
        require(!result.hasError(), "remove failed");
        require(result.wasFound(), "remove did not find document");
    }

    std::string id_for_gid(const document::GlobalId& gid) const {
        for (const auto& native_id : _known_ids) {
            if (DocumentId(native_id).getGlobalId() == gid) {
                return native_id;
            }
        }
        fail("matcher returned unknown global id");
    }

    SearchReply::UP search(std::string_view kind,
                           const std::vector<QueryTerm>& terms,
                           const std::string& rank_profile) const {
        auto request = make_request(kind, terms, rank_profile);
        auto reply = _db->match(*request, vespalib::ThreadBundle::trivial());
        require(static_cast<bool>(reply), "DocumentDB::match returned null");
        return reply;
    }
};

void emit_result(const std::string& search_id,
                 const SearchReply& reply,
                 const EmbeddedTextFixture& db) {
    std::cout << "RESULT\t" << search_id << "\t";
    bool first = true;
    std::cout << std::setprecision(std::numeric_limits<double>::max_digits10);
    for (const auto& hit : reply.hits) {
        if (!first) {
            std::cout << ';';
        }
        first = false;
        std::cout << db.id_for_gid(hit.gid) << ',' << hit.metric;
    }
    std::cout << '\n' << std::flush;
}

std::string required_env(const char* name) {
    const char* value = std::getenv(name);
    if (value == nullptr || *value == '\0') {
        fail(std::string("missing environment variable ") + name);
    }
    return value;
}

int run() {
    cleanup_workdir();
    std::filesystem::create_directory(std::filesystem::path(DOC_TYPE));
    DummyFileHeaderContext::setCreator("searchkernel_m4_cpp_oracle");

    EmbeddedTextFixture db(required_env("M4_ORACLE_CONFIG_DIR"));

    std::string line;
    while (std::getline(std::cin, line)) {
        const auto cols = split_tabs(line);
        require(!cols.empty(), "empty command");
        const auto& op = cols[0];

        std::cerr << "M4_ORACLE command: " << op << " begin\n" << std::flush;

        if (op == "PUT" || op == "UPDATE") {
            require(cols.size() == 4, "PUT/UPDATE expects 4 columns");
            const auto raw = hex_decode(cols[2]);
            const auto analysis = parse_document_analysis(cols[3]);
            if (op == "PUT") {
                db.put(cols[1], raw, analysis);
            } else {
                db.update(cols[1], raw, analysis);
            }
            std::cout << "OK\n" << std::flush;
            std::cerr << "M4_ORACLE command: " << op << " returned\n" << std::flush;
        } else if (op == "DELETE") {
            require(cols.size() == 2, "DELETE expects 2 columns");
            db.remove(cols[1]);
            std::cout << "OK\n" << std::flush;
            std::cerr << "M4_ORACLE command: DELETE returned\n" << std::flush;
        } else if (op == "TERM" || op == "PHRASE") {
            require(cols.size() == 4, "TERM/PHRASE expects 4 columns");
            const auto analysis = parse_query_analysis(cols[3]);
            auto reply = db.search(op, analysis, cols[2]);
            emit_result(cols[1], *reply, db);
            std::cerr << "M4_ORACLE command: " << op << " returned\n" << std::flush;
        } else if (op == "QUIT") {
            require(cols.size() == 1, "QUIT expects 1 column");
            std::cout << "OK\n" << std::flush;
            return 0;
        } else {
            fail(std::string("unknown command: ") + op);
        }
    }
    fail("stdin closed before QUIT");
}

} // namespace

int main() {
    try {
        const int rc = run();
        cleanup_workdir();
        return rc;
    } catch (const std::exception& e) {
        std::cerr << "m4_text_oracle: " << e.what() << '\n';
        cleanup_workdir();
        return 2;
    }
}

