# frozen_string_literal: true

require_relative "protocol"
require_relative "results"

module SearchKernel
  class Client
    attr_reader :transport, :target

    def initialize(transport, target: nil)
      unless transport.respond_to?(:execute)
        raise ArgumentError, "Search Kernel transport must respond to execute"
      end

      @transport = transport
      @target = target
      @expected_generation = nil
      @bind_on_use = false
      @closed = false
    end

    def require_open_before_operations
      @bind_on_use = true
    end

    def bind_generation(generation)
      unless generation.is_a?(String) && !generation.empty?
        raise ProtocolError, "lifecycle result is missing canonical target generation"
      end
      @expected_generation = generation
    end

    def execute(operation, payload)
      raise TransportError, "Search Kernel client is closed" if @closed
      if @bind_on_use && @expected_generation.nil? &&
         !%i[create_index open_index].include?(operation.to_sym)
        bind_generation(open_index({})["generation"])
      end
      response = @transport.execute(Protocol.request_envelope(
        operation, payload, target: @target, expected_generation: @expected_generation
      ))
      Protocol.expect_response(operation, response)
    end

    def execute_raw(envelope)
      raise TransportError, "Search Kernel client is closed" if @closed
      @transport.execute(envelope)
    end

    def search_results(payload)
      Results::SearchResults.new(search(payload))
    end

    def close
      return nil if @closed
      @closed = true
      @transport.close if @transport.respond_to?(:close)
      nil
    end

    Protocol::OPERATIONS.each_key do |operation|
      define_method(operation) do |payload|
        execute(operation, payload)
      end
    end
  end
end
