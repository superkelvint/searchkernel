# frozen_string_literal: true

require "json"

$LOAD_PATH.unshift(File.expand_path("../lib", __dir__))
require "searchkernel"

name = ARGV.fetch(0)
json = STDIN.read
raise "missing BFBS-printed response JSON" if json.empty?

envelope = JSON.parse(json)
raise "BFBS-printed response must be an object" unless envelope.is_a?(Hash)

if name == "rejected-value-response"
  begin
    raw = SearchKernel::Protocol.expect_response(:get, envelope)
    SearchKernel::Value.entries(raw.fetch("document").fetch("fields"))
    raise "malformed Value unexpectedly decoded"
  rescue SearchKernel::ProtocolError
    # A BFBS-printed response can preserve structurally valid duplicate map
    # entries; Ruby must reject them before converting to a Hash.
  end
elsif name == "portable-int64-default-response"
  raw = SearchKernel::Protocol.expect_response(:get, envelope)
  fields = SearchKernel::Value.entries(raw.fetch("document").fetch("fields"))
  raise "default Int64Value did not decode as zero" unless fields.fetch("zero") == 0
elsif name == "portable-values-response"
  raw = SearchKernel::Protocol.expect_response(:get, envelope)
  fields = SearchKernel::Value.entries(raw.fetch("document").fetch("fields"))
  raise "int64 minimum lost" unless fields.fetch("min") == -(2**63)
  raise "int64 maximum lost" unless fields.fetch("max") == 2**63 - 1
  raise "uint64 maximum lost" unless fields.fetch("unsigned") == 2**64 - 1
  raise "bytes lost" unless fields.fetch("bytes") == "\x00\xff".b
  raise "nested values lost" unless fields.fetch("nested") == [{"x" => true}, nil, [1.5]]
  %w[dense sparse mixed].each do |encoding|
    raise "#{encoding} tensor lost" unless fields.fetch(encoding).fetch("$searchkernel_tensor").key?(encoding)
  end
  raise "non-finite union lost" unless fields.fetch("nan").nan? && fields.fetch("pos_inf") == Float::INFINITY
elsif name == "recursive-search-response"
  raw = SearchKernel::Protocol.expect_response(:search, envelope)
  result = SearchKernel::Results::SearchResults.new(raw)
  raise "lost recursive order" unless result.root.children.map(&:id) == %w[a nested c]
  raise "lost nested hit" unless result.groups.first.hits.first.id == "b"
  raise "incorrect direct hits" unless result.hits.map(&:id) == %w[a c]
  raise "lost unsigned boundary" unless result.hits.first.fields.fetch("count") == 2**64 - 1
  raise "lost signed boundary" unless result.hits.first.sort_values == [2**63 - 1]
  raise "lost positive infinity" unless result.hits.first.relevance == Float::INFINITY
  raise "lost negative infinity" unless result.groups.first.hits.first.relevance == -Float::INFINITY
  raise "lost root aggregation" unless result.aggregations == { "count" => 3 }
  raise "lost group value or limits" unless result.groups.first.value == "bucket" && result.groups.first.limits.from == 1 && result.groups.first.limits.to == 9
  raise "lost hit feature maps" unless result.hits.first.match_features == {} && result.hits.first.summary_features == { "summary" => "ok" } && result.hits.first.rank_features == { "rank" => 0.25 }
  raise "lost hit source or types" unless result.hits.first.source == "sample" && result.hits.first.types == ["document"]
  raise "lost present-empty metadata" unless result.metadata == {} && result.root.metadata == {}
  raise "lost absent metadata" unless result.groups.first.metadata.nil?
  raise "lost continuation" unless result.groups.first.continuations.first.value == "opaque"
  raise "lost trace" unless result.trace == { "events" => [] }
  raise "lost coverage" unless result.coverage.degraded.timeout == true
  raise "lost present false coverage" unless result.coverage.full == false
  raise "lost timing" unless result.timing.query_time_seconds == 0.5
  raise "lost embedded issue" unless result.issues.first.code == 7 && result.issues.first.transient == true
  raise "lost highlight" unless result.hits.first.highlights.first.fetch("fragments").first.fetch("matches") == [[0, 5]]
elsif name == "portable-error-defaults"
  begin
    SearchKernel::Protocol.expect_response(:search, envelope)
    raise "canonical Error did not raise"
  rescue SearchKernel::SearchKernelError => error
    raise "default Error code changed" unless error.code == "Unspecified"
    raise "default Error retryable changed" unless error.retryable == false
    raise "default Error details changed" unless error.details == []
  end
else
  operation_name = name.delete_suffix("-response")
  operation = operation_name.to_sym
  unless SearchKernel::Protocol::OPERATIONS.key?(operation)
    raise "unknown BFBS response fixture #{name.inspect}"
  end

  payload = SearchKernel::Protocol.expect_response(operation, envelope)
  raise "#{name}: response payload must remain an object" unless payload.is_a?(Hash)
end

puts "PASS #{name}"
