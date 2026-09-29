# frozen_string_literal: true

require "fileutils"
require "json"
require "tmpdir"

unless ENV["SEARCHKERNEL_RUBY_USE_INSTALLED_GEM"] == "1"
  $LOAD_PATH.unshift(File.expand_path("../lib", __dir__))
end
require "searchkernel"

def check(condition, message)
  raise message unless condition
end

def same(expected, actual, message)
  return if expected == actual
  raise "#{message}\nexpected: #{expected.inspect}\nactual:   #{actual.inspect}"
end

def ids(results)
  results.hits.map(&:id)
end

def grouping_buckets(results)
  candidates = results.groups.select do |group|
    group.id.nil? &&
      group.label.nil? &&
      group.value.nil? &&
      (!group.fields || group.fields.empty?) &&
      (!group.hits || group.hits.empty?)
  end
  raise "expected exactly one canonical native grouping wrapper, got #{candidates.length}" unless candidates.length == 1

  candidates.first.groups || []
end

E2E_CORPUS_PATH = File.expand_path("../../../reference/client-real-e2e-parity/corpus.json", __dir__)
E2E_CORPUS = JSON.parse(File.read(E2E_CORPUS_PATH))
VECTOR_TYPE = E2E_CORPUS.fetch("schema").fetch("fields").fetch("embedding").fetch("type")

def corpus_vector(document_id)
  document = E2E_CORPUS.fetch("documents").find { |item| item.fetch("_id") == document_id }
  raise "missing corpus vector: #{document_id}" unless document && document["embedding"]

  document.fetch("embedding")
end

def embedding(*values)
  raise ArgumentError, "expected 8 embedding values" unless values.length == 8
  SearchKernel::Tensor.dense(VECTOR_TYPE, values)
end

PARITY_OBSERVATIONS = {}

def capture_real_engine_parity(index)
  ordered = index.search("shoes").sort("price", "asc").limit(20).execute
  filtered = index.search("shoes").filter("in_stock = true").sort("price", "asc").limit(20).execute
  faceted = index.search("shoes").facet("brand", name: "brands", limit: 10).limit(20).execute
  root = index.browse.filter("category = 'shoes'")
    .aggregate(SearchKernel::Agg.count.as("products"))
    .aggregate(SearchKernel::Agg.sum("price").as("sum_price"))
    .aggregate(SearchKernel::Agg.avg("price").as("avg_price"))
    .limit(0).execute
  grouped = index.search("shoes").group(
    SearchKernel::Group.by("brand").limit(10).precision(100)
      .order_by(SearchKernel::Grouping.desc(SearchKernel::Agg.count), SearchKernel::Grouping.asc(SearchKernel::Grouping.field("brand")))
      .output(SearchKernel::Agg.count.as("products"), SearchKernel::Agg.avg("price").as("avg_price"))
  ).execute
  nearest = index.query(SearchKernel::Query.nearest_neighbor("embedding", "query_embedding"))
    .ranking("semantic").input("query_embedding", embedding(*SHOE_1_VECTOR)).limit(3).execute
  hybrid = index.query(SearchKernel::Query.any_of(
    SearchKernel::Query.contains("title", "shoes"),
    SearchKernel::Query.nearest_neighbor("embedding", "query_embedding")
  )).ranking("hybrid").input("query_embedding", embedding(*BAG_1_VECTOR)).limit(20).execute
  reranked = index.search("candidate").ranking("two_phase").limit(4).execute
  begin
    index.search("shoes").filter("prcie = 1").execute
    raise "unknown field filter unexpectedly succeeded"
  rescue SearchKernel::SearchKernelError => error
    check(error.code == "InvalidArgument", "portable invalid-argument error code")
    error_codes = error.details.map { |detail| detail["code"] }.sort
  end
  groups = grouping_buckets(grouped)
  {
    "sorted_search" => {"ids" => ids(ordered), "total" => ordered.total_hits},
    "filtered_search" => {"ids" => ids(filtered), "total" => filtered.total_hits},
    "facets" => faceted.facets.fetch("brands").to_h { |item| [item.fetch("value"), item.fetch("count")] },
    "aggregations" => root.aggregations,
    "grouping" => groups.map { |group| {"brand" => group.value, "products" => group.fields.fetch("products"), "avg_price" => group.fields.fetch("avg_price")} },
    "nearest_neighbor" => {"ids" => ids(nearest), "scores" => nearest.hits.map(&:relevance)},
    "hybrid" => {"top_id" => hybrid.hits.first.id, "has_bag" => ids(hybrid).include?("bag-1")},
    "two_phase" => {"ids" => ids(reranked)},
    "unknown_field_error" => {"code" => "invalid_argument", "details" => error_codes}
  }
end

def write_real_engine_parity(observations)
  path = ENV["SEARCHKERNEL_REAL_E2E_PARITY_OUTPUT"]
  return unless path

  File.write(path, JSON.generate({"version" => 1, "scenarios" => observations}))
end

SHOE_1_VECTOR = corpus_vector("shoe-1").freeze
SHOE_2_VECTOR = corpus_vector("shoe-2").freeze
SHOE_3_VECTOR = corpus_vector("shoe-3").freeze
SHOE_4_VECTOR = corpus_vector("shoe-4").freeze
SHOE_5_VECTOR = corpus_vector("shoe-5").freeze
BAG_1_VECTOR = corpus_vector("bag-1").freeze

index_path = ENV.fetch("SEARCHKERNEL_RUBY_E2E_INDEX")
FileUtils.rm_rf(index_path)

field_types = {
  "text" => SearchKernel::Text,
  "keyword" => SearchKernel::Keyword,
  "int64" => SearchKernel::IntegerField,
  "float64" => SearchKernel::FloatField,
  "bool" => SearchKernel::BooleanField
}
fields = E2E_CORPUS.fetch("schema").fetch("fields").to_h do |name, spec|
  type = spec.fetch("type")
  options = spec.reject { |key, _value| key == "type" }.transform_keys(&:to_sym)
  field = if type.start_with?("tensor<")
            SearchKernel::TensorField.new(type, **options)
          else
            field_types.fetch(type).new(**options)
          end
  [name, field]
end
rank_profiles = E2E_CORPUS.fetch("schema").fetch("rank_profiles").map do |spec|
  profile = SearchKernel::RankProfile.new(spec.fetch("name")).first_phase(spec.fetch("first_phase"))
  profile = profile.second_phase(spec.fetch("second_phase")) if spec["second_phase"]
  profile = profile.rerank_count(spec.fetch("rerank_count")) if spec["rerank_count"]
  spec.fetch("query_inputs", {}).each { |name, type_spec| profile = profile.query_input(name, type_spec) }
  spec.fetch("match_features", []).each { |feature| profile = profile.match_feature(feature) }
  profile
end

index = SearchKernel::Index.create(index_path, fields: fields, rank_profiles: rank_profiles)
begin
  E2E_CORPUS.fetch("documents").each do |source|
    document = source.dup
    document["embedding"] = embedding(*document.fetch("embedding")) if document["embedding"]
    index.insert(document)
  end

  probe_spec = E2E_CORPUS.fetch("coverage_probes")
  probe_spec.fetch("count").times do |probe|
    document = probe_spec.fetch("document").dup
    document["_id"] = "degraded-#{probe}"
    document["embedding"] = embedding(*document.fetch("embedding"))
    index.insert(document)
  end

  PARITY_OBSERVATIONS.merge!(capture_real_engine_parity(index))
  index.insert("_id" => "parity-mutation", "title" => "Parity Mutation", "category" => "parity", "price" => 100.0, "sale_price" => 110.0, "rating" => 1, "inventory" => 1, "in_stock" => true)
  index.patch("parity-mutation").set("price", 75.0).unset("sale_price").execute
  mutated = index.get("parity-mutation")
  PARITY_OBSERVATIONS["mutation"] = {"price" => mutated.fetch("price"), "sale_price_present" => mutated.key?("sale_price"), "deleted" => index.delete("parity-mutation")}
  index.insert("_id" => "parity-reopen", "title" => "Parity Reopen", "category" => "parity", "price" => 42.0, "inventory" => 7, "in_stock" => true)

  results = index.search("shoes")
    .filter("price <= :max", "max" => 100.0)
    .sort("price", "asc")
    .fields("title", "price")
    .limit(10)
    .execute
  same(%w[shoe-3 shoe-1], ids(results), "text/filter/sort result")
  same("Budget Walking Shoes", results.hits.first.fields.fetch("title"), "stored field projection")
  check(results.total_hits >= 1, "total hits retained")

  highlighted = index.search("jump")
    .summary("default")
    .fields("category")
    .highlight("title")
    .execute
  highlighted_hit = highlighted.hits.find { |hit| hit.id == "shoe-1" }
  check(!highlighted_hit.nil?, "stemmed highlight returns shoe-1")
  same({ "category" => "shoes" }, highlighted_hit.fields, "named summary + field projection")
  title_highlight = highlighted_hit.highlights.find { |item| item.fetch("field") == "title" }
  check(!title_highlight.nil?, "structured title highlight present")
  fragment = title_highlight.fetch("fragments").first
  same("Jumped Running Shoes for Daily Training", fragment.fetch("text"), "highlight source text")
  start_byte, end_byte = fragment.fetch("matches").first
  same("Jumped", fragment.fetch("text").byteslice(start_byte...end_byte), "highlight follows stemming span")
  check(ids(index.search("jump").limit(10).execute).include?("shoe-1"), "stemming matches jump -> Jumped")

  repeated = index.search("shoes")
    .filter("price <= :value", "value" => 130.0)
    .filter("rating >= :value", "value" => 5)
    .filter("in_stock = true").sort("price", "asc").limit(10).execute
  same(%w[shoe-1 shoe-2], ids(repeated), "repeated filters AND with clause-local bindings")

  boolean_filter = index.search("shoes")
    .filter("(brand IN (:brands) OR rating = :rating) AND NOT (in_stock = false)",
      "brands" => ["Acme", "TrailCo"], "rating" => 3)
    .sort("price", "asc").limit(10).execute
  same(%w[shoe-3 shoe-1 shoe-5 shoe-2], ids(boolean_filter), "OR / NOT / IN / parentheses filter grammar")

  page_one = index.search("shoes").sort("price", "asc").page(1, 1).execute
  page_two = index.search("shoes").sort("price", "asc").page(2, 1).execute
  same(["shoe-3"], ids(page_one), "page one")
  same(["shoe-1"], ids(page_two), "page two")
  begin
    index.search("shoes").page(2, 20).offset(20)
    raise "page + offset should reject"
  rescue ArgumentError
    # Expected builder misuse rejection before execution.
  end

  typed = index.query(SearchKernel::Query.contains("title", "shoes")).limit(10).execute
  check(ids(typed).include?("shoe-1"), "typed contains query")

  boosted = index.search("shoes").ranking("boosted").input("boost", 3.0).limit(10).execute
  check(boosted.hits.all? { |hit| hit.score && hit.features.fetch("query(boost)") == 3.0 }, "scalar query input + match feature")

  rank_property_results = index.search("shoes")
    .ranking("boosted")
    .rank_property("boost", "4.0")
    .limit(10)
    .execute
  check(
    rank_property_results.hits.all? { |hit| hit.score && hit.features.fetch("query(boost)") == 4.0 },
    "rank property namespace reaches real ranking"
  )

  first_phase = index.search("candidate").ranking("first_phase_only").limit(4).execute
  same(%w[rerank-a rerank-b rerank-c rerank-d], ids(first_phase), "first-phase ordering")
  two_phase = index.search("candidate").ranking("two_phase").limit(4).execute
  same(%w[rerank-b rerank-c rerank-a rerank-d], ids(two_phase), "finite-window second-phase reranking")

  nearest = index.query(SearchKernel::Query.nearest_neighbor("embedding", "query_embedding"))
    .ranking("semantic").input("query_embedding", embedding(*SHOE_1_VECTOR)).limit(3).execute
  same("shoe-1", nearest.hits.first.id, "nearest-neighbor top hit")
  check(nearest.hits.first.features.key?("query(query_embedding)"), "tensor query input match feature")
  tensor_readback = index.get("shoe-1").fetch("embedding").fetch("$searchkernel_tensor")
  same("tensor<float>(x[8])", tensor_readback.fetch("type_spec"), "dense tensor type readback")
  check(tensor_readback.fetch("dense").zip(SHOE_1_VECTOR).all? { |actual, expected| (actual - expected).abs < 1e-5 }, "dense tensor values readback")

  hybrid = index.query(SearchKernel::Query.any_of(
    SearchKernel::Query.contains("title", "shoes"),
    SearchKernel::Query.nearest_neighbor("embedding", "query_embedding")
  )).ranking("hybrid").input("query_embedding", embedding(*BAG_1_VECTOR)).limit(10).execute
  check(ids(hybrid).include?("bag-1") && ids(hybrid).include?("shoe-1"), "hybrid lexical + vector candidate union")

  vector_only = hybrid.hits.find { |hit| hit.id == "bag-1" }
  lexical_hit = hybrid.hits.find { |hit| hit.id == "shoe-1" }
  check(!vector_only.nil? && !lexical_hit.nil?, "hybrid ranking exposes both vector-only and lexical hits")

  vector_bm25 = vector_only.features.fetch("bm25(title)")
  vector_closeness = vector_only.features.fetch("closeness(embedding)")
  check(vector_bm25.abs < 1e-6, "vector-only candidate has no lexical score")
  check(vector_closeness > 0.99, "vector-only candidate retains nearest-neighbor closeness")
  check(
    (vector_only.score - (vector_bm25 + 10 * vector_closeness)).abs < 1e-5,
    "hybrid score combines lexical and vector features for vector-only hit"
  )

  lexical_bm25 = lexical_hit.features.fetch("bm25(title)")
  lexical_closeness = lexical_hit.features.fetch("closeness(embedding)")
  check(lexical_bm25 > 0, "lexical hybrid candidate retains BM25 contribution")
  check(
    (lexical_hit.score - (lexical_bm25 + 10 * lexical_closeness)).abs < 1e-5,
    "hybrid score combines lexical and vector features for lexical hit"
  )

  normal_coverage = index.search("shoes").limit(10).execute.coverage
  check(!normal_coverage.nil? && normal_coverage.full == true, "normal coverage is full")

  degraded = index.browse
    .filter("inventory = :value", "value" => 18)
    .sort("inventory", "asc")
    .rank_property("vespa.matchphase.degradation.maxfiltercoverage", "0.9")
    .rank_property("vespa.matchphase.degradation.attribute", "inventory")
    .rank_property("vespa.matchphase.degradation.maxhits", "10")
    .limit(10)
    .execute
  check(!degraded.hits.empty?, "degradation query returns hits")
  check(!degraded.coverage.nil? && degraded.coverage.full == false, "degraded coverage is not full")
  check(!degraded.coverage.degraded.nil?, "degradation metadata present")
  check(
    degraded.coverage.degraded.timeout || degraded.coverage.degraded.match_phase,
    "degradation reason is preserved"
  )

  inspected = index.search("shoes").filter("price <= :max", "max" => 100.0).inspect
  check(inspected.is_a?(Hash), "inspect-query returns canonical resolved request")

  yql = index.yql("select * from sources * where title contains text(@q) order by price asc")
    .property("q", "shoes")
    .limit(10)
    .execute
  same(%w[shoe-3 shoe-1 shoe-5 shoe-2 shoe-4], ids(yql), "YQL property and ordering")

  yql_page = index.yql("select * from sources * where title contains text(@q) order by price asc")
    .property("q", "shoes")
    .page(2, 1)
    .execute
  same(["shoe-1"], ids(yql_page), "YQL pagination")
  same(5, yql_page.total_hits, "YQL pagination retains total hits")

  yql_grouped = index.yql("select * from sources * where title contains text(@q)")
    .property("q", "shoes")
    .group(SearchKernel::Group.by("brand").limit(10).output(SearchKernel::Agg.count.as("products")))
    .execute
  yql_brand_counts = grouping_buckets(yql_grouped).to_h { |group| [group.value, group.fields.fetch("products")] }
  same({ "Acme" => 2, "TrailCo" => 2, "Budget" => 1 }, yql_brand_counts, "YQL + programmatic grouping")

  begin
    index.search("shoes").filter("prcie <= :max", "max" => 100).execute
    raise "unknown field filter should fail"
  rescue SearchKernel::SearchKernelError => error
    check(error.details.any? { |detail| detail["code"] == "unknown_field" }, "structured unknown-field detail")
  end

  begin
    index.search("shoes").filter("price <= :max AND (").execute
    raise "malformed textual filter should fail"
  rescue SearchKernel::SearchKernelError => error
    check(error.details.any? { |detail| detail["code"] == "syntax_error" }, "textual syntax error detail")
  end

  grouped = index.search("shoes")
    .facet("brand", name: "brands", limit: 10)
    .facet("category", limit: 10)
    .limit(10)
    .execute
  check(!grouped.groups.empty?, "structured facet/grouping tree retained")
  same({"Acme" => 2, "TrailCo" => 2, "Budget" => 1}, grouped.facets.fetch("brands").to_h { |item| [item.fetch("value"), item.fetch("count")] }, "named facet exact counts")
  same({"shoes" => 5}, grouped.facets.fetch("category").to_h { |item| [item.fetch("value"), item.fetch("count")] }, "multiple facet projections accumulate")
  filtered_facet = index.browse.filter("category = 'shoes' AND in_stock = true").facet("brand").limit(10).execute
  same({"Acme" => 1, "TrailCo" => 2, "Budget" => 1}, filtered_facet.facets.fetch("brand").to_h { |item| [item.fetch("value"), item.fetch("count")] }, "filtered facet exact counts")

  aggregate = index.search("shoes")
    .aggregate(SearchKernel::Agg.count.as("products"))
    .aggregate(SearchKernel::Agg.sum("price").as("sum_price"))
    .aggregate(SearchKernel::Agg.avg("price").as("avg_price"))
    .aggregate(SearchKernel::Agg.min("price").as("min_price"))
    .aggregate(SearchKernel::Agg.max("price").as("max_price"))
    .aggregate(SearchKernel::Agg.stddev("price").as("stddev_price"))
    .limit(0).execute
  same(5, aggregate.aggregations.fetch("products"), "root count aggregation")
  same(535.0, aggregate.aggregations.fetch("sum_price"), "root sum aggregation")
  same(107.0, aggregate.aggregations.fetch("avg_price"), "root avg aggregation")
  same(49.0, aggregate.aggregations.fetch("min_price"), "root min aggregation")
  same(159.0, aggregate.aggregations.fetch("max_price"), "root max aggregation")
  check(aggregate.aggregations.fetch("stddev_price") > 0, "root stddev aggregation")

  grouped_counts = index.browse
    .filter("category IN (:categories)", "categories" => %w[shoes bags])
    .group(
      SearchKernel::Group.by("brand").limit(10).precision(100)
        .order_by(
          SearchKernel::Grouping.desc(SearchKernel::Agg.count),
          SearchKernel::Grouping.asc(SearchKernel::Grouping.field("brand"))
        )
        .output(
          SearchKernel::Agg.count.as("products"),
          SearchKernel::Agg.avg("price").as("avg_price")
        )
    ).limit(0).execute
  brand_groups = grouping_buckets(grouped_counts)
  same(3, brand_groups.length, "structured group count")
  same(["Acme", "TrailCo", "Budget"], brand_groups.map(&:value), "group ordering by count")
  same([3, 2, 1], brand_groups.map { |group| group.fields.fetch("products") }, "group count outputs")
  check((brand_groups[0].fields.fetch("avg_price") - 105.66666666666667).abs < 1e-9, "group avg output")
  same(119.0, brand_groups[1].fields.fetch("avg_price"), "group avg output")

  nested = index.browse.filter("category IN (:categories)", "categories" => %w[shoes bags]).group(
    SearchKernel::Group.by("category").limit(10)
      .output(SearchKernel::Agg.count.as("products"))
      .subgroup(SearchKernel::Group.by("brand").limit(10).output(SearchKernel::Agg.count.as("products")))
  ).limit(0).execute
  by_category = grouping_buckets(nested).to_h { |group| [group.value, group] }
  same(%w[bags shoes], by_category.keys.sort, "nested category groups")
  same(5, by_category.fetch("shoes").fields.fetch("products"), "nested shoes count")
  same(1, by_category.fetch("bags").fields.fetch("products"), "nested bags count")
  same(%w[Acme Budget TrailCo], by_category.fetch("shoes").groups.map(&:value).sort, "recursive subgroup decoding")

  aliased = index.search("shoes").group(
    SearchKernel::Group.by("brand").limit(10)
      .alias("revenue", SearchKernel::Agg.sum("price"))
      .order_by(SearchKernel::Grouping.desc(SearchKernel::Grouping.alias_ref("revenue")))
      .output(SearchKernel::Agg.count.as("products"), SearchKernel::Grouping.alias_ref("revenue").as("revenue"))
  ).limit(0).execute
  check(grouping_buckets(aliased).all? { |group| group.fields.key?("revenue") }, "group alias/output")

  kept = index.search("shoes").group(
    SearchKernel::Group.by("brand").limit(10)
      .keep(SearchKernel::GroupingFilter.regex(SearchKernel::Grouping.field("brand"), "^A.*"))
      .output(SearchKernel::Agg.count.as("products"))
  ).limit(0).execute
  same(["Acme"], grouping_buckets(kept).map(&:value), "group keep filter")

  summarized = index.search("shoes").group(
    SearchKernel::Group.by("brand")
      .output(
        SearchKernel::Agg.count.as("products"),
        SearchKernel::Agg.summary.as("hits")
      )
  ).execute
  grouped_hit_ids = grouping_buckets(summarized).flat_map { |group| (group.hits || []).map(&:id) }.sort
  same(%w[shoe-1 shoe-2 shoe-3 shoe-4 shoe-5], grouped_hit_ids, "summary aggregation preserves grouped Hit children")
  check(grouping_buckets(summarized).all? { |group| group.hits && !group.hits.empty? }, "summary groups retain hit children")

  same(79.0, index.get("shoe-1").fetch("sale_price"), "fixture stores sale_price before patch unset")
  index.patch("shoe-1").set("price", 79.0).set("inventory", 11).unset("sale_price").execute
  same(11, index.get("shoe-1").fetch("inventory"), "patch second set")
  patched = index.get("shoe-1")
  same(79.0, patched.fetch("price"), "patch set")
  check(!patched.key?("sale_price"), "patch unset removes an existing field")

  index.update("shoe-1").increment("inventory", 5).execute
  same(16, index.get("shoe-1").fetch("inventory"), "advanced arithmetic increment")
  index.update("shoe-1").decrement("inventory", 4).execute
  same(12, index.get("shoe-1").fetch("inventory"), "advanced arithmetic decrement")
  index.update("shoe-1").multiply("inventory", 2).execute
  same(24, index.get("shoe-1").fetch("inventory"), "advanced arithmetic multiply")
  index.update("shoe-1").divide("inventory", 4).execute
  same(6, index.get("shoe-1").fetch("inventory"), "advanced arithmetic divide")
  index.update("created-by-update").set("inventory", 7).create.execute
  same(7, index.get("created-by-update").fetch("inventory"), "create-if-nonexistent update")

  index.patch("shoe-1").set("sale_price", 75.0).execute
  same(75.0, index.get("shoe-1").fetch("sale_price"), "fixture restores sale_price before update unset")
  index.update("shoe-1").set("title", "Fast Marathon Shoes").unset("sale_price").execute
  updated_shoe1 = index.get("shoe-1")
  same("Fast Marathon Shoes", updated_shoe1.fetch("title"), "advanced set update")
  check(!updated_shoe1.key?("sale_price"), "advanced unset removes an existing field")
  check(ids(index.search("marathon").execute).include?("shoe-1"), "updated text is searchable")
  check(!ids(index.search("jumped").execute).include?("shoe-1"), "old text is removed from search")

  begin
    index.update("shoe-1").set("inventory", 16).condition("inventory == 15").execute
    raise "conditional mutation should be unsupported"
  rescue SearchKernel::SearchKernelError => error
    same("Unsupported", error.code, "conditional mutation returns typed Unsupported")
  end
  index.upsert("shoe-2", {"title" => "Trail Running Shoes Updated", "brand" => "TrailCo", "price" => 119.0, "inventory" => 4, "in_stock" => true})
  same(119.0, index.get("shoe-2").fetch("price"), "upsert replacement")

  tokens = index.analyze("Running shoes", field: "title")
  check(!tokens.empty? && tokens.first.fetch("term").is_a?(String), "analysis result")

  description = index.describe
  check(description.fetch("fields").any? { |field| field["name"] == "title" }, "describe schema")
ensure
  index.close
end

reopened = SearchKernel::Index.open(index_path)
begin
  same(79.0, reopened.get("shoe-1").fetch("price"), "reopen preserves patched field")
  same(6, reopened.get("shoe-1").fetch("inventory"), "reopen preserves arithmetic update")
  same(7, reopened.get("created-by-update").fetch("inventory"), "reopen preserves create update")
  same(119.0, reopened.get("shoe-2").fetch("price"), "reopen preserves upsert replacement")
  reopened_shoe1 = reopened.get("shoe-1")
  same("Fast Marathon Shoes", reopened_shoe1.fetch("title"), "reopen preserves text update")
  check(!reopened_shoe1.key?("sale_price"), "reopen preserves unset sale_price")
  reopened_tensor = reopened_shoe1.fetch("embedding").fetch("$searchkernel_tensor")
  same("tensor<float>(x[8])", reopened_tensor.fetch("type_spec"), "reopen preserves tensor type")
  check(
    reopened_tensor.fetch("dense").zip(SHOE_1_VECTOR).all? { |actual, expected| (actual - expected).abs < 1e-5 },
    "reopen preserves tensor values"
  )

  reopened_ranked = reopened.search("shoes").ranking("boosted").input("boost", 3.0).limit(10).execute
  check(reopened_ranked.hits.all? { |hit| hit.features.fetch("query(boost)") == 3.0 }, "ranking survives reopen")
  reopened_nearest = reopened.query(SearchKernel::Query.nearest_neighbor("embedding", "query_embedding"))
    .ranking("semantic").input("query_embedding", embedding(*SHOE_1_VECTOR)).limit(3).execute
  same("shoe-1", reopened_nearest.hits.first.id, "nearest-neighbor survives reopen")

  same(%w[shoe-3 shoe-1 shoe-5 shoe-2 shoe-4], ids(reopened.search("shoes").sort("price", "asc").limit(10).execute), "reopen search")
  persisted = reopened.get("parity-reopen")
  raise "parity-reopen document missing after reopen" unless persisted
  PARITY_OBSERVATIONS["persistence"] = {"title" => persisted.fetch("title"), "price" => persisted.fetch("price"), "inventory" => persisted.fetch("inventory")}
  check(reopened.delete("shoe-2"), "delete after reopen")
  same(false, reopened.delete("shoe-2"), "duplicate delete is false")
  same(nil, reopened.get("shoe-2"), "deleted document absent")
ensure
  reopened.close
end

write_real_engine_parity(PARITY_OBSERVATIONS)

typed_path = "#{index_path}-typed"
FileUtils.rm_rf(typed_path)
typed_index = SearchKernel::Index.create(
  typed_path,
  fields: {
    "title" => SearchKernel::Text.new(stored: true, searchable: true),
    "category" => SearchKernel::Keyword.new(stored: true, filterable: true),
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
begin
  typed_index.upsert("d1", {"title" => "quick brown fox", "category" => "fox"})
  typed_index.upsert("d2", {"title" => "brown quick fox", "category" => "fox"})
  typed_index.upsert("d3", {"title" => "quick fox", "category" => "fox"})
  typed_index.upsert("d4", {"title" => "fox", "category" => "fox"})
  typed_index.upsert("d5", {"title" => "cat", "category" => "cat"})
  typed_index.upsert("d6", {"title" => "quick agile brown fox", "category" => "fox"})
  typed_index.upsert(
    "people-1",
    {
      "people" => [
        {"first" => "Ada", "last" => "Lovelace"},
        {"first" => "Grace", "last" => "Hopper"}
      ]
    }
  )
  typed_index.upsert(
    "people-2",
    {
      "people" => [
        {"first" => "Ada", "last" => "Hopper"},
        {"first" => "Grace", "last" => "Lovelace"}
      ]
    }
  )

  phrase = typed_index.query(
    SearchKernel::Query.contains("title", SearchKernel::Query.phrase("quick", "brown"))
  ).limit(20).execute
  same(["d1"], ids(phrase), "typed phrase query distinguishes exact order and adjacency")

  near = typed_index.query(
    SearchKernel::Query.contains("title", SearchKernel::Query.near("quick", "brown"))
  ).limit(20).execute
  same(%w[d1 d2 d6], ids(near).sort, "typed near query accepts reversed order and one-gap proximity")

  ordered_near = typed_index.query(
    SearchKernel::Query.contains("title", SearchKernel::Query.onear("quick", "brown"))
  ).limit(20).execute
  same(%w[d1 d6], ids(ordered_near).sort, "typed ordered-near query permits a gap but rejects reversed order")

  fuzzy = typed_index.query(
    SearchKernel::Query.contains("category", SearchKernel::Query.fuzzy("foxx"))
  ).limit(20).execute
  same(%w[d1 d2 d3 d4 d6], ids(fuzzy).sort, "typed fuzzy query excludes unrelated terms")

  weak = typed_index.query(
    SearchKernel::Query.weak_and(
      SearchKernel::Query.contains("title", "quick"),
      SearchKernel::Query.contains("title", "fox")
    )
  ).limit(20).execute
  same(%w[d1 d2 d3 d4 d6], ids(weak).sort, "typed weak-and keeps one-term-only matches")

  wand = typed_index.query(
    SearchKernel::Query.wand("title", {"quick" => 10, "fox" => 1})
  ).limit(20).execute
  same(%w[d1 d2 d3 d4 d6], ids(wand).sort, "typed WAND keeps one-term-only matches")

  ranked_query = typed_index.query(
    SearchKernel::Query.rank(
      SearchKernel::Query.contains("title", "fox"),
      SearchKernel::Query.contains("title", "quick")
    )
  ).limit(20).execute
  same(%w[d1 d2 d3 d4 d6], ids(ranked_query).sort, "rank-only clause does not filter match candidates")

  same_element = typed_index.query(
    SearchKernel::Query.contains(
      "people",
      SearchKernel::Query.same_element(
        SearchKernel::Query.contains("first", "Ada"),
        SearchKernel::Query.contains("last", "Lovelace")
      )
    )
  ).limit(20).execute
  same(["people-1"], ids(same_element), "typed sameElement preserves per-element correlation")
ensure
  typed_index.close
  FileUtils.rm_rf(typed_path)
end

geo_path = "#{index_path}-geo"
FileUtils.rm_rf(geo_path)
geo = SearchKernel::Index.create(geo_path, fields: { "location" => SearchKernel::PositionField.new })
begin
  geo.upsert("inside", {"location" => {"latitude" => 37.3, "longitude" => -121.9}})
  geo.upsert("outside", {"location" => {"latitude" => 40.7, "longitude" => -74.0}})
  same(["inside"], ids(geo.query(SearchKernel::Query.geo_location("location", lat: 37.3, lon: -121.9, radius: "10km")).limit(10).execute), "typed geo-location query")
  same(["inside"], ids(geo.query(SearchKernel::Query.geo_box("location", south: 37.0, west: -122.2, north: 37.6, east: -121.5)).limit(10).execute), "typed geo bounding-box query")
ensure
  geo.close
  FileUtils.rm_rf(geo_path)
end

puts "Ruby real-engine facade + managed-local create/reopen E2E PASS"

# Executable capability evidence from the public Ruby facade.
# SKC: query.term/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: query.phrase/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: query.proximity/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: query.same_element/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: query.fuzzy/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: query.geo/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: query.weighted_and_wand/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: query.boolean/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: query.nearest_neighbor/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: filter.textual/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: search.sorting/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: search.pagination/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: result.docsum_field_projection/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: result.named_summary_class/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: result.highlighting/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: result.degradation_metadata/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: result.tensor_document_readback/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: result.match_features/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: result.grouping_tree/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: grouping.structured/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: grouping.aggregations/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: update.document_crud/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: update.arithmetic/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: update.create_if_nonexistent/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: tensor.dense_document_crud/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: ranking.profile_selection/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: ranking.numeric_query_inputs/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: ranking.rank_properties/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: ranking.tensor_query_inputs/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: query.yql/basic kinds=ergonomic,e2e runtime=managed-local
