# frozen_string_literal: true
require "json"; require "tempfile"; require "open3"; $LOAD_PATH.unshift(File.expand_path("../lib", __dir__)); require "searchkernel"
class NoopTransport; def execute(_request); raise "not executed"; end; end

def parity_value(value)
  return Integer(value.fetch("int64"), 10) if value.key?("int64")
  return SearchKernel::Value.uint64(Integer(value.fetch("uint64"), 10)) if value.key?("uint64")
  return value.fetch("string") if value.key?("string")
  return Float(value.fetch("float64")) if value.key?("float64")
  raise "unsupported parity corpus value: #{value.inspect}"
end
corpus=ENV.fetch("SEARCHKERNEL_PARITY_CORPUS"); output=ENV.fetch("SEARCHKERNEL_PARITY_OUTPUT"); snapshot=ENV.fetch("SEARCHKERNEL_SEMANTIC_SNAPSHOT"); fixture=ENV.fetch("SEARCHKERNEL_PARITY_INDEX"); data=JSON.parse(File.read(corpus)); index=SearchKernel::Index.new(SearchKernel::Client.new(NoopTransport.new)); out={}
data.fetch("scenarios").each do |scenario| i=scenario.fetch("intent"); b=i.fetch("kind")=="browse" ? index.browse : index.search(i.fetch("query")); if (f=i["filter"]); bindings=f.fetch("bindings").to_h{|k,v| [k, parity_value(v)]}; b.filter(f.fetch("source"),bindings); end; b.ranking(i["ranking"]) if i["ranking"]; (i["inputs"]||{}).each{|k,v| b.input(k,parity_value(v))}; (i["rank_properties"]||{}).each{|k,v| b.rank_property(k,v)}; b.limit(i["limit"]) if i.key?("limit"); b.offset(i["offset"]) if i.key?("offset"); Tempfile.create("sk-parity") do |f| f.write(JSON.generate(b.request)); f.flush; json,status=Open3.capture2(snapshot,"--index-json",fixture,"--request-json",f.path); raise "snapshot failed" unless status.success?; out[scenario.fetch("id")]=JSON.parse(json); end; end
data.fetch("facade_rejections", []).each do |rejection|
  next unless rejection.fetch("clients").include?("ruby")

  rejection.fetch("intent").fetch("rank_properties").each do |name, value|
    begin
      index.browse.rank_property(name, value)
    rescue TypeError
      next
    end
    raise "rank-property rejection accepted: #{rejection.fetch('id')}"
  end
end
File.write(output,JSON.generate(out))
