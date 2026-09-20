// Copyright Vespa.ai. Licensed under the terms of the Apache 2.0 license.
//
// Direct M8 oracle.  This executable deliberately talks to the pinned Proton
// DocumentDB and never calls Search Kernel production code.

#include <vespa/config-bucketspaces.h>
#include <vespa/config-stor-distribution.h>
#include <vespa/config/subscription/sourcespec.h>
#include <vespa/document/annotation/annotation.h>
#include <vespa/document/annotation/span.h>
#include <vespa/document/annotation/spanlist.h>
#include <vespa/document/annotation/spantree.h>
#include <vespa/document/datatype/datatype.h>
#include <vespa/document/datatype/documenttype.h>
#include <vespa/document/fieldvalue/boolfieldvalue.h>
#include <vespa/document/fieldvalue/document.h>
#include <vespa/document/fieldvalue/doublefieldvalue.h>
#include <vespa/document/fieldvalue/intfieldvalue.h>
#include <vespa/document/fieldvalue/stringfieldvalue.h>
#include <vespa/document/repo/documenttyperepo.h>
#include <vespa/document/repo/fixedtyperepo.h>
#include <vespa/document/test/make_bucket_space.h>
#include <vespa/fnet/transport.h>
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
#include <vespa/searchlib/engine/docsumreply.h>
#include <vespa/searchlib/engine/docsumrequest.h>
#include <vespa/searchlib/engine/searchreply.h>
#include <vespa/searchlib/engine/searchrequest.h>
#include <vespa/searchlib/index/dummyfileheadercontext.h>
#include <vespa/searchlib/query/tree/querybuilder.h>
#include <vespa/searchlib/query/tree/range.h>
#include <vespa/searchlib/query/tree/stackdumpcreator.h>
#include <vespa/searchlib/transactionlog/translogserver.h>
#include <vespa/vespalib/data/slime/json_format.h>
#include <vespa/vespalib/data/slime/slime.h>
#include <vespa/vespalib/net/socket_spec.h>
#include <vespa/vespalib/util/hw_info.h>
#include <vespa/vespalib/util/thread_bundle.h>
#include <vespa/vespalib/util/threadstackexecutor.h>
#include <vespa/vdslib/distribution/distribution.h>
#include <vespa/vdslib/state/clusterstate.h>

#include <algorithm>
#include <bit>
#include <cctype>
#include <chrono>
#include <cmath>
#include <cstdint>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <iterator>
#include <limits>
#include <memory>
#include <sstream>
#include <stdexcept>
#include <string>
#include <string_view>
#include <unordered_map>
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
using search::engine::DocsumReply;
using search::engine::DocsumRequest;
using search::engine::SearchReply;
using search::engine::SearchRequest;
using search::SerializedQueryTree;
using search::index::DummyFileHeaderContext;
using search::query::QueryBuilder;
using search::query::Range;
using search::query::StackDumpCreator;
using search::query::Weight;
using search::transactionlog::TransLogServer;
using storage::spi::Bucket;
using storage::spi::BucketInfo;
using storage::spi::Timestamp;
using vespalib::HwInfo;
using vespalib::Slime;
using vespalib::Memory;
using vespalib::slime::Inspector;
using vespalib::slime::JsonFormat;
using vespalib::slime::Type;
using vespa::config::content::core::BucketspacesConfig;
using vespa::config::search::core::ProtonConfig;
using cloud::config::filedistribution::FiledistributorrpcConfig;

namespace {

constexpr int TLS_PORT = proton::test::port_numbers::documentdb_tls_port;
constexpr const char* DOC_TYPE = "typea";
constexpr const char* LINGUISTICS_TREE = "linguistics";

[[noreturn]] void fail(std::string_view message) { throw std::runtime_error(std::string(message)); }

void require(bool condition, std::string_view message) {
    if (!condition) fail(message);
}

const Inspector& object_field(const Inspector& object, std::string_view name) {
    require(object.type() == vespalib::slime::OBJECT::instance, "expected JSON object");
    return object[Memory(name)];
}

std::string string_value(const Inspector& value, std::string_view what) {
    require(value.type() == vespalib::slime::STRING::instance, what);
    return value.asString().make_string();
}

int64_t integer_value(const Inspector& value, std::string_view what) {
    require(value.type() == vespalib::slime::LONG::instance, what);
    return value.asLong();
}

bool bool_value(const Inspector& value, std::string_view what) {
    require(value.type() == vespalib::slime::BOOL::instance, what);
    return value.asBool();
}

bool present(const Inspector& value) { return value.valid() && value.type() != vespalib::slime::NIX::instance; }

uint32_t nonnegative_u32(const Inspector& value, std::string_view what) {
    const auto number = integer_value(value, what);
    require(number >= 0 && static_cast<uint64_t>(number) <= std::numeric_limits<uint32_t>::max(), what);
    return static_cast<uint32_t>(number);
}

std::string read_file(const std::filesystem::path& path) {
    std::ifstream input(path, std::ios::binary);
    require(input.good(), "could not open scenario");
    return std::string(std::istreambuf_iterator<char>(input), std::istreambuf_iterator<char>());
}

Slime read_json(const std::filesystem::path& path) {
    auto bytes = read_file(path);
    Slime slime;
    const auto used = JsonFormat::decode(Memory(bytes), slime);
    require(used <= bytes.size() && std::all_of(bytes.begin() + static_cast<std::ptrdiff_t>(used), bytes.end(),
                                               [](unsigned char c) { return std::isspace(c) != 0; }),
            "malformed or trailing scenario JSON");
    return slime;
}

std::string json_escape(std::string_view value) {
    std::ostringstream out;
    out << '"';
    for (const unsigned char c : value) {
        switch (c) {
        case '"': out << "\\\""; break;
        case '\\': out << "\\\\"; break;
        case '\n': out << "\\n"; break;
        case '\r': out << "\\r"; break;
        case '\t': out << "\\t"; break;
        default:
            if (c < 0x20) out << "\\u" << std::hex << std::setw(4) << std::setfill('0') << int(c) << std::dec;
            else out << static_cast<char>(c);
        }
    }
    out << '"';
    return out.str();
}

std::string hex64(uint64_t value) {
    std::ostringstream out;
    out << "0x" << std::hex << std::setw(16) << std::setfill('0') << value;
    return out.str();
}

std::string double_bits(double value) { return hex64(std::bit_cast<uint64_t>(value)); }

struct FieldSpec {
    std::string name;
    std::string type;
    bool index = false;
    bool attribute = false;
    bool summary = false;
};

struct AnalyzedTerm {
    int32_t start;
    int32_t length;
    std::string term;
};

std::vector<FieldSpec> fields_from_schema(const Inspector& root) {
    const auto& fields = object_field(object_field(root, "schema"), "fields");
    require(fields.type() == vespalib::slime::ARRAY::instance, "schema.fields must be an array");
    std::vector<FieldSpec> result;
    for (size_t i = 0; i < fields.children(); ++i) {
        const auto& f = fields[i];
        FieldSpec spec;
        spec.name = string_value(object_field(f, "name"), "field name must be string");
        spec.type = string_value(object_field(f, "type"), "field type must be string");
        if (present(object_field(f, "index"))) spec.index = bool_value(object_field(f, "index"), "index must be bool");
        if (present(object_field(f, "attribute"))) spec.attribute = bool_value(object_field(f, "attribute"), "attribute must be bool");
        if (present(object_field(f, "summary"))) spec.summary = bool_value(object_field(f, "summary"), "summary must be bool");
        result.emplace_back(std::move(spec));
    }
    require(!result.empty(), "schema must contain fields");
    return result;
}

std::vector<FieldSpec> fields_from_fixture_schema(const Inspector& root) {
    const auto& fields = object_field(root, "fields");
    require(fields.type() == vespalib::slime::ARRAY::instance, "fixture fields must be an array");
    std::vector<FieldSpec> result;
    for (size_t i = 0; i < fields.children(); ++i) {
        const auto& f = fields[i];
        FieldSpec spec;
        spec.name = string_value(object_field(f, "name"), "field name must be string");
        spec.type = string_value(object_field(f, "type"), "field type must be string");
        if (present(object_field(f, "index"))) spec.index = bool_value(object_field(f, "index"), "index must be bool");
        if (present(object_field(f, "attribute"))) spec.attribute = bool_value(object_field(f, "attribute"), "attribute must be bool");
        if (present(object_field(f, "summary"))) spec.summary = bool_value(object_field(f, "summary"), "summary must be bool");
        result.emplace_back(std::move(spec));
    }
    require(!result.empty(), "fixture fields must be non-empty");
    return result;
}

std::vector<AnalyzedTerm> analysis_from_value(const Inspector& field_value) {
    const auto& analysis = object_field(field_value, "analysis");
    require(analysis.type() == vespalib::slime::ARRAY::instance, "text analysis must be an array");
    std::vector<AnalyzedTerm> result;
    for (size_t i = 0; i < analysis.children(); ++i) {
        const auto& item = analysis[i];
        const auto position = integer_value(object_field(item, "position"), "analysis position must be integer");
        require(position == static_cast<int64_t>(i), "analysis positions must be contiguous");
        result.push_back({static_cast<int32_t>(integer_value(object_field(item, "byte_start"), "byte_start")),
                          static_cast<int32_t>(integer_value(object_field(item, "byte_len"), "byte_len")),
                          string_value(object_field(item, "term"), "analysis term")});
    }
    return result;
}

std::vector<std::string> query_terms(const Inspector& query) {
    const auto& analysis = object_field(query, "analysis");
    require(analysis.type() == vespalib::slime::ARRAY::instance && analysis.children() > 0,
            "text query must contain analysis terms");
    std::vector<std::string> terms;
    for (size_t i = 0; i < analysis.children(); ++i) {
        terms.push_back(string_value(analysis[i], "query analysis term must be string"));
    }
    return terms;
}

std::unique_ptr<document::StringFieldValue> annotated_string(std::string_view raw,
                                                              const std::vector<AnalyzedTerm>& terms,
                                                              const DocumentTypeRepo& repo,
                                                              const DocumentType& type) {
    auto value = std::make_unique<document::StringFieldValue>(raw);
    auto spans = std::make_unique<document::SpanList>();
    auto* spans_ptr = spans.get();
    spans_ptr->reserve(terms.size());
    auto tree = std::make_unique<document::SpanTree>(LINGUISTICS_TREE, std::move(spans));
    for (const auto& term : terms) {
        require(term.start >= 0 && term.length >= 0 && static_cast<size_t>(term.start) <= raw.size() &&
                    static_cast<size_t>(term.length) <= raw.size() - static_cast<size_t>(term.start),
                "analysis span outside text value");
        const auto original = raw.substr(static_cast<size_t>(term.start), static_cast<size_t>(term.length));
        auto& span = spans_ptr->add(std::make_unique<document::Span>(term.start, term.length));
        if (original == term.term) tree->annotate(span, *document::AnnotationType::TERM);
        else tree->annotate(span, document::Annotation(*document::AnnotationType::TERM,
                                                        std::make_unique<document::StringFieldValue>(term.term)));
    }
    document::FixedTypeRepo fixed_repo(repo, type);
    document::StringFieldValue::SpanTrees trees;
    trees.emplace_back(std::move(tree));
    value->setSpanTrees(trees, fixed_repo);
    return value;
}

std::string tls_spec() {
    return vespalib::SocketSpec::from_host_port("localhost", TLS_PORT).spec();
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

storage::spi::ClusterState cluster_state() {
    using namespace storage::lib;
    storage::lib::ClusterState state;
    state.setNodeState(Node(NodeType::STORAGE, 0), NodeState(NodeType::STORAGE, State::UP, "embedded", 1.0));
    state.setClusterState(State::UP);
    vespa::config::content::StorDistributionConfigBuilder cfg;
    cfg.redundancy = 1;
    cfg.readyCopies = 1;
    cfg.group.emplace_back();
    auto& group = cfg.group.back();
    group.index = "invalid";
    group.name = "embedded";
    group.capacity = 1.0;
    group.nodes.emplace_back();
    group.nodes.back().index = 0;
    return storage::spi::ClusterState(state, 0, Distribution(cfg));
}

class Fixture {
    proton::DummyWireService _wire;
    proton::DummyDBOwner _owner;
    vespalib::ThreadStackExecutor _summary_executor;
    proton::MockSharedThreadingService _shared;
    HwInfo _hw;
    DummyFileHeaderContext _header;
    TransLogServer _tls;
    proton::matching::QueryLimiter _limiter;
    DocumentType _type;
    std::shared_ptr<DocumentTypeRepo> _repo;
    DocumentDB::SP _db;
    PersistenceOwner _persistence_owner;
    AcceptAllWrites _write_filter;
    proton::test::ResourceUsageNotifier _resource_usage;
    PersistenceEngine _persistence;
    std::shared_ptr<PersistenceHandlerProxy> _handler;
    std::unordered_set<uint64_t> _prepared;
    std::vector<std::pair<document::GlobalId, std::string>> _ids;
    uint64_t _timestamp = 1;

public:
    Fixture(const std::filesystem::path& config_dir, const std::vector<FieldSpec>& specs)
        : _wire(), _owner(), _summary_executor(8), _shared(_summary_executor), _hw(), _header(),
          _tls(_shared.transport(), "tmp", TLS_PORT, ".", _header), _limiter(), _type(DOC_TYPE, 0),
          _repo(), _db(), _persistence_owner(), _write_filter(), _resource_usage(),
          _persistence(_persistence_owner, _write_filter, _resource_usage, -1, false), _handler() {
        int32_t field_id = 1;
        for (const auto& spec : specs) {
            if (spec.type == "integer") _type.addField(document::Field(spec.name, field_id++, *document::DataType::INT));
            else if (spec.type == "text") _type.addField(document::Field(spec.name, field_id++, *document::DataType::STRING));
            else fail("unsupported M8 fixture field type");
        }
        _repo = std::make_shared<DocumentTypeRepo>(_type);
        config::DirSpec dir(config_dir.string());
        DocumentDBConfigHelper config(dir, DOC_TYPE);
        auto document_types = std::make_shared<DocumenttypesConfig>();
        auto proton_config = std::make_shared<ProtonConfig>();
        auto distribution = std::make_shared<FiledistributorrpcConfig>();
        auto bucket_spaces = std::make_shared<BucketspacesConfig>();
        auto tune = std::make_shared<search::TuneFileDocumentDB>();
        auto bootstrap = std::make_shared<proton::BootstrapConfig>(1, document_types, _repo, proton_config,
                                                                     distribution, bucket_spaces, tune, _hw);
        config.forwardConfig(bootstrap);
        config.nextGeneration(_shared.transport(), 0ms);
        _db = DocumentDB::create(".", config.getConfig(), tls_spec(), _limiter, DocTypeName(DOC_TYPE),
                                 makeBucketSpace(), *proton_config, _owner, _shared, _tls, _wire, _header,
                                 std::make_shared<search::attribute::Interlock>(),
                                 std::make_unique<MemoryConfigStore>(),
                                 std::make_shared<vespalib::ThreadStackExecutor>(16), _hw,
                                 std::shared_ptr<search::diskindex::IPostingListCache>());
        _db->start();
        _db->waitForOnlineState();
        _handler = std::make_shared<PersistenceHandlerProxy>(_db);
        _persistence.putHandler(_persistence.getWLock(), makeBucketSpace(), DocTypeName(DOC_TYPE), _handler);
        require(!_persistence.initialize().hasError(), "persistence initialization failed");
        require(!_persistence.setClusterState(makeBucketSpace(), cluster_state()).hasError(), "cluster state failed");
    }

    ~Fixture() {
        std::cerr << "M8_ORACLE shutdown removeHandler begin\n" << std::flush;
        {
            auto removed = _persistence.removeHandler(_persistence.getWLock(), makeBucketSpace(), DocTypeName(DOC_TYPE));
            require(static_cast<bool>(removed), "persistence handler removal failed");
        }
        _handler.reset();
        std::cerr << "M8_ORACLE shutdown removeHandler returned\n" << std::flush;
        if (_db) {
            std::cerr << "M8_ORACLE shutdown DocumentDB close begin\n" << std::flush;
            _db->close();
            std::cerr << "M8_ORACLE shutdown DocumentDB close returned\n" << std::flush;
            _db.reset();
        }
        std::cerr << "M8_ORACLE shutdown transport begin\n" << std::flush;
        _shared.transport().ShutDown(true);
        std::cerr << "M8_ORACLE shutdown transport returned\n" << std::flush;
    }

    const DocumentType& type() const { return _type; }
    const DocumentTypeRepo& repo() const { return *_repo; }

    void prepare(const Bucket& bucket) {
        if (_prepared.insert(bucket.getBucketId().getId()).second) {
            require(!_persistence.createBucket(bucket).hasError(), "createBucket failed");
            require(!_persistence.setActiveState(bucket, BucketInfo::ACTIVE).hasError(), "setActiveState failed");
        }
    }

    std::shared_ptr<Document> make_document(const Inspector& document) const {
        const auto id = string_value(object_field(document, "id"), "document id");
        auto result = std::make_shared<Document>(*_repo, _type, DocumentId(id));
        const auto& values = object_field(document, "fields");
        require(values.type() == vespalib::slime::OBJECT::instance, "document fields must be an object");
        size_t known_fields = 0;
        for (const auto& spec : current_specs) {
            const auto& encoded = object_field(values, spec.name);
            if (!present(encoded)) continue;
            ++known_fields;
            const auto& field = _type.getField(spec.name);
            if (spec.type == "integer") result->setFieldValue(field, std::make_unique<document::IntFieldValue>(integer_value(object_field(encoded, "integer"), "integer value")));
            else {
                const auto raw = string_value(object_field(encoded, "text"), "text value");
                result->setFieldValue(field, annotated_string(raw, analysis_from_value(encoded), *_repo, _type));
            }
        }
        require(known_fields == values.fields(), "unknown document field");
        return result;
    }

    void set_specs(const std::vector<FieldSpec>* specs) { current_specs = *specs; }

    void put(const Inspector& document) {
        auto doc = make_document(document);
        const auto id = doc->getId();
        auto bucket = proton::BucketFactory::getBucket(id);
        prepare(bucket);
        _ids.emplace_back(id.getGlobalId(), id.toString());
        auto result = _persistence.put(bucket, Timestamp(_timestamp++), std::move(doc));
        require(!result.hasError(), "put failed");
    }

    std::string id_for_gid(const document::GlobalId& gid) const {
        const auto found = std::find_if(_ids.begin(), _ids.end(), [&](const auto& item) { return item.first == gid; });
        require(found != _ids.end(), "unknown Vespa global id");
        return found->second;
    }

    SearchReply::UP search(const Inspector& request) const {
        auto native = std::make_shared<SearchRequest>();
        native->setTimeout(60s);
        native->ranking = present(object_field(request, "rank_profile")) ? string_value(object_field(request, "rank_profile"), "rank_profile") : "default";
        auto &rank = native->propertiesMap.lookupCreate(search::MapNames::RANK);
        rank.add("vespa.matching.numthreadspersearch", "1");
        const auto &rank_properties = object_field(request, "rank_properties");
        if (present(rank_properties)) {
            require(rank_properties.type() == vespalib::slime::ARRAY::instance,
                    "rank_properties must be an array");
            for (size_t i = 0; i < rank_properties.children(); ++i) {
                const auto &item = rank_properties[i];
                rank.add(string_value(object_field(item, "name"), "rank property name"),
                         string_value(object_field(item, "value"), "rank property value"));
            }
        }
        native->maxhits = present(object_field(request, "limit")) ? nonnegative_u32(object_field(request, "limit"), "limit") : 10;
        native->offset = present(object_field(request, "offset")) ? nonnegative_u32(object_field(request, "offset"), "offset") : 0;
        native->sortSpec = sort_spec(request);
        native->setSerializedQueryTree(SerializedQueryTree::fromStackDump(query_dump(object_field(request, "query"))));
        std::cerr << "M8_ORACLE match begin\n" << std::flush;
        auto result = _db->match(*native, vespalib::ThreadBundle::trivial());
        std::cerr << "M8_ORACLE match returned\n" << std::flush;
        require(static_cast<bool>(result), "DocumentDB::match returned null");
        return result;
    }

    std::unique_ptr<DocsumReply> docsums(const SearchReply& reply, const Inspector& request,
                                         const std::vector<FieldSpec>& specs) const {
        DocsumRequest native;
        native.resultClassName = "default";
        native.ranking = "default";
        native.setTimeout(60s);
        for (const auto& hit : reply.hits) native.hits.emplace_back(hit.gid);
        const auto& summary = object_field(request, "summary");
        if (summary.type() == vespalib::slime::OBJECT::instance) {
            const auto& fields = object_field(summary, "fields");
            require(fields.type() == vespalib::slime::ARRAY::instance, "summary.fields must be an array");
            std::vector<std::string> names;
            for (size_t i = 0; i < fields.children(); ++i) {
                const auto name = string_value(fields[i], "summary field");
                const auto found = std::find_if(specs.begin(), specs.end(), [&](const auto& spec) { return spec.name == name; });
                require(found != specs.end() && found->summary, "unknown or non-summary field");
                names.push_back(name);
            }
            native.setFields(std::move(names));
        } else if (summary.type() == vespalib::slime::STRING::instance && string_value(summary, "summary") == "none") {
            fail("docsums requested for summary none");
        }
        std::cerr << "M8_ORACLE docsums begin\n" << std::flush;
        auto result = _db->getDocsums(native);
        std::cerr << "M8_ORACLE docsums returned\n" << std::flush;
        return result;
    }

private:
    std::vector<FieldSpec> current_specs;

    std::string sort_spec(const Inspector& request) const {
        const auto& sort = object_field(request, "sort");
        if (!present(sort)) return {};
        require(sort.type() == vespalib::slime::ARRAY::instance, "sort must be an array");
        std::string result;
        for (size_t i = 0; i < sort.children(); ++i) {
            const auto& item = sort[i];
            const auto field = string_value(object_field(item, "field"), "sort field");
            const auto order = string_value(object_field(item, "order"), "sort order");
            require(order == "ascending" || order == "descending", "unknown sort order");
            const auto found = std::find_if(current_specs.begin(), current_specs.end(), [&](const auto& spec) { return spec.name == field; });
            require(found != current_specs.end(), "unknown sort field");
            require(found->attribute && found->type == "integer", "sort field is not an integer attribute");
            if (!result.empty()) result.push_back(' ');
            result.push_back(order == "ascending" ? '+' : '-');
            result += field;
        }
        return result;
    }

    static void add_query(QueryBuilder<proton::matching::ProtonNodeTypes>& builder,
                          const Inspector& query) {
        require(query.type() == vespalib::slime::OBJECT::instance && query.fields() == 1,
                "query must have one operation");
        const auto& conjunction = object_field(query, "and");
        if (present(conjunction)) {
            require(conjunction.type() == vespalib::slime::ARRAY::instance && conjunction.children() > 0,
                    "and query must contain children");
            builder.addAnd(conjunction.children());
            for (size_t i = 0; i < conjunction.children(); ++i) add_query(builder, conjunction[i]);
            return;
        }
        const auto& exists = object_field(query, "exists");
        if (present(exists)) {
            builder.addRangeTerm(Range("[;]"),
                                 string_value(object_field(exists, "field"), "exists field"), 1, Weight(1));
        } else {
            const auto& range = object_field(query, "range");
            if (present(range)) {
                const auto field = string_value(object_field(range, "field"), "range field");
                const auto& lb = object_field(range, "lower");
                const auto& ub = object_field(range, "upper");
                const auto parse_bound = [](const Inspector& bound, bool lower) {
                    require(bound.type() == vespalib::slime::OBJECT::instance, "range bound must be an object");
                    const auto& unbounded = object_field(bound, "unbounded");
                    if (present(unbounded)) {
                        require(bool_value(unbounded, "unbounded must be bool"), "invalid unbounded bound");
                        return std::pair<char, std::string>(lower ? '[' : ']', {});
                    }
                    const auto& included = object_field(bound, "included");
                    const auto& excluded = object_field(bound, "excluded");
                    require(present(included) != present(excluded), "range bound must be included, excluded, or unbounded");
                    const auto& value = present(included) ? included : excluded;
                    const auto number = std::to_string(integer_value(value, "range bound"));
                    return std::pair<char, std::string>(present(included) ? (lower ? '[' : ']') : (lower ? '<' : '>'), number);
                };
                const auto lower = parse_bound(lb, true);
                const auto upper = parse_bound(ub, false);
                const std::string range_spec = std::string(1, lower.first) + lower.second + ";" + upper.second + std::string(1, upper.first);
                builder.addRangeTerm(Range(range_spec), field, 1, Weight(1));
            } else {
                const auto& term = object_field(query, "term");
                require(present(term), "unknown M8 query operation");
                const auto field = string_value(object_field(term, "field"), "term field");
                const auto terms = query_terms(term);
                require(terms.size() == 1, "M8 term query must contain one analyzed term");
                builder.addStringTerm(terms.front(), field, 1, Weight(1));
            }
        }
    }

    static std::string query_dump(const Inspector& query) {
        QueryBuilder<proton::matching::ProtonNodeTypes> builder;
        add_query(builder, query);
        auto tree = builder.build();
        require(static_cast<bool>(tree), builder.error());
        return StackDumpCreator::create(*tree);
    }
};

void emit_summary_value(const Inspector& value) {
    if (value.type() == vespalib::slime::LONG::instance) {
        std::cout << "{\"type\":\"integer\",\"value\":" << value.asLong() << '}';
    } else if (value.type() == vespalib::slime::DOUBLE::instance) {
        std::cout << "{\"type\":\"float\",\"bits\":\"" << double_bits(value.asDouble()) << "\"}";
    } else if (value.type() == vespalib::slime::STRING::instance) {
        std::cout << "{\"type\":\"string\",\"value\":" << json_escape(value.asString().make_string()) << '}';
    } else if (value.type() == vespalib::slime::BOOL::instance) {
        std::cout << "{\"type\":\"boolean\",\"value\":" << (value.asBool() ? "true" : "false") << '}';
    } else {
        fail("unsupported docsum value type");
    }
}

void emit_result(const std::string& id, const SearchReply& reply, const Fixture& fixture,
                 const Inspector& request, const std::vector<FieldSpec>& specs) {
    std::unique_ptr<DocsumReply> sums;
    const auto& summary = object_field(request, "summary");
    const bool need_summary = summary.type() == vespalib::slime::OBJECT::instance ||
                              (summary.type() == vespalib::slime::STRING::instance && string_value(summary, "summary") != "none");
    if (need_summary) sums = fixture.docsums(reply, request, specs);
    std::cout << "{\"id\":" << json_escape(id) << ",\"total_hit_count\":" << reply.totalHitCount << ",\"hits\":[";
    for (size_t i = 0; i < reply.hits.size(); ++i) {
        if (i) std::cout << ',';
        const auto& hit = reply.hits[i];
        std::cout << "{\"document_id\":" << json_escape(fixture.id_for_gid(hit.gid))
                  << ",\"score_bits\":\"" << double_bits(hit.metric) << "\",\"summary\":{";
        bool first = true;
        if (sums) {
            const auto& docsum = sums->root()["docsums"][i]["docsum"];
            if (docsum.valid()) {
                const auto emit_field = [&](const std::string& field) {
                    const auto& value = docsum[Memory(field)];
                    if (!value.valid()) return;
                    if (!first) std::cout << ',';
                    first = false;
                    std::cout << json_escape(field) << ':';
                    emit_summary_value(value);
                };
                if (summary.type() == vespalib::slime::OBJECT::instance) {
                    const auto& requested = object_field(summary, "fields");
                    for (size_t n = 0; n < requested.children(); ++n) emit_field(string_value(requested[n], "summary field"));
                } else {
                    for (const auto& field : specs) if (field.summary) emit_field(field.name);
                }
            }
        }
        std::cout << "}}";
    }
    std::cout << "]}\n" << std::flush;
}

std::string required_env(const char* name) {
    const auto* value = std::getenv(name);
    require(value && *value, std::string("missing environment variable ") + name);
    return value;
}

int run(const std::filesystem::path& scenario_path) {
    auto scenario = read_json(scenario_path);
    const auto& root = scenario.get();
    const auto id = string_value(object_field(root, "id"), "scenario id");
    const auto specs = fields_from_schema(root);
    const bool text_fixture = std::any_of(specs.begin(), specs.end(), [](const auto& field) { return field.type == "text"; });
    const auto config = std::filesystem::path(required_env("M8_ORACLE_FIXTURE_ROOT")) /
                        (text_fixture ? "text" : "attribute");
    const auto fixture_schema = read_json(config / "searchkernel-schema.json");
    const auto engine_specs = fields_from_fixture_schema(fixture_schema.get());
    DummyFileHeaderContext::setCreator("searchkernel_m8_cpp_oracle");
    std::filesystem::remove_all("typea");
    std::filesystem::remove_all("tmp");
    std::filesystem::create_directory("typea");
    Fixture fixture(config, engine_specs);
    fixture.set_specs(&specs);
    const auto& documents = object_field(root, "documents");
    require(documents.type() == vespalib::slime::ARRAY::instance, "documents must be an array");
    for (size_t i = 0; i < documents.children(); ++i) fixture.put(documents[i]);
    const auto& request = object_field(root, "request");
    auto reply = fixture.search(request);
    emit_result(id, *reply, fixture, request, specs);
    std::cerr << "M8_ORACLE result emitted\n" << std::flush;
    return 0;
}

} // namespace

int main(int argc, char** argv) {
    try {
        require(argc == 2, "usage: m8_local_oracle SCENARIO_JSON");
        return run(argv[1]);
    } catch (const std::exception& error) {
        std::cerr << "m8_local_oracle: " << error.what() << '\n';
        std::filesystem::remove_all("typea");
        std::filesystem::remove_all("tmp");
        return 2;
    }
}
