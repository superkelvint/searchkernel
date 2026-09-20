// Direct pinned-Vespa SearchReply oracle. This translation unit deliberately
// uses the embedded Proton DocumentDB and never calls Search Kernel code.
#define main searchreply_embedded_m8_main
#include "../../m8/oracle/m8_local_oracle.cpp"
#undef main

#include <filesystem>
#include <iostream>
#include <string>
#include <vector>

namespace {

constexpr uint32_t MATCH_PHASE = 0x01U;
constexpr uint32_t TIMEOUT = 0x02U;
constexpr uint32_t ANN_TIMEOUT = 0x04U;

void emit_reply(const char *name, const search::engine::SearchReply &reply) {
    require(!reply.hits.empty(), std::string(name) + " reply has no usable hits");
    uint32_t flags = 0;
    if (reply.coverage.wasDegradedByMatchPhase()) flags |= MATCH_PHASE;
    if (reply.coverage.wasDegradedByTimeout()) flags |= TIMEOUT;
    if (reply.coverage.was_degraded_by_ann_timeout()) flags |= ANN_TIMEOUT;
    std::cout << '"' << name << R"(":{"usable":true,"total_hit_count":)"
              << reply.totalHitCount << R"(,"hit_count":)" << reply.hits.size()
              << R"(,"coverage":{"active":)" << reply.coverage.getActive()
              << R"(,"covered":)" << reply.coverage.getCovered()
              << R"(,"target_active":)" << reply.coverage.getTargetActive()
              << R"(},"degradation_flags":)" << flags << R"(,"issues":[)";
    bool first = true;
    if (reply.my_issues) {
        reply.my_issues->for_each_message([&](const std::string &message) {
            if (!first) std::cout << ',';
            first = false;
            std::cout << json_escape(message);
        });
    }
    std::cout << "]}";
}

int run(const std::filesystem::path &scenario_path,
        const std::filesystem::path &data_path,
        const std::filesystem::path &config_path) {
    auto scenario = read_json(scenario_path);
    const auto &root = scenario.get();
    const std::vector<FieldSpec> specs = {
        {"title", "text", true, false, true},
        {"qty", "integer", false, true, true},
    };
    std::filesystem::create_directories(data_path);
    std::filesystem::current_path(data_path);
    std::filesystem::remove_all("typea");
    std::filesystem::remove_all("tmp");
    std::filesystem::create_directory("typea");
    DummyFileHeaderContext::setCreator("searchkernel_searchreply_cpp_oracle");
    Fixture fixture(config_path, specs);
    fixture.set_specs(&specs);

    const auto &documents = object_field(root, "documents");
    for (size_t i = 0; i < documents.children(); ++i) fixture.put(documents[i]);
    auto normal = fixture.search(object_field(root, "normal_request"));

    const auto &degraded_documents = object_field(root, "degraded_documents");
    for (size_t i = 0; i < degraded_documents.children(); ++i) {
        fixture.put(degraded_documents[i]);
    }
    const auto repeats = integer_value(object_field(root, "repeat_degraded_documents"),
                                       "repeat_degraded_documents");
    require(repeats >= 0, "repeat_degraded_documents must be nonnegative");
    for (int64_t repeat = 0; repeat < repeats; ++repeat) {
        const auto id = "id:test:typea::degraded-repeat-" + std::to_string(repeat);
        // The fixed ten-document block is repeated by constructing the same
        // minimal document through a tiny parsed Slime value.
        Slime one;
        const std::string encoded = "{\"id\":\"" + id +
                                    "\",\"fields\":{\"qty\":{\"integer\":18}}}";
        require(JsonFormat::decode(Memory(encoded), one) == encoded.size(),
                "repeated document JSON did not decode completely");
        fixture.put(one.get());
    }
    auto degraded = fixture.search(object_field(root, "degraded_request"));

    std::cout << "{";
    emit_reply("normal", *normal);
    std::cout << ',';
    emit_reply("degraded", *degraded);
    std::cout << "}\n";
    return 0;
}

} // namespace

int main(int argc, char **argv) {
    try {
        require(argc == 4, "usage: searchreply_oracle SCENARIO DATA_DIR CONFIG_DIR");
        return run(argv[1], argv[2], argv[3]);
    } catch (const std::exception &error) {
        std::cerr << "searchreply_oracle: " << error.what() << '\n';
        return 2;
    }
}
