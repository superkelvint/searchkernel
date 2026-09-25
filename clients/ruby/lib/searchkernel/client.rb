# frozen_string_literal: true

require_relative "protocol"
require_relative "results"

module SearchKernel
  class Client
    attr_reader :transport

    def initialize(transport)
      unless transport.respond_to?(:execute)
        raise ArgumentError, "Search Kernel transport must respond to execute"
      end

      @transport = transport
    end

    def execute(operation, payload)
      response = @transport.execute(Protocol.request_envelope(operation, payload))
      Protocol.expect_response(operation, response)
    end

    def execute_raw(envelope)
      @transport.execute(envelope)
    end

    def search_results(payload)
      Results::SearchResults.new(search(payload))
    end

    Protocol::OPERATIONS.each_key do |operation|
      define_method(operation) do |payload|
        execute(operation, payload)
      end
    end
  end
end
