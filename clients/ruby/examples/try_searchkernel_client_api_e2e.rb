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

def normalize_corpus_field_value(name, value)
  field = E2E_CORPUS.fetch("schema").fetch("fields").fetch(name, {})
  field["type"] == "float64" && !value.nil? ? Float(value) : value
end

def normalize_corpus_document(document)
  document.to_h do |name, value|
    [name, normalize_corpus_field_value(name, value)]
  end
end
PARITY_CONSUMED_INTENTS = {}

class ConsumedIntent
  def initialize(scenario, source)
    @scenario = scenario
    @source = source
  end

  def fetch(key, *defaults)
    if @source.key?(key)
      value = @source.fetch(key)
      (PARITY_CONSUMED_INTENTS[@scenario] ||= {})[key] = JSON.parse(JSON.generate(value))
      value
    elsif block_given?
      yield key
    elsif defaults.empty?
      raise KeyError, "key not found: #{key}"
    else
      defaults.first
    end
  end

  def [](key)
    return nil unless @source.key?(key)
    fetch(key)
  end
end

class ConsumedIntentMap
  def initialize(source)
    @source = source
  end

  def fetch(key)
    value = @source.fetch(key)
    raise TypeError, "parity scenario intent #{key} must be an object" unless value.is_a?(Hash)
    ConsumedIntent.new(key, value)
  end

  def [](key)
    fetch(key)
  end
end

PARITY_INTENTS = ConsumedIntentMap.new(E2E_CORPUS.fetch("scenario_inputs"))
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

def typed_query_for_scenario(scenario, intent)
  field = intent["field"]
  case scenario
  when "typed_query_term" then SearchKernel::Query.term(field, intent.fetch("value"))
  when "typed_query_contains" then SearchKernel::Query.contains(field, intent.fetch("value"))
  when "typed_query_phrase" then SearchKernel::Query.contains(field, SearchKernel::Query.phrase(*intent.fetch("terms")))
  when "typed_query_near" then SearchKernel::Query.contains(field, SearchKernel::Query.near(*intent.fetch("terms")))
  when "typed_query_ordered_near" then SearchKernel::Query.contains(field, SearchKernel::Query.onear(*intent.fetch("terms")))
  when "typed_query_range", "typed_query_range_exclusive"
    SearchKernel::Query.range(
      field,
      gte: intent.fetch("include_lower") ? normalize_corpus_field_value(field, intent.fetch("lower")) : nil,
      gt: intent.fetch("include_lower") ? nil : normalize_corpus_field_value(field, intent.fetch("lower")),
      lte: intent.fetch("include_upper") ? normalize_corpus_field_value(field, intent.fetch("upper")) : nil,
      lt: intent.fetch("include_upper") ? nil : normalize_corpus_field_value(field, intent.fetch("upper"))
    )
  when "typed_query_boolean"
    left, right = intent.fetch("or_terms")
    negative = intent.fetch("not_term")
    SearchKernel::Query.all_of(
      SearchKernel::Query.any_of(
        SearchKernel::Query.contains(left.fetch("field"), left.fetch("value")),
        SearchKernel::Query.contains(right.fetch("field"), right.fetch("value"))
      ),
      SearchKernel::Query.negate(
        SearchKernel::Query.contains(negative.fetch("field"), negative.fetch("value"))
      )
    )
  when "typed_query_in" then SearchKernel::Query.in_values(field, intent.fetch("values").map { |value| normalize_corpus_field_value(field, value) })
  when "typed_query_equals" then SearchKernel::Query.eq(field, normalize_corpus_field_value(field, intent.fetch("value")))
  when "typed_query_fuzzy" then SearchKernel::Query.contains(field, SearchKernel::Query.fuzzy(intent.fetch("value")))
  when "typed_query_weak_and"
    SearchKernel::Query.weak_and(*intent.fetch("terms").map { |term| SearchKernel::Query.contains(term.fetch("field"), term.fetch("value")) })
  when "typed_query_wand" then SearchKernel::Query.wand(field, intent.fetch("items"))
  when "typed_query_weighted_set" then SearchKernel::Query.weighted_set(field, intent.fetch("items"))
  when "typed_query_dot_product" then SearchKernel::Query.dot_product(field, intent.fetch("items"))
  when "typed_query_prefix" then SearchKernel::Query.prefix(field, intent.fetch("value"))
  when "typed_query_substring" then SearchKernel::Query.substring(field, intent.fetch("value"))
  when "typed_query_suffix" then SearchKernel::Query.suffix(field, intent.fetch("value"))
  when "typed_query_same_element"
    SearchKernel::Query.contains(
      field,
      SearchKernel::Query.same_element(
        SearchKernel::Query.contains(intent.fetch("first_field"), intent.fetch("first_value")),
        SearchKernel::Query.contains(intent.fetch("last_field"), intent.fetch("last_value"))
      )
    )
  when "typed_query_geo_location"
    SearchKernel::Query.geo_location(field, lat: intent.fetch("latitude"), lon: intent.fetch("longitude"), radius: intent.fetch("radius"))
  when "typed_query_geo_box"
    SearchKernel::Query.geo_box(field, south: intent.fetch("south"), west: intent.fetch("west"), north: intent.fetch("north"), east: intent.fetch("east"))
  else
    raise ArgumentError, "unsupported typed parity scenario: #{scenario}"
  end
end

def shared_typed_query_observations(index)
  E2E_CORPUS.fetch("scenario_inputs").filter_map do |scenario, _raw_intent|
    next unless scenario.start_with?("typed_query_")
    intent = PARITY_INTENTS.fetch(scenario)
    builder = index.query(typed_query_for_scenario(scenario, intent))
    builder = builder.ranking(intent.fetch("ranking_profile")) if intent["ranking_profile"]
    result = builder.limit(intent.fetch("limit")).execute
    result_order = intent.fetch("result_order")
    query_result = if intent["feature"]
      feature = intent.fetch("feature")
      result.hits.map { |hit| {"id" => hit.id, "feature" => hit.features[feature]} }
    else
      result_ids = ids(result)
      result_ids.sort! if result_order == "sorted_ids"
      result_ids
    end
    [scenario, {
      "observation" => {"intent" => PARITY_CONSUMED_INTENTS.fetch(scenario), "query_result" => query_result},
      "capabilities" => E2E_CORPUS.fetch("client_scenario_contract").fetch(scenario).fetch("ruby")
    }]
  end.to_h
end

def parity_aggregation_expression(spec)
  case spec.fetch("kind")
  when "count" then SearchKernel::Agg.count
  when "sum" then SearchKernel::Agg.sum(spec.fetch("field"))
  when "avg" then SearchKernel::Agg.avg(spec.fetch("field"))
  else raise "unsupported parity aggregation kind: #{spec.fetch("kind")}"
  end
end

def parity_aggregation_output(spec)
  return SearchKernel::Grouping.alias_ref(spec.fetch("name")).as(spec.fetch("output_name")) if spec.fetch("kind") == "alias"
  parity_aggregation_expression(spec).as(spec.fetch("name"))
end

def capture_real_engine_parity(index)
  typed_observations = shared_typed_query_observations(index)
  intents = PARITY_INTENTS
  ordered_spec = intents.fetch("pagination")
  ordered = index.search(ordered_spec.fetch("query")).sort(ordered_spec.fetch("sort")[0].fetch("field"), ordered_spec.fetch("sort")[0].fetch("direction")).limit(ordered_spec.fetch("all_limit")).execute
  filtered_spec = intents.fetch("filtered_facets")
  filtered = index.search(filtered_spec.fetch("query")).filter(filtered_spec.fetch("filter")).sort(ordered_spec.fetch("sort")[0].fetch("field"), ordered_spec.fetch("sort")[0].fetch("direction")).limit(filtered_spec.fetch("page_limit")).execute
  facet_spec = intents.fetch("facets")
  faceted = index.search(facet_spec.fetch("query")).facet(facet_spec.fetch("field"), name: facet_spec.fetch("name"), limit: facet_spec.fetch("limit"), sort: facet_spec.fetch("sort")).limit(facet_spec.fetch("page_limit")).execute
  root_spec = intents.fetch("root_aggregations")
  raise "root_aggregations requires browse intent" unless root_spec.fetch("kind") == "browse"
  root_builder = index.browse.filter(root_spec.fetch("filter"))
  root_spec.fetch("aggregations").each { |aggregation_spec| root_builder = root_builder.aggregate(parity_aggregation_output(aggregation_spec)) }
  root = root_builder.limit(root_spec.fetch("limit")).execute
  grouped_spec = intents.fetch("grouping_aggregations")
  group_order = grouped_spec.fetch("order").map do |order_spec|
    expression = order_spec.fetch("kind") == "count" ? SearchKernel::Agg.count : SearchKernel::Grouping.field(order_spec.fetch("field"))
    order_spec.fetch("direction") == "desc" ? SearchKernel::Grouping.desc(expression) : SearchKernel::Grouping.asc(expression)
  end
  grouped = index.search(grouped_spec.fetch("query")).group(
    SearchKernel::Group.by(grouped_spec.fetch("group_by")).limit(grouped_spec.fetch("limit")).precision(grouped_spec.fetch("precision"))
      .order_by(*group_order)
      .output(*grouped_spec.fetch("outputs").map { |output| parity_aggregation_output(output) })
  ).execute
  nearest_intent = PARITY_INTENTS.fetch("nearest_neighbor")
  nearest = index.query(SearchKernel::Query.nearest_neighbor(nearest_intent.fetch("field"), nearest_intent.fetch("query_input")))
    .ranking(nearest_intent.fetch("ranking_profile"))
    .input(nearest_intent.fetch("query_input"), embedding(*corpus_vector(nearest_intent.fetch("query_input_document"))))
    .limit(nearest_intent.fetch("limit")).execute
  hybrid_intent = PARITY_INTENTS.fetch("hybrid_text_vector")
  hybrid = index.query(SearchKernel::Query.any_of(
    SearchKernel::Query.contains(hybrid_intent.fetch("text_field"), hybrid_intent.fetch("text_value")),
    SearchKernel::Query.nearest_neighbor(hybrid_intent.fetch("vector_field"), hybrid_intent.fetch("query_input"))
  )).ranking(hybrid_intent.fetch("ranking_profile"))
    .input(hybrid_intent.fetch("query_input"), embedding(*corpus_vector(hybrid_intent.fetch("query_input_document"))))
    .limit(hybrid_intent.fetch("limit")).execute
  reranked_spec = intents.fetch("two_phase_reranking")
  reranked = index.search(reranked_spec.fetch("query")).ranking(reranked_spec.fetch("ranking_profile")).limit(reranked_spec.fetch("limit")).execute
  begin
    error_spec = intents.fetch("structured_filter_error")
    index.search(error_spec.fetch("query")).filter(error_spec.fetch("filter")).execute
    raise "unknown field filter unexpectedly succeeded"
  rescue SearchKernel::SearchKernelError => error
    check(error.code == "InvalidArgument", "portable invalid-argument error code")
    error_codes = error.details.map { |detail| detail["code"] }.sort
  end
  document = index.get(intents.fetch("document_crud").fetch("document_id"))
  tensor_spec = intents.fetch("tensor_readback")
  tensor_document = index.get(tensor_spec.fetch("document_id"))
  raise "tensor readback document is missing" unless tensor_document
  tensor_value = tensor_document.fetch(tensor_spec.fetch("field"))
  full_spec = intents.fetch("full_text_search")
  full_text = index.search(full_spec.fetch("query")).limit(full_spec.fetch("limit")).execute
  stemming_spec = intents.fetch("stemming")
  stemming = index.search(stemming_spec.fetch("query")).limit(stemming_spec.fetch("limit")).execute
  binding_spec = intents.fetch("text_filter_binding")
  text_binding = index.search(binding_spec.fetch("query")).filter(binding_spec.fetch("filter"), binding_spec.fetch("bindings")).sort(binding_spec.fetch("sort")[0].fetch("field"), binding_spec.fetch("sort")[0].fetch("direction")).limit(binding_spec.fetch("limit")).execute
  repeated_spec = intents.fetch("repeated_filters")
  repeated_builder = index.search(repeated_spec.fetch("query"))
  repeated_spec.fetch("filters").each { |filter_spec| repeated_builder = repeated_builder.filter(filter_spec.fetch("source"), filter_spec.fetch("bindings")) }
  repeated = repeated_builder.sort(repeated_spec.fetch("sort")[0].fetch("field"), repeated_spec.fetch("sort")[0].fetch("direction")).limit(repeated_spec.fetch("limit")).execute
  boolean_spec = intents.fetch("boolean_filter_grammar")
  boolean_filter = index.search(boolean_spec.fetch("query")).filter(boolean_spec.fetch("filter"), boolean_spec.fetch("bindings")).sort(boolean_spec.fetch("sort")[0].fetch("field"), boolean_spec.fetch("sort")[0].fetch("direction")).limit(boolean_spec.fetch("limit")).execute
  projection_spec = intents.fetch("sorting_projection")
  projection = index.search(projection_spec.fetch("query")).sort(projection_spec.fetch("sort")[0].fetch("field"), projection_spec.fetch("sort")[0].fetch("direction")).fields(*projection_spec.fetch("projection")).limit(projection_spec.fetch("limit")).execute
  all_sorted = index.search(ordered_spec.fetch("query")).sort(ordered_spec.fetch("sort")[0].fetch("field"), ordered_spec.fetch("sort")[0].fetch("direction")).limit(ordered_spec.fetch("all_limit")).execute
  page = index.search(ordered_spec.fetch("query")).sort(ordered_spec.fetch("sort")[0].fetch("field"), ordered_spec.fetch("sort")[0].fetch("direction")).offset(ordered_spec.fetch("offset")).limit(ordered_spec.fetch("limit")).execute
  filtered_facet = index.search(filtered_spec.fetch("query")).filter(filtered_spec.fetch("filter")).facet(filtered_spec.fetch("field"), limit: filtered_spec.fetch("limit"), sort: filtered_spec.fetch("sort")).limit(filtered_spec.fetch("page_limit")).execute
  nested_spec = intents.fetch("nested_grouping")
  raise "nested_grouping requires browse intent" unless nested_spec.fetch("kind") == "browse"
  nested = index.browse.filter(nested_spec.fetch("filter").fetch("source"), nested_spec.fetch("filter").fetch("bindings")).group(
    SearchKernel::Group.by(nested_spec.fetch("group_by")).limit(nested_spec.fetch("limit")).output(*nested_spec.fetch("outputs").map { |output| parity_aggregation_output(output) })
      .subgroup(SearchKernel::Group.by(nested_spec.fetch("nested").fetch("group_by")).limit(nested_spec.fetch("nested").fetch("limit")).output(*nested_spec.fetch("nested").fetch("outputs").map { |output| parity_aggregation_output(output) }))
  ).execute
  alias_spec = intents.fetch("group_alias")
  alias_expression = SearchKernel::Grouping.alias_ref(alias_spec.fetch("order_by_alias").fetch("name"))
  alias_order = alias_spec.fetch("order_by_alias").fetch("direction") == "desc" ? SearchKernel::Grouping.desc(alias_expression) : SearchKernel::Grouping.asc(alias_expression)
  alias_group = SearchKernel::Group.by(alias_spec.fetch("group_by")).limit(alias_spec.fetch("limit"))
  alias_spec.fetch("aliases").each do |alias_specification|
    alias_group = alias_group.alias(alias_specification.fetch("name"), parity_aggregation_expression(alias_specification))
  end
  aliased = index.search(alias_spec.fetch("query")).group(
    alias_group.order_by(alias_order)
      .output(*alias_spec.fetch("outputs").map { |output| parity_aggregation_output(output) })
  ).execute
  keep_spec = intents.fetch("group_keep_filter")
  kept = index.search(keep_spec.fetch("query")).group(
    SearchKernel::Group.by(keep_spec.fetch("group_by")).limit(keep_spec.fetch("limit"))
      .keep(SearchKernel::GroupingFilter.regex(SearchKernel::Grouping.field(keep_spec.fetch("keep_regex").fetch("field")), keep_spec.fetch("keep_regex").fetch("pattern")))
      .output(*keep_spec.fetch("outputs").map { |output| parity_aggregation_output(output) })
  ).execute
  typed = index.query(typed_query_for_scenario("typed_query_contains", intents.fetch("typed_query_contains"))).ranking("default").limit(full_spec.fetch("limit")).execute
  rank_input_spec = intents.fetch("rank_inputs")
  rank_input_builder = index.search(rank_input_spec.fetch("query")).ranking(rank_input_spec.fetch("ranking_profile"))
  rank_input_spec.fetch("inputs").each do |key, value|
    value = Float(value) if rank_input_spec.fetch("input_types").fetch(key) == "float64"
    rank_input_builder = rank_input_builder.input(key, value)
  end
  rank_input = rank_input_builder.limit(rank_input_spec.fetch("limit")).execute
  rank_property_spec = intents.fetch("rank_properties")
  rank_property_builder = index.search(rank_property_spec.fetch("query")).ranking(rank_property_spec.fetch("ranking_profile"))
  rank_property_spec.fetch("rank_properties").each { |key, value| rank_property_builder = rank_property_builder.rank_property(key, value) }
  rank_property = rank_property_builder.limit(rank_property_spec.fetch("limit")).execute
  degraded_spec = intents.fetch("coverage_degradation")
  raise "coverage_degradation requires browse intent" unless degraded_spec.fetch("kind") == "browse"
  degraded_builder = index.browse.filter(degraded_spec.fetch("filter").fetch("source"), degraded_spec.fetch("filter").fetch("bindings")).sort(degraded_spec.fetch("sort")[0].fetch("field"), degraded_spec.fetch("sort")[0].fetch("direction"))
  degraded_spec.fetch("rank_properties").each { |key, value| degraded_builder = degraded_builder.rank_property(key, value) }
  degraded = degraded_builder.limit(degraded_spec.fetch("limit")).execute
  yql_spec = intents.fetch("yql")
  yql_builder = index.yql(yql_spec.fetch("source"))
  yql_spec.fetch("bindings").each { |key, value| yql_builder = yql_builder.property(key, value) }
  yql = yql_builder.ranking(yql_spec.fetch("ranking_profile")).page(yql_spec.fetch("page"), yql_spec.fetch("page_size")).execute
  begin
    syntax_spec = intents.fetch("text_filter_syntax_error")
    index.search(syntax_spec.fetch("query")).filter(syntax_spec.fetch("filter")).execute
    raise "malformed textual filter unexpectedly succeeded"
  rescue SearchKernel::SearchKernelError => error
    syntax_error = {"code" => error.code.downcase, "details" => error.details.map { |detail| detail["code"] }.sort}
  end
  groups = grouping_buckets(grouped)
  misuse_spec = intents.fetch("builder_misuse")
  negative_limit_rejected = begin
    index.search(misuse_spec.fetch("query")).limit(misuse_spec.fetch("negative_limit"))
    false
  rescue ArgumentError, TypeError
    true
  end
  page_and_limit_rejected = begin
    index.search(misuse_spec.fetch("query")).page(misuse_spec.fetch("page"), misuse_spec.fetch("page_size")).limit(misuse_spec.fetch("limit"))
    false
  rescue ArgumentError, TypeError
    true
  end
  page_and_offset_rejected = begin
    index.search(misuse_spec.fetch("query")).page(misuse_spec.fetch("page"), misuse_spec.fetch("page_size")).offset(misuse_spec.fetch("page_size"))
    false
  rescue ArgumentError, TypeError
    true
  end
  negative_offset_rejected = begin
    index.search(misuse_spec.fetch("query")).offset(misuse_spec.fetch("negative_offset"))
    false
  rescue ArgumentError, TypeError
    true
  end
  check(negative_limit_rejected && negative_offset_rejected && page_and_limit_rejected && page_and_offset_rejected,
        "builder misuse inputs are rejected")
  update_spec = intents.fetch("advanced_update")
  index.insert(normalize_corpus_document(update_spec.fetch("initial")).merge("_id" => update_spec.fetch("document_id")))
  update_builder = index.update(update_spec.fetch("document_id"))
  update_spec.fetch("set").each { |key, value| update_builder = update_builder.set(key, normalize_corpus_field_value(key, value)) }
  update_spec.fetch("unset").each { |field| update_builder = update_builder.unset(field) }
  update_spec.fetch("increments").each { |key, value| update_builder = update_builder.increment(key, normalize_corpus_field_value(key, value)) }
  update_builder.execute
  update_spec.fetch("decrements").each { |key, value| index.update(update_spec.fetch("document_id")).decrement(key, normalize_corpus_field_value(key, value)).execute }
  update_spec.fetch("multipliers").each { |key, value| index.update(update_spec.fetch("document_id")).multiply(key, normalize_corpus_field_value(key, value)).execute }
  update_spec.fetch("divisors").each { |key, value| index.update(update_spec.fetch("document_id")).divide(key, normalize_corpus_field_value(key, value)).execute }
  advanced_update = index.get(update_spec.fetch("document_id"))
  observations = {
    "document_crud" => {"id" => document.fetch("_id"), "title" => document.fetch("title"), "brand" => document.fetch("brand"), "price" => document.fetch("price")},
    "full_text_search" => {"ids" => ids(full_text), "total" => full_text.total_hits},
    "stemming" => {"ids" => ids(stemming)},
    "text_filter_binding" => {"ids" => ids(text_binding)},
    "repeated_filters" => {"ids" => ids(repeated)},
    "boolean_filter_grammar" => {"ids" => ids(boolean_filter)},
    "sorting_projection" => {"hits" => projection.hits.map { |hit| {"id" => hit.id, "fields" => hit.fields} }},
    "pagination" => {"all_ids" => ids(all_sorted), "offset_ids" => ids(page), "total" => page.total_hits},
    "builder_misuse" => {
      "observation" => {
        "page_and_limit_rejected" => page_and_limit_rejected,
        "page_and_offset_rejected" => page_and_offset_rejected,
        "negative_limit_rejected" => negative_limit_rejected,
        "negative_offset_rejected" => negative_offset_rejected
      },
      "capabilities" => {
        "negative_limit_rejected" => {"kind" => "runtime_rejected"},
        "negative_offset_rejected" => {"kind" => "runtime_rejected"}
      }
    },
    "sorted_search" => {"ids" => ids(ordered), "total" => ordered.total_hits},
    "filtered_search" => {"ids" => ids(filtered), "total" => filtered.total_hits},
    "facets" => faceted.facets.fetch("brands").map { |item| {"value" => item.fetch("value").to_s, "count" => item.fetch("count")} },
    "root_aggregations" => root.aggregations,
    "advanced_update" => {"title" => advanced_update.fetch("title"), "inventory" => advanced_update.fetch("inventory"), "sale_price_present" => advanced_update.key?("sale_price")},
    "grouping_aggregations" => groups.map { |group| {"brand" => group.value, "products" => group.fields.fetch("products"), "avg_price" => group.fields.fetch("avg_price")} },
    "filtered_facets" => filtered_facet.facets.fetch("brand").map { |item| {"value" => item.fetch("value").to_s, "count" => item.fetch("count")} },
    "nested_grouping" => grouping_buckets(nested).map { |group|
      {"category" => group.value, "products" => group.fields.fetch("products"),
       "ratings" => group.groups.map { |rating| {"rating" => rating.value, "products" => rating.fields.fetch("products"), "avg_price" => rating.fields.fetch("avg_price")} }}
    },
    "group_alias" => grouping_buckets(aliased).map { |group| {"value" => group.value, "fields" => group.fields} },
    "group_keep_filter" => grouping_buckets(kept).map { |group| {"value" => group.value, "fields" => group.fields} },
    **typed_observations,
    "rank_inputs" => rank_input.hits.map { |hit| {"id" => hit.id, "score" => hit.score, "feature" => hit.features["query(boost)"]} },
    "rank_properties" => rank_property.hits.map { |hit| {"id" => hit.id, "score" => hit.score, "feature" => hit.features["query(boost)"]} },
    "tensor_readback" => tensor_value.fetch("$searchkernel_tensor").fetch("dense"),
    "nearest_neighbor" => {"observation" => {"intent" => PARITY_CONSUMED_INTENTS.fetch("nearest_neighbor"), "ids" => ids(nearest), "scores" => nearest.hits.map(&:relevance)}, "capabilities" => {}},
    "hybrid_text_vector" => {"observation" => {"intent" => PARITY_CONSUMED_INTENTS.fetch("hybrid_text_vector"), "ids" => ids(hybrid), "top_score" => hybrid.hits.first.score}, "capabilities" => {}},
    "hybrid" => {"top_id" => hybrid.hits.first.id, "has_bag" => ids(hybrid).include?("bag-1")},
    "two_phase_reranking" => {"ids" => ids(reranked)},
    "coverage_degradation" => {"ids" => ids(degraded), "full" => degraded.coverage.full, "degraded" => !degraded.coverage.degraded.nil?},
    "yql" => {"ids" => ids(yql), "total" => yql.total_hits},
    "unknown_field_error" => {"code" => "invalid_argument", "details" => error_codes},
    "structured_filter_error" => {"code" => "invalid_argument", "details" => error_codes},
    "text_filter_syntax_error" => {"code" => "invalid_argument", "details" => syntax_error.fetch("details")}
  }
  observations
end

def write_real_engine_parity(observations)
  path = ENV["SEARCHKERNEL_REAL_E2E_PARITY_OUTPUT"]
  return unless path

  # SDK result maps can be frozen; the parity writer adds intent metadata.
  observations = JSON.parse(JSON.generate(observations))
  intent_aliases = {
    "sorted_search" => "pagination",
    "filtered_search" => "filtered_facets",
    "hybrid" => "hybrid_text_vector",
    "unknown_field_error" => "structured_filter_error",
  }
  observations.each do |scenario, value|
    next if scenario.start_with?("typed_query_")
    intent = PARITY_CONSUMED_INTENTS[intent_aliases.fetch(scenario, scenario)]
    raise "parity scenario #{scenario} emitted an observation without consuming its shared intent" unless intent

    payload = value.is_a?(Hash) && value.key?("observation") ? value.fetch("observation") : value
    payload["intent"] = intent if payload.is_a?(Hash) && !payload.key?("intent")
    observations[scenario] = {"intent" => intent, "result" => value} unless payload.is_a?(Hash)
  end
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
  "bool" => SearchKernel::BooleanField,
  "position" => SearchKernel::PositionField
}
field_from_corpus = lambda do |spec|
  type = spec.fetch("type")
  options = spec.reject { |key, _value| %w[type element fields].include?(key) }.transform_keys(&:to_sym)
  field = case type
          when "array"
            element = spec.fetch("element")
            members = element.fetch("fields").transform_values { |member| field_from_corpus.call(member) }
            SearchKernel::ArrayField.new(SearchKernel::StructField.new(members), **options)
          when "struct"
            SearchKernel::StructField.new(spec.fetch("fields").transform_values { |member| field_from_corpus.call(member) }, **options)
          else
            type.start_with?("tensor<") ? SearchKernel::TensorField.new(type, **options) : field_types.fetch(type).new(**options)
          end
  field
end
fields = E2E_CORPUS.fetch("schema").fetch("fields").to_h do |name, spec|
  [name, field_from_corpus.call(spec)]
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
    document = normalize_corpus_document(source)
    document["embedding"] = embedding(*document.fetch("embedding")) if document["embedding"]
    index.insert(document)
  end

  probe_spec = E2E_CORPUS.fetch("coverage_probes")
  probe_spec.fetch("count").times do |probe|
    document = normalize_corpus_document(probe_spec.fetch("document"))
    document["_id"] = "degraded-#{probe}"
    document["embedding"] = embedding(*document.fetch("embedding"))
    index.insert(document)
  end

  PARITY_OBSERVATIONS.merge!(capture_real_engine_parity(index))
  patch_spec = PARITY_INTENTS.fetch("patch")
  index.insert(normalize_corpus_document(patch_spec.fetch("initial")).merge("_id" => patch_spec.fetch("document_id")))
  patch_builder = index.patch(patch_spec.fetch("document_id"))
  patch_spec.fetch("set").each { |key, value| patch_builder = patch_builder.set(key, normalize_corpus_field_value(key, value)) }
  patch_spec.fetch("unset").each { |field| patch_builder = patch_builder.unset(field) }
  patch_builder.execute
  mutated = index.get(patch_spec.fetch("document_id"))
  PARITY_OBSERVATIONS["patch"] = {"price" => mutated.fetch("price"), "sale_price_present" => mutated.key?("sale_price")}
  delete_spec = PARITY_INTENTS.fetch("delete")
  parity_mutation_deleted = index.delete(delete_spec.fetch("document_id"))
  PARITY_OBSERVATIONS["delete"] = {"deleted" => parity_mutation_deleted, "absent_after_delete" => index.get(delete_spec.fetch("document_id")).nil?}
  reopen_spec = PARITY_INTENTS.fetch("reopen_persistence")
  index.insert(normalize_corpus_document(reopen_spec.fetch("marker")).merge("_id" => reopen_spec.fetch("marker_id")))

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
  upsert_spec = PARITY_INTENTS.fetch("upsert_replacement")
  replacement = upsert_spec.fetch("replacement").reject { |key, _value| key == "vector_document_id" }
  replacement = normalize_corpus_document(replacement)
  replacement["embedding"] = embedding(*corpus_vector(upsert_spec.fetch("replacement").fetch("vector_document_id")))
  index.upsert(upsert_spec.fetch("document_id"), replacement)
  upserted_shoe2 = index.get(upsert_spec.fetch("document_id"))
  same(119.0, upserted_shoe2.fetch("price"), "upsert replacement")
  PARITY_OBSERVATIONS["upsert_replacement"] = {"price" => upserted_shoe2.fetch("price"), "updated_title_matches" => upserted_shoe2.fetch("title").include?("Updated"), "search_ids" => ids(index.search(upsert_spec.fetch("search_query")).execute)}

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
  reopen_spec = PARITY_INTENTS.fetch("reopen_persistence")
  upsert_spec = PARITY_INTENTS.fetch("upsert_replacement")
  persisted = reopened.get(reopen_spec.fetch("marker_id"))
  raise "parity-reopen document missing after reopen" unless persisted
  persisted_shoe2 = reopened.get(upsert_spec.fetch("document_id"))
  PARITY_OBSERVATIONS["reopen_persistence"] = {
    "marker" => {"title" => persisted.fetch("title"), "price" => persisted.fetch("price"), "inventory" => persisted.fetch("inventory")},
    "upserted" => {"updated_title_matches" => persisted_shoe2.fetch("title").include?("Updated"), "price" => persisted_shoe2.fetch("price")},
    "deleted_absent" => reopened.get(PARITY_INTENTS.fetch("delete").fetch("document_id")).nil?
  }
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
