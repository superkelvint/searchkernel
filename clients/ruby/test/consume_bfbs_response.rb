# frozen_string_literal: true

require "json"

$LOAD_PATH.unshift(File.expand_path("../lib", __dir__))
require "searchkernel"

name = ARGV.fetch(0)
json = STDIN.read
raise "missing BFBS-printed response JSON" if json.empty?

envelope = JSON.parse(json)
raise "BFBS-printed response must be an object" unless envelope.is_a?(Hash)

if name == "portable-error-defaults"
  begin
    SearchKernel::Protocol.expect_response(:search, envelope)
    raise "canonical Error did not raise"
  rescue SearchKernel::SearchKernelError => error
    raise "default Error code changed" unless error.code == "Unspecified"
    raise "default Error retryable changed" unless error.retryable == false
    raise "default Error details changed" unless error.details == []
  end
else
  operation_name = name.delete_suffix("-response")
  operation = operation_name.to_sym
  unless SearchKernel::Protocol::OPERATIONS.key?(operation)
    raise "unknown BFBS response fixture #{name.inspect}"
  end

  payload = SearchKernel::Protocol.expect_response(operation, envelope)
  raise "#{name}: response payload must remain an object" unless payload.is_a?(Hash)
end

puts "PASS #{name}"
