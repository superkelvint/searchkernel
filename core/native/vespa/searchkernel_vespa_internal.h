#pragma once
#include <set>
#include <vespa/searchlib/query/query_stack_iterator.h>

#include "searchkernel_vespa.h"
#include "diagnostic_projection.h"
#include <vespa/config-rank-profiles.h>
#include <vespa/searchcore/proton/matching/matcher.h>
#include <vespa/searchlib/fef/ranking_assets_repo.h>
#include <vespa/eval/eval/value_cache/constant_tensor_loader.h>
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
#include "searchkernel_vespa_test.h"
#endif
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
#include <vespa/document/datatype/structdatatype.h>
#include <vespa/document/fieldvalue/document.h>
#include <vespa/document/fieldvalue/intfieldvalue.h>
#include <vespa/document/fieldvalue/longfieldvalue.h>
#include <vespa/document/fieldvalue/boolfieldvalue.h>
#include <vespa/document/fieldvalue/arrayfieldvalue.h>
#include <vespa/document/fieldvalue/structfieldvalue.h>
#include <vespa/document/fieldvalue/predicatefieldvalue.h>
#include <vespa/document/fieldvalue/doublefieldvalue.h>
#include <vespa/document/fieldvalue/stringfieldvalue.h>
#include <vespa/document/fieldvalue/tensorfieldvalue.h>
#include <vespa/document/datatype/tensor_data_type.h>
#include <vespa/document/datatype/positiondatatype.h>
#include <vespa/document/predicate/predicate.h>
#include <vespa/document/predicate/predicate_slime_builder.h>
#include <vespa/document/repo/documenttyperepo.h>
#include <vespa/document/repo/fixedtyperepo.h>
#include <vespa/document/repo/newconfigbuilder.h>
#include <vespa/searchlib/predicate/predicate_hash.h>
#include <vespa/vespalib/geo/zcurve.h>
#include <vespa/vespalib/data/slime/json_format.h>
#include <vespa/vespalib/data/slime/slime.h>
#include <vespa/document/update/assignvalueupdate.h>
#include <vespa/document/update/arithmeticvalueupdate.h>
#include <vespa/document/update/clearvalueupdate.h>
#include <vespa/document/update/documentupdate.h>
#include <vespa/document/update/fieldupdate.h>
#include <vespa/fnet/transport.h>
#include <vespa/searchlib/docstore/idocumentstore.h>
#include <vespa/searchcore/proton/docsummary/isummarymanager.h>
#include <vespa/searchcore/proton/matching/querylimiter.h>
#include <vespa/searchcore/proton/matching/querynodes.h>
#include <vespa/searchcore/proton/metrics/metricswireservice.h>
#include <vespa/searchcore/proton/persistenceengine/i_resource_write_filter.h>
#include <vespa/searchcore/proton/persistenceengine/ipersistenceengineowner.h>
#include <vespa/searchcore/proton/persistenceengine/persistenceengine.h>
#include <vespa/searchcore/proton/persistenceengine/i_document_retriever.h>
#include <vespa/searchcore/proton/server/bootstrapconfig.h>
#include <vespa/searchcore/proton/server/configstore.h>
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
#include <vespa/searchlib/common/identifiable.h>
#include <vespa/searchlib/engine/searchreply.h>
#include <vespa/searchlib/engine/searchrequest.h>
#include <vespa/searchlib/engine/docsumreply.h>
#include <vespa/searchlib/engine/docsumrequest.h>
#include <vespa/searchlib/query/tree/query_to_protobuf.h>
#include <vespa/vespalib/util/featureset.h>
#include <vespa/searchlib/aggregation/averageaggregationresult.h>
#include <vespa/searchlib/aggregation/countaggregationresult.h>
#include <vespa/searchlib/aggregation/group.h>
#include <vespa/searchlib/aggregation/grouping.h>
#include <vespa/searchlib/aggregation/groupinglevel.h>
#include <vespa/searchlib/aggregation/maxaggregationresult.h>
#include <vespa/searchlib/aggregation/minaggregationresult.h>
#include <vespa/searchlib/aggregation/sumaggregationresult.h>
#include <vespa/searchlib/aggregation/standarddeviationaggregationresult.h>
#include <vespa/searchlib/aggregation/quantile_aggregation_result.h>
#include <vespa/searchlib/aggregation/hitsaggregationresult.h>
#include <vespa/searchlib/aggregation/hitlist.h>
#include <vespa/searchlib/aggregation/fs4hit.h>
#include <vespa/searchlib/aggregation/vdshit.h>
#include <vespa/searchlib/aggregation/xoraggregationresult.h>
#include <vespa/searchlib/expression/attributenode.h>
#include <vespa/searchlib/expression/addfunctionnode.h>
#include <vespa/searchlib/expression/catfunctionnode.h>
#include <vespa/searchlib/expression/dividefunctionnode.h>
#include <vespa/searchlib/expression/multiplyfunctionnode.h>
#include <vespa/searchlib/expression/modulofunctionnode.h>
#include <vespa/searchlib/expression/negatefunctionnode.h>
#include <vespa/searchlib/expression/normalizesubjectfunctionnode.h>
#include <vespa/searchlib/expression/reversefunctionnode.h>
#include <vespa/searchlib/expression/strlenfunctionnode.h>
#include <vespa/searchlib/expression/xorfunctionnode.h>
#include <vespa/searchlib/expression/regex_predicate_node.h>
#include <vespa/searchlib/expression/filter_predicate_node.h>
#include <vespa/searchlib/expression/and_predicate_node.h>
#include <vespa/searchlib/expression/not_predicate_node.h>
#include <vespa/searchlib/expression/or_predicate_node.h>
#include <vespa/searchlib/expression/istrue_predicate_node.h>
#include <vespa/searchlib/expression/range_predicate_node.h>
#include <vespa/searchlib/expression/aggregationrefnode.h>
#include <vespa/searchlib/expression/constantnode.h>
#include <vespa/searchlib/expression/floatresultnode.h>
#include <vespa/searchlib/expression/integerresultnode.h>
#include <vespa/searchlib/expression/stringresultnode.h>
#include <vespa/searchlib/common/fileheadercontext.h>
#include <vespa/searchlib/util/fileheadertk.h>
#include <vespa/searchlib/common/serialized_query_tree.h>
#include <vespa/searchlib/query/numeric_range_spec.h>
#include <vespa/searchlib/query/tree/querybuilder.h>
#include <vespa/searchlib/query/tree/location.h>
#include <vespa/searchlib/query/tree/point.h>
#include <vespa/searchlib/query/tree/rectangle.h>
#include <vespa/searchlib/query/tree/range.h>
#include <vespa/searchlib/query/tree/string_range.h>
#include <vespa/searchlib/query/tree/stackdumpcreator.h>
#include <vespa/searchlib/query/tree/weighted_integer_term_vector.h>
#include <vespa/searchlib/query/tree/weighted_string_term_vector.h>
#include <vespa/searchlib/query/tree/string_term_vector.h>
#include <vespa/searchlib/query/tree/integer_term_vector.h>
#include <vespa/searchlib/transactionlog/translogserver.h>
#include <vespa/vdslib/distribution/distribution.h>
#include <vespa/vdslib/state/clusterstate.h>
#include <vespa/vespalib/net/socket_spec.h>
#include <vespa/vespalib/util/hw_info.h>
#include <vespa/vespalib/util/threadstackexecutor.h>
#include <vespa/vespalib/data/fileheader.h>
#include <vespa/vespalib/data/slime/slime.h>
#include <vespa/vespalib/data/slime/json_format.h>
#include <vespa/vespalib/data/smart_buffer.h>
#include <vespa/vespalib/objects/nbostream.h>
#include <vespa/vespalib/objects/nboserializer.h>
#include <vespa/vespalib/objects/objectvisitor.h>
#include <vespa/vespalib/util/destructor_callbacks.h>
#include <vespa/vespalib/util/host_name.h>
#include <vespa/vespalib/util/gate.h>
#include <vespa/vespalib/util/lambdatask.h>
#include <vespa/vespalib/util/invokeserviceimpl.h>
#include <vespa/vespalib/util/sequencedtaskexecutor.h>
#include <vespa/eval/eval/fast_value.h>
#include <vespa/eval/eval/tensor_spec.h>
#include <vespa/eval/eval/typed_cells.h>
#include <vespa/eval/eval/value_codec.h>
#include <vespa/vespalib/util/shared_string_repo.h>
#include <vespa/vespalib/util/shared_operation_throttler.h>

#include <vespa/persistence/dummyimpl/dummy_bucket_executor.h>

#include <algorithm>
#include <array>
#include <atomic>
#include <chrono>
#include <cerrno>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <condition_variable>
#include <cmath>
#include <filesystem>
#include <exception>
#include <fcntl.h>
#include <fstream>
#include <iomanip>
#include <iterator>
#include <initializer_list>
#include <locale>
#include <limits>
#include <map>
#include <memory>
#include <mutex>
#include <new>
#include <optional>
#include <set>
#include <sstream>
#include <stdexcept>
#include <string>
#include <string_view>
#include <shared_mutex>
#include <thread>
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
using search::engine::DocsumReply;
using search::engine::DocsumRequest;
using search::aggregation::AggregationResult;
using search::aggregation::AverageAggregationResult;
using search::aggregation::CountAggregationResult;
using search::aggregation::Group;
using search::aggregation::Grouping;
using search::aggregation::GroupingLevel;
using search::aggregation::MaxAggregationResult;
using search::aggregation::MinAggregationResult;
using search::aggregation::SumAggregationResult;
using search::aggregation::StandardDeviationAggregationResult;
using search::aggregation::QuantileAggregationResult;
using search::aggregation::HitsAggregationResult;
using search::aggregation::XorAggregationResult;
using search::expression::AttributeNode;
using search::expression::AddFunctionNode;
using search::expression::AggregationRefNode;
using search::expression::BoolResultNode;
using search::expression::CatFunctionNode;
using search::expression::ConstantNode;
using search::expression::DivideFunctionNode;
using search::expression::FloatResultNode;
using search::expression::HoldString;
using search::expression::Int64ResultNode;
using search::expression::ModuloFunctionNode;
using search::expression::MultiplyFunctionNode;
using search::expression::NegateFunctionNode;
using search::expression::NormalizeSubjectFunctionNode;
using search::expression::ReverseFunctionNode;
using search::expression::StrLenFunctionNode;
using search::expression::XorFunctionNode;
using search::expression::StringResultNode;
using search::SerializedQueryTree;
using search::query::QueryBuilder;
using search::query::QueryToProtobuf;
using search::query::StringRange;
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


// These controllers are private implementation types defined by hardening.cc.
class ResultFaultController;
class FailpointController;

struct sk_feature_value {
    vespalib::FeatureSet::Value value;
};

enum class GroupingAggregationKind : uint32_t {
    COUNT,
    AVERAGE,
    SUM,
    MIN,
    MAX,
    STDDEV,
    XOR,
    QUANTILES,
    SUMMARY,
};

struct sk_grouping_expression {
    std::unique_ptr<search::expression::ExpressionNode> expression;
};

struct sk_grouping_aggregation {
    GroupingAggregationKind kind = GroupingAggregationKind::COUNT;
    std::unique_ptr<search::expression::ExpressionNode> expression;
    std::vector<double> quantiles;
    std::string summary_class;
    uint32_t max_hits = 0;
};

struct sk_grouping_filter {
    std::string pattern;
    std::unique_ptr<search::expression::ExpressionNode> expression;
    std::unique_ptr<search::expression::FilterPredicateNode> predicate;
};

struct sk_grouping_order_by {
    std::unique_ptr<search::expression::ExpressionNode> expression;
    bool ascending = true;
};

struct sk_grouping_level {
    std::unique_ptr<search::expression::ExpressionNode> classifier;
    std::vector<sk_grouping_aggregation> aggregations;
    int64_t max_groups = -1;
    int64_t precision = -1;
    std::optional<sk_grouping_filter> filter;
    std::vector<sk_grouping_order_by> order_by;
};

struct sk_grouping_plan {
    std::vector<std::unique_ptr<sk_grouping_level>> levels;
    std::vector<sk_grouping_aggregation> root_aggregations;
    bool all = false;
};

struct sk_grouping_result_value {
    uint32_t kind = 0;
    std::string string;
    int64_t integer = 0;
    double floating = 0.0;
    bool boolean = false;
    std::vector<std::pair<double, double>> quantiles;
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    std::shared_ptr<const ResultFaultController> result_faults;
#endif
};

struct sk_grouping_tree_output {
    uint32_t kind = SK_GROUPING_TREE_OUTPUT_VALUE;
    std::unique_ptr<sk_grouping_result_value> value;
    struct HitField {
        std::string name;
        uint32_t kind = SK_VALUE_STRING;
        std::string string;
        int64_t integer = 0;
        double floating = 0.0;
        bool boolean = false;
    };
    struct Hit {
        std::string id;
        std::string global_id;
        std::string external_id;
        uint32_t local_id = 0;
        bool has_local_id = false;
        double score = 0.0;
        std::vector<HitField> summary;
    };
    std::string summary_class;
    std::vector<Hit> hits;
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    std::shared_ptr<const ResultFaultController> result_faults;
#endif
};

struct sk_grouping_tree_node {
    std::unique_ptr<sk_grouping_result_value> key;
    std::vector<std::unique_ptr<sk_grouping_tree_output>> outputs;
    std::vector<std::unique_ptr<sk_grouping_tree_node>> groups;
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    std::shared_ptr<const ResultFaultController> result_faults;
#endif
};

struct sk_grouping_tree_result {
    std::unique_ptr<sk_grouping_tree_node> root;
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    std::shared_ptr<const ResultFaultController> result_faults;
    std::shared_ptr<std::atomic<uint64_t>> result_destroyed;

    sk_grouping_tree_result() = default;
    sk_grouping_tree_result(const sk_grouping_tree_result &) = delete;
    sk_grouping_tree_result &operator=(const sk_grouping_tree_result &) = delete;
    sk_grouping_tree_result(sk_grouping_tree_result &&) = default;
    sk_grouping_tree_result &operator=(sk_grouping_tree_result &&) = default;
    ~sk_grouping_tree_result() {
        if (result_destroyed) {
            result_destroyed->fetch_add(1);
        }
    }
#endif
};

struct sk_nearest_neighbor_options {
    std::optional<double> distance_threshold;
    uint32_t explore_additional_hits = 0;
};

struct sk_struct_array_value {
    uint32_t kind = 0;
    std::string string;
    int64_t integer = 0;
    double floating = 0.0;
    bool boolean = false;
};
struct sk_struct_array {
    std::vector<std::vector<std::pair<std::string, sk_struct_array_value>>> elements;
};


struct QueryFieldMetadata {
    std::string name;
    std::string type;
    bool index = false;
    bool attribute = false;
    bool virtual_field = false;
};

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
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    std::shared_ptr<const FailpointController> failpoints;
#endif
};

struct OwnedValue {
    uint32_t kind = 0;
    std::string string;
    std::string tensor_type;
    int64_t integer = 0;
    double floating = 0.0;
    bool boolean = false;
    std::vector<double> tensor_cells;
    std::vector<std::string> strings;
    double latitude = 0.0;
    double longitude = 0.0;
    std::vector<std::pair<std::string, std::string>> predicate_features;
    std::string predicate_json;
    std::vector<std::vector<std::pair<std::string, OwnedValue>>> struct_elements;
};

struct DiagnosticTensorBlockState {
    std::vector<std::pair<std::string, std::string>> address;
    std::vector<double> values;
};

struct DiagnosticTensorState {
    uint32_t kind = 0;
    std::string type_spec;
    std::vector<DiagnosticTensorBlockState> blocks;
};

struct DocumentFieldState {
    std::string name;
    OwnedValue value;
    std::vector<TextOccurrence> occurrences;
    std::vector<std::vector<TextOccurrence>> array_occurrences;
};

struct ValueDocument {
    std::string id;
    std::string field;
    int64_t value = 0;
    std::vector<DocumentFieldState> fields;
    bool exists = false;
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    std::shared_ptr<const ResultFaultController> result_faults;
#endif
};

struct ValueUpdate {
    std::string id;
    std::string field;
    int64_t value = 0;
    std::vector<std::pair<std::string, std::optional<OwnedValue>>> fields;
    struct Arithmetic {
        std::string field;
        uint32_t operation = SK_ARITHMETIC_ADD;
        double operand = 0.0;
    };
    std::vector<Arithmetic> arithmetic;
    std::vector<std::pair<std::string, std::vector<TextOccurrence>>> annotated_fields;
    std::vector<std::pair<std::string, std::vector<std::vector<TextOccurrence>>>> annotated_array_fields;
    bool create_if_nonexistent = false;
};

struct AttributeDocumentState {
    std::string id;
    std::vector<std::pair<std::string, OwnedValue>> fields;
};

struct AttributeUpdateState {
    std::string id;
    std::vector<std::pair<std::string, OwnedValue>> assignments;
};

struct LocalAggregationSpec {
    uint32_t kind = 0;
    std::string field;
};

struct LocalGroupingRequestState {
    std::string group_field;
    uint32_t max_groups = std::numeric_limits<uint32_t>::max();
    uint32_t order = SK_GROUP_ORDER_NONE;
    std::vector<LocalAggregationSpec> aggregations;
};

struct LocalGroupState {
    OwnedValue key;
    uint64_t count = 0;
    std::vector<OwnedValue> aggregations;
};

struct LocalGroupingResultState {
    std::vector<LocalGroupState> groups;
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    std::shared_ptr<const FailpointController> failpoints;
    std::shared_ptr<const ResultFaultController> result_faults;
    std::shared_ptr<std::atomic<uint64_t>> result_destroyed;
#endif
};

struct LocalSearchRequestState {
    std::string rank_profile;
    uint32_t limit = 10;
    uint32_t offset = 0;
    uint64_t timeout_millis = 60000;
    uint32_t diagnostic_trace_level = 0;
    uint32_t diagnostic_structure_depth = 0;
    uint32_t diagnostic_matching_depth = 0;
    uint32_t diagnostic_first_phase_depth = 0;
    uint32_t diagnostic_second_phase_depth = 0;
    uint32_t diagnostic_raw_bytes = 0;
    uint32_t summary_kind = SK_SUMMARY_NONE;
    struct Sort {
        std::string field;
        uint32_t order = SK_SORT_ASCENDING;
        uint32_t missing = SK_SORT_MISSING_DEFAULT;
    };
    std::vector<Sort> sort;
    std::optional<std::string> raw_sort_spec;
    std::optional<std::string> summary_class;
    std::vector<std::string> summary_fields;
    std::vector<std::string> highlight_fields;
    std::vector<std::pair<std::string, double>> query_features;
    std::vector<std::pair<std::string, std::string>> query_tensors;
    std::vector<std::pair<std::string, std::string>> rank_properties;
    std::vector<LocalGroupingRequestState> groupings;
    // Generic requests own copies of structured plans, so the opaque plan
    // handles used to build them may be released before the match begins.
    std::vector<std::unique_ptr<sk_grouping_plan>> grouping_plans;
};

struct LocalHighlightSpanState {
    uint32_t start_byte = 0;
    uint32_t end_byte = 0;
};

struct LocalHighlightFragmentState {
    std::string text;
    std::vector<LocalHighlightSpanState> matches;
};

struct LocalHighlightState {
    std::string field;
    std::vector<LocalHighlightFragmentState> fragments;
};

struct LocalHitState {
    std::string id;
    double score = 0.0;
    std::vector<std::pair<std::string, OwnedValue>> summary;
    std::vector<LocalHighlightState> highlights;
};

struct LocalSearchResultState {
    std::string diagnostic_trace;
    sk_diagnostics::Sections diagnostic_sections;
    bool diagnostic_truncated = false;
    bool diagnostic_features_truncated = false;
    std::vector<std::string> diagnostic_truncated_feature_names;
    double diagnostic_matching_seconds = 0.0;
    double diagnostic_summary_seconds = 0.0;
    double diagnostic_query_seconds = 0.0;
    uint64_t total_hit_count = 0;
    uint64_t coverage_active = 0;
    uint64_t coverage_covered = 0;
    uint64_t coverage_target_active = 0;
    uint32_t degradation_flags = 0;
    std::vector<LocalHitState> hits;
    std::vector<std::string> match_feature_names;
    std::vector<sk_feature_value> match_feature_values;
    std::vector<std::string> issues;
    std::vector<LocalGroupingResultState> groupings;
    std::vector<std::unique_ptr<sk_grouping_tree_result>> grouping_trees;
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    std::shared_ptr<const FailpointController> failpoints;
    std::shared_ptr<const ResultFaultController> result_faults;
    std::shared_ptr<std::atomic<uint64_t>> result_destroyed;
#endif
};

class UnsupportedNativeError final : public std::runtime_error {
    const char *_detail_code;

public:
    UnsupportedNativeError(const char *detail_code, const std::string &message)
        : std::runtime_error(message), _detail_code(detail_code) {}

    const char *detail_code() const noexcept { return _detail_code; }
};

class TypeErrorNativeError final : public std::runtime_error {
public:
    explicit TypeErrorNativeError(const std::string &message)
        : std::runtime_error(message) {}
};

class DeadlineExceededNativeError final : public std::runtime_error {
public:
    explicit DeadlineExceededNativeError(const std::string &message)
        : std::runtime_error(message) {}
};
struct NativeQueryState {
    std::vector<QueryFieldMetadata> fields;
    mutable VespaQueryBuilder builder;
    mutable search::SerializedQueryTreeSP serialized_tree;
    std::optional<std::string> same_element_parent;
    uint32_t next_term_id = 1;
    mutable std::atomic_bool consumed = false;

    bool consume() const noexcept {
        return !consumed.exchange(true, std::memory_order_acq_rel);
    }

    void ensure_active() const {
        if (consumed.load(std::memory_order_acquire)) {
            throw std::invalid_argument("query handle has already been consumed");
        }
    }

    const search::SerializedQueryTreeSP &serialized_query_tree() const {
        if (!serialized_tree) {
            auto tree = builder.build();
            if (!tree) {
                throw std::invalid_argument("could not build query tree");
            }
            // Vespa's stack-dump format does not carry SameElement's
            // element_filter, so the matching path silently broadens a
            // filtered query. The protobuf form preserves that field.
            auto proto = std::make_unique<SerializedQueryTree::ProtobufQueryTree>(
                QueryToProtobuf().serialize(*tree));
            serialized_tree = search::SerializedQueryTree::fromProtobuf(std::move(proto));
        }
        return serialized_tree;
    }

    int32_t allocate_term_id() {
        ensure_active();
        if (next_term_id > static_cast<uint32_t>(std::numeric_limits<int32_t>::max())) {
            throw std::invalid_argument("query term ID cannot be represented by Vespa");
        }
        return static_cast<int32_t>(next_term_id++);
    }
};
struct ValueRequest {
    std::string field;
    int64_t value;
    uint32_t limit;
};


struct ErrorResult {
    std::vector<std::string> ids;
#ifdef SK_ENABLE_HARDENING_FAILPOINTS
    std::shared_ptr<const FailpointController> failpoints;
#endif
};

class TensorValidationError final : public std::invalid_argument {
    const char *_detail_code;

public:
    TensorValidationError(const char *detail_code, const std::string &message)
        : std::invalid_argument(message), _detail_code(detail_code) {}

    const char *detail_code() const noexcept { return _detail_code; }
};
