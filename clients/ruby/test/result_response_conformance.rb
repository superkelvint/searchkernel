# frozen_string_literal: true

require "json"
$LOAD_PATH.unshift(File.expand_path("../lib", __dir__))
require "searchkernel"

cases = JSON.parse(File.read(ENV.fetch("SEARCHKERNEL_RESULT_CASES")))
case_directory = ENV.fetch("SEARCHKERNEL_RESULT_CASE_DIR")
traces = {}
cases.fetch("valid").each do |scenario|
  wire = JSON.parse(File.read(File.join(case_directory, scenario.fetch("json"))))
  result = SearchKernel::Results::SearchResults.new(wire.fetch("payload"))
  raise "SearchResult root missing: #{scenario.fetch('id')}" if result.root.nil?
  if scenario.fetch("id") == "value-variants"
    expected = (2**64) - 1
    raise "uint64 maximum lost during result decoding" unless result.root.fields.fetch("uint64_max") == expected
  end

  traces[scenario.fetch("id")] = File.read(File.join(case_directory, scenario.fetch("json")))
end

rejected = []
cases.fetch("malformed").each do |scenario|
  if scenario.fetch("bridge_rejected")
    rejected << scenario.fetch("id")
    next
  end
  begin
    wire = JSON.parse(File.read(File.join(case_directory, scenario.fetch("json"))))
    SearchKernel::Results::SearchResults.new(wire.fetch("payload"))
  rescue StandardError
    rejected << scenario.fetch("id")
    next
  end
  raise "malformed result was accepted: #{scenario.fetch('id')}"
end
traces["malformed_rejected"] = rejected
File.write(File.join(ENV.fetch("SEARCHKERNEL_RESULT_TRACE_DIR"), "ruby.json"), JSON.generate(traces))
