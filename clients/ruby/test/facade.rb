# frozen_string_literal: true

require "fileutils"
require "json"
require "tempfile"
require "tmpdir"

class FacadeRecordingTransport
  attr_reader :requests

  def initialize
    @requests = []
  end

  def execute(request)
    @requests << request
    request_type = request.fetch("payload_type").sub("SearchKernel_V2_", "")
    operation, spec = SearchKernel::Protocol::OPERATIONS.find { |_name, item| item.fetch(:request) == request_type }
    raise "unknown request type #{request_type}" unless operation

    payload = case operation
              when :insert, :upsert, :patch, :update
                { "id" => request.dig("payload", "id") || request.dig("payload", "document", "id") || "doc-1" }
              when :delete
                { "id" => request.dig("payload", "id"), "deleted" => true }
              when :get
                {
                  "document" => {
                    "id" => request.dig("payload", "id"),
                    "fields" => [
                      { "key" => "title", "value" => SearchKernel::Value.encode("Running Shoes") },
                      { "key" => "price", "value" => SearchKernel::Value.encode(99.0) }
                    ]
                  }
                }
              when :search
                {
                  "root" => nil,
                  "total_hits" => { "value" => "0", "relation" => "Exact" }
                }
              when :analyze
                {
                  "tokens" => [
                    { "surface" => "Shoes", "term" => "shoe", "position" => 0, "start_byte" => 0, "end_byte" => 5 }
                  ]
                }
              when :describe
                { "index" => { "name" => "index", "fields" => [] } }
              when :inspect_query
                { "resolved_request" => request.dig("payload", "request") }
              else
                {}
              end

    {
      "protocol_version" => 2,
      "payload_type" => "SearchKernel_V2_#{spec.fetch(:response)}",
      "payload" => payload
    }
  end
end

transport = FacadeRecordingTransport.new
index = SearchKernel::Index.new(SearchKernel::Client.new(transport))

same("doc-1", index.insert("_id" => "doc-1", "title" => "Running Shoes", "price" => 99.0), "facade insert id")
insert_request = transport.requests.last.fetch("payload")
same("doc-1", insert_request.dig("document", "id"), "facade insert document id")
same(%w[price title], insert_request.dig("document", "fields").map { |entry| entry.fetch("key") }, "document fields are deterministic")

same("Running Shoes", index.get("doc-1").fetch("title"), "facade get decodes portable fields")
assert(index.delete("doc-1"), "facade delete decodes boolean")

search = index.search("shoes")
  .filter("price <= :max", "max" => 120)
  .filter("price >= :min", "min" => 50)
  .sort("price", "asc")
  .fields("title", "price")
  .highlight("title")
  .ranking("default")
  .input("boost", 2.0)
  .rank_property("vespa.rank", "yes")
  .property("trace.level", 1)
  .page(2, 3)

request = search.request
same("SearchKernelTextQuery", request.dig("query", "source_type"), "text search query source")
same(2, request.dig("filters", "textual").length, "filter clauses remain clause-local")
same(["max"], request.dig("filters", "textual", 0, "bindings").map { |entry| entry.fetch("key") }, "first filter bindings do not leak")
same(["min"], request.dig("filters", "textual", 1, "bindings").map { |entry| entry.fetch("key") }, "second filter bindings do not leak")
same(3, request.dig("offset", "value"), "page computes offset")
same(3, request.dig("limit", "value"), "page sets limit")
raises(ArgumentError, "page zero rejects") { index.search("shoes").page(0, 3) }
raises(ArgumentError, "page offset overflow rejects") { index.search("shoes").page(0xffff_ffff, 2) }
raises(ArgumentError, "page size outside uint32 rejects") { index.search("shoes").page(1, 0x1_0000_0000) }
same("Asc", request.dig("sort", 0, "direction"), "sort direction")
same(%w[title price], request.dig("presentation", "fields", "fields"), "projection fields retain caller order")
same(["title"], request.dig("presentation", "highlighting", "fields"), "highlight fields retained")
assert(search.execute.is_a?(SearchKernel::Results::SearchResults), "search executes into canonical SearchResults")

nearest = SearchKernel::Query.nearest_neighbor(
  "embedding",
  "query_embedding",
  approximate: true,
  target_hits: 10,
  explore_additional_hits: 20
)
nearest_expr = nearest.dig("source", "query")
same("QueryNearestNeighbor", nearest_expr.fetch("expr_type"), "nearest-neighbor query kind")
same(
  %w[approximate exploreAdditionalHits targetNumHits],
  nearest_expr.fetch("annotations").map { |entry| entry.fetch("key") },
  "nearest-neighbor annotations are deterministic"
)

combined = SearchKernel::Query.any_of(
  SearchKernel::Query.contains("title", "shoes"),
  SearchKernel::Query.gte("price", 50)
)
same("QueryOr", combined.dig("source", "query", "expr_type"), "boolean typed query")

group = SearchKernel::Group.by("brand").limit(10).output(SearchKernel::Agg.count.as("products"))
grouping_request = group.to_request
same("StructuredGroupingSource", grouping_request.fetch("source_type"), "structured grouping source")
same(2, grouping_request.dig("source", "root", "operations").length, "grouping root shape")

definition = SearchKernel::Schema.index_definition(
  {
    "title" => SearchKernel::Text.new(stored: true, searchable: true),
    "price" => SearchKernel::FloatField.new(stored: true, filterable: true, sortable: true),
    "active" => SearchKernel::BooleanField.new(stored: true, filterable: true)
  },
  [SearchKernel::RankProfile.new("custom").first_phase("bm25(title)").match_feature("bm25(title)")]
)
same(%w[active price title], definition.fetch("fields").map { |field| field.fetch("name") }, "schema fields are deterministic")
same("Float64", definition.fetch("fields")[1].dig("data_type", "kind", "kind"), "float schema kind")
same("custom", definition.dig("rank_profiles", 0, "name"), "rank profile schema")

same("doc-1", index.patch("doc-1").set("price", 79.0).unset("sale_price").execute, "patch builder executes")
patch = transport.requests.last.fetch("payload")
same(%w[SetField UnsetField], patch.fetch("operations").map { |item| item.fetch("operation_type") }, "patch operations")

same(
  "doc-1",
  index.update("doc-1").set("price", 80.0).increment("inventory", 2).create.execute,
  "update builder executes"
)
update = transport.requests.last.fetch("payload")
same(true, update.dig("create", "value"), "update create flag")
same(%w[AssignUpdate ArithmeticUpdate], update.fetch("operations").map { |item| item.fetch("operation_type") }, "update operations")

tokens = index.analyze("Shoes", field: "title")
same("shoe", tokens.first.fetch("term"), "analysis tokens decode")
same("index", index.describe.fetch("name"), "describe result")

Dir.mktmpdir("searchkernel-ruby-managed-test-") do |root|
  script = File.join(root, "fake-searchkerneld")
  log = File.join(root, "args.json")
  index_path = File.join(root, "index")
  File.write(
    script,
    <<~'RUBY'
      #!/usr/bin/env ruby
      require "fileutils"
      require "json"
      require "socket"

      args = ARGV.each_slice(2).to_h
      index_path = args.fetch("--index")
      definition_path = args["--definition-json"]
      if definition_path
        FileUtils.mkdir_p(index_path)
        File.write(File.join(index_path, "definition.json"), File.read(definition_path))
      elsif !Dir.exist?(index_path)
        warn "missing index"
        exit 3
      end

      File.write(ENV.fetch("FAKE_HOST_LOG"), JSON.generate(ARGV))
      host, port = args.fetch("--web-listen").split(":", 2)
      server = TCPServer.new(host, Integer(port, 10))
      trap("INT") { server.close rescue nil; exit 0 }
      trap("TERM") { server.close rescue nil; exit 0 }
      loop do
        socket = server.accept
        socket.close
      end
    RUBY
  )
  File.chmod(0o755, script)

  previous_host = ENV["SEARCHKERNEL_LOCAL_HOST"]
  previous_log = ENV["FAKE_HOST_LOG"]
  ENV["SEARCHKERNEL_LOCAL_HOST"] = script
  ENV["FAKE_HOST_LOG"] = log
  begin
    managed = SearchKernel::Index.create(
      index_path,
      fields: { "title" => SearchKernel::Text.new(stored: true, searchable: true) }
    )
    assert(File.exist?(File.join(index_path, "definition.json")), "managed create supplies canonical definition JSON")
    args = JSON.parse(File.read(log))
    assert(args.include?("--definition-json"), "managed create uses definition-json bootstrap")
    managed.close
    assert(managed.closed?, "managed close is idempotent state")
    managed.close

    reopened = SearchKernel::Index.open(index_path)
    args = JSON.parse(File.read(log))
    assert(!args.include?("--definition-json"), "managed open does not resend a definition")
    reopened.close
  ensure
    ENV["SEARCHKERNEL_LOCAL_HOST"] = previous_host
    ENV["FAKE_HOST_LOG"] = previous_log
  end
end

Dir.mktmpdir("searchkernel-ruby-startup-race-") do |root|
  script = File.join(root, "crashing-searchkerneld")
  index_path = File.join(root, "index")
  File.write(
    script,
    <<~'RUBY'
      #!/usr/bin/env ruby
      warn "startup boom"
      exit 23
    RUBY
  )
  File.chmod(0o755, script)

  previous_host = ENV["SEARCHKERNEL_LOCAL_HOST"]
  registry_before = SearchKernel::ManagedLocalHost.registry.length
  ENV["SEARCHKERNEL_LOCAL_HOST"] = script
  begin
    error = raises(SearchKernel::ClientError, "managed startup exit fails closed") do
      SearchKernel::Index.create(
        index_path,
        fields: { "title" => SearchKernel::Text.new(stored: true, searchable: true) }
      )
    end
    assert(
      error.message.include?("managed local host exited during startup"),
      "startup race retains stable managed-host failure"
    )
    assert(
      error.message.include?("startup boom"),
      "startup race includes bounded child stderr diagnostics"
    )
    same(
      registry_before,
      SearchKernel::ManagedLocalHost.registry.length,
      "failed startup must not leak managed-host registry state"
    )
  ensure
    ENV["SEARCHKERNEL_LOCAL_HOST"] = previous_host
  end
end

puts "Ruby facade/lifecycle tests PASS"
