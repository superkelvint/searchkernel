# frozen_string_literal: true

require "fileutils"
require "json"
require "tempfile"
require "tmpdir"
require_relative "../examples/client_real_e2e_parity_helpers"

e2e_corpus = JSON.parse(File.read(File.expand_path("../../../reference/client-real-e2e-parity/corpus.json", __dir__)))
typed_corpus_document = ClientRealE2EParityHelpers.document_from_corpus(
  e2e_corpus,
  { "_id" => "typed-corpus-document", "price" => 89, "sale_price" => 79, "rating" => 5 }
)
TestSupport.same(89.0, typed_corpus_document.fetch("price"), "E2E Float64 field is normalized from JSON integer")
TestSupport.same(79.0, typed_corpus_document.fetch("sale_price"), "E2E optional Float64 field is normalized from JSON integer")
TestSupport.same(5, typed_corpus_document.fetch("rating"), "E2E Int64 field remains an integer")
TestSupport.same(50.0, ClientRealE2EParityHelpers.query_value(e2e_corpus, "price", 50), "E2E range bound matches the declared Float64 field")
TestSupport.same(5, ClientRealE2EParityHelpers.query_value(e2e_corpus, "rating", 5), "E2E query value preserves declared Int64 field")

class CloseCountingOwner
  attr_reader :close_count

  def initialize
    @close_count = 0
  end

  def close
    @close_count += 1
  end
end

class FailingCloseTransport
  def execute(_request)
    raise "unexpected execute"
  end

  def close
    raise "client close boom"
  end
end

class FacadeFixedResponseTransport
  def initialize(response)
    @response = response
  end

  def execute(_request)
    @response
  end
end

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
                  "root" => { "node_type" => "ResultGroup", "node" => { "children" => [] } },
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

malformed_get_fields = {
  "duplicate key" => [
    { "key" => "same", "value" => SearchKernel::Value.encode(1) },
    { "key" => "same", "value" => SearchKernel::Value.encode(2) }
  ],
  "null entry" => [nil],
  "missing key" => [{ "value" => SearchKernel::Value.encode(1) }],
  "missing value" => [{ "key" => "missing" }]
}
malformed_get_fields.each do |label, fields|
  response = {
    "protocol_version" => 2,
    "payload_type" => "SearchKernel_V2_GetResult",
    "payload" => { "document" => { "id" => "doc-1", "fields" => fields } }
  }
  malformed = SearchKernel::Index.new(
    SearchKernel::Client.new(FacadeFixedResponseTransport.new(response))
  )
  raises(SearchKernel::ProtocolError, "GET rejects malformed semantic map: #{label}") do
    malformed.get("doc-1")
  end
end
valid_response = {
  "protocol_version" => 2,
  "payload_type" => "SearchKernel_V2_GetResult",
  "payload" => {
    "document" => {
      "id" => "doc-1",
      "fields" => [
        { "key" => "nothing", "value" => SearchKernel::Value.encode(nil) },
        { "key" => "count", "value" => SearchKernel::Value.encode(3) }
      ]
    }
  }
}
valid_get = SearchKernel::Index.new(
  SearchKernel::Client.new(FacadeFixedResponseTransport.new(valid_response))
)
same(
  { "_id" => "doc-1", "nothing" => nil, "count" => 3 },
  valid_get.get("doc-1"),
  "GET preserves explicit canonical null and valid semantic-map values"
)

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

raises(ArgumentError, "field sort direction is required") do
  index.search("shoes").sort("price")
end

request = search.request
same("SearchKernelTextQuery", request.dig("query", "source_type"), "text search query source")
same(2, request.dig("filters", "textual").length, "filter clauses remain clause-local")
same(["max"], request.dig("filters", "textual", 0, "bindings").map { |entry| entry.fetch("key") }, "first filter bindings do not leak")
same(["min"], request.dig("filters", "textual", 1, "bindings").map { |entry| entry.fetch("key") }, "second filter bindings do not leak")
same(3, request.dig("offset", "value"), "page computes offset")
same(3, request.dig("limit", "value"), "page sets limit")
same("yes", request.dig("ranking", "properties", 0, "value"), "rank property values stay strings")
[false, nil, 7, [], {}].each do |value|
  raises(TypeError, "rank property values reject #{value.inspect}") do
    index.search("shoes").rank_property("enabled", value)
  end
end
raises(TypeError, "rank_properties rejects non-string values") do
  index.search("shoes").rank_properties("enabled" => false)
end
atomic_rank_properties = index.search("shoes").rank_property("existing", "keep")
raises(TypeError, "rank_properties failure is atomic") do
  atomic_rank_properties.rank_properties("valid" => "yes", "invalid" => false)
end
same(
  [{ "key" => "existing", "value" => "keep" }],
  atomic_rank_properties.request.dig("ranking", "properties"),
  "failed rank_properties does not partially mutate builder",
)
implicit_ranking = index.search("shoes").rank_property("source", "catalog").request.fetch("ranking")
same(false, implicit_ranking.key?("profile"), "rank properties preserve absent ranking profile")
same([{ "key" => "source", "value" => "catalog" }], implicit_ranking.fetch("properties"), "rank properties remain present")
explicit_ranking = index.search("shoes").ranking("default").rank_property("source", "catalog").request.fetch("ranking")
same("default", explicit_ranking.fetch("profile"), "explicit default ranking profile remains explicit")
raises(ArgumentError, "page zero rejects") { index.search("shoes").page(0, 3) }
raises(ArgumentError, "page offset overflow rejects") { index.search("shoes").page(0xffff_ffff, 2) }
raises(ArgumentError, "page size outside uint32 rejects") { index.search("shoes").page(1, 0x1_0000_0000) }
raises(ArgumentError, "page then offset rejects") { index.search("shoes").page(1, 3).offset(1) }
raises(ArgumentError, "offset then page rejects") { index.search("shoes").offset(1).page(1, 3) }
raises(ArgumentError, "page then limit rejects") { index.search("shoes").page(1, 3).limit(3) }
raises(ArgumentError, "limit then page rejects") { index.search("shoes").limit(3).page(1, 3) }
raises(ArgumentError, "page range overflow rejects") { index.search("shoes").page(0x8000_0000, 2) }
same("Asc", request.dig("sort", 0, "direction"), "sort direction")
same(%w[title price], request.dig("presentation", "fields", "fields"), "projection fields retain caller order")
same(["title"], request.dig("presentation", "highlighting", "fields"), "highlight fields retained")
assert(search.execute.is_a?(SearchKernel::Results::SearchResults), "search executes into canonical SearchResults")
closed_transport = FacadeRecordingTransport.new
closed_index = SearchKernel::Index.new(SearchKernel::Client.new(closed_transport))
closed_index.close
closed_index.close
raises(SearchKernel::TransportError, "operation after close rejects") { closed_index.get("doc-1") }

owned_owner = CloseCountingOwner.new
owned_index = SearchKernel::Index.new(
  SearchKernel::Client.new(FacadeRecordingTransport.new),
  owner: owned_owner
)
owned_index.close
owned_index.close
same(1, owned_owner.close_count, "managed owner closes exactly once")

failing_owner = CloseCountingOwner.new
failing_index = SearchKernel::Index.new(
  SearchKernel::Client.new(FailingCloseTransport.new),
  owner: failing_owner
)
raises(RuntimeError, "owner still closes when client close fails") { failing_index.close }
same(1, failing_owner.close_count, "owner cleanup runs despite client close failure")
assert(failing_index.closed?, "index remains closed after cleanup failure")

connected_index = SearchKernel::Index.connect("http://127.0.0.1:1", target: "products")
connected_index.close
raises(SearchKernel::TransportError, "connect-to-daemon index rejects operations after close") do
  connected_index.get("doc-1")
end

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

same("QueryPhrase", SearchKernel::Query.phrase("running", "shoes").dig("source", "query", "expr_type"),
     "phrase query helper")
same("QueryNear", SearchKernel::Query.near("running", "shoes").dig("source", "query", "expr_type"),
     "near query helper")
same("QueryONear", SearchKernel::Query.onear("running", "shoes").dig("source", "query", "expr_type"),
     "ordered-near query helper")
same(
  "QuerySameElement",
  SearchKernel::Query.same_element(
    SearchKernel::Query.contains("title", "running"),
    SearchKernel::Query.contains("title", "shoes")
  ).dig("source", "query", "expr_type"),
  "sameElement query helper"
)
same("QueryFuzzy", SearchKernel::Query.fuzzy("shoes").dig("source", "query", "expr_type"),
     "fuzzy query helper")
# SKC: query.prefix_suffix_substring/basic kinds=ergonomic runtime=http
fielded_term = SearchKernel::Query.term("title", "quick").dig("source", "query")
same("QueryContains", fielded_term.fetch("expr_type"), "fielded term query helper")
same("TermQuery", fielded_term.dig("expr", "item", "expr_type"), "fielded term canonical item")
{
  "A\\.c\\+.*" => SearchKernel::Query.prefix("brand", "A.c+"),
  ".*A\\.c\\+.*" => SearchKernel::Query.substring("brand", "A.c+"),
  ".*A\\.c\\+" => SearchKernel::Query.suffix("brand", "A.c+")
}.each do |pattern, query|
  same("QueryMatches", query.dig("source", "query", "expr_type"), "literal query canonical kind")
  same(pattern, query.dig("source", "query", "expr", "pattern"), "literal query escaped pattern")
end
same(
  "QueryGeoLocation",
  SearchKernel::Query.geo_location("position", lat: 37.3, lon: -121.9, radius: "10km")
    .dig("source", "query", "expr_type"),
  "geo-location query helper"
)
same(
  "QueryGeoBoundingBox",
  SearchKernel::Query.geo_box("position", south: 37.0, west: -122.0, north: 38.0, east: -121.0)
    .dig("source", "query", "expr_type"),
  "geo bounding-box query helper"
)
same(
  "QueryWeakAnd",
  SearchKernel::Query.weak_and(
    SearchKernel::Query.contains("title", "running"),
    SearchKernel::Query.contains("title", "shoes")
  ).dig("source", "query", "expr_type"),
  "WeakAnd query helper"
)
same(
  "QueryWand",
  SearchKernel::Query.wand("title", "running" => 10, "shoes" => 5)
    .dig("source", "query", "expr_type"),
  "WAND query helper"
)
weighted_queries = {
  "WAND" => SearchKernel::Query.wand("title", "running" => 10, "shoes" => 5),
  "dot-product" => SearchKernel::Query.dot_product("title", "running" => 10, "shoes" => 5),
  "weighted-set" => SearchKernel::Query.weighted_set("title", "running" => 10, "shoes" => 5)
}
weighted_queries.each do |label, query|
  items = query.dig("source", "query", "expr", "items")
  assert(items.all? { |item| item.fetch("weight").is_a?(String) }, "#{label} weights use FlatBuffers int64 JSON strings")
end
raises(ArgumentError, "weighted query rejects int64 overflow") do
  SearchKernel::Query.wand("title", "running" => 2**63)
end

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

nested_definition = SearchKernel::Schema.index_definition(
  {
    "people" => SearchKernel::ArrayField.new(
      SearchKernel::StructField.new(
        {
          "first" => SearchKernel::Keyword.new,
          "last" => SearchKernel::Keyword.new
        }
      ),
      stored: false,
      searchable: false
    )
  }
)
people_type = nested_definition.fetch("fields").first.fetch("data_type")
same("ArrayDataType", people_type.fetch("kind_type"), "array schema kind")
struct_type = people_type.dig("kind", "element_type")
same("StructDataType", struct_type.fetch("kind_type"), "struct element schema kind")
same(
  %w[first last],
  struct_type.dig("kind", "fields").map { |field| field.fetch("name") },
  "struct fields are deterministic"
)
same(
  %w[String String],
  struct_type.dig("kind", "fields").map { |field| field.dig("data_type", "kind", "kind") },
  "struct child data types are preserved"
)

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
analyze_request = transport.requests.last.fetch("payload")
same(%w[text target_type target], analyze_request.keys, "analysis union discriminator precedes payload")
same("AnalyzeFieldTarget", analyze_request.fetch("target_type"), "analysis target discriminator")
same({ "field" => "title" }, analyze_request.fetch("target"), "analysis target payload")
same("index", index.describe.fetch("name"), "describe result")

Dir.mktmpdir("searchkernel-ruby-managed-test-") do |root|
  script = File.join(root, "fake-searchkerneld")
  log = File.join(root, "args.json")
  request_log = File.join(root, "requests.jsonl")
  index_path = File.join(root, "Search Index.v1")
  File.write(
    script,
    <<~'RUBY'
      #!/usr/bin/env ruby
      require "fileutils"
      require "json"
      require "socket"

      args = ARGV.each_slice(2).to_h
      FileUtils.mkdir_p(args.fetch("--data-root"))

      File.write(ENV.fetch("FAKE_HOST_LOG"), JSON.generate(ARGV))
      host, port = args.fetch("--web-listen").split(":", 2)
      server = TCPServer.new(host, Integer(port, 10))
      trap("INT") { server.close rescue nil; exit 0 }
      trap("TERM") { server.close rescue nil; exit 0 }
      loop do
        socket = server.accept
        request_line = socket.gets
        unless request_line
          socket.close
          next
        end
        headers = {}
        while (line = socket.gets) && line != "\r\n"
          key, value = line.split(":", 2)
          headers[key.downcase] = value.strip
        end
        request = JSON.parse(socket.read(Integer(headers.fetch("content-length"), 10)))
        File.open(ENV.fetch("FAKE_HOST_REQUEST_LOG"), "a") { |file| file.puts(JSON.generate(request)) }
        response_name = case request.fetch("payload_type")
                        when "SearchKernel_V2_CreateIndexRequest" then "CreateIndexResult"
                        when "SearchKernel_V2_OpenIndexRequest" then "OpenIndexResult"
                        else raise "unexpected lifecycle request"
                        end
        body = JSON.generate(
          "protocol_version" => 2,
          "payload_type" => "SearchKernel_V2_#{response_name}",
          "payload" => { "index" => {} }
        )
        socket.write("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: #{body.bytesize}\r\nConnection: close\r\n\r\n#{body}")
        socket.close
      end
    RUBY
  )
  File.chmod(0o755, script)

  previous_host = ENV["SEARCHKERNEL_LOCAL_HOST"]
  previous_log = ENV["FAKE_HOST_LOG"]
  previous_request_log = ENV["FAKE_HOST_REQUEST_LOG"]
  ENV["SEARCHKERNEL_LOCAL_HOST"] = script
  ENV["FAKE_HOST_LOG"] = log
  ENV["FAKE_HOST_REQUEST_LOG"] = request_log
  begin
    managed = SearchKernel::Index.create(
      index_path,
      fields: { "title" => SearchKernel::Text.new(stored: true, searchable: true) }
    )
    args = JSON.parse(File.read(log))
    assert(args.include?("--data-root"), "managed create starts the multi-index daemon with a data root")
    assert(!args.include?("--index"), "managed create does not select a physical index at daemon startup")
    assert(!args.include?("--definition-json"), "managed create provisions through canonical lifecycle requests")
    create_request = JSON.parse(File.readlines(request_log).fetch(0))
    assert(create_request.fetch("payload_type").end_with?("CreateIndexRequest"), "managed create sends lifecycle request")
    managed_path, managed_root, managed_target = SearchKernel::ManagedLocalHost.location(index_path)
    same(managed_target, create_request.fetch("target").fetch("name"), "managed path hashes to a canonical logical target")
    data_root_index = args.index("--data-root")
    same(managed_root, args.fetch(data_root_index + 1), "managed daemon authority is confined to the dedicated data root")
    assert(File.file?(SearchKernel::ManagedLocalHost.marker_path(managed_path)), "managed path keeps a local marker")
    sibling_path = File.join(root, "sibling")
    sibling = SearchKernel::Index.create(
      sibling_path,
      fields: { "title" => SearchKernel::Text.new(stored: true, searchable: true) }
    )
    same(1, SearchKernel::ManagedLocalHost.pool.length, "sibling managed indexes share one daemon data root")
    managed.close
    same(1, SearchKernel::ManagedLocalHost.pool.length, "closing one managed handle keeps the shared daemon alive")

    data_root = SearchKernel::ManagedLocalHost.location(sibling_path)[1]
    stale_owner = sibling.instance_variable_get(:@owner)
    stale_pid = stale_owner.instance_variable_get(:@pid)
    Process.kill("KILL", -stale_pid)
    begin
      Process.waitpid(stale_pid)
    rescue Errno::ECHILD
      nil
    end
    replacement_owner = SearchKernel::ManagedLocalHost.acquire(data_root)
    assert(!replacement_owner.equal?(stale_owner), "dead pooled daemon is replaced on the next acquire")
    replacement_owner.close

    sibling.close
    assert(managed.closed?, "managed close is idempotent state")
    managed.close

    reopened = SearchKernel::Index.open(index_path)
    args = JSON.parse(File.read(log))
    assert(args.include?("--data-root"), "managed open reuses multi-index daemon mode")
    assert(!args.include?("--index"), "managed open selects a logical target after startup")
    open_request = JSON.parse(File.readlines(request_log).last)
    assert(open_request.fetch("payload_type").end_with?("OpenIndexRequest"), "managed open sends lifecycle request")
    reopened.close
  ensure
    ENV["SEARCHKERNEL_LOCAL_HOST"] = previous_host
    ENV["FAKE_HOST_LOG"] = previous_log
    ENV["FAKE_HOST_REQUEST_LOG"] = previous_request_log
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

# Executable ergonomic evidence for typed-query families that share Ruby's
# standard FlatBuffers-JSON/HTTP facade path.
# SKC: query.phrase/basic kinds=ergonomic runtime=http
# SKC: query.proximity/basic kinds=ergonomic runtime=http
# SKC: query.same_element/basic kinds=ergonomic runtime=http
# SKC: query.fuzzy/basic kinds=ergonomic runtime=http
# SKC: query.geo/basic kinds=ergonomic runtime=http
# SKC: query.weighted_and_wand/basic kinds=ergonomic runtime=http

Dir.mktmpdir("searchkernel-ruby-managed-root-race-") do |root|
  data_root = File.join(root, ".searchkernel-managed")
  errors = []
  mutex = Mutex.new
  threads = 8.times.map do
    Thread.new do
      begin
        SearchKernel::ManagedLocalHost.prepare_data_root(data_root)
      rescue StandardError => error
        mutex.synchronize { errors << error }
      end
    end
  end
  threads.each(&:join)
  same([], errors, "concurrent managed-root first use converges")
  same(
    "searchkernel-managed-v1\n",
    File.read(File.join(data_root, SearchKernel::ManagedLocalHost::MANAGED_ROOT_MARKER_NAME)),
    "managed-root ownership marker is atomically published",
  )
end

Dir.mktmpdir("searchkernel-ruby-unowned-empty-root-") do |root|
  data_root = File.join(root, ".searchkernel-managed")
  Dir.mkdir(data_root)
  inode = File.stat(data_root).ino
  raises(SearchKernel::ClientError, "pre-existing empty unowned managed root rejects") do
    SearchKernel::ManagedLocalHost.prepare_data_root(data_root)
  end
  same(inode, File.stat(data_root).ino, "unowned managed root is never replaced")
  same([], Dir.children(data_root), "unowned managed root remains empty")
end

Dir.mktmpdir("searchkernel-ruby-managed-root-marker-") do |root|
  data_root = File.join(root, ".searchkernel-managed")
  Dir.mkdir(data_root)
  marker = File.join(data_root, SearchKernel::ManagedLocalHost::MANAGED_ROOT_MARKER_NAME)
  File.write(marker, "searchkernel-managed-v2\n")
  raises(SearchKernel::ClientError, "wrong managed-root marker version rejects") do
    SearchKernel::ManagedLocalHost.prepare_data_root(data_root)
  end
  File.delete(marker)
  Dir.mkdir(marker)
  raises(SearchKernel::ClientError, "managed-root marker directory rejects") do
    SearchKernel::ManagedLocalHost.prepare_data_root(data_root)
  end
end

Dir.mktmpdir("searchkernel-ruby-migrate-symlink-") do |root|
  real = File.join(root, "real-index")
  source = File.join(root, "legacy-index")
  data_root = File.join(root, ".searchkernel-managed")
  Dir.mkdir(real)
  Dir.mkdir(data_root)
  File.symlink(real, source)
  raises(SearchKernel::ClientError, "legacy migration rejects a symlink source") do
    SearchKernel::ManagedLocalHost.migrate_legacy(source, data_root, "local-test")
  end
  assert(File.symlink?(source), "rejected legacy symlink remains untouched")
  assert(File.directory?(real), "rejected legacy symlink target remains untouched")
end

legacy_env = %w[
  SEARCHKERNEL_INDEX_PATH
  SEARCHKERNEL_INDEX_DEFINITION
  SEARCHKERNEL_INDEX_DEFINITION_JSON
].to_h { |key| [key, ENV[key]] }
begin
  ENV["SEARCHKERNEL_INDEX_PATH"] = "/legacy/index"
  ENV["SEARCHKERNEL_INDEX_DEFINITION"] = "/legacy/definition.fb"
  ENV["SEARCHKERNEL_INDEX_DEFINITION_JSON"] = "/legacy/definition.json"
  child_env = SearchKernel::ManagedLocalHost.host_environment
  same(nil, child_env["SEARCHKERNEL_INDEX_PATH"], "managed host unsets legacy index path")
  same(nil, child_env["SEARCHKERNEL_INDEX_DEFINITION"], "managed host unsets legacy definition")
  same(nil, child_env["SEARCHKERNEL_INDEX_DEFINITION_JSON"], "managed host unsets legacy JSON definition")
ensure
  legacy_env.each do |key, value|
    value.nil? ? ENV.delete(key) : ENV[key] = value
  end
end
