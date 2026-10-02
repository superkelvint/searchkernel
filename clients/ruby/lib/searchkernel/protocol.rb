# frozen_string_literal: true

require_relative "errors"

module SearchKernel
  module Protocol
    VERSION = 1
    TYPE_PREFIX = "SearchKernel_Wire_"
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
      inspect_query: { request: "InspectQueryRequest", response: "InspectQueryResult" },
      create_index: { request: "CreateIndexRequest", response: "CreateIndexResult" },
      open_index: { request: "OpenIndexRequest", response: "OpenIndexResult" },
      delete_index: { request: "DeleteIndexRequest", response: "DeleteIndexResult" },
      list_indexes: { request: "ListIndexesRequest", response: "ListIndexesResult" }
    }.transform_values(&:freeze).freeze

    module_function

    def request_envelope(operation, payload, target: nil, expected_index_generation: nil)
      spec = operation_spec(operation)
      raise ArgumentError, "Search Kernel request payload must be an object" unless payload.is_a?(Hash)

      envelope = {
        "protocol_version" => VERSION,
        "payload_type" => wire_type(spec.fetch(:request)),
        "payload" => payload
      }
      unless target.nil?
        raise ArgumentError, "Search Kernel target must be a string" unless target.is_a?(String)

        envelope["target"] = { "name" => target }
        unless expected_index_generation.nil?
          unless expected_index_generation.is_a?(String) && !expected_index_generation.empty?
            raise ArgumentError, "Search Kernel expected index generation must be a non-empty string"
          end
          envelope["target"]["generation"] = expected_index_generation
        end
      end
      envelope
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
