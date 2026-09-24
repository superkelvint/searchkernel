# frozen_string_literal: true

module SearchKernel
  class ClientError < StandardError
  end

  class TransportError < ClientError
    attr_reader :status

    def initialize(message, status: nil)
      @status = status
      super(message)
    end
  end

  class ProtocolError < ClientError
  end

  class SearchKernelError < ClientError
    ERROR_CODES = %w[
      Unspecified
      InvalidArgument
      NotFound
      AlreadyExists
      FailedPrecondition
      Conflict
      Unsupported
      ResourceExhausted
      Internal
      Unavailable
      DeadlineExceeded
    ].freeze

    attr_reader :code, :details, :retryable

    def initialize(code:, message:, details:, retryable:)
      @code = code
      @details = details.freeze
      @retryable = retryable
      super(message)
    end

    def self.from_payload(payload)
      unless payload.is_a?(Hash)
        raise ProtocolError, "Search Kernel Error payload must be an object"
      end

      code = payload.fetch("code", "Unspecified")
      message = payload["message"]
      retryable = payload.fetch("retryable", false)
      details = payload.fetch("details", [])

      unless code.is_a?(String) && ERROR_CODES.include?(code)
        raise ProtocolError, "Search Kernel Error code is not a supported canonical value"
      end
      unless message.is_a?(String)
        raise ProtocolError, "Search Kernel Error message must be a string"
      end
      unless retryable == true || retryable == false
        raise ProtocolError, "Search Kernel Error retryable must be a boolean"
      end
      unless details.is_a?(Array)
        raise ProtocolError, "Search Kernel Error details must be an array"
      end

      decoded_details = details.map { |detail| validate_detail(detail) }
      new(code: code, message: message, details: decoded_details, retryable: retryable)
    end

    def self.validate_detail(detail)
      unless detail.is_a?(Hash)
        raise ProtocolError, "Search Kernel Error detail must be an object"
      end

      code = detail["code"]
      message = detail["message"]
      path = detail["path"] if detail.key?("path")
      metadata = detail.fetch("metadata", [])

      unless code.is_a?(String) && message.is_a?(String)
        raise ProtocolError, "Search Kernel Error detail code and message must be strings"
      end
      if detail.key?("path") && !path.nil? && !path.is_a?(String)
        raise ProtocolError, "Search Kernel Error detail path must be a string when present"
      end
      unless metadata.is_a?(Array)
        raise ProtocolError, "Search Kernel Error detail metadata must be an array"
      end

      seen = {}
      copied_metadata = metadata.map do |entry|
        unless entry.is_a?(Hash) && entry["key"].is_a?(String) && entry["value"].is_a?(String)
          raise ProtocolError,
                "Search Kernel Error detail metadata entries must contain string key/value pairs"
        end

        key = entry["key"]
        if seen.key?(key)
          raise ProtocolError, "duplicate Search Kernel Error detail metadata key #{key.inspect}"
        end

        seen[key] = true
        entry.dup
      end

      copy = detail.dup
      copy["metadata"] = copied_metadata if detail.key?("metadata")
      copy
    end
    private_class_method :validate_detail
  end
end
