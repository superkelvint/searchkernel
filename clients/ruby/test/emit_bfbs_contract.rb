# frozen_string_literal: true

require "json"

$LOAD_PATH.unshift(File.expand_path("../lib", __dir__))
require "searchkernel"

def emit_case(schema, name, value)
  puts [schema, name, JSON.generate(value)].join("\t")
end

SearchKernel::Protocol::OPERATIONS.each do |operation, spec|
  emit_case(
    "request",
    "#{operation}-request",
    SearchKernel::Protocol.request_envelope(operation, {})
  )
  emit_case(
    "response",
    "#{operation}-response",
    {
      "protocol_version" => SearchKernel::Protocol::VERSION,
      "payload_type" => SearchKernel::Protocol.wire_type(spec.fetch(:response)),
      "payload" => {}
    }
  )
end

emit_case(
  "response",
  "portable-error-defaults",
  {
    "protocol_version" => SearchKernel::Protocol::VERSION,
    "payload_type" => SearchKernel::Protocol::ERROR_TYPE,
    "payload" => { "message" => "bfbs error" }
  }
)

value = SearchKernel::Value
tensor = ->(encoding, payload) { value.encode(value.tensor("tensor<float>(x[2])", encoding, payload)) }
portable_fields = [
    { "key" => "min", "value" => value.encode(-(2**63)) },
    { "key" => "max", "value" => value.encode(2**63 - 1) },
    { "key" => "unsigned", "value" => value.encode(value.uint64(2**64 - 1)) },
    { "key" => "bytes", "value" => value.encode(value.bytes("\x00\xff".b)) },
    { "key" => "nested", "value" => value.encode([{ "x" => true }, nil, [1.5]]) },
    { "key" => "unicode_object", "value" => value.encode({ "😀" => 4, "é" => 3, "z" => 2, "a" => 1 }) },
    { "key" => "dense", "value" => tensor.call(:dense, [1.0, 2.0]) },
    { "key" => "sparse", "value" => tensor.call(:sparse, [{ "address" => [{ "dimension" => "x", "label" => "0" }], "value" => 1.0 }]) },
    { "key" => "mixed", "value" => tensor.call(:mixed, [{ "address" => [{ "dimension" => "paragraph", "label" => "a" }], "values" => [1.0, 2.0] }]) }
  ]
emit_case("request", "portable-values-request", SearchKernel::Protocol.request_envelope(:insert, {
  "document" => { "id" => "id:values", "fields" => portable_fields }
}))
emit_case("response", "portable-values-response", {
  "protocol_version" => 1, "payload_type" => "SearchKernel_Wire_GetResult",
  "payload" => { "document" => { "id" => "id:values", "fields" => portable_fields + [
    { "key" => "nan", "value" => { "kind_type" => "NonFiniteFloatValue", "kind" => { "kind" => "NaN" } } },
    { "key" => "pos_inf", "value" => { "kind_type" => "NonFiniteFloatValue", "kind" => { "kind" => "PositiveInfinity" } } }
  ] } }
})

def result_node(kind, payload)
  { "node_type" => kind, "node" => payload }
end

emit_case("response", "recursive-search-response", {
  "protocol_version" => 1,
  "payload_type" => "SearchKernel_Wire_SearchResult",
  "payload" => {
    "root" => result_node("ResultGroup", {
      "id" => "root", "fields" => [{ "key" => "count", "value" => value.encode(3) }], "metadata" => [], "children" => [
        result_node("Hit", { "id" => "a", "relevance" => { "value_type" => "PositiveInfinityRelevance", "value" => {} },
          "fields" => [{ "key" => "count", "value" => value.encode(value.uint64(2**64 - 1)) }], "sort_values" => [value.encode(2**63 - 1)],
          "match_features" => [], "summary_features" => [{ "key" => "summary", "value" => value.encode("ok") }],
          "rank_features" => [{ "key" => "rank", "value" => value.encode(0.25) }], "source" => "sample", "types" => ["document"],
          "highlights" => [{ "field" => "title", "fragments" => [{ "text" => "hello", "matches" => [{ "start_byte" => 0, "end_byte" => 5 }] }] }] }),
        result_node("ResultGroup", { "id" => "nested", "value" => value.encode("bucket"),
          "limits" => { "from" => value.encode(1), "to" => value.encode(9) }, "children" => [
          result_node("Hit", { "id" => "b", "relevance" => { "value_type" => "NegativeInfinityRelevance", "value" => {} } })
        ], "continuations" => [{ "key" => "next", "value" => "opaque" }] }),
        result_node("Hit", { "id" => "c" })
      ]
    }),
    "total_hits" => { "value" => (2**64 - 1).to_s, "relation" => "LowerBound" },
    "trace" => value.encode({ "events" => [] }), "metadata" => [],
    "coverage" => { "full" => { "value" => false }, "degraded" => { "timeout" => { "value" => true } } },
    "timing" => { "query_time_seconds" => { "value" => 0.5 } },
    "errors" => [{ "code" => "7", "summary" => "partial", "transient" => { "value" => true }, "metadata" => [] }]
  }
})
