// Direct pinned-Vespa oracle for the first recursive grouping ABI slice.
//
// Reuse the already-reviewed M9 embedded DocumentDB harness in the same
// translation unit. Renaming its main lets this verifier share fixture setup,
// query construction and deterministic JSON helpers without copying the whole
// DocumentDB harness into a second oracle.
#define main searchkernel_m9_flat_oracle_main_unused
#include "../../m9/oracle/m9_attribute_grouping_oracle.cpp"
#undef main

namespace {

void configure_recursive_grouping(SearchRequest& request) {
    auto category = GroupingLevel().setExpression(
        std::make_unique<AttributeNode>("category"));
    category.addResult(CountAggregationResult().setExpression(
        std::make_unique<AttributeNode>("category")));
    category.addResult(AverageAggregationResult().setExpression(
        std::make_unique<AttributeNode>("price")));

    auto qty = GroupingLevel().setExpression(
        std::make_unique<AttributeNode>("qty"));
    qty.addResult(CountAggregationResult().setExpression(
        std::make_unique<AttributeNode>("qty")));
    qty.addResult(AverageAggregationResult().setExpression(
        std::make_unique<AttributeNode>("price")));

    Grouping grouping;
    grouping.setFirstLevel(0)
            .setLastLevel(2)
            .addLevel(std::move(category))
            .addLevel(std::move(qty));

    vespalib::nbostream buffer;
    vespalib::NBOSerializer output(buffer);
    uint32_t grouping_count = 1;
    output << grouping_count;
    grouping.serialize(output);
    request.groupSpec.assign(buffer.data(), buffer.data() + buffer.size());
    request.maxhits = 0;
}

void emit_group_key(const Group& group) {
    const auto& key = group.getId();
    if (key.getClass().inherits(search::expression::StringResultNode::classId)) {
        HoldString value(key);
        std::cout << "{\"type\":\"string\",\"value\":"
                  << json_escape(std::string_view(value)) << '}';
    } else if (key.getClass().inherits(Int64ResultNode::classId)) {
        std::cout << "{\"type\":\"integer\",\"value\":"
                  << key.getInteger() << '}';
    } else {
        fail("unexpected recursive grouping key type");
    }
}

void emit_recursive_group(const Group& group, uint32_t depth) {
    require(depth < 2, "unexpected recursive grouping depth");
    std::cout << "{\"key\":";
    emit_group_key(group);
    std::cout << ",\"outputs\":[";

    const auto& count = group.getAggregationResult(0);
    require(count.getClass().inherits(CountAggregationResult::classId),
            "recursive grouping count missing");
    std::cout << "{\"type\":\"integer\",\"value\":"
              << static_cast<const CountAggregationResult&>(count).getCount()
              << "},";

    const auto& average = group.getAggregationResult(1);
    require(average.getClass().inherits(AverageAggregationResult::classId),
            "recursive grouping average missing");
    const auto value = static_cast<const AverageAggregationResult&>(average)
                           .getAverage()
                           .getFloat();
    std::cout << "{\"type\":\"float\",\"bits\":\""
              << bits(value) << "\"}";

    std::cout << "],\"groups\":[";
    for (uint32_t i = 0; i < group.getChildrenSize(); ++i) {
        if (i) std::cout << ',';
        emit_recursive_group(group.getChild(i), depth + 1);
    }
    std::cout << "]}";
}

int run_recursive(const std::filesystem::path& path) {
    const auto scenario = read_json(path);
    const auto& root = scenario.get();
    const auto id = string_value(field(root, "id"), "scenario id");

    // Reuse the frozen M9 fixture schema: category:string, qty:integer,
    // price:float, active:boolean. The scenario itself contains only four
    // documents so this verifier stays focused on recursive grouping.
    std::vector<FieldSpec> specs = {
        {"category", "string", true, true},
        {"qty", "integer", true, true},
        {"price", "float", true, true},
        {"active", "boolean", true, true},
    };

    DummyFileHeaderContext::setCreator("searchkernel_recursive_grouping_oracle");
    std::filesystem::remove_all("typea");
    std::filesystem::remove_all("tmp");
    std::filesystem::create_directory("typea");

    Fixture fixture(required_env("M9_ORACLE_FIXTURE_ROOT"), specs);

    const auto& documents = field(root, "documents");
    require(documents.type() == vespalib::slime::ARRAY::instance,
            "documents must be an array");
    for (size_t i = 0; i < documents.children(); ++i) {
        fixture.put(documents[i]);
    }

    auto request = std::make_shared<SearchRequest>();
    request->setTimeout(60s);
    request->ranking = "default";
    request->propertiesMap.lookupCreate(search::MapNames::RANK)
        .add("vespa.matching.numthreadspersearch", "1");

    // Match the same 0 <= price <= 100 predicate used by the native probe.
    QueryBuilder<proton::matching::ProtonNodeTypes> builder;
    builder.addRangeTerm(Range("[0;100]"), "price", 1, Weight(1));
    auto tree = builder.build();
    require(static_cast<bool>(tree), builder.error());
    request->setSerializedQueryTree(
        SerializedQueryTree::fromStackDump(StackDumpCreator::create(*tree)));

    configure_recursive_grouping(*request);
    auto reply = fixture.match(*request);
    require(reply->hits.empty(), "recursive grouping returned ordinary hits");
    require(!reply->groupResult.empty(), "recursive grouping result missing");

    vespalib::nbostream buffer(reply->groupResult.data(), reply->groupResult.size());
    vespalib::NBOSerializer input(buffer);
    uint32_t result_count = 0;
    input >> result_count;
    require(result_count == 1, "unexpected grouping result count");
    Grouping result;
    result.deserialize(input);

    std::cout << "{\"id\":" << json_escape(id) << ",\"groups\":[";
    const auto& grouping_root = result.getRoot();
    for (uint32_t i = 0; i < grouping_root.getChildrenSize(); ++i) {
        if (i) std::cout << ',';
        emit_recursive_group(grouping_root.getChild(i), 0);
    }
    std::cout << "]}\n" << std::flush;
    return 0;
}

} // namespace

int main(int argc, char** argv) {
    try {
        require(argc == 2,
                "usage: m9_recursive_grouping_oracle SCENARIO_JSON");
        return run_recursive(argv[1]);
    } catch (const std::exception& error) {
        std::cerr << "m9_recursive_grouping_oracle: " << error.what() << '\n';
        std::filesystem::remove_all("typea");
        std::filesystem::remove_all("tmp");
        return 2;
    }
}
