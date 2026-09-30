# frozen_string_literal: true

require "json"
$LOAD_PATH.unshift(File.expand_path("../lib", __dir__))
require "searchkernel"

def trace_value(value)
  case value
  when nil, true, false
    value
  when String
    value.encoding == Encoding::ASCII_8BIT ? value.bytes.map(&:to_s) : value
  when Integer
    value.to_s
  when Float
    return "nan" if value.nan?
    return "+inf" if value.infinite? == 1
    return "-inf" if value.infinite? == -1

    value
  when Array
    value.map { |item| trace_value(item) }
  when Hash
    value.keys.sort.each_with_object({}) do |key, normalized|
      normalized[key] = trace_value(value.fetch(key))
    end
  else
    raise "unsupported trace value: #{value.class}"
  end
end

def trace_node(node)
  return nil if node.nil?

  if node.is_a?(SearchKernel::Results::Hit)
    return {
      "kind" => "hit",
      "id" => node.id,
      "relevance" => trace_value(node.relevance),
      "fields" => trace_value(node.fields),
      "highlights" => node.highlights.map do |highlight|
        {
          "field" => highlight.fetch("field"),
          "fragments" => highlight.fetch("fragments").map do |fragment|
            {
              "text" => fragment.fetch("text"),
              "matches" => fragment.fetch("matches").map do |match|
                { "start_byte" => match[0].to_s, "end_byte" => match[1].to_s }
              end
            }
          end
        }
      end,
      "source" => node.source,
      "types" => node.types,
      "match_features" => trace_value(node.match_features),
      "summary_features" => trace_value(node.summary_features),
      "rank_features" => trace_value(node.rank_features),
      "sort_values" => trace_value(node.sort_values)
    }
  end

  {
    "kind" => "group",
    "id" => node.id,
    "label" => node.label,
    "value" => trace_value(node.value),
    "fields" => trace_value(node.fields),
    "children" => node.children.map { |child| trace_node(child) },
    "continuations" => node.continuations.map { |item| { "key" => item.key, "value" => item.value } },
    "source" => node.source,
    "types" => node.types,
    "relevance" => trace_value(node.relevance),
    "limits" => node.limits.nil? ? nil : {
      "from" => trace_value(node.limits.from),
      "to" => trace_value(node.limits.to)
    },
    "metadata" => trace_value(node.metadata)
  }
end

def trace_relation(relation)
  {
    "Unspecified" => "unspecified",
    "Exact" => "exact",
    "LowerBound" => "lower_bound",
    "Approximate" => "approximate",
    "Unknown" => "unknown",
    "unknown" => "unknown"
  }.fetch(relation)
end

def trace_coverage(coverage)
  return nil if coverage.nil?

  degraded = coverage.degraded
  {
    "coverage_percent" => trace_value(coverage.coverage_percent),
    "documents" => trace_value(coverage.documents),
    "full" => coverage.full,
    "nodes" => trace_value(coverage.nodes),
    "results" => trace_value(coverage.results),
    "results_full" => trace_value(coverage.results_full),
    "degraded" => degraded.nil? ? nil : {
      "match_phase" => degraded.match_phase,
      "timeout" => degraded.timeout,
      "adaptive_timeout" => degraded.adaptive_timeout,
      "ann_timeout" => degraded.ann_timeout,
      "non_ideal_state" => degraded.non_ideal_state,
      "metadata" => trace_value(degraded.metadata)
    },
    "metadata" => trace_value(coverage.metadata)
  }
end

def trace_timing(timing)
  return nil if timing.nil?

  {
    "query_time_seconds" => trace_value(timing.query_time_seconds),
    "search_time_seconds" => trace_value(timing.search_time_seconds),
    "summary_fetch_time_seconds" => trace_value(timing.summary_fetch_time_seconds),
    "metadata" => trace_value(timing.metadata)
  }
end

def trace_result(result)
  {
    "root" => trace_node(result.root),
    "total_hits" => {
      "value" => result.total_hits.to_s,
      "relation" => trace_relation(result.total_hits_relation)
    },
    "coverage" => trace_coverage(result.coverage),
    "timing" => trace_timing(result.timing),
    "errors" => result.errors.map do |error|
      {
        "code" => error.code.to_s,
        "summary" => error.summary,
        "message" => error.message,
        "source" => error.source,
        "stack_trace" => error.stack_trace,
        "transient" => error.transient,
        "metadata" => trace_value(error.metadata)
      }
    end,
    "trace" => trace_value(result.trace),
    "metadata" => trace_value(result.metadata)
  }
end

cases = JSON.parse(File.read(ENV.fetch("SEARCHKERNEL_RESULT_CASES")))
case_directory = ENV.fetch("SEARCHKERNEL_RESULT_CASE_DIR")
traces = {}
cases.fetch("valid").each do |scenario|
  wire = JSON.parse(File.read(File.join(case_directory, scenario.fetch("json"))))
  payload = SearchKernel::Protocol.expect_response(:search, wire)
  result = SearchKernel::Results::SearchResults.new(payload)
  raise "SearchResult root missing: #{scenario.fetch('id')}" if result.root.nil?

  traces[scenario.fetch("id")] = trace_result(result)
end

# Keep a few high-risk shared fixtures explicit in the Ruby suite rather than
# relying only on whole-trace equality. These assertions document the semantic
# invariants that are easiest for a permissive decoder to erase.
infinite_children = traces.fetch("infinite-relevance").dig("root", "children")
raise "shared positive infinity relevance was not preserved" unless infinite_children.fetch(1).fetch("relevance") == "+inf"
raise "shared negative infinity relevance was not preserved" unless infinite_children.fetch(2).fetch("relevance") == "-inf"

empty = traces.fetch("empty")
raise "shared empty result gained children" unless empty.dig("root", "children") == []
raise "shared empty result gained coverage" unless empty.fetch("coverage").nil?
raise "shared empty result gained timing" unless empty.fetch("timing").nil?
raise "shared empty result metadata changed" unless empty.fetch("metadata") == {}

tensor_trace = traces.fetch("sparse-mixed-tensors")
sparse = tensor_trace.dig("root", "children", 0, "fields", "sparse", "$searchkernel_tensor")
raise "shared sparse tensor type was not preserved" unless sparse.fetch("type_spec") == "tensor<float>(label{},region{})"
raise "shared sparse tensor values were not preserved" unless sparse.fetch("sparse").map { |cell| cell.fetch("value") } == [0.5, -0.25]
raise "shared sparse tensor addresses were not preserved" unless sparse.fetch("sparse").first.fetch("address") == [
  { "dimension" => "label", "label" => "a" },
  { "dimension" => "region", "label" => "east" }
]
mixed = tensor_trace.dig("root", "value", "$searchkernel_tensor")
raise "shared mixed tensor type was not preserved" unless mixed.fetch("type_spec") == "tensor<float>(label{},region{},x[2])"
raise "shared mixed tensor blocks were not preserved" unless mixed.fetch("mixed").map { |block| block.fetch("values") } == [[0.25, 0.75], [1.5, -2.0]]

outcomes = {}
cases.fetch("malformed").each do |scenario|
  if scenario.fetch("bridge_rejected")
    outcomes[scenario.fetch("id")] = "bridge"
    next
  end

  begin
    wire = JSON.parse(File.read(File.join(case_directory, scenario.fetch("json"))))
    payload = SearchKernel::Protocol.expect_response(:search, wire)
    SearchKernel::Results::SearchResults.new(payload)
  rescue StandardError
    outcomes[scenario.fetch("id")] = "client"
    next
  end
  raise "malformed result was accepted: #{scenario.fetch('id')}"
end

%w[
  unknown-tensor-payload-union
  non-finite-dense-tensor-cell
  non-finite-sparse-tensor-cell
  non-finite-mixed-tensor-cell
  tensor-dense-nan
  tensor-sparse-positive-infinity
  tensor-mixed-negative-infinity
].each do |scenario_id|
  outcome = outcomes.fetch(scenario_id)
  raise "shared malformed tensor fixture was not rejected: #{scenario_id}" unless %w[bridge client].include?(outcome)
end

traces["malformed_outcomes"] = outcomes
File.write(File.join(ENV.fetch("SEARCHKERNEL_RESULT_TRACE_DIR"), "ruby.json"), JSON.generate(traces))
