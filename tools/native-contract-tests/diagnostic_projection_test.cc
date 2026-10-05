#include "searchkernel_vespa.h"
#include "diagnostic_projection.h"
#include <vespa/searchlib/engine/trace.h>
#include <vespa/searchlib/queryeval/emptysearch.h>
#include <vespa/searchlib/queryeval/andsearch.h>
#include <vespa/searchlib/queryeval/intermediate_blueprints.h>
#include <vespa/searchlib/queryeval/leaf_blueprints.h>
#include <vespa/vespalib/util/execution_profiler.h>

using namespace sk_diagnostics;
using namespace vespalib::literals;

extern "C" int searchkernel_diagnostic_projection_regression() {
    using namespace search::engine;
    RelativeTime time(std::make_unique<SteadyClock>());
    Trace trace(time, 7);
    auto thread = trace.make_trace();
    vespalib::ExecutionProfiler profiler(8);
    auto parent = profiler.resolve("parent");
    auto child = profiler.resolve("child");
    {
        vespalib::ExecutionProfiler::TaskGuard outer(profiler, parent);
        for (int i = 0; i < 3; ++i) { vespalib::ExecutionProfiler::TaskGuard inner(profiler, child); }
    }
    for (auto tag : {"match_profiling", "first_phase_profiling", "second_phase_profiling"}) {
        profiler.report(thread.createCursor(tag));
    }
    search::queryeval::MultiSearch::Children iterators;
    iterators.push_back(std::make_unique<search::queryeval::EmptySearch>());
    iterators.push_back(std::make_unique<search::queryeval::EmptySearch>());
    auto iterator = search::queryeval::AndSearch::create(std::move(iterators), true);
    vespalib::slime::ObjectInserter iterator_inserter(thread.createCursor("iterator"), "optimized");
    iterator->asSlime(iterator_inserter);
    search::queryeval::AndBlueprint blueprint;
    blueprint.addChild(std::make_unique<search::queryeval::EmptyBlueprint>());
    blueprint.addChild(std::make_unique<search::queryeval::EmptyBlueprint>());
    vespalib::slime::ObjectInserter blueprint_inserter(trace.createCursor("query_execution_plan"), "optimized");
    blueprint.asSlime(blueprint_inserter);
    auto insertion = trace.make_inserter("query_execution"_ssv);
    insertion.handle_thread(thread);
    insertion.handle_thread(thread); // Preserve two separate report trees.
    auto sections = project(trace.getSlime().get(), {8,8,8}, 8);
    for (size_t k = 0; k < 3; ++k) {
        if (sections[k].status != Available) return 1;
        vespalib::Slime decoded;
        vespalib::slime::JsonFormat::decode(vespalib::Memory(sections[k].payload), decoded);
        const auto& nodes = decoded.get()["nodes"];
        if (nodes.entries() != 6 || !equal(nodes[1]["name"], "parent") || nodes[1]["count"].asLong() != 1 ||
            nodes[2]["count"].asLong() != 3 || nodes[2]["self_ms"].valid()) return 2;
        const auto& report = trace.getSlime().get()["traces"][1]["threads"][0]["traces"][k];
        if (nodes[0]["total_ms"].asDouble() != report["total_time_ms"].asDouble() ||
            nodes[1]["self_ms"].asDouble() != report["roots"][0]["self_time_ms"].asDouble() ||
            nodes[3]["thread"].asLong() != 1) return 3;
        if (!equal(decoded.get()["revision"], SK_PINNED_VESPA_COMMIT) ||
            !equal(decoded.get()["engine"], "Vespa") || decoded.get()["native_abi"].asLong() != SK_NATIVE_ABI_VERSION) return 4;
    }
    if (sections[3].status != Available || sections[4].status != Available) return 5;
    for (size_t k = 3; k < 5; ++k) {
        vespalib::Slime decoded;
        vespalib::slime::JsonFormat::decode(vespalib::Memory(sections[k].payload), decoded);
        const auto& nodes = decoded.get()["nodes"];
        if (nodes.entries() != (k == 3 ? 6 : 3) || nodes[1]["parent"].asLong() != 0) return 12;
    }
    auto bounded = project(trace.getSlime().get(), {1,1,1}, 0);
    if (bounded[0].status != Truncated || bounded[3].status != Truncated || bounded[4].status != Truncated) return 6;
    auto disabled = project(trace.getSlime().get(), {0,0,0}, 8);
    if (disabled[0].status != UnavailableForQuery || disabled[4].status != Available) return 7;

    // Regression: actual Trace/LazyTraceInserter path with malformed report.
    Trace malformed(time, 7);
    auto bad_thread = malformed.make_trace();
    auto& bad = bad_thread.createCursor("first_phase_profiling");
    bad.setString("profiler", "unexpected");
    bad.setDouble("total_time_ms", 12);
    malformed.make_inserter("query_execution"_ssv).handle_thread(bad_thread);
    auto rejected = project(malformed.getSlime().get(), {8,8,8}, 8);
    if (rejected[1].status != Failed || rejected[0].status != UnavailableForQuery) return 8;
    // Magic tags elsewhere are not engine diagnostic reports.
    malformed.createCursor("unrelated").setString("first_phase_profiling", "not evidence");
    if (project(malformed.getSlime().get(), {8,8,8}, 8)[0].status != UnavailableForQuery) return 9;
    Trace empty(time, 7);
    auto empty_thread = empty.make_trace();
    vespalib::ExecutionProfiler inactive(8);
    inactive.report(empty_thread.createCursor("second_phase_profiling"));
    empty.make_inserter("query_execution"_ssv).handle_thread(empty_thread);
    if (project(empty.getSlime().get(), {8,8,8}, 8)[2].status != UnavailableForQuery) return 10;
    // Saturate sibling nodes; no unbounded allocation or recursive report summation.
    Trace large(time, 7);
    auto large_thread = large.make_trace();
    vespalib::ExecutionProfiler wide(8);
    for (int i = 0; i < 256; ++i) {
        auto id = wide.resolve("task" + std::to_string(i));
        vespalib::ExecutionProfiler::TaskGuard guard(wide, id);
    }
    wide.report(large_thread.createCursor("match_profiling"));
    large.make_inserter("query_execution"_ssv).handle_thread(large_thread);
    auto limited = project(large.getSlime().get(), {8,8,8}, 8);
    if (limited[0].status != Truncated || limited[0].payload.size() > max_bytes || limited[1].status != UnavailableForQuery) return 11;
    return 0;
}
