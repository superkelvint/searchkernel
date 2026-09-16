// Copyright Vespa.ai. Licensed under the terms of the Apache 2.0 license.
//
// Direct M9 oracle.  All matching, sorting, ranking, mutation, grouping, and
// aggregation in this program is performed by the pinned Vespa engine.

#include <vespa/config-bucketspaces.h>
#include <vespa/config-stor-distribution.h>
#include <vespa/config/subscription/sourcespec.h>
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
#include <vespa/document/update/assignvalueupdate.h>
#include <vespa/document/update/documentupdate.h>
#include <vespa/document/update/fieldupdate.h>
#include <vespa/fnet/transport.h>
#include <vespa/searchcore/proton/matching/querylimiter.h>
#include <vespa/searchcore/proton/matching/querynodes.h>
#include <vespa/searchcore/proton/metrics/dummy_wire_service.h>
#include <vespa/searchcore/proton/persistenceengine/i_resource_write_filter.h>
#include <vespa/searchcore/proton/persistenceengine/ipersistenceengineowner.h>
#include <vespa/searchcore/proton/persistenceengine/persistenceengine.h>
#include <vespa/searchcore/proton/server/documentdb.h>
#include <vespa/searchcore/proton/server/documentdbconfigmanager.h>
#include <vespa/searchcore/proton/server/memoryconfigstore.h>
#include <vespa/searchcore/proton/server/persistencehandlerproxy.h>
#include <vespa/searchcore/proton/server/bootstrapconfig.h>
#include <vespa/searchcore/proton/test/bucketfactory.h>
#include <vespa/searchcore/proton/test/dummydbowner.h>
#include <vespa/searchcore/proton/test/mock_shared_threading_service.h>
#include <vespa/searchcore/proton/test/port_numbers.h>
#include <vespa/searchcore/proton/test/resource_usage_notifier.h>
#include <vespa/searchlib/attribute/interlock.h>
#include <vespa/searchlib/aggregation/averageaggregationresult.h>
#include <vespa/searchlib/aggregation/countaggregationresult.h>
#include <vespa/searchlib/aggregation/group.h>
#include <vespa/searchlib/aggregation/grouping.h>
#include <vespa/searchlib/aggregation/groupinglevel.h>
#include <vespa/searchlib/aggregation/maxaggregationresult.h>
#include <vespa/searchlib/aggregation/minaggregationresult.h>
#include <vespa/searchlib/aggregation/sumaggregationresult.h>
#include <vespa/searchlib/engine/docsumreply.h>
#include <vespa/searchlib/engine/docsumrequest.h>
#include <vespa/searchlib/engine/searchreply.h>
#include <vespa/searchlib/engine/searchrequest.h>
#include <vespa/searchlib/expression/attributenode.h>
#include <vespa/searchlib/expression/constantnode.h>
#include <vespa/searchlib/expression/floatresultnode.h>
#include <vespa/searchlib/expression/resultnode.h>
#include <vespa/searchlib/expression/stringresultnode.h>
#include <vespa/searchlib/expression/integerresultnode.h>
#include <vespa/searchlib/index/dummyfileheadercontext.h>
#include <vespa/searchlib/query/tree/querybuilder.h>
#include <vespa/searchlib/query/tree/range.h>
#include <vespa/searchlib/query/tree/stackdumpcreator.h>
#include <vespa/searchlib/transactionlog/translogserver.h>
#include <vespa/vespalib/data/slime/json_format.h>
#include <vespa/vespalib/data/slime/slime.h>
#include <vespa/vespalib/net/socket_spec.h>
#include <vespa/vespalib/objects/nbostream.h>
#include <vespa/vespalib/objects/nboserializer.h>
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
#include <locale>
#include <memory>
#include <sstream>
#include <stdexcept>
#include <string>
#include <string_view>
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
using search::SerializedQueryTree;
using search::aggregation::AggregationResult;
using search::aggregation::AverageAggregationResult;
using search::aggregation::CountAggregationResult;
using search::aggregation::Group;
using search::aggregation::Grouping;
using search::aggregation::GroupingLevel;
using search::aggregation::MaxAggregationResult;
using search::aggregation::MinAggregationResult;
using search::aggregation::SumAggregationResult;
using search::engine::DocsumReply;
using search::engine::DocsumRequest;
using search::engine::SearchReply;
using search::engine::SearchRequest;
using search::expression::AttributeNode;
using search::expression::FloatResultNode;
using search::expression::HoldString;
using search::expression::Int64ResultNode;
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
using vespalib::Memory;
using vespalib::Slime;
using vespalib::slime::Inspector;
using vespalib::slime::JsonFormat;
using vespa::config::content::core::BucketspacesConfig;
using vespa::config::search::core::ProtonConfig;
using cloud::config::filedistribution::FiledistributorrpcConfig;

namespace {

constexpr int TLS_PORT = proton::test::port_numbers::documentdb_tls_port;
constexpr const char* DOC_TYPE = "typea";

[[noreturn]] void fail(std::string_view message) { throw std::runtime_error(std::string(message)); }
void require(bool condition, std::string_view message) { if (!condition) fail(message); }

const Inspector& field(const Inspector& object, std::string_view name) {
    require(object.type() == vespalib::slime::OBJECT::instance, "expected JSON object");
    return object[Memory(name)];
}
bool present(const Inspector& value) { return value.valid() && value.type() != vespalib::slime::NIX::instance; }
std::string string_value(const Inspector& value, std::string_view what) {
    require(value.type() == vespalib::slime::STRING::instance, what);
    return value.asString().make_string();
}
int64_t integer_value(const Inspector& value, std::string_view what) {
    require(value.type() == vespalib::slime::LONG::instance, what);
    return value.asLong();
}
uint32_t nonnegative_u32(const Inspector& value, std::string_view what) {
    const auto number = integer_value(value, what);
    require(number >= 0 && static_cast<uint64_t>(number) <= std::numeric_limits<uint32_t>::max(), what);
    return static_cast<uint32_t>(number);
}
double number_value(const Inspector& value, std::string_view what) {
    require(value.type() == vespalib::slime::DOUBLE::instance || value.type() == vespalib::slime::LONG::instance, what);
    const double result = value.type() == vespalib::slime::DOUBLE::instance ? value.asDouble() : value.asLong();
    require(std::isfinite(result), "non-finite JSON number");
    return result;
}
bool bool_value(const Inspector& value, std::string_view what) {
    require(value.type() == vespalib::slime::BOOL::instance, what);
    return value.asBool();
}

std::string read_file(const std::filesystem::path& path) {
    std::ifstream input(path, std::ios::binary);
    require(input.good(), "could not open scenario");
    return {std::istreambuf_iterator<char>(input), std::istreambuf_iterator<char>()};
}
Slime read_json(const std::filesystem::path& path) {
    const auto bytes = read_file(path);
    Slime result;
    const auto used = JsonFormat::decode(Memory(bytes), result);
    require(used <= bytes.size() && std::all_of(bytes.begin() + static_cast<std::ptrdiff_t>(used), bytes.end(),
                                                [](unsigned char c) { return std::isspace(c) != 0; }),
            "malformed or trailing scenario JSON");
    return result;
}
std::string json_escape(std::string_view value) {
    std::ostringstream out;
    out << '"';
    for (const unsigned char c : value) {
        if (c == '"') out << "\\\"";
        else if (c == '\\') out << "\\\\";
        else if (c == '\n') out << "\\n";
        else if (c == '\r') out << "\\r";
        else if (c == '\t') out << "\\t";
        else if (c < 0x20) out << "\\u" << std::hex << std::setw(4) << std::setfill('0') << int(c) << std::dec;
        else out << static_cast<char>(c);
    }
    out << '"';
    return out.str();
}
std::string hex64(uint64_t value) {
    std::ostringstream out;
    out << "0x" << std::hex << std::setw(16) << std::setfill('0') << value;
    return out.str();
}
std::string bits(double value) { return hex64(std::bit_cast<uint64_t>(value)); }

struct FieldSpec { std::string name; std::string type; bool attribute; bool summary; };
struct Value {
    std::string type;
    std::string string;
    int64_t integer = 0;
    double floating = 0.0;
    bool boolean = false;
};
struct AggregateSpec { std::string op; std::string field; };

std::vector<FieldSpec> schema(const Inspector& root) {
    const auto& fields = field(field(root, "schema"), "fields");
    require(fields.type() == vespalib::slime::ARRAY::instance, "schema.fields must be an array");
    std::vector<FieldSpec> result;
    for (size_t i = 0; i < fields.children(); ++i) {
        const auto& item = fields[i];
        result.push_back({string_value(field(item, "name"), "field name"),
                          string_value(field(item, "type"), "field type"),
                          bool_value(field(item, "attribute"), "attribute must be bool"),
                          bool_value(field(item, "summary"), "summary must be bool")});
    }
    require(result.size() == 4, "M9 schema must contain four fields");
    return result;
}

const FieldSpec& spec_for(const std::vector<FieldSpec>& specs, std::string_view name) {
    const auto found = std::find_if(specs.begin(), specs.end(), [&](const auto& item) { return item.name == name; });
    require(found != specs.end(), "unknown field");
    return *found;
}

Value parse_value(const Inspector& encoded) {
    require(encoded.type() == vespalib::slime::OBJECT::instance && encoded.fields() == 1, "value must have one type");
    Value result;
    const auto& string = field(encoded, "string");
    const auto& integer = field(encoded, "integer");
    const auto& floating = field(encoded, "float");
    const auto& boolean = field(encoded, "boolean");
    if (present(string)) { result.type = "string"; result.string = string_value(string, "string value"); }
    else if (present(integer)) { result.type = "integer"; result.integer = integer_value(integer, "integer value"); }
    else if (present(floating)) { result.type = "float"; result.floating = number_value(floating, "float value"); }
    else if (present(boolean)) { result.type = "boolean"; result.boolean = bool_value(boolean, "boolean value"); }
    else fail("unknown value type");
    return result;
}

std::string number_string(const Value& value) {
    std::ostringstream out;
    out.imbue(std::locale::classic());
    out << std::setprecision(std::numeric_limits<double>::max_digits10) << value.floating;
    return out.str();
}
std::string value_string(const Value& value) {
    if (value.type == "integer") return std::to_string(value.integer);
    if (value.type == "float") return number_string(value);
    if (value.type == "boolean") return value.boolean ? "1" : "0";
    return value.string;
}

storage::spi::ClusterState cluster_state() {
    using namespace storage::lib;
    storage::lib::ClusterState state;
    state.setNodeState(Node(NodeType::STORAGE, 0), NodeState(NodeType::STORAGE, State::UP, "embedded", 1.0));
    state.setClusterState(State::UP);
    vespa::config::content::StorDistributionConfigBuilder cfg;
    cfg.redundancy = 1; cfg.readyCopies = 1; cfg.group.emplace_back();
    auto& group = cfg.group.back(); group.index = "invalid"; group.name = "embedded"; group.capacity = 1.0;
    group.nodes.emplace_back(); group.nodes.back().index = 0;
    return storage::spi::ClusterState(state, 0, Distribution(cfg));
}

class PersistenceOwner final : public proton::IPersistenceEngineOwner {
public: void setClusterState(document::BucketSpace, const storage::spi::ClusterState&) override {}
};
class AcceptAllWrites final : public proton::IResourceWriteFilter {
public:
    bool acceptWriteOperation() const override { return true; }
    State getAcceptState() const override { return State(true, ""); }
};

class Fixture {
    proton::DummyWireService _wire;
    proton::DummyDBOwner _owner;
    vespalib::ThreadStackExecutor _summary_executor;
    proton::MockSharedThreadingService _shared;
    HwInfo _hw;
    search::index::DummyFileHeaderContext _header;
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
    std::vector<FieldSpec> _specs;

    const FieldSpec& checked(std::string_view name, std::string_view type = {}) const {
        const auto& result = spec_for(_specs, name);
        require(result.attribute, "field is not an attribute");
        if (!type.empty()) require(result.type == type, "wrong field type");
        return result;
    }
    std::unique_ptr<document::FieldValue> field_value(const FieldSpec& spec, const Value& value) const {
        require(spec.attribute, "field is not an attribute");
        if (spec.type == "string") { require(value.type == "string", "wrong value type"); return std::make_unique<document::StringFieldValue>(value.string); }
        if (spec.type == "integer") { require(value.type == "integer", "wrong value type"); return std::make_unique<document::IntFieldValue>(value.integer); }
        if (spec.type == "float") { require(value.type == "float", "wrong value type"); return std::make_unique<document::DoubleFieldValue>(value.floating); }
        if (spec.type == "boolean") { require(value.type == "boolean", "wrong value type"); return std::make_unique<document::BoolFieldValue>(value.boolean); }
        fail("unsupported field type");
    }
    std::shared_ptr<Document> make_document(const Inspector& source) const {
        const auto id = string_value(field(source, "id"), "document id");
        auto result = std::make_shared<Document>(*_repo, _type, DocumentId(id));
        const auto& values = field(source, "fields");
        require(values.type() == vespalib::slime::OBJECT::instance, "document fields must be an object");
        size_t known_fields = 0;
        for (const auto& spec : _specs) {
            const auto& encoded = field(values, spec.name);
            if (present(encoded)) {
                ++known_fields;
                result->setFieldValue(_type.getField(spec.name), field_value(spec, parse_value(encoded)));
            }
        }
        require(known_fields == values.fields(), "unknown document field");
        return result;
    }
    std::shared_ptr<document::DocumentUpdate> make_update(const std::string& id, const Inspector& assignments) const {
        require(assignments.type() == vespalib::slime::OBJECT::instance && assignments.fields() > 0, "empty update");
        auto result = std::make_shared<document::DocumentUpdate>(*_repo, _type, DocumentId(id));
        size_t update_count = 0;
        for (const auto& spec : _specs) {
            const auto& encoded = field(assignments, spec.name);
            if (!present(encoded)) continue;
            document::FieldUpdate update(_type.getField(spec.name));
            update.addUpdate(std::make_unique<document::AssignValueUpdate>(field_value(spec, parse_value(encoded))));
            result->addUpdate(std::move(update));
            ++update_count;
        }
        require(update_count > 0 && update_count == assignments.fields(), "unknown update field");
        return result;
    }
    void prepare(const Bucket& bucket) {
        if (_prepared.insert(bucket.getBucketId().getId()).second) {
            require(!_persistence.createBucket(bucket).hasError(), "createBucket failed");
            require(!_persistence.setActiveState(bucket, BucketInfo::ACTIVE).hasError(), "setActiveState failed");
        }
    }
    std::string sort_spec(const Inspector& request) const {
        const auto& sort = field(request, "sort");
        require(sort.type() == vespalib::slime::ARRAY::instance, "sort must be an array");
        std::string result;
        for (size_t i = 0; i < sort.children(); ++i) {
            const auto name = string_value(field(sort[i], "field"), "sort field");
            const auto order = string_value(field(sort[i], "order"), "sort order");
            require(order == "ascending" || order == "descending", "unknown sort order");
            const auto& spec = checked(name);
            require(spec.type == "string" || spec.type == "integer" || spec.type == "float", "unsupported sort field type");
            if (!result.empty()) result.push_back(' ');
            result += order == "ascending" ? "+" : "-"; result += name;
        }
        return result;
    }
    static std::string range_string(const Inspector& range, const FieldSpec& spec) {
        const auto bound = [&](const Inspector& wrapper, bool lower) {
            require(wrapper.type() == vespalib::slime::OBJECT::instance, "range bound must be object");
            const auto& unbounded = field(wrapper, "unbounded");
            if (present(unbounded)) { require(bool_value(unbounded, "unbounded must be bool"), "invalid unbounded bound"); return std::pair<char, std::string>(lower ? '[' : ']', {}); }
            const auto& included = field(wrapper, "included");
            const auto& excluded = field(wrapper, "excluded");
            require(present(included) != present(excluded), "range bound must be included, excluded, or unbounded");
            const auto value = parse_value(present(included) ? included : excluded);
            require(value.type == spec.type, "wrong range bound type");
            return std::pair<char, std::string>(present(included) ? (lower ? '[' : ']') : (lower ? '<' : '>'), value_string(value));
        };
        const auto low = bound(field(range, "lower"), true);
        const auto high = bound(field(range, "upper"), false);
        return std::string(1, low.first) + low.second + ";" + high.second + std::string(1, high.first);
    }
    void add_query(QueryBuilder<proton::matching::ProtonNodeTypes>& builder, const Inspector& query) const {
        require(query.type() == vespalib::slime::OBJECT::instance && query.fields() == 1, "query must have one operation");
        const auto& conjunction = field(query, "and");
        if (present(conjunction)) {
            require(conjunction.type() == vespalib::slime::ARRAY::instance && conjunction.children() > 0,
                    "and query must contain children");
            builder.addAnd(conjunction.children());
            for (size_t i = 0; i < conjunction.children(); ++i) add_query(builder, conjunction[i]);
            return;
        }
        const auto& exists = field(query, "exists");
        if (present(exists)) {
            checked(string_value(field(exists, "field"), "exists field"), "integer");
            builder.addRangeTerm(Range("[;]"),
                                 string_value(field(exists, "field"), "exists field"), 1, Weight(1));
        } else {
            const auto& equals = field(query, "attribute_equals");
            if (present(equals)) {
                const auto name = string_value(field(equals, "field"), "equals field");
                const auto& spec = checked(name);
                const auto value = parse_value(field(equals, "value"));
                require(value.type == spec.type, "wrong equality value type");
                if (spec.type == "string") builder.addStringTerm(value.string, name, 1, Weight(1));
                else builder.addNumberTerm(value_string(value), name, 1, Weight(1));
            } else {
                const auto& range = field(query, "attribute_range");
                require(present(range), "unknown query operation");
                const auto name = string_value(field(range, "field"), "range field");
                const auto& spec = checked(name);
                require(spec.type == "integer" || spec.type == "float", "range requires numeric field");
                builder.addRangeTerm(Range(range_string(range, spec)), name, 1, Weight(1));
            }
        }
    }

    std::string query_dump(const Inspector& query) const {
        QueryBuilder<proton::matching::ProtonNodeTypes> builder;
        add_query(builder, query);
        auto tree = builder.build();
        require(static_cast<bool>(tree), builder.error());
        return StackDumpCreator::create(*tree);
    }
public:
    Fixture(const std::filesystem::path& config_dir, std::vector<FieldSpec> specs)
        : _wire(), _owner(), _summary_executor(8), _shared(_summary_executor), _hw(), _header(),
          _tls(_shared.transport(), "tmp", TLS_PORT, ".", _header), _limiter(), _type(DOC_TYPE, 0),
          _repo(), _db(), _persistence_owner(), _write_filter(), _resource_usage(),
          _persistence(_persistence_owner, _write_filter, _resource_usage, -1, false), _handler(), _specs(std::move(specs)) {
        int32_t id = 1;
        for (const auto& spec : _specs) {
            if (spec.type == "string") _type.addField(document::Field(spec.name, id++, *document::DataType::STRING));
            else if (spec.type == "integer") _type.addField(document::Field(spec.name, id++, *document::DataType::INT));
            else if (spec.type == "float") _type.addField(document::Field(spec.name, id++, *document::DataType::DOUBLE));
            else if (spec.type == "boolean") _type.addField(document::Field(spec.name, id++, *document::DataType::BOOL));
            else fail("unsupported M9 field type");
        }
        _repo = std::make_shared<DocumentTypeRepo>(_type);
        config::DirSpec dir(config_dir.string()); DocumentDBConfigHelper config(dir, DOC_TYPE);
        auto document_types = std::make_shared<DocumenttypesConfig>();
        auto proton_config = std::make_shared<ProtonConfig>();
        auto distribution = std::make_shared<FiledistributorrpcConfig>();
        auto bucket_spaces = std::make_shared<BucketspacesConfig>();
        auto tune = std::make_shared<search::TuneFileDocumentDB>();
        auto bootstrap = std::make_shared<proton::BootstrapConfig>(1, document_types, _repo, proton_config, distribution, bucket_spaces, tune, _hw);
        config.forwardConfig(bootstrap); config.nextGeneration(_shared.transport(), 0ms);
        _db = DocumentDB::create(".", config.getConfig(), vespalib::SocketSpec::from_host_port("localhost", TLS_PORT).spec(), _limiter,
                                 DocTypeName(DOC_TYPE), makeBucketSpace(), *proton_config, _owner, _shared, _tls, _wire, _header,
                                 std::make_shared<search::attribute::Interlock>(), std::make_unique<MemoryConfigStore>(),
                                 std::make_shared<vespalib::ThreadStackExecutor>(16), _hw, std::shared_ptr<search::diskindex::IPostingListCache>());
        _db->start(); _db->waitForOnlineState();
        _handler = std::make_shared<PersistenceHandlerProxy>(_db);
        _persistence.putHandler(_persistence.getWLock(), makeBucketSpace(), DocTypeName(DOC_TYPE), _handler);
        require(!_persistence.initialize().hasError(), "persistence initialization failed");
        require(!_persistence.setClusterState(makeBucketSpace(), cluster_state()).hasError(), "cluster state failed");
    }
    ~Fixture() {
        std::cerr << "M9_ORACLE shutdown removeHandler begin\n" << std::flush;
        {
            auto removed = _persistence.removeHandler(_persistence.getWLock(), makeBucketSpace(), DocTypeName(DOC_TYPE));
            require(static_cast<bool>(removed), "persistence handler removal failed");
        }
        _handler.reset();
        std::cerr << "M9_ORACLE shutdown removeHandler returned\n" << std::flush;
        if (_db) {
            std::cerr << "M9_ORACLE shutdown DocumentDB close begin\n" << std::flush;
            _db->close();
            std::cerr << "M9_ORACLE shutdown DocumentDB close returned\n" << std::flush;
            _db.reset();
        }
        std::cerr << "M9_ORACLE shutdown transport begin\n" << std::flush;
        _shared.transport().ShutDown(true);
        std::cerr << "M9_ORACLE shutdown transport returned\n" << std::flush;
    }
    void put(const Inspector& source) {
        auto doc = make_document(source); const auto id = doc->getId(); const auto bucket = proton::BucketFactory::getBucket(id); prepare(bucket);
        _ids.emplace_back(id.getGlobalId(), id.toString());
        require(!_persistence.put(bucket, Timestamp(_timestamp++), std::move(doc)).hasError(), "put failed");
    }
    void update(const std::string& id, const Inspector& assignments) {
        const DocumentId doc_id(id); const auto bucket = proton::BucketFactory::getBucket(doc_id); prepare(bucket);
        require(!_persistence.update(bucket, Timestamp(_timestamp++), make_update(id, assignments)).hasError(), "update failed");
    }
    std::string id_for_gid(const document::GlobalId& gid) const {
        const auto found = std::find_if(_ids.begin(), _ids.end(), [&](const auto& item) { return item.first == gid; });
        require(found != _ids.end(), "unknown Vespa global id"); return found->second;
    }
    SearchReply::UP search(const Inspector& request) const {
        auto native = std::make_shared<SearchRequest>(); native->setTimeout(60s);
        native->ranking = present(field(request, "rank_profile")) ? string_value(field(request, "rank_profile"), "rank profile") : "default";
        native->propertiesMap.lookupCreate(search::MapNames::RANK).add("vespa.matching.numthreadspersearch", "1");
        native->maxhits = present(field(request, "limit")) ? nonnegative_u32(field(request, "limit"), "limit") : 10;
        native->offset = present(field(request, "offset")) ? nonnegative_u32(field(request, "offset"), "offset") : 0;
        native->sortSpec = sort_spec(request);
        native->setSerializedQueryTree(SerializedQueryTree::fromStackDump(query_dump(field(request, "query"))));
        return match(*native);
    }
    SearchReply::UP match(SearchRequest& request) const {
        std::cerr << "M9_ORACLE match begin\n" << std::flush;
        auto result = _db->match(request, vespalib::ThreadBundle::trivial());
        std::cerr << "M9_ORACLE match returned\n" << std::flush;
        require(static_cast<bool>(result), "match returned null");
        return result;
    }
    std::unique_ptr<DocsumReply> docsums(const SearchReply& reply, const Inspector& request) const {
        DocsumRequest native; native.resultClassName = "default"; native.ranking = "default";
        for (const auto& hit : reply.hits) native.hits.emplace_back(hit.gid);
        const auto& summary = field(request, "summary");
        if (summary.type() == vespalib::slime::OBJECT::instance) {
            const auto& requested = field(summary, "fields"); std::vector<std::string> names;
            require(requested.type() == vespalib::slime::ARRAY::instance, "summary.fields must be an array");
            for (size_t i = 0; i < requested.children(); ++i) {
                const auto name = string_value(requested[i], "summary field");
                const auto& spec = spec_for(_specs, name);
                require(spec.summary, "field is not a summary field");
                names.push_back(name);
            }
            native.setFields(std::move(names));
        }
        return _db->getDocsums(native);
    }
    const std::vector<FieldSpec>& specs() const { return _specs; }
    const DocumentType& type() const { return _type; }
    std::string query_dump_for(const Inspector& query) const { return query_dump(query); }
};

void emit_summary(const Inspector& value) {
    if (value.type() == vespalib::slime::LONG::instance) std::cout << "{\"type\":\"integer\",\"value\":" << value.asLong() << '}';
    else if (value.type() == vespalib::slime::DOUBLE::instance) std::cout << "{\"type\":\"float\",\"bits\":\"" << bits(value.asDouble()) << "\"}";
    else if (value.type() == vespalib::slime::STRING::instance) std::cout << "{\"type\":\"string\",\"value\":" << json_escape(value.asString().make_string()) << '}';
    else if (value.type() == vespalib::slime::BOOL::instance) std::cout << "{\"type\":\"boolean\",\"value\":" << (value.asBool() ? "true" : "false") << '}';
    else fail("unsupported docsum value");
}

void emit_search(const std::string& id, const Inspector& request, const SearchReply& reply, const Fixture& fixture) {
    const auto& summary = field(request, "summary");
    std::unique_ptr<DocsumReply> sums;
    if (summary.type() == vespalib::slime::OBJECT::instance) sums = fixture.docsums(reply, request);
    std::cout << "{\"id\":" << json_escape(id) << ",\"op\":\"search\",\"total_hit_count\":" << reply.totalHitCount << ",\"hits\":[";
    for (size_t i = 0; i < reply.hits.size(); ++i) {
        if (i) std::cout << ',';
        const auto& hit = reply.hits[i]; std::cout << "{\"document_id\":" << json_escape(fixture.id_for_gid(hit.gid)) << ",\"score_bits\":\"" << bits(hit.metric) << "\",\"summary\":{";
        bool first = true;
        if (sums) {
            const auto& docsum = sums->root()["docsums"][i]["docsum"];
            const auto& requested = field(summary, "fields");
            for (size_t n = 0; n < requested.children(); ++n) {
                const auto name = string_value(requested[n], "summary field"); const auto& value = docsum[Memory(name)];
                if (!value.valid()) continue;
                if (!first) std::cout << ',';
                first = false;
                std::cout << json_escape(name) << ':';
                emit_summary(value);
            }
        }
        std::cout << "}}";
    }
    std::cout << "]}" << std::flush;
}

void emit_aggregate(const AggregationResult& result) {
    const auto emit_node = [](const auto& node) {
        if (node.getClass().inherits(Int64ResultNode::classId)) std::cout << "{\"type\":\"integer\",\"value\":" << node.getInteger() << '}';
        else std::cout << "{\"type\":\"float\",\"bits\":\"" << bits(node.getFloat()) << "\"}";
    };
    if (result.getClass().inherits(CountAggregationResult::classId)) std::cout << "{\"type\":\"count\",\"value\":" << static_cast<const CountAggregationResult&>(result).getCount() << '}';
    else if (result.getClass().inherits(SumAggregationResult::classId)) emit_node(static_cast<const SumAggregationResult&>(result).getSum());
    else if (result.getClass().inherits(MinAggregationResult::classId)) emit_node(static_cast<const MinAggregationResult&>(result).getMin());
    else if (result.getClass().inherits(MaxAggregationResult::classId)) emit_node(static_cast<const MaxAggregationResult&>(result).getMax());
    else if (result.getClass().inherits(AverageAggregationResult::classId)) emit_node(static_cast<const AverageAggregationResult&>(result).getAverage());
    else fail("unsupported native aggregation result");
}

std::vector<AggregateSpec> aggregations(const Inspector& request) {
    const auto& values = field(request, "aggregations"); require(values.type() == vespalib::slime::ARRAY::instance, "aggregations must be an array");
    std::vector<AggregateSpec> result;
        for (size_t i = 0; i < values.children(); ++i) {
            require(values[i].type() == vespalib::slime::OBJECT::instance && values[i].fields() == 1, "aggregation must have one operation");
            bool recognized = false;
            for (const std::string_view op : {"sum", "min", "max", "average"}) {
                const auto& name = field(values[i], op);
                if (present(name)) {
                    result.push_back({std::string(op), string_value(name, "aggregation field")});
                    recognized = true;
                    break;
                }
            }
            require(recognized, "unsupported aggregation operation");
        }
    return result;
}

void emit_group(const std::string& id, const Inspector& request, const SearchReply& reply) {
    require(reply.hits.empty(), "native zero-hit grouping returned ordinary hits"); require(!reply.groupResult.empty(), "native grouping result missing");
    const auto specs = aggregations(request);
    vespalib::nbostream buffer(reply.groupResult.data(), reply.groupResult.size()); vespalib::NBOSerializer input(buffer); uint32_t count = 0; input >> count; require(count == 1, "unexpected grouping result count");
    Grouping result; result.deserialize(input);
    const auto& root = result.getRoot();
    std::cout << "{\"id\":" << json_escape(id) << ",\"op\":\"group\",\"groups\":[";
    for (uint32_t i = 0; i < root.getChildrenSize(); ++i) {
        if (i) std::cout << ',';
        const auto& group = root.getChild(i);
        std::cout << "{\"key\":{";
        const auto& key = group.getId();
        if (key.getClass().inherits(search::expression::StringResultNode::classId)) { HoldString value(key); std::cout << "\"type\":\"string\",\"value\":" << json_escape(std::string_view(value)); }
        else std::cout << "\"type\":\"integer\",\"value\":" << key.getInteger();
        const auto& count_result = group.getAggregationResult(0);
        require(count_result.getClass().inherits(CountAggregationResult::classId), "native grouping count missing");
        std::cout << "},\"count\":" << static_cast<const CountAggregationResult&>(count_result).getCount() << ",\"aggregations\":[";
        for (size_t n = 0; n < specs.size(); ++n) {
            if (n) std::cout << ',';
            emit_aggregate(group.getAggregationResult(n + 1));
        }
        std::cout << "]}";
    }
    std::cout << "]}" << std::flush;
}

std::string required_env(const char* name) { const auto* value = std::getenv(name); require(value && *value, std::string("missing environment variable ") + name); return value; }

void configure_grouping(SearchRequest& request, const Inspector& group_request, const Fixture& fixture) {
    const auto group_by = string_value(field(group_request, "group_by"), "group_by");
    const auto& group_spec = spec_for(fixture.specs(), group_by); require(group_spec.type == "string" || group_spec.type == "integer", "unsupported group key");
    auto level = GroupingLevel().setExpression(std::make_unique<AttributeNode>(group_by));
    level.addResult(CountAggregationResult().setExpression(std::make_unique<AttributeNode>(group_by)));
    for (const auto& spec : aggregations(group_request)) {
        const auto& aggregate_field = spec_for(fixture.specs(), spec.field); require(aggregate_field.type == "integer" || aggregate_field.type == "float", "unsupported aggregation field");
        std::unique_ptr<AggregationResult> result;
        if (spec.op == "sum") result = std::make_unique<SumAggregationResult>();
        else if (spec.op == "min") result = std::make_unique<MinAggregationResult>();
        else if (spec.op == "max") result = std::make_unique<MaxAggregationResult>();
        else if (spec.op == "average") result = std::make_unique<AverageAggregationResult>();
        else fail("unsupported aggregation");
        result->setExpression(std::make_unique<AttributeNode>(spec.field)); level.addResult(std::move(result));
    }
    Grouping grouping; grouping.setFirstLevel(0).setLastLevel(1).addLevel(std::move(level));
    vespalib::nbostream buffer; vespalib::NBOSerializer output(buffer); uint32_t count = 1; output << count; grouping.serialize(output);
    request.groupSpec.assign(buffer.data(), buffer.data() + buffer.size()); request.maxhits = 0;
}

int run(const std::filesystem::path& path) {
    const auto scenario = read_json(path); const auto& root = scenario.get(); const auto id = string_value(field(root, "id"), "scenario id");
    const auto specs = schema(root); DummyFileHeaderContext::setCreator("searchkernel_m9_cpp_oracle");
    std::filesystem::remove_all("typea"); std::filesystem::remove_all("tmp"); std::filesystem::create_directory("typea");
    Fixture fixture(required_env("M9_ORACLE_FIXTURE_ROOT"), specs); const auto& documents = field(root, "documents");
    require(documents.type() == vespalib::slime::ARRAY::instance, "documents must be an array"); for (size_t i = 0; i < documents.children(); ++i) fixture.put(documents[i]);
    const auto& steps = field(root, "steps"); require(steps.type() == vespalib::slime::ARRAY::instance && steps.children() > 0, "steps must be a non-empty array");
    std::cout << "{\"id\":" << json_escape(id) << ",\"steps\":[";
    for (size_t i = 0; i < steps.children(); ++i) {
        if (i) std::cout << ',';
        const auto& step = steps[i];
        const auto step_id = string_value(field(step, "id"), "step id");
        const auto op = string_value(field(step, "op"), "step op");
        if (op == "update") { fixture.update(string_value(field(step, "document_id"), "document id"), field(step, "assign")); std::cout << "{\"id\":" << json_escape(step_id) << ",\"op\":\"update\",\"ok\":true}"; }
        else if (op == "search") { const auto& request = field(step, "request"); auto reply = fixture.search(request); emit_search(step_id, request, *reply, fixture); }
        else if (op == "group") {
            const auto& group_request = field(step, "request"); auto native = std::make_shared<SearchRequest>(); native->setTimeout(60s); native->ranking = "default";
            native->propertiesMap.lookupCreate(search::MapNames::RANK).add("vespa.matching.numthreadspersearch", "1");
            native->setSerializedQueryTree(SerializedQueryTree::fromStackDump(fixture.query_dump_for(field(group_request, "query"))));
            configure_grouping(*native, group_request, fixture);
            auto reply = fixture.match(*native);
            emit_group(step_id, group_request, *reply);
        } else fail("unknown step op");
    }
    std::cout << "]}\n" << std::flush; return 0;
}

} // namespace

int main(int argc, char** argv) {
    try { require(argc == 2, "usage: m9_attribute_grouping_oracle SCENARIO_JSON"); return run(argv[1]); }
    catch (const std::exception& error) { std::cerr << "m9_attribute_grouping_oracle: " << error.what() << '\n'; std::filesystem::remove_all("typea"); std::filesystem::remove_all("tmp"); return 2; }
}
