# frozen_string_literal: true

require "searchkernel"

endpoint = ENV.fetch("SEARCHKERNEL_ENDPOINT", "http://127.0.0.1:50052")
target = ENV.fetch("SEARCHKERNEL_TARGET", "quickstart")
fields = {
  "title" => SearchKernel::Text.new(stored: true, searchable: true),
  "kind" => SearchKernel::Keyword.new(stored: true, filterable: true),
  "score" => SearchKernel::Integer.new(stored: true, sortable: true)
}
documents = [
  {"_id" => "quickstart-a", "title" => "quickstart alpha", "kind" => "guide", "score" => 2},
  {"_id" => "quickstart-b", "title" => "quickstart beta", "kind" => "guide", "score" => 1},
  {"_id" => "quickstart-c", "title" => "quickstart other", "kind" => "other", "score" => 0}
]

matching_ids = lambda do |index|
  index.search("quickstart")
    .filter("kind = 'guide'")
    .sort("score", "asc")
    .execute
    .hits
    .map(&:id)
end

index = SearchKernel::Index.create(endpoint, fields: fields, target: target)
begin
  documents.each { |document| index.insert(document) }
  ids = matching_ids.call(index)
  raise "unexpected filtered/sorted ids: #{ids.inspect}" unless ids == %w[quickstart-b quickstart-a]
ensure
  index.close
end

index = SearchKernel::Index.open(endpoint, target: target)
begin
  raise "reopened index lost quickstart-b" unless index.get("quickstart-b").fetch("score") == 1
  ids = matching_ids.call(index)
  raise "unexpected reopened ids: #{ids.inspect}" unless ids == %w[quickstart-b quickstart-a]
ensure
  index.close
end

puts "Ruby release quickstart PASS"
