// Copyright Vespa.ai. Licensed under the terms of the Apache 2.0 license.
//
// Direct M12 dense-tensor / nearest-neighbor oracle.
//
// This executable deliberately talks directly to the pinned Proton DocumentDB
// and never calls Search Kernel production code. It is intended to freeze the
// M12 verifier semantics before the Search Kernel adapter implementation exists.
//
// Supported modes:
//   m12_tensor_oracle run SCENARIO_JSON
//   m12_tensor_oracle prepare SCENARIO_JSON CONFIG_DIR DATA_DIR
//
// M12 v1 deliberately supports only:
//   - tensor<float>(x[3]) document/query tensors
//   - a single tensor attribute named "embedding"
//   - Euclidean distance
//   - exact nearest-neighbor matching (no HNSW)
//   - ordered hit IDs as oracle output

#include <vespa/config-bucketspaces.h>
#include <vespa/config-stor-distribution.h>
#include <vespa/config/subscription/sourcespec.h>
#include <vespa/document/datatype/datatype.h>
#include <vespa/document/datatype/documenttype.h>
#include <vespa/document/datatype/tensor_data_type.h>
#include <vespa/document/fieldvalue/document.h>
#include <vespa/document/fieldvalue/intfieldvalue.h>
#include <vespa/document/fieldvalue/tensorfieldvalue.h>
#include <vespa/document/repo/documenttyperepo.h>
#include <vespa/document/test/make_bucket_space.h>
#include <vespa/eval/eval/fast_value.h>
#include <vespa/eval/eval/value.h>
#include <vespa/eval/eval/value_codec.h>
#include <vespa/eval/eval/value_type.h>
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
#include <vespa/searchlib/engine/searchreply.h>
#include <vespa/searchlib/engine/searchrequest.h>
#include <vespa/searchlib/index/dummyfileheadercontext.h>
#include <vespa/searchlib/query/tree/querybuilder.h>
#include <vespa/searchlib/query/tree/stackdumpcreator.h>
#include <vespa/searchlib/transactionlog/translogserver.h>
#include <vespa/vespalib/data/slime/json_format.h>
#include <vespa/vespalib/data/slime/slime.h>
#include <vespa/vespalib/net/socket_spec.h>
#include <vespa/vespalib/objects/nbostream.h>
#include <vespa/vespalib/util/hw_info.h>
#include <vespa/vespalib/util/thread_bundle.h>
#include <vespa/vespalib/util/threadstackexecutor.h>
#include <vespa/vdslib/distribution/distribution.h>
#include <vespa/vdslib/state/clusterstate.h>

#include <algorithm>
#include <cctype>
#include <chrono>
#include <cmath>
#include <cstdint>
#include <filesystem>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <iterator>
#include <limits>
#include <memory>
#include <set>
#include <sstream>
#include <stdexcept>
#include <string>
#include <string_view>
#include <system_error>
#include <unordered_map>
#include <unordered_set>
#include <utility>
#include <vector>

using namespace std::chrono_literals;
using document::Document;
using document::DocumentId;
using document::DocumentType;
using document::DocumentTypeRepo;
using document::TensorDataType;
using document::TensorFieldValue;
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
constexpr const char* ORDINAL_FIELD = "ordinal";
constexpr const char* RANK_PROFILE = "m12_nn";
constexpr const char* M12_TENSOR_TYPE = "tensor<float>(x[3])";

[[noreturn]] void fail(std::string_view message) {
    throw std::runtime_error(std::string(message));
}

void require(bool condition, std::string_view message) {
    if (!condition) {
        fail(message);
    }
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

double number_value(const Inspector& value, std::string_view what) {
    double result = 0.0;
    if (value.type() == vespalib::slime::DOUBLE::instance) {
        result = value.asDouble();
    } else if (value.type() == vespalib::slime::LONG::instance) {
        result = static_cast<double>(value.asLong());
    } else {
        fail(what);
    }
    require(std::isfinite(result), what);
    return result;
}

uint32_t positive_u32(const Inspector& value, std::string_view what) {
    const auto number = integer_value(value, what);
    require(number > 0 && static_cast<uint64_t>(number) <= std::numeric_limits<uint32_t>::max(), what);
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
    require(used <= bytes.size() &&
                std::all_of(bytes.begin() + static_cast<std::ptrdiff_t>(used), bytes.end(),
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
            if (c < 0x20) {
                out << "\\u" << std::hex << std::setw(4) << std::setfill('0') << int(c) << std::dec;
            } else {
                out << static_cast<char>(c);
            }
        }
    }
    out << '"';
    return out.str();
}

std::string number_text(double value) {
    require(std::isfinite(value), "query feature must be finite");
    std::ostringstream out;
    out << std::setprecision(std::numeric_limits<double>::max_digits10) << value;
    return out.str();
}

void write_text(const std::filesystem::path& path, std::string_view text) {
    std::ofstream out(path, std::ios::binary | std::ios::trunc);
    require(out.good(), "could not create M12 fixture config file");
    out.write(text.data(), static_cast<std::streamsize>(text.size()));
    require(out.good(), "could not write M12 fixture config file");
}

struct DenseTensorInput {
    std::string name;
    std::string type_spec;
    std::vector<float> values;
};

struct FixtureSpec {
    std::string tensor_field;
    std::string tensor_type;
    std::vector<std::string> query_tensor_names;
};

std::vector<float> float_cells(const Inspector& array, std::string_view what) {
    require(array.type() == vespalib::slime::ARRAY::instance, what);
    std::vector<float> result;
    result.reserve(array.children());
    for (size_t i = 0; i < array.children(); ++i) {
        const auto value = number_value(array[i], what);
        require(value >= -std::numeric_limits<float>::max() && value <= std::numeric_limits<float>::max(), what);
        result.push_back(static_cast<float>(value));
    }
    return result;
}

DenseTensorInput dense_input(const Inspector& value) {
    require(value.type() == vespalib::slime::OBJECT::instance, "query input must be an object");
    DenseTensorInput result;
    result.name = string_value(object_field(value, "name"), "query input name must be string");
    require(!result.name.empty(), "query input name must be non-empty");
    require(string_value(object_field(value, "kind"), "query input kind must be string") == "dense",
            "M12 oracle only accepts dense query tensor inputs");
    result.type_spec = string_value(object_field(value, "type_spec"), "query input type_spec must be string");
    require(result.type_spec == M12_TENSOR_TYPE, "M12 oracle only accepts tensor<float>(x[3]) query tensors");
    result.values = float_cells(object_field(value, "values"), "query tensor values must be numeric array");
    require(result.values.size() == 3, "M12 query tensor must contain exactly 3 cells");
    return result;
}

FixtureSpec fixture_spec(const Inspector& root) {
    const auto& fixture = object_field(root, "fixture");
    require(string_value(object_field(fixture, "document_type"), "fixture document_type must be string") == DOC_TYPE,
            "M12 oracle only supports document type typea");
    const auto& tensor = object_field(fixture, "tensor_field");
    FixtureSpec spec;
    spec.tensor_field = string_value(object_field(tensor, "name"), "tensor field name must be string");
    spec.tensor_type = string_value(object_field(tensor, "type_spec"), "tensor field type_spec must be string");
    require(spec.tensor_field == "embedding", "M12 v1 oracle requires tensor field embedding");
    require(spec.tensor_type == M12_TENSOR_TYPE, "M12 v1 oracle requires tensor<float>(x[3])");
    require(string_value(object_field(tensor, "distance_metric"), "distance_metric must be string") == "euclidean",
            "M12 v1 oracle requires Euclidean distance");
    require(!bool_value(object_field(tensor, "hnsw"), "hnsw must be bool"),
            "M12 v1 oracle requires exact search without HNSW");

    std::set<std::string> names;
    const auto& steps = object_field(root, "steps");
    require(steps.type() == vespalib::slime::ARRAY::instance && steps.children() > 0, "steps required");
    for (size_t s = 0; s < steps.children(); ++s) {
        const auto& inputs = object_field(steps[s], "inputs");
        require(inputs.type() == vespalib::slime::ARRAY::instance, "step.inputs must be an array");
        for (size_t i = 0; i < inputs.children(); ++i) {
            const auto input = dense_input(inputs[i]);
            require(input.type_spec == spec.tensor_type, "query tensor type must match document tensor type");
            names.insert(input.name);
        }
    }
    spec.query_tensor_names.assign(names.begin(), names.end());
    require(!spec.query_tensor_names.empty(), "at least one query tensor input is required");
    return spec;
}

std::string quote_cfg(std::string_view value) {
    std::string out = "\"";
    for (char c : value) {
        if (c == '\\' || c == '"') {
            out.push_back('\\');
        }
        out.push_back(c);
    }
    out.push_back('"');
    return out;
}

void write_fixture_config(const std::filesystem::path& config_dir, const FixtureSpec& spec) {
    std::filesystem::remove_all(config_dir);
    std::filesystem::create_directories(config_dir);

    std::ostringstream attributes;
    attributes << "attribute[2]\n"
               << "attribute[0].name " << quote_cfg(spec.tensor_field) << "\n"
               << "attribute[0].datatype TENSOR\n"
               << "attribute[0].collectiontype SINGLE\n"
               << "attribute[0].tensortype " << quote_cfg(spec.tensor_type) << "\n"
               << "attribute[0].distancemetric EUCLIDEAN\n"
               << "attribute[0].index.hnsw.enabled false\n"
               << "attribute[1].name " << quote_cfg(ORDINAL_FIELD) << "\n"
               << "attribute[1].datatype INT32\n"
               << "attribute[1].collectiontype SINGLE\n";
    write_text(config_dir / "attributes.cfg", attributes.str());

    // M12 has no inverted index fields. The tensor is an attribute.
    write_text(config_dir / "indexschema.cfg", "indexfield[0]\n");
    write_text(config_dir / "juniperrc.cfg", "");

    std::ostringstream ranking;
    ranking << "rankprofile[2]\n"
            << "rankprofile[0].name \"default\"\n\n"
            << "rankprofile[1].name " << quote_cfg(RANK_PROFILE) << "\n"
            << "rankprofile[1].fef.property[" << (1 + spec.query_tensor_names.size()) << "]\n"
            << "rankprofile[1].fef.property[0].name \"vespa.rank.firstphase\"\n"
            << "rankprofile[1].fef.property[0].value "
            << quote_cfg("closeness(field," + spec.tensor_field + ")") << "\n";
    for (size_t i = 0; i < spec.query_tensor_names.size(); ++i) {
        ranking << "rankprofile[1].fef.property[" << (i + 1) << "].name "
                << quote_cfg("vespa.type.query." + spec.query_tensor_names[i]) << "\n"
                << "rankprofile[1].fef.property[" << (i + 1) << "].value "
                << quote_cfg(spec.tensor_type) << "\n";
    }
    write_text(config_dir / "rank-profiles.cfg", ranking.str());

    std::ostringstream summary;
    summary << "defaultsummaryid 1\n"
            << "classes[1]\n"
            << "classes[0].id 1\n"
            << "classes[0].name \"default\"\n"
            << "classes[0].fields[2]\n"
            << "classes[0].fields[0].name \"documentid\"\n"
            << "classes[0].fields[0].type \"longstring\"\n"
            << "classes[0].fields[0].command \"documentid\"\n"
            << "classes[0].fields[0].source \"\"\n"
            << "classes[0].fields[1].name " << quote_cfg(ORDINAL_FIELD) << "\n"
            << "classes[0].fields[1].type \"integer\"\n"
            << "classes[0].fields[1].command \"attribute\"\n"
            << "classes[0].fields[1].source " << quote_cfg(ORDINAL_FIELD) << "\n";
    write_text(config_dir / "summary.cfg", summary.str());

    std::ostringstream schema;
    schema << "{\"version\":1,\"document_type\":\"typea\",\"fields\":["
           << "{\"name\":" << json_escape(spec.tensor_field)
           << ",\"type\":\"tensor\",\"type_spec\":" << json_escape(spec.tensor_type)
           << ",\"attribute\":true,\"distance_metric\":\"euclidean\",\"hnsw\":false},"
           << "{\"name\":\"ordinal\",\"type\":\"integer\",\"attribute\":true}]}\n";
    write_text(config_dir / "searchkernel-schema.json", schema.str());
}

std::unique_ptr<vespalib::eval::Value> make_dense_tensor(std::string_view type_spec,
                                                          const std::vector<float>& values) {
    auto type = vespalib::eval::ValueType::from_spec(std::string(type_spec));
    require(!type.is_error(), "invalid tensor type specification");
    require(type.count_mapped_dimensions() == 0, "M12 tensor must not have mapped dimensions");
    require(type.count_indexed_dimensions() == 1, "M12 tensor must have exactly one indexed dimension");
    require(type.dense_subspace_size() == values.size(), "dense tensor cell count mismatch");

    const auto& factory = vespalib::eval::FastValueBuilderFactory::get();
    auto builder = factory.create_value_builder<float>(type);
    auto cells = builder->add_subspace();
    require(cells.size() == values.size(), "dense tensor builder cell count mismatch");
    std::copy(values.begin(), values.end(), cells.begin());
    return builder->build(std::move(builder));
}

std::string encode_query_tensor(const DenseTensorInput& input) {
    auto tensor = make_dense_tensor(input.type_spec, input.values);
    vespalib::nbostream encoded;
    vespalib::eval::encode_value(*tensor, encoded);
    return std::string(encoded.data(), encoded.size());
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

class CurrentDirectoryGuard {
    std::filesystem::path _old;
public:
    explicit CurrentDirectoryGuard(const std::filesystem::path& next)
        : _old(std::filesystem::current_path()) {
        std::filesystem::create_directories(next);
        std::filesystem::current_path(next);
    }
    ~CurrentDirectoryGuard() {
        std::error_code ignored;
        std::filesystem::current_path(_old, ignored);
    }
};

class Fixture {
    proton::DummyWireService _wire;
    proton::DummyDBOwner _owner;
    vespalib::ThreadStackExecutor _summary_executor;
    proton::MockSharedThreadingService _shared;
    HwInfo _hw;
    DummyFileHeaderContext _header;
    TransLogServer _tls;
    proton::matching::QueryLimiter _limiter;
    std::unique_ptr<const TensorDataType> _tensor_type;
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
    std::string _tensor_field;
    std::string _tensor_type_spec;

public:
    Fixture(const std::filesystem::path& config_dir, const FixtureSpec& spec)
        : _wire(),
          _owner(),
          _summary_executor(8),
          _shared(_summary_executor),
          _hw(),
          _header(),
          _tls(_shared.transport(), "tmp", TLS_PORT, ".", _header),
          _limiter(),
          _tensor_type(TensorDataType::fromSpec(spec.tensor_type)),
          _type(DOC_TYPE, 0),
          _repo(),
          _db(),
          _persistence_owner(),
          _write_filter(),
          _resource_usage(),
          _persistence(_persistence_owner, _write_filter, _resource_usage, -1, false),
          _handler(),
          _tensor_field(spec.tensor_field),
          _tensor_type_spec(spec.tensor_type) {
        require(static_cast<bool>(_tensor_type), "could not create Vespa tensor data type");
        _type.addField(document::Field(_tensor_field, 1, *_tensor_type));
        _type.addField(document::Field(ORDINAL_FIELD, 2, *document::DataType::INT));
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
        std::cerr << "M12_ORACLE shutdown removeHandler begin\n" << std::flush;
        {
            auto removed = _persistence.removeHandler(_persistence.getWLock(), makeBucketSpace(), DocTypeName(DOC_TYPE));
            require(static_cast<bool>(removed), "persistence handler removal failed");
        }
        _handler.reset();
        if (_db) {
            std::cerr << "M12_ORACLE shutdown DocumentDB close begin\n" << std::flush;
            _db->close();
            _db.reset();
        }
        _shared.transport().ShutDown(true);
        std::cerr << "M12_ORACLE shutdown complete\n" << std::flush;
    }

    void prepare_bucket(const Bucket& bucket) {
        if (_prepared.insert(bucket.getBucketId().getId()).second) {
            require(!_persistence.createBucket(bucket).hasError(), "createBucket failed");
            require(!_persistence.setActiveState(bucket, BucketInfo::ACTIVE).hasError(), "setActiveState failed");
        }
    }

    void put(const Inspector& encoded, int32_t ordinal) {
        const auto id = string_value(object_field(encoded, "id"), "document id must be string");
        const auto values = float_cells(object_field(encoded, "values"), "document tensor values must be numeric array");
        require(values.size() == 3, "M12 document tensor must contain exactly 3 cells");

        auto document = std::make_shared<Document>(*_repo, _type, DocumentId(id));
        auto tensor_value = std::make_unique<TensorFieldValue>(*_tensor_type);
        *tensor_value = make_dense_tensor(_tensor_type_spec, values);
        document->setFieldValue(_type.getField(_tensor_field), std::move(tensor_value));
        document->setFieldValue(_type.getField(ORDINAL_FIELD),
                                std::make_unique<document::IntFieldValue>(ordinal));

        const auto document_id = document->getId();
        auto bucket = proton::BucketFactory::getBucket(document_id);
        prepare_bucket(bucket);
        _ids.emplace_back(document_id.getGlobalId(), document_id.toString());
        auto result = _persistence.put(bucket, Timestamp(_timestamp++), std::move(document));
        require(!result.hasError(), "put failed");
    }

    std::string id_for_gid(const document::GlobalId& gid) const {
        const auto found = std::find_if(_ids.begin(), _ids.end(), [&](const auto& item) {
            return item.first == gid;
        });
        require(found != _ids.end(), "unknown Vespa global id");
        return found->second;
    }

    SearchReply::UP search(const Inspector& step) const {
        auto native = std::make_shared<SearchRequest>();
        native->setTimeout(60s);
        native->dumpFeatures = false;
        native->ranking = RANK_PROFILE;
        native->maxhits = positive_u32(object_field(step, "limit"), "limit must be positive integer");
        native->offset = 0;

        auto& rank = native->propertiesMap.lookupCreate(search::MapNames::RANK);
        rank.add("vespa.matching.numthreadspersearch", "1");

        const auto& scalar_features = object_field(step, "scalar_query_features");
        require(scalar_features.type() == vespalib::slime::ARRAY::instance,
                "scalar_query_features must be an array");
        for (size_t i = 0; i < scalar_features.children(); ++i) {
            const auto& item = scalar_features[i];
            const auto name = string_value(object_field(item, "name"), "scalar query feature name must be string");
            require(!name.empty(), "scalar query feature name must be non-empty");
            rank.add(name, number_text(number_value(object_field(item, "value"), "scalar query feature must be numeric")));
        }

        const auto& rank_properties = object_field(step, "rank_properties");
        require(rank_properties.type() == vespalib::slime::ARRAY::instance,
                "rank_properties must be an array");
        for (size_t i = 0; i < rank_properties.children(); ++i) {
            const auto& item = rank_properties[i];
            rank.add(string_value(object_field(item, "name"), "rank property name must be string"),
                     string_value(object_field(item, "value"), "rank property value must be string"));
        }

        const auto& inputs = object_field(step, "inputs");
        require(inputs.type() == vespalib::slime::ARRAY::instance, "inputs must be an array");
        std::unordered_map<std::string, DenseTensorInput> tensors;
        for (size_t i = 0; i < inputs.children(); ++i) {
            auto input = dense_input(inputs[i]);
            const auto name = input.name;
            require(tensors.emplace(name, input).second, "duplicate query tensor input");
            const auto payload = encode_query_tensor(tensors.at(name));
            // QueryValue::request_lookup() first looks for query(name), then name,
            // then $name. Use the canonical query(name) form and preserve binary bytes.
            rank.add("query(" + name + ")", payload);
        }

        const auto query_tensor = string_value(object_field(step, "query_tensor"),
                                               "query_tensor must be string");
        require(tensors.find(query_tensor) != tensors.end(), "nearest-neighbor query tensor input is missing");
        const auto document_field = string_value(object_field(step, "document_field"),
                                                 "document_field must be string");

        QueryBuilder<proton::matching::ProtonNodeTypes> builder;
        builder.add_nearest_neighbor_term(query_tensor,
                                          document_field,
                                          1,
                                          Weight(1),
                                          native->maxhits,
                                          false);
        auto tree = builder.build();
        require(static_cast<bool>(tree), builder.error());
        native->setSerializedQueryTree(SerializedQueryTree::fromStackDump(StackDumpCreator::create(*tree)));

        std::cerr << "M12_ORACLE match begin step="
                  << string_value(object_field(step, "id"), "step id must be string") << "\n" << std::flush;
        auto result = _db->match(*native, vespalib::ThreadBundle::trivial());
        std::cerr << "M12_ORACLE match returned\n" << std::flush;
        require(static_cast<bool>(result), "DocumentDB::match returned null");
        return result;
    }
};

void feed_documents(Fixture& fixture, const Inspector& root) {
    const auto& documents = object_field(object_field(root, "fixture"), "documents");
    require(documents.type() == vespalib::slime::ARRAY::instance && documents.children() > 0,
            "fixture.documents must be a non-empty array");
    for (size_t i = 0; i < documents.children(); ++i) {
        require(i <= static_cast<size_t>(std::numeric_limits<int32_t>::max()), "too many documents");
        fixture.put(documents[i], static_cast<int32_t>(i));
    }
}

void emit_step(const std::string& step_id, const SearchReply& reply, const Fixture& fixture) {
    std::cout << "{\"id\":" << json_escape(step_id) << ",\"hits\":[";
    for (size_t i = 0; i < reply.hits.size(); ++i) {
        if (i) {
            std::cout << ',';
        }
        std::cout << json_escape(fixture.id_for_gid(reply.hits[i].gid));
    }
    std::cout << "]}";
}

void execute_steps(const Inspector& root, Fixture& fixture) {
    const auto id = string_value(object_field(root, "id"), "scenario id must be string");
    const auto& steps = object_field(root, "steps");
    require(steps.type() == vespalib::slime::ARRAY::instance && steps.children() > 0, "steps required");

    std::cout << "{\"id\":" << json_escape(id) << ",\"steps\":[";
    for (size_t i = 0; i < steps.children(); ++i) {
        if (i) {
            std::cout << ',';
        }
        const auto& step = steps[i];
        const auto step_id = string_value(object_field(step, "id"), "step id must be string");
        auto reply = fixture.search(step);
        emit_step(step_id, *reply, fixture);
    }
    std::cout << "]}\n" << std::flush;
}

std::filesystem::path unique_work_dir() {
    const auto stamp = std::chrono::high_resolution_clock::now().time_since_epoch().count();
    return std::filesystem::temp_directory_path() /
           ("searchkernel-m12-oracle-" + std::to_string(static_cast<long long>(stamp)));
}

void prepare_data_dir(const std::filesystem::path& data_dir) {
    std::filesystem::create_directories(data_dir);
    // Proton creates readiness files below the document-type directory while
    // opening DocumentDB; create the intermediate directory first.
    std::filesystem::create_directories(data_dir / DOC_TYPE);
}

int run_mode(const std::filesystem::path& scenario_path) {
    auto scenario = read_json(scenario_path);
    const auto& root = scenario.get();
    const auto spec = fixture_spec(root);

    auto work = unique_work_dir();
    auto config_dir = work / "config";
    auto data_dir = work / "data";
    prepare_data_dir(data_dir);
    write_fixture_config(config_dir, spec);

    DummyFileHeaderContext::setCreator("searchkernel_m12_cpp_oracle");
    try {
        {
            CurrentDirectoryGuard cwd(data_dir);
            Fixture fixture(std::filesystem::absolute(config_dir), spec);
            feed_documents(fixture, root);
            execute_steps(root, fixture);
        }
        std::filesystem::remove_all(work);
    } catch (...) {
        std::error_code ignored;
        std::filesystem::remove_all(work, ignored);
        throw;
    }
    return 0;
}

int prepare_mode(const std::filesystem::path& scenario_path,
                 const std::filesystem::path& config_dir_arg,
                 const std::filesystem::path& data_dir_arg) {
    auto scenario = read_json(scenario_path);
    const auto& root = scenario.get();
    const auto spec = fixture_spec(root);

    const auto config_dir = std::filesystem::absolute(config_dir_arg);
    const auto data_dir = std::filesystem::absolute(data_dir_arg);
    std::filesystem::remove_all(data_dir);
    prepare_data_dir(data_dir);
    write_fixture_config(config_dir, spec);

    DummyFileHeaderContext::setCreator("searchkernel_m12_cpp_oracle_prepare");
    {
        CurrentDirectoryGuard cwd(data_dir);
        Fixture fixture(config_dir, spec);
        feed_documents(fixture, root);
    }
    return 0;
}

} // namespace

int main(int argc, char** argv) {
    try {
        require(argc >= 2, "usage: m12_tensor_oracle run SCENARIO_JSON | prepare SCENARIO_JSON CONFIG_DIR DATA_DIR");
        const std::string mode = argv[1];
        if (mode == "run") {
            require(argc == 3, "usage: m12_tensor_oracle run SCENARIO_JSON");
            return run_mode(argv[2]);
        }
        if (mode == "prepare") {
            require(argc == 5, "usage: m12_tensor_oracle prepare SCENARIO_JSON CONFIG_DIR DATA_DIR");
            return prepare_mode(argv[2], argv[3], argv[4]);
        }
        fail("unknown mode; expected run or prepare");
    } catch (const std::exception& error) {
        std::cerr << "m12_tensor_oracle: " << error.what() << '\n';
        return 2;
    }
}
