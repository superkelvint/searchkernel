# frozen_string_literal: true

require "searchkernel"

endpoint = ENV.fetch("SEARCHKERNEL_ENDPOINT", "http://127.0.0.1:8080")
target = ENV.fetch("SEARCHKERNEL_TARGET")
index = SearchKernel::Index.connect(endpoint, target: target)

begin
  index.insert(
    "_id" => "quickstart-ruby",
    "title" => "quickstart example",
    "kind" => "quickstart",
    "score" => 1
  )
  hits = index.search("quickstart").execute.hits.map(&:id)
  raise "quickstart search did not return the inserted document" unless hits == ["quickstart-ruby"]
ensure
  index.close
end

puts "Ruby release quickstart PASS"
