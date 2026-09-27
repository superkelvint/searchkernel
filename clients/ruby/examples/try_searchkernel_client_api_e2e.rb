# frozen_string_literal: true

require "fileutils"
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

def embedding(*values)
  raise ArgumentError, "expected 8 embedding values" unless values.length == 8
  SearchKernel::Tensor.dense("tensor<float>(x[8])", values)
end

SHOE_1_VECTOR = [0.92, 0.14, 0.05, 0.74, 0.76, 0.42, 0.20, 0.78].freeze
SHOE_2_VECTOR = [0.74, 0.55, 0.08, 0.58, 0.42, 0.86, 0.18, 0.64].freeze
BAG_1_VECTOR = [0.45, 0.25, 0.05, 0.35, 0.95, 0.15, 0.05, 0.55].freeze

index_path = ENV.fetch("SEARCHKERNEL_RUBY_E2E_INDEX")
FileUtils.rm_rf(index_path)

fields = {
  "title" => SearchKernel::Text.new(stored: true, searchable: true),
  "brand" => SearchKernel::Keyword.new(stored: true, filterable: true, sortable: true, facetable: true),
  "price" => SearchKernel::FloatField.new(stored: true, filterable: true, sortable: true),
  "sale_price" => SearchKernel::FloatField.new(stored: true, filterable: true, sortable: true),
  "inventory" => SearchKernel::IntegerField.new(stored: true, filterable: true, sortable: true),
  "active" => SearchKernel::BooleanField.new(stored: true, filterable: true),
  "category" => SearchKernel::Keyword.new(stored: true, filterable: true, facetable: true),
  "rating" => SearchKernel::IntegerField.new(stored: true, filterable: true, sortable: true),
  "embedding" => SearchKernel::TensorField.new("tensor<float>(x[8])", stored: true)
}

rank_profiles = [
  SearchKernel::RankProfile.new("boosted").first_phase("query(boost)").match_feature("query(boost)"),
  SearchKernel::RankProfile.new("semantic").first_phase("closeness(embedding)")
    .query_input("query_embedding", "tensor<float>(x[8])").match_feature("query(query_embedding)"),
  SearchKernel::RankProfile.new("hybrid").first_phase("bm25(title) + 10 * closeness(embedding)")
    .query_input("query_embedding", "tensor<float>(x[8])").match_feature("bm25(title)")
    .match_feature("closeness(embedding)").match_feature("query(query_embedding)"),
  SearchKernel::RankProfile.new("two_phase").first_phase("attribute(rating)")
    .second_phase("attribute(inventory)").rerank_count(3),
  SearchKernel::RankProfile.new("first_phase_only").first_phase("attribute(rating)")
]

index = SearchKernel::Index.create(index_path, fields: fields, rank_profiles: rank_profiles)
begin
  same("shoe-1", index.insert(
    "_id" => "shoe-1",
    "title" => "Jumped Running Shoes for Daily Training",
    "brand" => "Acme",
    "price" => 89.0,
    "inventory" => 10,
    "active" => true, "category" => "shoes", "rating" => 5,
    "embedding" => embedding(*SHOE_1_VECTOR)
  ), "insert id")
  index.insert(
    "_id" => "shoe-2",
    "title" => "Trail Running Shoes",
    "brand" => "TrailCo",
    "price" => 129.0,
    "inventory" => 4,
    "active" => true, "category" => "shoes", "rating" => 5,
    "embedding" => embedding(*SHOE_2_VECTOR)
  )
  index.insert(
    "_id" => "bag-1",
    "title" => "Travel Bag",
    "brand" => "Acme",
    "price" => 49.0,
    "inventory" => 7,
    "active" => true, "category" => "bags", "rating" => 4,
    "embedding" => embedding(*BAG_1_VECTOR)
  )
  index.insert("_id" => "rerank-a", "title" => "Rerank Candidate A", "category" => "two-phase", "rating" => 100, "inventory" => 1)
  index.insert("_id" => "rerank-b", "title" => "Rerank Candidate B", "category" => "two-phase", "rating" => 90, "inventory" => 100)
  index.insert("_id" => "rerank-c", "title" => "Rerank Candidate C", "category" => "two-phase", "rating" => 80, "inventory" => 90)
  index.insert("_id" => "rerank-d", "title" => "Rerank Candidate D", "category" => "two-phase", "rating" => 70, "inventory" => 1000)

  200.times do |probe|
    index.insert(
      "_id" => "degraded-#{probe}",
      "title" => "Coverage Degradation Probe",
      "category" => "degradation",
      "price" => 18.0,
      "rating" => 1,
      "inventory" => 18,
      "embedding" => embedding(*SHOE_2_VECTOR)
    )
  end

  results = index.search("shoes")
    .filter("price <= :max", "max" => 100.0)
    .sort("price", "asc")
    .fields("title", "price")
    .limit(10)
    .execute
  same(["shoe-1"], ids(results), "text/filter/sort result")
  same("Jumped Running Shoes for Daily Training", results.hits.first.fields.fetch("title"), "stored field projection")
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
    .filter("active = true").sort("price", "asc").limit(10).execute
  same(%w[shoe-1 shoe-2], ids(repeated), "repeated filters AND with clause-local bindings")

  boolean_filter = index.search("shoes")
    .filter("(brand IN (:brands) OR rating = :rating) AND NOT (active = false)",
      "brands" => ["Acme", "TrailCo"], "rating" => 3)
    .sort("price", "asc").limit(10).execute
  same(%w[shoe-1 shoe-2], ids(boolean_filter), "OR / NOT / IN / parentheses filter grammar")

  page_one = index.search("shoes").sort("price", "asc").page(1, 1).execute
  page_two = index.search("shoes").sort("price", "asc").page(2, 1).execute
  same(["shoe-1"], ids(page_one), "page one")
  same(["shoe-2"], ids(page_two), "page two")
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
  same(%w[shoe-1 shoe-2], ids(yql), "YQL property and ordering")

  yql_page = index.yql("select * from sources * where title contains text(@q) order by price asc")
    .property("q", "shoes")
    .page(2, 1)
    .execute
  same(["shoe-2"], ids(yql_page), "YQL pagination")
  same(2, yql_page.total_hits, "YQL pagination retains total hits")

  yql_grouped = index.yql("select * from sources * where title contains text(@q)")
    .property("q", "shoes")
    .group(SearchKernel::Group.by("brand").limit(10).output(SearchKernel::Agg.count.as("products")))
    .execute
  yql_brand_counts = grouping_buckets(yql_grouped).to_h { |group| [group.value, group.fields.fetch("products")] }
  same({ "Acme" => 1, "TrailCo" => 1 }, yql_brand_counts, "YQL + programmatic grouping")

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
  same({"Acme" => 1, "TrailCo" => 1}, grouped.facets.fetch("brands").to_h { |item| [item.fetch("value"), item.fetch("count")] }, "named facet exact counts")
  same({"shoes" => 2}, grouped.facets.fetch("category").to_h { |item| [item.fetch("value"), item.fetch("count")] }, "multiple facet projections accumulate")
  filtered_facet = index.browse.filter("active = true").facet("brand").limit(10).execute
  same({"Acme" => 2, "TrailCo" => 1}, filtered_facet.facets.fetch("brand").to_h { |item| [item.fetch("value"), item.fetch("count")] }, "filtered facet exact counts")

  aggregate = index.search("shoes")
    .aggregate(SearchKernel::Agg.count.as("products"))
    .aggregate(SearchKernel::Agg.sum("price").as("sum_price"))
    .aggregate(SearchKernel::Agg.avg("price").as("avg_price"))
    .aggregate(SearchKernel::Agg.min("price").as("min_price"))
    .aggregate(SearchKernel::Agg.max("price").as("max_price"))
    .aggregate(SearchKernel::Agg.stddev("price").as("stddev_price"))
    .limit(0).execute
  same(2, aggregate.aggregations.fetch("products"), "root count aggregation")
  same(218.0, aggregate.aggregations.fetch("sum_price"), "root sum aggregation")
  same(109.0, aggregate.aggregations.fetch("avg_price"), "root avg aggregation")
  same(89.0, aggregate.aggregations.fetch("min_price"), "root min aggregation")
  same(129.0, aggregate.aggregations.fetch("max_price"), "root max aggregation")
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
  same(2, brand_groups.length, "structured group count")
  same(["Acme", "TrailCo"], brand_groups.map(&:value), "group ordering by count")
  same([2, 1], brand_groups.map { |group| group.fields.fetch("products") }, "group count outputs")
  same(69.0, brand_groups[0].fields.fetch("avg_price"), "group avg output")
  same(129.0, brand_groups[1].fields.fetch("avg_price"), "group avg output")

  nested = index.browse.filter("category IN (:categories)", "categories" => %w[shoes bags]).group(
    SearchKernel::Group.by("category").limit(10)
      .output(SearchKernel::Agg.count.as("products"))
      .subgroup(SearchKernel::Group.by("brand").limit(10).output(SearchKernel::Agg.count.as("products")))
  ).limit(0).execute
  by_category = grouping_buckets(nested).to_h { |group| [group.value, group] }
  same(%w[bags shoes], by_category.keys.sort, "nested category groups")
  same(2, by_category.fetch("shoes").fields.fetch("products"), "nested shoes count")
  same(1, by_category.fetch("bags").fields.fetch("products"), "nested bags count")
  same(%w[Acme TrailCo], by_category.fetch("shoes").groups.map(&:value).sort, "recursive subgroup decoding")

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
  same(%w[shoe-1 shoe-2], grouped_hit_ids, "summary aggregation preserves grouped Hit children")
  check(grouping_buckets(summarized).all? { |group| group.hits && !group.hits.empty? }, "summary groups retain hit children")

  index.patch("shoe-1").set("price", 79.0).set("inventory", 11).unset("sale_price").execute
  same(11, index.get("shoe-1").fetch("inventory"), "patch second set")
  patched = index.get("shoe-1")
  same(79.0, patched.fetch("price"), "patch set")
  check(!patched.key?("sale_price"), "patch unset")

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
  index.update("shoe-1").set("title", "Fast Marathon Shoes").unset("sale_price").execute
  same("Fast Marathon Shoes", index.get("shoe-1").fetch("title"), "advanced set update")
  check(ids(index.search("marathon").execute).include?("shoe-1"), "updated text is searchable")
  check(!ids(index.search("jumped").execute).include?("shoe-1"), "old text is removed from search")

  begin
    index.update("shoe-1").set("inventory", 16).condition("inventory == 15").execute
    raise "conditional mutation should be unsupported"
  rescue SearchKernel::SearchKernelError => error
    same("Unsupported", error.code, "conditional mutation returns typed Unsupported")
  end
  index.upsert("shoe-2", {"title" => "Trail Running Shoes Updated", "brand" => "TrailCo", "price" => 119.0, "inventory" => 4, "active" => true})
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

  same(%w[shoe-1 shoe-2], ids(reopened.search("shoes").sort("price", "asc").limit(10).execute), "reopen search")
  check(reopened.delete("shoe-2"), "delete after reopen")
  same(false, reopened.delete("shoe-2"), "duplicate delete is false")
  same(nil, reopened.get("shoe-2"), "deleted document absent")
ensure
  reopened.close
end

puts "Ruby real-engine facade + managed-local create/reopen E2E PASS"

# Executable capability evidence from the public Ruby facade.
# SKC: query.term/basic kinds=ergonomic,e2e runtime=managed-local
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
