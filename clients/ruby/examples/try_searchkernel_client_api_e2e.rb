# frozen_string_literal: true

require "fileutils"
require "tmpdir"

$LOAD_PATH.unshift(File.expand_path("../lib", __dir__))
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

index_path = ENV.fetch("SEARCHKERNEL_RUBY_E2E_INDEX")
FileUtils.rm_rf(index_path)

fields = {
  "title" => SearchKernel::Text.new(stored: true, searchable: true),
  "brand" => SearchKernel::Keyword.new(stored: true, filterable: true, sortable: true, facetable: true),
  "price" => SearchKernel::FloatField.new(stored: true, filterable: true, sortable: true),
  "inventory" => SearchKernel::IntegerField.new(stored: true, filterable: true, sortable: true),
  "active" => SearchKernel::BooleanField.new(stored: true, filterable: true)
}

index = SearchKernel::Index.create(index_path, fields: fields)
begin
  same("shoe-1", index.insert(
    "_id" => "shoe-1",
    "title" => "Fast Running Shoes",
    "brand" => "Acme",
    "price" => 89.0,
    "inventory" => 10,
    "active" => true
  ), "insert id")
  index.insert(
    "_id" => "shoe-2",
    "title" => "Trail Running Shoes",
    "brand" => "TrailCo",
    "price" => 129.0,
    "inventory" => 4,
    "active" => true
  )
  index.insert(
    "_id" => "bag-1",
    "title" => "Travel Bag",
    "brand" => "Acme",
    "price" => 49.0,
    "inventory" => 7,
    "active" => true
  )

  results = index.search("shoes")
    .filter("price <= :max", "max" => 100.0)
    .sort("price", "asc")
    .fields("title", "price")
    .limit(10)
    .execute
  same(["shoe-1"], ids(results), "text/filter/sort result")
  same("Fast Running Shoes", results.hits.first.fields.fetch("title"), "stored field projection")
  check(results.total_hits >= 1, "total hits retained")

  typed = index.query(SearchKernel::Query.contains("title", "shoes")).limit(10).execute
  check(ids(typed).include?("shoe-1"), "typed contains query")

  inspected = index.search("shoes").filter("price <= :max", "max" => 100.0).inspect
  check(inspected.is_a?(Hash), "inspect-query returns canonical resolved request")

  yql = index.yql("select * from sources * where title contains text(@q) order by price asc")
    .property("q", "shoes")
    .limit(10)
    .execute
  same(%w[shoe-1 shoe-2], ids(yql), "YQL property and ordering")

  grouped = index.search("shoes").facet("brand", limit: 10).limit(10).execute
  check(!grouped.groups.empty?, "structured facet/grouping tree retained")

  index.patch("shoe-1").set("price", 79.0).execute
  same(79.0, index.get("shoe-1").fetch("price"), "patch set")

  index.update("shoe-1").increment("inventory", 5).execute
  same(15, index.get("shoe-1").fetch("inventory"), "advanced arithmetic update")

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
  same(15, reopened.get("shoe-1").fetch("inventory"), "reopen preserves arithmetic update")
  same(%w[shoe-1 shoe-2], ids(reopened.search("shoes").sort("price", "asc").limit(10).execute), "reopen search")
  check(reopened.delete("shoe-2"), "delete after reopen")
  same(nil, reopened.get("shoe-2"), "deleted document absent")
ensure
  reopened.close
end

puts "Ruby real-engine facade + managed-local create/reopen E2E PASS"

# Executable capability evidence from the public Ruby facade.
# SKC: query.term/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: query.boolean/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: filter.textual/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: search.sorting/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: search.pagination/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: result.docsum_field_projection/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: grouping.structured/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: update.document_crud/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: update.arithmetic/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: query.yql/basic kinds=ergonomic,e2e runtime=managed-local
# SKC: result.grouping_tree/basic kinds=ergonomic,e2e runtime=managed-local
