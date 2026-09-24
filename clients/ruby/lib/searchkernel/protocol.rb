# frozen_string_literal: true

require_relative "errors"

module SearchKernel
  module Protocol
    VERSION = 2
    TYPE_PREFIX = "SearchKernel_V2_"
    ERROR_TYPE = "#{TYPE_PREFIX}Error"

    OPERATIONS = {
      insert: { request: "InsertRequest", response: "InsertResult" },
      upsert: { request: "UpsertRequest", response: "UpsertResult" },
      patch: { request: "PatchRequest", response: "PatchResult" },
      update: { request: "UpdateRequest", response: "UpdateResult" },
      delete: { request: "DeleteRequest", response: "DeleteResult" },
      get: { request: "GetRequest", response: "GetResult" },
      search: { request: "SearchRequest", response: "SearchResult" },
      analyze: { request: "AnalyzeRequest", response: "AnalyzeResult" },
      describe: { request: "DescribeRequest", response: "DescribeResult" },
      inspect_query: { request: "InspectQueryRequest", response: "InspectQueryResult" }
    }.transform_values(&:freeze).freeze

    module_function

    def request_envelope(operation, payload)
      spec = operation_spec(operation)
      raise ArgumentError, "Search Kernel request payload must be an object" unless payload.is_a?(Hash)

      {
        "protocol_version" => VERSION,
        "payload_type" => wire_type(spec.fetch(:request)),
        "payload" => payload
      }
    end

    def expect_response(operation, response)
      spec = operation_spec(operation)
      raise ProtocolError, "Search Kernel response envelope must be an object" unless response.is_a?(Hash)

      version = response["protocol_version"]
      unless version == VERSION
        raise ProtocolError, "unsupported Search Kernel response protocol version #{version.inspect}"
      end

      payload_type = response["payload_type"]
      raise ProtocolError, "Search Kernel response payload_type must be a string" unless payload_type.is_a?(String)

      payload = response["payload"]
      raise ProtocolError, "Search Kernel response is missing its union payload" unless payload.is_a?(Hash)

      raise SearchKernelError.from_payload(payload) if payload_type == ERROR_TYPE

      expected = wire_type(spec.fetch(:response))
      unless payload_type == expected
        raise ProtocolError, "unexpected Search Kernel response type #{payload_type.inspect} (expected #{expected})"
      end

      payload
    end

    def operation_spec(operation)
      key = operation.respond_to?(:to_sym) ? operation.to_sym : operation
      OPERATIONS.fetch(key) do
        raise ArgumentError, "unknown Search Kernel operation #{operation.inspect}"
      end
    end

    def wire_type(type_name)
      "#{TYPE_PREFIX}#{type_name}"
    end
  end
end
