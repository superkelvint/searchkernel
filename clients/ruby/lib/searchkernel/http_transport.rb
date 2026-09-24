# frozen_string_literal: true

require "json"
require "net/http"
require "openssl"
require "uri"

require_relative "errors"

module SearchKernel
  class HttpTransport
    EXECUTE_PATH = "/v1/execute"
    DEFAULT_TIMEOUT_SECONDS = 30.0
    DEFAULT_MAX_MESSAGE_BYTES = 256 * 1024 * 1024
    ERROR_BODY_LIMIT = 1024

    def initialize(base_url, timeout: DEFAULT_TIMEOUT_SECONDS, max_message_bytes: DEFAULT_MAX_MESSAGE_BYTES)
      @execute_uri = build_execute_uri(base_url)
      @timeout = normalize_timeout(timeout)
      @max_message_bytes = normalize_message_limit(max_message_bytes)
    end

    def execute(envelope)
      raise ProtocolError, "Search Kernel request envelope must be an object" unless envelope.is_a?(Hash)

      body = encode_json(envelope)
      enforce_size!(body.bytesize, "request")

      response, response_body = perform_request(body)
      status = response.code.to_i
      unless status.between?(200, 299)
        snippet = sanitize_error_snippet(response_body)
        suffix = snippet.empty? ? "" : ": #{snippet}"
        raise TransportError.new("Search Kernel HTTP request failed with status #{status}#{suffix}", status: status)
      end

      decode_json(response_body)
    end

    private

    def build_execute_uri(base_url)
      unless base_url.is_a?(String) && !base_url.empty?
        raise ArgumentError, "Search Kernel HTTP base URL must be a non-empty string"
      end

      uri = URI.parse(base_url)
      unless %w[http https].include?(uri.scheme) && uri.host
        raise ArgumentError, "Search Kernel HTTP base URL must use http or https and include a host"
      end
      if uri.userinfo
        raise ArgumentError, "Search Kernel HTTP base URL must not contain userinfo"
      end
      if uri.query || uri.fragment
        raise ArgumentError, "Search Kernel HTTP base URL must not contain a query or fragment"
      end
      unless uri.port.between?(1, 65_535)
        raise ArgumentError, "Search Kernel HTTP base URL port must be between 1 and 65535"
      end

      base_path = uri.path.to_s.sub(%r{/+\z}, "")
      uri.path = "#{base_path}#{EXECUTE_PATH}"
      uri.path = EXECUTE_PATH if uri.path.empty?
      uri
    rescue URI::Error => error
      raise ArgumentError, "invalid Search Kernel HTTP base URL: #{error.message}"
    end

    def normalize_timeout(timeout)
      value = Float(timeout)
      raise ArgumentError, "Search Kernel HTTP timeout must be positive and finite" unless value.positive? && value.finite?

      value
    rescue TypeError, ArgumentError
      raise ArgumentError, "Search Kernel HTTP timeout must be positive and finite"
    end

    def normalize_message_limit(limit)
      unless limit.is_a?(Integer) && limit.positive?
        raise ArgumentError, "Search Kernel HTTP message limit must be a positive integer"
      end

      limit
    end

    def encode_json(envelope)
      JSON.generate(envelope)
    rescue JSON::GeneratorError => error
      raise ProtocolError, "encode Search Kernel request JSON: #{error.message}"
    end

    def decode_json(body)
      decoded = JSON.parse(body)
      raise ProtocolError, "Search Kernel response JSON must be an object" unless decoded.is_a?(Hash)

      decoded
    rescue JSON::ParserError => error
      raise ProtocolError, "decode Search Kernel response JSON: #{error.message}"
    end

    def enforce_size!(size, direction)
      return if size <= @max_message_bytes

      raise TransportError, "Search Kernel HTTP #{direction} exceeds #{@max_message_bytes}-byte transport limit"
    end

    def perform_request(body)
      request = Net::HTTP::Post.new(@execute_uri.request_uri)
      request["Content-Type"] = "application/json"
      request["Accept"] = "application/json"
      request.body = body

      http = Net::HTTP.new(@execute_uri.host, @execute_uri.port, nil)
      http.use_ssl = @execute_uri.scheme == "https"
      http.open_timeout = @timeout
      http.read_timeout = @timeout
      http.write_timeout = @timeout if http.respond_to?(:write_timeout=)

      response = nil
      response_body = nil
      http.request(request) do |current_response|
        response = current_response
        response_body = read_response_body(current_response)
      end
      [response, response_body]
    rescue Net::OpenTimeout, Net::ReadTimeout, Net::WriteTimeout, Net::HTTPBadResponse,
           Net::HTTPHeaderSyntaxError, Net::ProtocolError, OpenSSL::SSL::SSLError,
           IOError, EOFError, SystemCallError, SocketError => error
      raise TransportError, "Search Kernel HTTP transport failed: #{error.message}"
    end

    def read_response_body(response)
      content_length = response.content_length
      enforce_size!(content_length, "response") if content_length

      success = response.code.to_i.between?(200, 299)
      capture_limit = success ? @max_message_bytes : [@max_message_bytes, ERROR_BODY_LIMIT].min
      body = +"".b
      total = 0

      response.read_body do |chunk|
        total += chunk.bytesize
        enforce_size!(total, "response")

        remaining = capture_limit - body.bytesize
        body << chunk.byteslice(0, remaining) if remaining.positive?
      end
      body
    end

    def sanitize_error_snippet(body)
      text = body.dup.force_encoding(Encoding::UTF_8).scrub("\uFFFD")
      text.gsub(/[\u0000-\u001F\u007F]/, " ").strip
    end
  end
end
