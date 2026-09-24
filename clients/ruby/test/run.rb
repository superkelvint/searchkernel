# frozen_string_literal: true

require "json"
require "socket"

$LOAD_PATH.unshift(File.expand_path("../lib", __dir__))
require "searchkernel"

module TestSupport
  module_function

  def assert(condition, message)
    raise message unless condition
  end

  def same(expected, actual, message)
    return if expected == actual

    raise "#{message}\nexpected: #{expected.inspect}\nactual:   #{actual.inspect}"
  end

  def raises(error_class, message)
    yield
  rescue error_class => error
    return error
  rescue StandardError => error
    raise "#{message}: expected #{error_class}, got #{error.class}: #{error.message}"
  else
    raise "#{message}: expected #{error_class}, nothing was raised"
  end

  def with_http_server(status: 200, body:, content_type: "application/json", content_length: body.bytesize)
    server = TCPServer.new("127.0.0.1", 0)
    port = server.addr[1]
    state = {}
    thread_error = nil

    thread = Thread.new do
      socket = nil
      begin
        socket = server.accept
        state[:request_line] = socket.gets&.strip
        headers = {}
        while (line = socket.gets)
          break if line == "\r\n"

          key, value = line.split(":", 2)
          headers[key.downcase] = value.to_s.strip
        end
        state[:headers] = headers
        length = Integer(headers.fetch("content-length", "0"), 10)
        state[:body] = socket.read(length)

        reason = status == 200 ? "OK" : "Error"
        socket.write("HTTP/1.1 #{status} #{reason}\r\n")
        socket.write("Content-Type: #{content_type}\r\n")
        socket.write("Content-Length: #{content_length}\r\n")
        socket.write("Connection: close\r\n\r\n")
        socket.write(body)
      rescue StandardError => error
        thread_error = error
      ensure
        socket&.close
      end
    end

    yield "http://127.0.0.1:#{port}", state
  ensure
    server&.close
    if thread
      thread.join(5)
      raise "test HTTP server did not terminate" if thread.alive?
    end
    raise thread_error if thread_error
  end

  def with_raw_http_server(raw_response)
    server = TCPServer.new("127.0.0.1", 0)
    port = server.addr[1]
    thread_error = nil

    thread = Thread.new do
      socket = nil
      begin
        socket = server.accept
        headers = {}
        socket.gets
        while (line = socket.gets)
          break if line == "\r\n"

          key, value = line.split(":", 2)
          headers[key.downcase] = value.to_s.strip
        end
        length = Integer(headers.fetch("content-length", "0"), 10)
        socket.read(length)
        socket.write(raw_response)
      rescue Errno::EPIPE, Errno::ECONNRESET
        # The hardened client may close immediately after detecting malformed HTTP.
      rescue StandardError => error
        thread_error = error
      ensure
        socket&.close
      end
    end

    yield "http://127.0.0.1:#{port}"
  ensure
    server&.close
    if thread
      thread.join(5)
      raise "test raw HTTP server did not terminate" if thread.alive?
    end
    raise thread_error if thread_error
  end
end

include TestSupport

require "rubygems"
gemspec = Gem::Specification.load(File.expand_path("../searchkernel.gemspec", __dir__))
assert(!gemspec.nil?, "Ruby gemspec must load")
same(["Apache-2.0"], gemspec.licenses, "Ruby gemspec license metadata")

expected_operations = {
  insert: %w[InsertRequest InsertResult],
  upsert: %w[UpsertRequest UpsertResult],
  patch: %w[PatchRequest PatchResult],
  update: %w[UpdateRequest UpdateResult],
  delete: %w[DeleteRequest DeleteResult],
  get: %w[GetRequest GetResult],
  search: %w[SearchRequest SearchResult],
  analyze: %w[AnalyzeRequest AnalyzeResult],
  describe: %w[DescribeRequest DescribeResult],
  inspect_query: %w[InspectQueryRequest InspectQueryResult]
}

same(expected_operations.keys.sort, SearchKernel::Protocol::OPERATIONS.keys.sort, "all portable operations are mapped")
expected_operations.each do |operation, (request_type, response_type)|
  payload = { "marker" => operation.to_s }
  envelope = SearchKernel::Protocol.request_envelope(operation, payload)
  same(2, envelope["protocol_version"], "#{operation} request protocol version")
  same("SearchKernel_V2_#{request_type}", envelope["payload_type"], "#{operation} request discriminator")
  same(payload, envelope["payload"], "#{operation} request payload")

  result = SearchKernel::Protocol.expect_response(
    operation,
    {
      "protocol_version" => 2,
      "payload_type" => "SearchKernel_V2_#{response_type}",
      "payload" => { "ok" => operation.to_s }
    }
  )
  same({ "ok" => operation.to_s }, result, "#{operation} response discriminator")
end

raises(ArgumentError, "unknown operation rejects") do
  SearchKernel::Protocol.request_envelope(:unknown, {})
end
raises(ArgumentError, "non-object request payload rejects") do
  SearchKernel::Protocol.request_envelope(:search, [])
end
raises(SearchKernel::ProtocolError, "non-object response rejects") do
  SearchKernel::Protocol.expect_response(:search, [])
end
raises(SearchKernel::ProtocolError, "future protocol rejects") do
  SearchKernel::Protocol.expect_response(
    :search,
    { "protocol_version" => 3, "payload_type" => "SearchKernel_V2_SearchResult", "payload" => {} }
  )
end
raises(SearchKernel::ProtocolError, "missing union payload rejects") do
  SearchKernel::Protocol.expect_response(
    :search,
    { "protocol_version" => 2, "payload_type" => "SearchKernel_V2_SearchResult" }
  )
end
raises(SearchKernel::ProtocolError, "wrong response union rejects") do
  SearchKernel::Protocol.expect_response(
    :search,
    { "protocol_version" => 2, "payload_type" => "SearchKernel_V2_GetResult", "payload" => {} }
  )
end

portable_error = raises(SearchKernel::SearchKernelError, "canonical Error is portable error") do
  SearchKernel::Protocol.expect_response(
    :search,
    {
      "protocol_version" => 2,
      "payload_type" => "SearchKernel_V2_Error",
      "payload" => {
        "code" => "InvalidArgument",
        "message" => "bad query",
        "details" => [{ "code" => "unknown_field", "path" => "query", "message" => "bad" }],
        "retryable" => false
      }
    }
  )
end
same("InvalidArgument", portable_error.code, "portable error code")
same(false, portable_error.retryable, "portable error retryable")
same("unknown_field", portable_error.details.first["code"], "portable error details")
default_error = raises(SearchKernel::SearchKernelError, "FlatBuffers default Error fields decode canonically") do
  SearchKernel::Protocol.expect_response(
    :search,
    {
      "protocol_version" => 2,
      "payload_type" => "SearchKernel_V2_Error",
      "payload" => { "message" => "default fields" }
    }
  )
end
same("Unspecified", default_error.code, "omitted default error code")
same(false, default_error.retryable, "omitted default retryable")

raises(SearchKernel::ProtocolError, "malformed portable Error fails closed") do
  SearchKernel::Protocol.expect_response(
    :search,
    {
      "protocol_version" => 2,
      "payload_type" => "SearchKernel_V2_Error",
      "payload" => { "message" => "bad retryable", "retryable" => "false" }
    }
  )
end

[
  { "code" => "FutureCode", "message" => "bad code" },
  {
    "code" => "InvalidArgument",
    "message" => "bad detail",
    "details" => [{ "code" => "unknown_field" }]
  },
  {
    "code" => "InvalidArgument",
    "message" => "bad path",
    "details" => [{ "code" => "unknown_field", "message" => "bad", "path" => 7 }]
  },
  {
    "code" => "InvalidArgument",
    "message" => "bad metadata",
    "details" => [{ "code" => "unknown_field", "message" => "bad", "metadata" => "wrong" }]
  },
  {
    "code" => "InvalidArgument",
    "message" => "duplicate metadata",
    "details" => [{
      "code" => "unknown_field",
      "message" => "bad",
      "metadata" => [
        { "key" => "field", "value" => "one" },
        { "key" => "field", "value" => "two" }
      ]
    }]
  }
].each do |payload|
  raises(SearchKernel::ProtocolError, "malformed canonical Error detail fails closed") do
    SearchKernel::Protocol.expect_response(
      :search,
      {
        "protocol_version" => 2,
        "payload_type" => "SearchKernel_V2_Error",
        "payload" => payload
      }
    )
  end
end

class RecordingTransport
  attr_reader :last_request

  def execute(request)
    @last_request = request
    {
      "protocol_version" => 2,
      "payload_type" => "SearchKernel_V2_SearchResult",
      "payload" => { "root" => nil }
    }
  end
end

recording = RecordingTransport.new
client = SearchKernel::Client.new(recording)
same({ "root" => nil }, client.search({ "query" => { "source_type" => "MatchAllQuery" } }), "client returns operation payload")
same("SearchKernel_V2_SearchRequest", recording.last_request["payload_type"], "client uses centralized operation mapping")
raw = { "protocol_version" => 2, "payload_type" => "Custom", "payload" => {} }
same(recording.execute(raw), client.execute_raw(raw), "raw envelope escape hatch delegates unchanged")

success_response = JSON.generate(
  "protocol_version" => 2,
  "payload_type" => "SearchKernel_V2_SearchResult",
  "payload" => { "total_hits" => { "value" => "0", "relation" => "Exact" } }
)
with_http_server(body: success_response) do |base_url, state|
  transport = SearchKernel::HttpTransport.new(base_url, timeout: 1)
  response = transport.execute(SearchKernel::Protocol.request_envelope(:search, { "query" => {} }))
  same(2, response["protocol_version"], "HTTP response decoded")
  same("POST /v1/execute HTTP/1.1", state[:request_line], "HTTP execute path")
  same("application/json", state[:headers]["content-type"], "HTTP request content type")
  same("application/json", state[:headers]["accept"], "HTTP request accept")
  request_json = JSON.parse(state[:body])
  same("SearchKernel_V2_SearchRequest", request_json["payload_type"], "HTTP body preserves canonical envelope")
end

with_http_server(status: 503, body: "backend unavailable", content_type: "text/plain") do |base_url, _state|
  error = raises(SearchKernel::TransportError, "non-2xx response is transport error") do
    SearchKernel::HttpTransport.new(base_url, timeout: 1).execute({})
  end
  same(503, error.status, "transport error retains HTTP status")
end

with_http_server(body: "not-json") do |base_url, _state|
  raises(SearchKernel::ProtocolError, "malformed JSON response is protocol error") do
    SearchKernel::HttpTransport.new(base_url, timeout: 1).execute({})
  end
end

with_http_server(body: "[]") do |base_url, _state|
  raises(SearchKernel::ProtocolError, "non-object JSON response is protocol error") do
    SearchKernel::HttpTransport.new(base_url, timeout: 1).execute({})
  end
end

raises(SearchKernel::TransportError, "request message limit enforced before network") do
  SearchKernel::HttpTransport.new("http://127.0.0.1:1", max_message_bytes: 16).execute("payload" => "x" * 32)
end

with_http_server(body: JSON.generate("payload" => "x" * 64)) do |base_url, _state|
  raises(SearchKernel::TransportError, "response message limit enforced") do
    SearchKernel::HttpTransport.new(base_url, timeout: 1, max_message_bytes: 32).execute({})
  end
end

raises(SearchKernel::ProtocolError, "non-finite JSON is rejected locally") do
  SearchKernel::HttpTransport.new("http://127.0.0.1:1", timeout: 1).execute("value" => Float::NAN)
end

closed = TCPServer.new("127.0.0.1", 0)
closed_port = closed.addr[1]
closed.close
raises(SearchKernel::TransportError, "connection failure is transport error") do
  SearchKernel::HttpTransport.new("http://127.0.0.1:#{closed_port}", timeout: 0.1).execute({})
end

raises(ArgumentError, "invalid HTTP scheme rejects") { SearchKernel::HttpTransport.new("ftp://example.test") }
raises(ArgumentError, "query in base URL rejects") { SearchKernel::HttpTransport.new("http://example.test?x=1") }
raises(ArgumentError, "userinfo in base URL rejects") { SearchKernel::HttpTransport.new("http://user:pass@example.test") }
raises(ArgumentError, "fractional message limit rejects") do
  SearchKernel::HttpTransport.new("http://example.test", max_message_bytes: 1.5)
end

with_http_server(body: "", content_length: 1024) do |base_url, _state|
  error = raises(SearchKernel::TransportError, "declared oversized response fails before body buffering") do
    SearchKernel::HttpTransport.new(base_url, timeout: 1, max_message_bytes: 32).execute({})
  end
  assert(
    error.message.include?("response exceeds 32-byte transport limit"),
    "oversized response retains the transport-limit error domain"
  )
end

with_raw_http_server("this is not HTTP\r\n") do |base_url|
  error = raises(SearchKernel::TransportError, "malformed HTTP response is transport error") do
    SearchKernel::HttpTransport.new(base_url, timeout: 1).execute({})
  end
  assert(
    error.message.start_with?("Search Kernel HTTP transport failed:"),
    "malformed HTTP response is wrapped in the transport error domain"
  )
end

with_http_server(status: 503, body: "bad\r\ninjected\x00\xff".b, content_type: "text/plain") do |base_url, _state|
  error = raises(SearchKernel::TransportError, "HTTP error body is safely bounded for diagnostics") do
    SearchKernel::HttpTransport.new(base_url, timeout: 1).execute({})
  end
  assert(!error.message.include?("\n"), "transport error diagnostic must not contain injected newlines")
  assert(error.message.valid_encoding?, "transport error diagnostic must remain valid UTF-8")
end

puts "Ruby protocol/transport tests PASS"
