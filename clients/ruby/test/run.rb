# frozen_string_literal: true

require "json"
require "socket"

$LOAD_PATH.unshift(File.expand_path("../lib", __dir__))
require "searchkernel"

module TestSupport
  module_function

  def assert(condition, message)
    raise message unless condition
  end

  def same(expected, actual, message)
    return if expected == actual

    raise "#{message}\nexpected: #{expected.inspect}\nactual:   #{actual.inspect}"
  end

  def raises(error_class, message)
    yield
  rescue error_class => error
    return error
  rescue StandardError => error
    raise "#{message}: expected #{error_class}, got #{error.class}: #{error.message}"
  else
    raise "#{message}: expected #{error_class}, nothing was raised"
  end

  def with_http_server(status: 200, body:, content_type: "application/json", content_length: body.bytesize)
    server = TCPServer.new("127.0.0.1", 0)
    port = server.addr[1]
    state = {}
    thread_error = nil

    thread = Thread.new do
      socket = nil
      begin
        socket = server.accept
        state[:request_line] = socket.gets&.strip
        headers = {}
        while (line = socket.gets)
          break if line == "\r\n"

          key, value = line.split(":", 2)
          headers[key.downcase] = value.to_s.strip
        end
        state[:headers] = headers
        length = Integer(headers.fetch("content-length", "0"), 10)
        state[:body] = socket.read(length)

        reason = status == 200 ? "OK" : "Error"
        socket.write("HTTP/1.1 #{status} #{reason}\r\n")
        socket.write("Content-Type: #{content_type}\r\n")
        socket.write("Content-Length: #{content_length}\r\n")
        socket.write("Connection: close\r\n\r\n")
        socket.write(body)
      rescue StandardError => error
        thread_error = error
      ensure
        socket&.close
      end
    end

    yield "http://127.0.0.1:#{port}", state
  ensure
    server&.close
    if thread
      thread.join(5)
      raise "test HTTP server did not terminate" if thread.alive?
    end
    raise thread_error if thread_error
  end

  def with_raw_http_server(raw_response)
    server = TCPServer.new("127.0.0.1", 0)
    port = server.addr[1]
    thread_error = nil

    thread = Thread.new do
      socket = nil
      begin
        socket = server.accept
        headers = {}
        socket.gets
        while (line = socket.gets)
          break if line == "\r\n"

          key, value = line.split(":", 2)
          headers[key.downcase] = value.to_s.strip
        end
        length = Integer(headers.fetch("content-length", "0"), 10)
        socket.read(length)
        socket.write(raw_response)
      rescue Errno::EPIPE, Errno::ECONNRESET
        # The hardened client may close immediately after detecting malformed HTTP.
      rescue StandardError => error
        thread_error = error
      ensure
        socket&.close
      end
    end

    yield "http://127.0.0.1:#{port}"
  ensure
    server&.close
    if thread
      thread.join(5)
      raise "test raw HTTP server did not terminate" if thread.alive?
    end
    raise thread_error if thread_error
  end
end

include TestSupport

def canonical_search_result(overrides = {})
  {
    "root" => { "node_type" => "ResultGroup", "node" => {} },
    "total_hits" => { "value" => 0, "relation" => "Exact" }
  }.merge(overrides)
end

portable = SearchKernel::Value
u64_boundary = portable.encode(portable.uint64(2**64 - 1))
i64_min_boundary = portable.encode(-(2**63))
i64_max_boundary = portable.encode(2**63 - 1)
same("18446744073709551615", u64_boundary.fetch("kind").fetch("value"), "uint64 JSON uses decimal text")
same("-9223372036854775808", i64_min_boundary.fetch("kind").fetch("value"), "int64 min JSON uses decimal text")
same("9223372036854775807", i64_max_boundary.fetch("kind").fetch("value"), "int64 max JSON uses decimal text")
same(2**64 - 1, portable.decode(u64_boundary), "uint64 boundary")
same(-(2**63), portable.decode(i64_min_boundary), "int64 boundary")
same(2**63 - 1, portable.decode(i64_max_boundary), "int64 max boundary")
same(true, JSON.generate(u64_boundary).include?('"value":"18446744073709551615"'),
     "Ruby HTTP JSON keeps uint64 max quoted")
same("\x00\xff".b, portable.decode(portable.encode(portable.bytes("\x00\xff".b))), "bytes retain octets")
same([{"key" => [nil, true, 3.0]}], portable.decode(portable.encode([{"key" => [nil, true, 3.0]}])), "nested values")
unicode_object = {"😀" => 4, "é" => 3, "z" => 2, "a" => 1}
unicode_wire = portable.encode(unicode_object)
unicode_order = ["a", "z", "é", "😀"]
same(unicode_order, unicode_wire.fetch("kind").fetch("fields").map { |entry| entry.fetch("key") },
     "object keys use canonical UTF-8 ordering including non-BMP Unicode")
same(unicode_order, portable.decode(unicode_wire).keys,
     "decoded object preserves canonical Unicode key ordering")
%i[dense sparse mixed].each do |encoding|
  payload = case encoding
  when :dense then [1.0, 2.0]
  when :sparse then [{"address" => [{"dimension" => "x", "label" => "0"}], "value" => 1.0}]
  when :mixed then [{"address" => [{"dimension" => "p", "label" => "a"}], "values" => [1.0]}]
  end
  wire = portable.encode(portable.tensor("tensor<float>(x[2])", encoding, payload))
  same(payload, portable.decode(wire).fetch("$searchkernel_tensor").fetch(encoding.to_s), "#{encoding} tensor")
end
raises(ArgumentError, "reject signed overflow") { portable.encode(2**63) }
raises(ArgumentError, "reject unsigned overflow") { portable.uint64(2**64) }
raises(ArgumentError, "reject non-finite request") { portable.encode(Float::INFINITY) }
empty_dense = portable.encode(portable.tensor("tensor<float>(x[8])", :dense, []))
same({}, empty_dense.fetch("kind").fetch("payload"), "empty dense tensor omits zero-length vector for BFBS JSON alignment")
same(
  [],
  portable.decode(empty_dense).fetch("$searchkernel_tensor").fetch("dense"),
  "empty dense tensor round-trips through public Value API"
)
raises(ArgumentError, "reject non-finite tensor") { portable.encode(portable.tensor("tensor(x[1])", :dense, [Float::NAN])) }
raises(ArgumentError, "reject overflowing tensor conversion") { portable.encode(portable.tensor("tensor(x[1])", :dense, [10**400])) }
raises(SearchKernel::ProtocolError, "reject missing discriminator") { portable.decode({"kind" => {"value" => 1}}) }
raises(SearchKernel::ProtocolError, "reject mismatched discriminator") do
  portable.decode({"kind_type" => "ValueList", "kind" => {"value" => [1]}})
end
raises(SearchKernel::ProtocolError, "reject duplicate entries before Hash conversion") do
  portable.entries([{"key" => "a", "value" => portable.encode(1)}, {"key" => "a", "value" => portable.encode(2)}])
end
raises(SearchKernel::ProtocolError, "reject malformed tensor address") do
  wire = portable.encode(portable.tensor("tensor(x[1])", :sparse, []))
  wire.fetch("kind").fetch("payload")["cells"] = [{"address" => [{}], "value" => 1.0}]
  portable.decode(wire)
end
raises(SearchKernel::ProtocolError, "reject malformed result node") do
  SearchKernel::Results::SearchResults.new(canonical_search_result(
    "root" => {"node_type" => "Hit", "node" => {"id" => "a"}},
    "metadata" => [{"key" => "x"}]
  ))
end
raises(SearchKernel::ProtocolError, "reject missing SearchResult root") do
  SearchKernel::Results::SearchResults.new({"total_hits" => {"value" => 0}})
end
raises(SearchKernel::ProtocolError, "reject missing SearchResult total hits") do
  SearchKernel::Results::SearchResults.new(
    "root" => {"node_type" => "ResultGroup", "node" => {"children" => []}}
  )
end

require "rubygems"
gemspec = Gem::Specification.load(File.expand_path("../searchkernel.gemspec", __dir__))
assert(!gemspec.nil?, "Ruby gemspec must load")
same(["Apache-2.0"], gemspec.licenses, "Ruby gemspec license metadata")

expected_operations = {
  insert: %w[InsertRequest InsertResult],
  upsert: %w[UpsertRequest UpsertResult],
  patch: %w[PatchRequest PatchResult],
  update: %w[UpdateRequest UpdateResult],
  delete: %w[DeleteRequest DeleteResult],
  get: %w[GetRequest GetResult],
  search: %w[SearchRequest SearchResult],
  analyze: %w[AnalyzeRequest AnalyzeResult],
  describe: %w[DescribeRequest DescribeResult],
  inspect_query: %w[InspectQueryRequest InspectQueryResult],
  create_index: %w[CreateIndexRequest CreateIndexResult],
  open_index: %w[OpenIndexRequest OpenIndexResult],
  delete_index: %w[DeleteIndexRequest DeleteIndexResult]
}

same(expected_operations.keys.sort, SearchKernel::Protocol::OPERATIONS.keys.sort, "all portable operations are mapped")
expected_operations.each do |operation, (request_type, response_type)|
  payload = { "marker" => operation.to_s }
  envelope = SearchKernel::Protocol.request_envelope(operation, payload)
  same(1, envelope["protocol_version"], "#{operation} request protocol version")
  same("SearchKernel_Wire_#{request_type}", envelope["payload_type"], "#{operation} request discriminator")
  same(payload, envelope["payload"], "#{operation} request payload")

  result = SearchKernel::Protocol.expect_response(
    operation,
    {
      "protocol_version" => 1,
      "payload_type" => "SearchKernel_Wire_#{response_type}",
      "payload" => { "ok" => operation.to_s }
    }
  )
  same({ "ok" => operation.to_s }, result, "#{operation} response discriminator")
end

targeted = SearchKernel::Protocol.request_envelope(:search, { "query" => {} }, target: "products")
same({ "name" => "products" }, targeted["target"], "targeted request carries logical daemon target")
targeted_delete = SearchKernel::Protocol.request_envelope(:delete_index, {}, target: "products")
same({ "name" => "products" }, targeted_delete["target"], "targeted delete carries logical daemon target")
invalid_but_representable = SearchKernel::Protocol.request_envelope(:search, {}, target: "Products")
same(
  { "name" => "Products" },
  invalid_but_representable["target"],
  "domain-invalid target reaches canonical semantic validation"
)

raises(ArgumentError, "unknown operation rejects") do
  SearchKernel::Protocol.request_envelope(:unknown, {})
end
raises(ArgumentError, "non-object request payload rejects") do
  SearchKernel::Protocol.request_envelope(:search, [])
end
raises(SearchKernel::ProtocolError, "non-object response rejects") do
  SearchKernel::Protocol.expect_response(:search, [])
end
raises(SearchKernel::ProtocolError, "future protocol rejects") do
  SearchKernel::Protocol.expect_response(
    :search,
    { "protocol_version" => 3, "payload_type" => "SearchKernel_Wire_SearchResult", "payload" => {} }
  )
end
raises(SearchKernel::ProtocolError, "missing union payload rejects") do
  SearchKernel::Protocol.expect_response(
    :search,
    { "protocol_version" => 1, "payload_type" => "SearchKernel_Wire_SearchResult" }
  )
end
raises(SearchKernel::ProtocolError, "wrong response union rejects") do
  SearchKernel::Protocol.expect_response(
    :search,
    { "protocol_version" => 1, "payload_type" => "SearchKernel_Wire_GetResult", "payload" => {} }
  )
end

portable_error = raises(SearchKernel::SearchKernelError, "canonical Error is portable error") do
  SearchKernel::Protocol.expect_response(
    :search,
    {
      "protocol_version" => 1,
      "payload_type" => "SearchKernel_Wire_Error",
      "payload" => {
        "code" => "InvalidArgument",
        "message" => "bad query",
        "details" => [{ "code" => "unknown_field", "path" => "query", "message" => "bad" }],
        "retryable" => false
      }
    }
  )
end
same("InvalidArgument", portable_error.code, "portable error code")
same(false, portable_error.retryable, "portable error retryable")
same("unknown_field", portable_error.details.first["code"], "portable error details")
default_error = raises(SearchKernel::SearchKernelError, "FlatBuffers default Error fields decode canonically") do
  SearchKernel::Protocol.expect_response(
    :search,
    {
      "protocol_version" => 1,
      "payload_type" => "SearchKernel_Wire_Error",
      "payload" => { "message" => "default fields" }
    }
  )
end
same("Unspecified", default_error.code, "omitted default error code")
same(false, default_error.retryable, "omitted default retryable")

raises(SearchKernel::ProtocolError, "malformed portable Error fails closed") do
  SearchKernel::Protocol.expect_response(
    :search,
    {
      "protocol_version" => 1,
      "payload_type" => "SearchKernel_Wire_Error",
      "payload" => { "message" => "bad retryable", "retryable" => "false" }
    }
  )
end

[
  { "code" => "FutureCode", "message" => "bad code" },
  {
    "code" => "InvalidArgument",
    "message" => "bad detail",
    "details" => [{ "code" => "unknown_field" }]
  },
  {
    "code" => "InvalidArgument",
    "message" => "bad path",
    "details" => [{ "code" => "unknown_field", "message" => "bad", "path" => 7 }]
  },
  {
    "code" => "InvalidArgument",
    "message" => "bad metadata",
    "details" => [{ "code" => "unknown_field", "message" => "bad", "metadata" => "wrong" }]
  },
  {
    "code" => "InvalidArgument",
    "message" => "duplicate metadata",
    "details" => [{
      "code" => "unknown_field",
      "message" => "bad",
      "metadata" => [
        { "key" => "field", "value" => "one" },
        { "key" => "field", "value" => "two" }
      ]
    }]
  }
].each do |payload|
  raises(SearchKernel::ProtocolError, "malformed canonical Error detail fails closed") do
    SearchKernel::Protocol.expect_response(
      :search,
      {
        "protocol_version" => 1,
        "payload_type" => "SearchKernel_Wire_Error",
        "payload" => payload
      }
    )
  end
end

class RecordingTransport
  attr_reader :last_request

  def execute(request)
    @last_request = request
    {
      "protocol_version" => 1,
      "payload_type" => "SearchKernel_Wire_SearchResult",
      "payload" => canonical_search_result
    }
  end
end

recording = RecordingTransport.new
client = SearchKernel::Client.new(recording)
same(canonical_search_result, client.search({ "query" => { "source_type" => "MatchAllQuery" } }), "client returns operation payload")
same([], client.search_results({}).hits, "client result wrapper derives hits from root")

empty_results = SearchKernel::Results::SearchResults.new(canonical_search_result)
same([], empty_results.hits, "empty result hits match Python facade semantics")
same([], empty_results.groups, "empty result groups match Python facade semantics")
same(0, empty_results.total_hits, "absent total hits defaults to zero")
same("Exact", empty_results.total_hits_relation, "canonical empty result relation")
same([], empty_results.errors, "absent result errors default to an empty list")
same({}, empty_results.metadata, "absent result metadata defaults to an empty map")
same([], empty_results.continuations, "absent result continuations default to an empty list")
same({}, empty_results.aggregations, "absent result aggregations default to an empty map")
same({}, empty_results.facets, "absent result facets default to an empty map")

empty_hit = SearchKernel::Results::SearchResults.new(
  canonical_search_result("root" => {"node_type" => "Hit", "node" => {}})
).hits.first
same("", empty_hit.id, "absent hit id defaults to an empty string")
same({}, empty_hit.fields, "absent hit fields default to an empty map")
same([], empty_hit.highlights, "absent hit highlights default to an empty list")
same([], empty_hit.types, "absent hit types default to an empty list")
same({}, empty_hit.match_features, "absent match features default to an empty map")
same({}, empty_hit.summary_features, "absent summary features default to an empty map")
same({}, empty_hit.rank_features, "absent rank features default to an empty map")
same([], empty_hit.sort_values, "absent hit sort values default to an empty list")

empty_group = SearchKernel::Results::SearchResults.new(
  canonical_search_result("root" => {"node_type" => "ResultGroup", "node" => {}})
).root
same({}, empty_group.fields, "absent group fields default to an empty map")
same([], empty_group.children, "absent group children default to an empty list")
same([], empty_group.groups, "absent nested groups default to an empty list")
same([], empty_group.hits, "absent grouped hits default to an empty list")
same([], empty_group.continuations, "absent group continuations default to an empty list")
same([], empty_group.types, "absent group types default to an empty list")
same({}, empty_group.metadata, "absent group metadata defaults to an empty map")

empty_result_metadata = SearchKernel::Results::SearchResults.new(
  canonical_search_result(
  "coverage" => {},
  "timing" => {},
  "errors" => [{"code" => "0"}]
  )
)
same({}, empty_result_metadata.coverage.metadata, "absent coverage metadata defaults to an empty map")
same({}, empty_result_metadata.timing.metadata, "absent timing metadata defaults to an empty map")
same({}, empty_result_metadata.errors.first.metadata, "absent issue metadata defaults to an empty map")
same(false, SearchKernel::Results::SearchResults.new(canonical_search_result("coverage" => {"full" => {}})).coverage.full,
     "present OptionalBool retains omitted default false")
same(nil, SearchKernel::Results::SearchResults.new(canonical_search_result("coverage" => {})).coverage.full,
     "absent OptionalBool differs from present false")
integer_result = SearchKernel::Results::SearchResults.new(
  canonical_search_result(
  "total_hits" => {"value" => "18446744073709551615", "relation" => "LowerBound"},
  "coverage" => {"documents" => {"value" => "18446744073709551615"}},
  "errors" => [{"code" => "-9223372036854775808", "message" => "boundary"}]
  )
)
same(2**64 - 1, integer_result.total_hits, "total hits decode canonical uint64 text")
same(2**64 - 1, integer_result.coverage.documents, "coverage decodes canonical uint64 text")
same(-(2**63), integer_result.errors.first.code, "result errors decode canonical int64 text")

def facet_projection_result(count_value, generic: false)
  bucket = {
    "value" => SearchKernel::Value.encode("Acme"),
    "fields" => [{ "key" => "count", "value" => count_value }]
  }
  if generic
    bucket["id"] = "grouping-0"
    bucket["label"] = "brands"
  end

  SearchKernel::Results::SearchResults.new(
    canonical_search_result("root" => {
      "node_type" => "ResultGroup",
      "node" => {
        "children" => [{
          "node_type" => "ResultGroup",
          "node" => {
            "id" => "brand",
            "label" => "brands",
            "children" => [{ "node_type" => "ResultGroup", "node" => bucket }]
          }
        }]
      }
    })
  )
end

unsigned_facet = facet_projection_result(
  SearchKernel::Value.encode(SearchKernel::Value.uint64(2))
).project_facet("brands", "brands", "brand")
same(
  { "brands" => [{ "value" => "Acme", "count" => 2 }] },
  unsigned_facet.facets,
  "optimized facet accepts canonical uint64 count"
)
assert(unsigned_facet.frozen?, "projected SearchResults remains read-only")
assert(unsigned_facet.facets.frozen?, "facet projection map is frozen")
assert(unsigned_facet.facets.fetch("brands").frozen?, "facet bucket list is frozen")
assert(unsigned_facet.facets.fetch("brands").first.frozen?, "facet bucket entries are frozen")

signed_optimized = facet_projection_result(
  SearchKernel::Value.encode(2)
).project_facet("brands", "brands", "brand")
same({}, signed_optimized.facets, "optimized facet rejects signed Int64 count")

negative_optimized = facet_projection_result(
  SearchKernel::Value.encode(-1)
).project_facet("brands", "brands", "brand")
same({}, negative_optimized.facets, "optimized facet rejects negative signed count")

generic_facet = facet_projection_result(
  SearchKernel::Value.encode(2),
  generic: true
).project_facet("brands", "brands", "brand")
same(
  { "brands" => [{ "value" => "Acme", "count" => 2 }] },
  generic_facet.facets,
  "generic facet accepts non-negative signed Int64 count"
)

negative_generic = facet_projection_result(
  SearchKernel::Value.encode(-1),
  generic: true
).project_facet("brands", "brands", "brand")
same({}, negative_generic.facets, "generic facet rejects negative signed count")

# Strict result-tree interpretation: generic groups remain canonical groups until
# an explicitly requested projection proves the exact supported facet shape.
unlabeled_bucket = { "node_type" => "ResultGroup", "node" => { "label" => "bucket" } }
unlabeled_wrapper = { "node_type" => "ResultGroup", "node" => { "children" => [unlabeled_bucket] } }
unlabeled_results = SearchKernel::Results::SearchResults.new(
  canonical_search_result("root" => {
    "node_type" => "ResultGroup",
    "node" => { "children" => [unlabeled_wrapper] }
  })
)
same(1, unlabeled_results.groups.length, "unlabeled group wrapper is preserved")
assert(unlabeled_results.groups.first.equal?(unlabeled_results.root.children.first),
       "top-level group is the canonical child object")
same(nil, unlabeled_results.groups.first.label, "unlabeled wrapper is not inferred")
same("bucket", unlabeled_results.groups.first.groups.first.label, "nested group remains nested")

facet_like = facet_projection_result(SearchKernel::Value.encode(SearchKernel::Value.uint64(3)))
same({}, facet_like.facets, "generic labeled result group is not inferred as a facet")
same("brands", facet_like.groups.first.label, "generic labeled group remains accessible")

missing_count = SearchKernel::Results::SearchResults.new(
  canonical_search_result("root" => {
    "node_type" => "ResultGroup", "node" => { "children" => [{
      "node_type" => "ResultGroup", "node" => {
        "id" => "brand", "label" => "brands", "children" => [{
          "node_type" => "ResultGroup", "node" => {
            "value" => SearchKernel::Value.encode("Acme"),
            "fields" => [{ "key" => "other", "value" => SearchKernel::Value.encode(SearchKernel::Value.uint64(3)) }]
          }
        }]
      }
    }] }
  })
).project_facet("brands", "brands", "brand")
same({}, missing_count.facets, "facet projection rejects missing count field")

wrong_count = facet_projection_result(SearchKernel::Value.encode(3)).project_facet("brands", "brands", "brand")
same({}, wrong_count.facets, "facet projection rejects wrong count value kind")

mixed_children = SearchKernel::Results::SearchResults.new(
  canonical_search_result("root" => {
    "node_type" => "ResultGroup", "node" => { "children" => [{
      "node_type" => "ResultGroup", "node" => {
        "id" => "brand", "label" => "brands", "children" => [
          { "node_type" => "ResultGroup", "node" => {
              "value" => SearchKernel::Value.encode("Acme"),
              "fields" => [{ "key" => "count", "value" => SearchKernel::Value.encode(SearchKernel::Value.uint64(3)) }]
          } },
          { "node_type" => "Hit", "node" => { "id" => "doc-1" } }
        ]
      }
    }] }
  })
).project_facet("brands", "brands", "brand")
same({}, mixed_children.facets, "facet projection rejects mixed hit/group children")
same(2, mixed_children.groups.first.children.length, "mixed canonical result tree remains accessible")

metadata_facet = SearchKernel::Results::SearchResults.new(
  canonical_search_result("root" => {
    "node_type" => "ResultGroup", "node" => { "children" => [{
      "node_type" => "ResultGroup", "node" => {
        "id" => "brand", "label" => "brands",
        "metadata" => [{ "key" => "unexpected", "value" => SearchKernel::Value.encode(true) }],
        "children" => [{ "node_type" => "ResultGroup", "node" => {
          "value" => SearchKernel::Value.encode("Acme"),
          "fields" => [{ "key" => "count", "value" => SearchKernel::Value.encode(SearchKernel::Value.uint64(3)) }]
        } }]
      }
    }] }
  })
).project_facet("brands", "brands", "brand")
same({}, metadata_facet.facets, "facet projection rejects ambiguous extra metadata")
same({ "unexpected" => true }, metadata_facet.groups.first.metadata, "facet metadata remains on canonical tree")

# Result union/value fidelity and fail-closed malformed states.
fidelity_hit = SearchKernel::Results::SearchResults.new(
  canonical_search_result("root" => {
    "node_type" => "ResultGroup", "node" => { "children" => [
      { "node_type" => "Hit", "node" => {
        "id" => "doc-finite", "source" => "catalog", "types" => ["product", "featured"],
        "relevance" => { "value_type" => "FiniteRelevance", "value" => { "value" => 2.5 } },
        "match_features" => [{ "key" => "match", "value" => SearchKernel::Value.encode(["one", true]) }],
        "summary_features" => [{ "key" => "summary", "value" => SearchKernel::Value.encode({ "ready" => true }) }],
        "rank_features" => [{ "key" => "rank", "value" => SearchKernel::Value.encode(7) }],
        "sort_values" => [SearchKernel::Value.encode("a"), SearchKernel::Value.encode(9)],
        "highlights" => [{ "field" => "title", "fragments" => [{
          "text" => "quick fox", "matches" => [{ "start_byte" => 6, "end_byte" => 9 }]
        }] }]
      } },
      { "node_type" => "Hit", "node" => {
        "id" => "doc-positive",
        "relevance" => { "value_type" => "PositiveInfinityRelevance", "value" => {} }
      } },
      { "node_type" => "Hit", "node" => {
        "id" => "doc-negative",
        "relevance" => { "value_type" => "NegativeInfinityRelevance", "value" => {} }
      } }
    ] }
  })
)
finite_hit, positive_hit, negative_hit = fidelity_hit.hits
same("catalog", finite_hit.source, "hit source is preserved")
same(["product", "featured"], finite_hit.types, "hit types are preserved")
same({ "match" => ["one", true] }, finite_hit.match_features, "match feature values preserve recursive types")
same({ "summary" => { "ready" => true } }, finite_hit.summary_features, "summary feature map is preserved")
same({ "rank" => 7 }, finite_hit.rank_features, "rank feature map is preserved")
same(["a", 9], finite_hit.sort_values, "sort values preserve canonical kinds")
same(2.5, finite_hit.relevance, "finite relevance is preserved")
same(Float::INFINITY, positive_hit.relevance, "positive infinity relevance remains distinct")
same(-Float::INFINITY, negative_hit.relevance, "negative infinity relevance remains distinct")
same(
  [{ "field" => "title", "fragments" => [{ "text" => "quick fox", "matches" => [[6, 9]] }] }],
  finite_hit.highlights,
  "structured response highlights preserve fragments and byte spans"
)

raises(SearchKernel::ProtocolError, "finite relevance rejects non-finite payload") do
  SearchKernel::Results::SearchResults.new(canonical_search_result("root" => {
    "node_type" => "Hit", "node" => {
      "relevance" => { "value_type" => "FiniteRelevance", "value" => { "value" => Float::INFINITY } }
    }
  }))
end
%w[PositiveInfinityRelevance NegativeInfinityRelevance].each do |kind|
  raises(SearchKernel::ProtocolError, "#{kind} rejects mismatched finite payload") do
    SearchKernel::Results::SearchResults.new(canonical_search_result("root" => {
      "node_type" => "Hit", "node" => {
        "relevance" => { "value_type" => kind, "value" => { "value" => 1.0 } }
      }
    }))
  end
end

%w[FiniteRelevance PositiveInfinityRelevance NegativeInfinityRelevance].each do |kind|
  raises(SearchKernel::ProtocolError, "#{kind} rejects missing union payload") do
    SearchKernel::Results::SearchResults.new(canonical_search_result("root" => {
      "node_type" => "Hit", "node" => {
        "relevance" => { "value_type" => kind, "value" => nil }
      }
    }))
  end
end

[
  ["FiniteRelevance", { "value" => 2.5, "future_extension" => { "version" => 1 } }, 2.5],
  ["PositiveInfinityRelevance", { "future_extension" => { "version" => 1 } }, Float::INFINITY],
  ["NegativeInfinityRelevance", { "future_extension" => { "version" => 1 } }, -Float::INFINITY]
].each do |kind, payload, expected|
  result = SearchKernel::Results::SearchResults.new(canonical_search_result("root" => {
    "node_type" => "Hit", "node" => {
      "relevance" => { "value_type" => kind, "value" => payload }
    }
  }))
  same(expected, result.hits.first.relevance, "#{kind} ignores unknown appended table fields")
end
raises(SearchKernel::ProtocolError, "unknown result node union rejects") do
  SearchKernel::Results::SearchResults.new(canonical_search_result("root" => {
    "node_type" => "FutureNode", "node" => {}
  }))
end
raises(SearchKernel::ProtocolError, "missing result node payload rejects") do
  SearchKernel::Results::SearchResults.new(canonical_search_result("root" => {
    "node_type" => "Hit", "node" => nil
  }))
end
raises(SearchKernel::ProtocolError, "unknown value union rejects") do
  portable.decode({ "kind_type" => "FutureValue", "kind" => {} })
end
raises(SearchKernel::ProtocolError, "mismatched value union payload rejects") do
  portable.decode({ "kind_type" => "ValueList", "kind" => { "value" => [] } })
end

malformed_sparse = portable.encode(portable.tensor("tensor<float>(x{})", :sparse, []))
malformed_sparse.fetch("kind").fetch("payload")["cells"] = [{
  "address" => [{ "dimension" => "x" }], "value" => 0.5
}]
raises(SearchKernel::ProtocolError, "result tensor rejects missing address label") { portable.decode(malformed_sparse) }

malformed_dense = {
  "kind_type" => "TensorValue",
  "kind" => {
    "type_spec" => "tensor<float>(x[1])",
    "payload_type" => "DenseTensorValues",
    "payload" => { "cells" => [] }
  }
}
raises(SearchKernel::ProtocolError, "tensor discriminator rejects mismatched payload shape") do
  portable.decode(malformed_dense)
end

tensor_value = lambda do |payload_type, payload, type_spec|
  {
    "kind_type" => "TensorValue",
    "kind" => {
      "type_spec" => type_spec,
      "payload_type" => payload_type,
      "payload" => payload
    }
  }
end

raises(SearchKernel::ProtocolError, "unknown tensor payload union rejects in Ruby decoder") do
  portable.decode(tensor_value.call("FutureTensorValues", {}, "tensor<float>(x[1])"))
end

{
  "DenseTensorValues" => [{ "cells" => [] }, "tensor<float>(x[1])"],
  "SparseTensorCells" => [{ "blocks" => [] }, "tensor<float>(category{})"],
  "MixedTensorBlocks" => [{ "values" => [] }, "tensor<float>(category{},x[1])"]
}.each do |payload_type, (payload, type_spec)|
  raises(SearchKernel::ProtocolError, "#{payload_type} rejects a known foreign variant field") do
    portable.decode(tensor_value.call(payload_type, payload, type_spec))
  end
end

[
  [
    "DenseTensorValues",
    { "values" => [1.0], "future_extension" => { "version" => 1 } },
    "tensor<float>(x[1])",
    "dense",
    [1.0]
  ],
  [
    "SparseTensorCells",
    { "cells" => [], "future_extension" => { "version" => 1 } },
    "tensor<float>(category{})",
    "sparse",
    []
  ],
  [
    "MixedTensorBlocks",
    { "blocks" => [], "future_extension" => { "version" => 1 } },
    "tensor<float>(category{},x[1])",
    "mixed",
    []
  ]
].each do |payload_type, payload, type_spec, decoded_key, expected|
  decoded = portable.decode(tensor_value.call(payload_type, payload, type_spec))
  same(
    expected,
    decoded.fetch("$searchkernel_tensor").fetch(decoded_key),
    "#{payload_type} ignores unknown appended table fields"
  )
end

valid_address = [{ "dimension" => "category", "label" => "books" }]
[
  [
    "dense NaN",
    tensor_value.call("DenseTensorValues", { "values" => [Float::NAN] }, "tensor<float>(x[1])")
  ],
  [
    "sparse positive infinity",
    tensor_value.call(
      "SparseTensorCells",
      { "cells" => [{ "address" => valid_address, "value" => Float::INFINITY }] },
      "tensor<float>(category{})"
    )
  ],
  [
    "mixed negative infinity",
    tensor_value.call(
      "MixedTensorBlocks",
      { "blocks" => [{ "address" => valid_address, "values" => [-Float::INFINITY] }] },
      "tensor<float>(category{},x[1])"
    )
  ]
].each do |label, malformed|
  raises(SearchKernel::ProtocolError, "#{label} fails closed in Ruby decoder") do
    portable.decode(malformed)
  end
end

metadata_result = SearchKernel::Results::SearchResults.new(
  canonical_search_result("root" => {
    "node_type" => "ResultGroup",
    "node" => {
      "continuations" => [{ "key" => "next", "value" => "opaque-token" }],
      "children" => [{
        "node_type" => "Hit",
        "node" => {
          "id" => "doc-1",
          "fields" => [{ "key" => "title", "value" => SearchKernel::Value.encode("Running Shoes") }],
          "match_features" => [{ "key" => "match", "value" => SearchKernel::Value.encode(1.5) }],
          "summary_features" => [{ "key" => "summary", "value" => SearchKernel::Value.encode(2.5) }],
          "rank_features" => [{ "key" => "rank", "value" => SearchKernel::Value.encode(3.5) }],
          "sort_values" => [SearchKernel::Value.encode(99.0)]
        }
      }]
    }
  },
  "timing" => {
    "query_time_seconds" => { "value" => 0.01 },
    "search_time_seconds" => { "value" => 0.02 },
    "summary_fetch_time_seconds" => { "value" => 0.03 }
  },
  "trace" => SearchKernel::Value.encode("trace-token"),
  "metadata" => [{ "key" => "origin", "value" => SearchKernel::Value.encode("fixture") }],
  "errors" => [{ "code" => "7", "summary" => "warning", "message" => "fixture issue" }]
  )
)
metadata_hit = metadata_result.hits.first
same({ "match" => 1.5 }, metadata_hit.match_features, "match features decode")
same({ "summary" => 2.5 }, metadata_hit.summary_features, "summary features decode")
same({ "rank" => 3.5 }, metadata_hit.rank_features, "rank features decode")
same([99.0], metadata_hit.sort_values, "sort values decode")
same(0.01, metadata_result.timing.query_time_seconds, "query timing decodes")
same("trace-token", metadata_result.trace, "trace value decodes")
same({ "origin" => "fixture" }, metadata_result.metadata, "result metadata decodes")
same("opaque-token", metadata_result.continuations.first.value, "continuation remains opaque")
same("fixture issue", metadata_result.issues.first.message, "result issues decode")
same("SearchKernel_Wire_SearchRequest", recording.last_request["payload_type"], "client uses centralized operation mapping")
raw = { "protocol_version" => 1, "payload_type" => "Custom", "payload" => {} }
same(recording.execute(raw), client.execute_raw(raw), "raw envelope escape hatch delegates unchanged")

success_response = JSON.generate(
  "protocol_version" => 1,
  "payload_type" => "SearchKernel_Wire_SearchResult",
  "payload" => { "total_hits" => { "value" => "0", "relation" => "Exact" } }
)
with_http_server(body: success_response) do |base_url, state|
  transport = SearchKernel::HttpTransport.new(base_url, timeout: 1)
  response = transport.execute(SearchKernel::Protocol.request_envelope(:search, { "query" => {} }))
  same(1, response["protocol_version"], "HTTP response decoded")
  same("POST /v1/execute HTTP/1.1", state[:request_line], "HTTP execute path")
  same("application/json", state[:headers]["content-type"], "HTTP request content type")
  same("application/json", state[:headers]["accept"], "HTTP request accept")
  request_json = JSON.parse(state[:body])
  same("SearchKernel_Wire_SearchRequest", request_json["payload_type"], "HTTP body preserves canonical envelope")
end

with_http_server(status: 503, body: "backend unavailable", content_type: "text/plain") do |base_url, _state|
  error = raises(SearchKernel::TransportError, "non-2xx response is transport error") do
    SearchKernel::HttpTransport.new(base_url, timeout: 1).execute({})
  end
  same(503, error.status, "transport error retains HTTP status")
end

with_http_server(body: "not-json") do |base_url, _state|
  raises(SearchKernel::ProtocolError, "malformed JSON response is protocol error") do
    SearchKernel::HttpTransport.new(base_url, timeout: 1).execute({})
  end
end

with_http_server(body: "[]") do |base_url, _state|
  raises(SearchKernel::ProtocolError, "non-object JSON response is protocol error") do
    SearchKernel::HttpTransport.new(base_url, timeout: 1).execute({})
  end
end

raises(SearchKernel::TransportError, "request message limit enforced before network") do
  SearchKernel::HttpTransport.new("http://127.0.0.1:1", max_message_bytes: 16).execute("payload" => "x" * 32)
end

with_http_server(body: JSON.generate("payload" => "x" * 64)) do |base_url, _state|
  raises(SearchKernel::TransportError, "response message limit enforced") do
    SearchKernel::HttpTransport.new(base_url, timeout: 1, max_message_bytes: 32).execute({})
  end
end

raises(SearchKernel::ProtocolError, "non-finite JSON is rejected locally") do
  SearchKernel::HttpTransport.new("http://127.0.0.1:1", timeout: 1).execute("value" => Float::NAN)
end

closed = TCPServer.new("127.0.0.1", 0)
closed_port = closed.addr[1]
closed.close
connection_error = raises(SearchKernel::TransportError, "connection failure is transport error") do
  SearchKernel::HttpTransport.new("http://127.0.0.1:#{closed_port}", timeout: 0.1).execute({})
end
assert(
  connection_error.message.include?("http://127.0.0.1:#{closed_port}/v1/execute"),
  "connection failure identifies the target execute endpoint"
)

raises(ArgumentError, "client rejects objects that do not implement transport execute") do
  SearchKernel::Client.new(Object.new)
end

raises(ArgumentError, "empty HTTP base URL rejects") { SearchKernel::HttpTransport.new("") }
raises(ArgumentError, "non-string HTTP base URL rejects") { SearchKernel::HttpTransport.new(123) }
raises(ArgumentError, "fragment in base URL rejects") do
  SearchKernel::HttpTransport.new("http://example.test#fragment")
end
raises(ArgumentError, "zero timeout rejects") do
  SearchKernel::HttpTransport.new("http://example.test", timeout: 0)
end
raises(ArgumentError, "non-finite timeout rejects") do
  SearchKernel::HttpTransport.new("http://example.test", timeout: Float::INFINITY)
end
raises(ArgumentError, "zero message limit rejects") do
  SearchKernel::HttpTransport.new("http://example.test", max_message_bytes: 0)
end
raises(ArgumentError, "negative message limit rejects") do
  SearchKernel::HttpTransport.new("http://example.test", max_message_bytes: -1)
end

with_http_server(body: success_response) do |base_url, state|
  transport = SearchKernel::HttpTransport.new("#{base_url}/tenant/search/")
  transport.execute(SearchKernel::Protocol.request_envelope(:search, { "query" => {} }))
  same(
    "POST /tenant/search/v1/execute HTTP/1.1",
    state[:request_line],
    "HTTP transport appends execute path after configured base path"
  )
end

raises(ArgumentError, "invalid HTTP scheme rejects") { SearchKernel::HttpTransport.new("ftp://example.test") }
raises(ArgumentError, "query in base URL rejects") { SearchKernel::HttpTransport.new("http://example.test?x=1") }
raises(ArgumentError, "userinfo in base URL rejects") { SearchKernel::HttpTransport.new("http://user:pass@example.test") }
raises(ArgumentError, "fractional message limit rejects") do
  SearchKernel::HttpTransport.new("http://example.test", max_message_bytes: 1.5)
end

with_http_server(body: "", content_length: 1024) do |base_url, _state|
  error = raises(SearchKernel::TransportError, "declared oversized response fails before body buffering") do
    SearchKernel::HttpTransport.new(base_url, timeout: 1, max_message_bytes: 32).execute({})
  end
  assert(
    error.message.include?("response exceeds 32-byte transport limit"),
    "oversized response retains the transport-limit error domain"
  )
end

with_raw_http_server("this is not HTTP\r\n") do |base_url|
  error = raises(SearchKernel::TransportError, "malformed HTTP response is transport error") do
    SearchKernel::HttpTransport.new(base_url, timeout: 1).execute({})
  end
  assert(
    error.message.start_with?("Search Kernel HTTP transport failed:"),
    "malformed HTTP response is wrapped in the transport error domain"
  )
end

large_error_body = ("a" * SearchKernel::HttpTransport::ERROR_BODY_LIMIT) + "SECRET-TAIL"
with_http_server(status: 503, body: large_error_body, content_type: "text/plain") do |base_url, _state|
  error = raises(SearchKernel::TransportError, "large HTTP error body is bounded in diagnostics") do
    SearchKernel::HttpTransport.new(base_url, timeout: 1).execute({})
  end
  assert(
    !error.message.include?("SECRET-TAIL"),
    "transport error diagnostic must not expose bytes beyond the bounded error snippet"
  )
end

with_http_server(status: 503, body: "bad\r\ninjected\x00\xff".b, content_type: "text/plain") do |base_url, _state|
  error = raises(SearchKernel::TransportError, "HTTP error body is safely bounded for diagnostics") do
    SearchKernel::HttpTransport.new(base_url, timeout: 1).execute({})
  end
  assert(!error.message.include?("\n"), "transport error diagnostic must not contain injected newlines")
  assert(error.message.valid_encoding?, "transport error diagnostic must remain valid UTF-8")
end

puts "Ruby protocol/transport tests PASS"

# SKC: result.summary_features/basic kinds=conformance runtime=http

require_relative "facade"
