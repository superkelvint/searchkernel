#pragma once

#include <vespa/vespalib/data/slime/slime.h>
#include <vespa/vespalib/data/slime/json_format.h>
#include <array>
#include <cmath>
#include <string>
#include <vector>

namespace sk_diagnostics {
// ABI 17, Search Kernel-owned schema. No borrowed Slime nodes escape projection.
enum Status : uint32_t { Available, Unsupported, UnavailableForQuery, Truncated, TimedOut, Failed };
struct Section { uint32_t status = UnavailableForQuery; std::string payload; };
struct Node {
    int32_t parent = -1;
    uint32_t thread = 0;
    std::string name;
    int64_t count = -1;
    double total = -1;
    double self = -1;
};
using Sections = std::array<Section, 5>;
using Inspector = vespalib::slime::Inspector;
constexpr size_t max_nodes = 128;
constexpr size_t max_name = 256;
constexpr size_t max_bytes = 65536;
constexpr size_t max_entries = 1024;

inline bool object(const Inspector& value) { return value.type().getId() == vespalib::slime::OBJECT::ID; }
inline bool array(const Inspector& value) { return value.type().getId() == vespalib::slime::ARRAY::ID; }
inline bool text(const Inspector& value) { return value.type().getId() == vespalib::slime::STRING::ID; }
inline bool number(const Inspector& value) {
    auto id = value.type().getId();
    return (id == vespalib::slime::DOUBLE::ID || id == vespalib::slime::LONG::ID) &&
           std::isfinite(value.asDouble()) && value.asDouble() >= 0;
}
inline bool equal(const Inspector& value, std::string_view expected) {
    auto memory = value.asString();
    return text(value) && std::string_view(memory.data, memory.size) == expected;
}

struct Builder {
    std::vector<Node> nodes;
    bool truncated = false;
    bool failed = false;
    bool observed = false;
    uint32_t depth_limit;
    size_t examined = 0;
    int32_t add(const Inspector& name, int32_t parent, uint32_t thread, uint32_t depth) {
        if (!text(name)) { failed = true; return -1; }
        if (depth > depth_limit || nodes.size() == max_nodes) { truncated = true; return -1; }
        auto memory = name.asString();
        if (memory.size > max_name) { truncated = true; return -1; }
        nodes.push_back({parent, thread, std::string(memory.data, memory.size), -1, -1, -1});
        return static_cast<int32_t>(nodes.size() - 1);
    }
    void profile_node(const Inspector& value, int32_t parent, uint32_t thread, uint32_t depth) {
        if (++examined > max_entries) { truncated = true; return; }
        if (!object(value) || value["count"].type().getId() != vespalib::slime::LONG::ID ||
            value["count"].asLong() < 0 || !number(value["total_time_ms"]) ||
            (value["self_time_ms"].valid() && !number(value["self_time_ms"]))) {
            failed = true; return;
        }
        auto index = add(value["name"], parent, thread, depth);
        if (index < 0) return;
        nodes[index].count = value["count"].asLong();
        nodes[index].total = value["total_time_ms"].asDouble();
        // Preserve absence on leaves. Never derive self time from a pruned tree.
        if (value["self_time_ms"].valid()) nodes[index].self = value["self_time_ms"].asDouble();
        auto& children = value["children"];
        if (!children.valid()) return;
        if (!array(children) || !value["self_time_ms"].valid()) { failed = true; return; }
        for (size_t i = 0; i < children.entries(); ++i) {
            if (nodes.size() == max_nodes || examined >= max_entries) { truncated = true; break; }
            profile_node(children[i], index, thread, depth + 1);
        }
    }
    void profile(const Inspector& report, uint32_t thread) {
        if (!equal(report["profiler"], "tree") || !number(report["total_time_ms"]) ||
            report["depth"].type().getId() != vespalib::slime::LONG::ID || report["depth"].asLong() < 0) {
            failed = true; return;
        }
        // A report with no roots is emitted even when a phase never executes.
        auto& roots = report["roots"];
        if ((roots.valid() && !array(roots)) ||
            (roots.entries() == 0 && report["total_time_ms"].asDouble() != 0)) { failed = true; return; }
        vespalib::Slime label;
        label.setString("profile");
        auto index = add(label.get(), -1, thread, 0);
        if (index < 0) return;
        nodes[index].total = report["total_time_ms"].asDouble();
        observed |= roots.entries() > 0;
        for (size_t i = 0; i < roots.entries(); ++i) {
            if (nodes.size() == max_nodes || examined >= max_entries) { truncated = true; break; }
            profile_node(roots[i], index, thread, 1);
        }
    }
    // Object2Slime's exact children/std::vector/[N] and unary search edges.
    // Other members (fields, estimates, term data) are not execution children.
    void plan(const Inspector& value, int32_t parent, uint32_t thread, uint32_t depth) {
        if (++examined > max_entries) { truncated = true; return; }
        if (!object(value)) { failed = true; return; }
        auto index = add(value["[type]"], parent, thread, depth);
        if (index < 0) return;
        observed = true;
        auto& children = value["children"];
        if (children.valid()) {
            if (!object(children) || !equal(children["[type]"], "std::vector")) { failed = true; return; }
            size_t count = children.fields() - 1;
            for (size_t i = 0; i < count; ++i) {
                if (nodes.size() == max_nodes || examined >= max_entries) { truncated = true; break; }
                auto& child = children["[" + std::to_string(i) + "]"];
                if (child.type().getId() == vespalib::slime::LONG::ID && child.asLong() >= 0 && child.asLong() <= 255 &&
                    (equal(value["[type]"], "search::queryeval::SourceBlenderSearchStrict") ||
                     equal(value["[type]"], "search::queryeval::SourceBlenderSearchNonStrict"))) {
                    // SourceBlenderSearch visits source IDs in children, with the
                    // corresponding iterator under its explicit Source N member.
                    plan(value["Source " + std::to_string(child.asLong())], index, thread, depth + 1);
                } else {
                    if (!object(child)) { failed = true; break; }
                    plan(child, index, thread, depth + 1);
                }
            }
        }
        if (value["search"].valid()) plan(value["search"], index, thread, depth + 1);
    }
    Section finish() const {
        Section section;
        section.status = failed ? Failed : truncated ? Truncated : observed ? Available : UnavailableForQuery;
        vespalib::Slime output;
        auto& root = output.setObject();
        root.setString("engine", "Vespa");
        root.setString("revision", SK_PINNED_VESPA_COMMIT);
        root.setLong("native_abi", SK_NATIVE_ABI_VERSION);
        auto& list = root.setArray("nodes");
        // Failed projections provide provenance but never partially trusted evidence.
        if (!failed) for (const auto& node : nodes) {
            auto& entry = list.addObject();
            entry.setLong("parent", node.parent);
            entry.setLong("thread", node.thread);
            entry.setString("name", node.name);
            if (node.count >= 0) entry.setLong("count", node.count);
            if (node.total >= 0) entry.setDouble("total_ms", node.total);
            if (node.self >= 0) entry.setDouble("self_ms", node.self);
        }
        // Construction above is bounded: <=128 names of <=256 bytes, fixed fields.
        section.payload = output.toString();
        if (section.payload.size() > max_bytes) {
            vespalib::Slime provenance;
            auto& metadata = provenance.setObject();
            metadata.setString("engine", "Vespa");
            metadata.setString("revision", SK_PINNED_VESPA_COMMIT);
            metadata.setLong("native_abi", SK_NATIVE_ABI_VERSION);
            metadata.setArray("nodes");
            section.payload = provenance.toString();
            section.status = Truncated;
        }
        return section;
    }
};

inline Sections project(const Inspector& root, const std::array<uint32_t, 3>& profile_depths,
                        uint32_t structural_depth) {
    std::array<Builder, 5> builders{{{{}, false, false, false, profile_depths[0]},
                                   {{}, false, false, false, profile_depths[1]},
                                   {{}, false, false, false, profile_depths[2]},
                                   {{}, false, false, false, structural_depth},
                                   {{}, false, false, false, structural_depth}}};
    auto& traces = root["traces"];
    if (!object(root) || (traces.valid() && !array(traces))) {
        for (auto& b : builders) b.failed = true;
    } else {
        size_t visited = 0;
        for (size_t i = 0; i < traces.entries(); ++i) {
            if (++visited > max_entries) { for (auto& b : builders) b.truncated = true; break; }
            auto& entry = traces[i];
            if (!object(entry)) { for (auto& b : builders) b.failed = true; break; }
            if (equal(entry["tag"], "query_execution_plan")) builders[4].plan(entry["optimized"], -1, 0, 0);
            if (!equal(entry["tag"], "query_execution")) continue;
            auto& threads = entry["threads"];
            if (!array(threads)) { for (auto& b : builders) b.failed = true; continue; }
            for (size_t t = 0; t < threads.entries(); ++t) {
                if (++visited > max_entries) { for (auto& b : builders) b.truncated = true; break; }
                auto& events = threads[t]["traces"];
                if (!object(threads[t]) || !array(events)) { for (auto& b : builders) b.failed = true; continue; }
                for (size_t j = 0; j < events.entries(); ++j) {
                    if (++visited > max_entries) { for (auto& b : builders) b.truncated = true; break; }
                    auto& event = events[j];
                    if (!object(event)) { for (auto& b : builders) b.failed = true; continue; }
                    constexpr std::array<std::string_view, 3> tags{"match_profiling", "first_phase_profiling", "second_phase_profiling"};
                    for (size_t k = 0; k < tags.size(); ++k) {
                        if (profile_depths[k] && equal(event["tag"], tags[k])) builders[k].profile(event, t);
                    }
                    if (equal(event["tag"], "iterator")) builders[3].plan(event["optimized"], -1, t, 0);
                }
            }
        }
    }
    Sections result;
    for (size_t i = 0; i < result.size(); ++i) result[i] = builders[i].finish();
    return result;
}
} // namespace sk_diagnostics
