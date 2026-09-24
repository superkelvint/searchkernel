# frozen_string_literal: true

require "json"

$LOAD_PATH.unshift(File.expand_path("../lib", __dir__))
require "searchkernel"

def emit_case(schema, name, value)
  puts [schema, name, JSON.generate(value)].join("\t")
end

SearchKernel::Protocol::OPERATIONS.each do |operation, spec|
  emit_case(
    "request",
    "#{operation}-request",
    SearchKernel::Protocol.request_envelope(operation, {})
  )
  emit_case(
    "response",
    "#{operation}-response",
    {
      "protocol_version" => SearchKernel::Protocol::VERSION,
      "payload_type" => SearchKernel::Protocol.wire_type(spec.fetch(:response)),
      "payload" => {}
    }
  )
end

emit_case(
  "response",
  "portable-error-defaults",
  {
    "protocol_version" => SearchKernel::Protocol::VERSION,
    "payload_type" => SearchKernel::Protocol::ERROR_TYPE,
    "payload" => { "message" => "bfbs error" }
  }
)
