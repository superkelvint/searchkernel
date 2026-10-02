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
      @index_generation = nil
      @bind_on_use = false
      @closed = false
    end

    def require_open_before_operations
      @bind_on_use = true
    end

    def bind_generation(generation)
      unless generation.is_a?(String) && !generation.empty?
        raise ProtocolError, "lifecycle result is missing canonical index generation"
      end
      @index_generation = generation
    end

    def execute(operation, payload)
      raise TransportError, "Search Kernel client is closed" if @closed
      if @bind_on_use && @index_generation.nil? && !%i[create_index open_index].include?(operation.to_sym)
        open_index({})
      end

      lifecycle = %i[create_index open_index delete_index].include?(operation.to_sym)
      response = @transport.execute(
        Protocol.request_envelope(
          operation,
          payload,
          target: @target,
          expected_index_generation: lifecycle ? nil : @index_generation
        )
      )
      result = Protocol.expect_response(operation, response)
      if %i[create_index open_index].include?(operation.to_sym)
        generation = result["generation"]
        unless generation.is_a?(String) && !generation.empty?
          raise ProtocolError, "#{operation} response is missing canonical index generation"
        end
        @index_generation = generation
      end
      result
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
