# frozen_string_literal: true

require "fileutils"
require "json"
require "rubygems"
require "socket"
require "tmpdir"

Gem.use_paths(ENV.fetch("GEM_HOME"), [ENV.fetch("GEM_HOME")])
Gem::Specification.reset
gem "searchkernel"
require "searchkernel"

def check(condition, message)
  raise message unless condition
end

def same(expected, actual, message)
  return if expected == actual

  raise "#{message}\nexpected: #{expected.inspect}\nactual:   #{actual.inspect}"
end

def daemon_diagnostic(path)
  return "" unless File.file?(path)

  text = File.binread(path)
  text = text.byteslice(-4096, 4096) if text.bytesize > 4096
  text.to_s.force_encoding(Encoding::UTF_8).scrub.gsub(/[\x00-\x1f\x7f]+/, " ").strip
rescue StandardError
  ""
end

def wait_until_ready(pid, port, stderr_path)
  deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 60.0

  loop do
    begin
      reaped = Process.waitpid(pid, Process::WNOHANG)
      if reaped
        detail = daemon_diagnostic(stderr_path)
        suffix = detail.empty? ? "" : ": #{detail}"
        raise "externally owned searchkerneld exited during startup#{suffix}"
      end
    rescue Errno::ECHILD
      detail = daemon_diagnostic(stderr_path)
      suffix = detail.empty? ? "" : ": #{detail}"
      raise "externally owned searchkerneld disappeared during startup#{suffix}"
    end

    begin
      socket = TCPSocket.new("127.0.0.1", port)
      socket.close
      return
    rescue Errno::ECONNREFUSED, Errno::EHOSTUNREACH, Errno::ENOENT
      nil
    end

    if Process.clock_gettime(Process::CLOCK_MONOTONIC) >= deadline
      detail = daemon_diagnostic(stderr_path)
      suffix = detail.empty? ? "" : ": #{detail}"
      raise "externally owned searchkerneld did not open its HTTP listener#{suffix}"
    end

    sleep 0.05
  end
end

def process_alive?(pid)
  Process.kill(0, pid)
  true
rescue Errno::ESRCH
  false
rescue Errno::EPERM
  true
end

def stop_daemon(pid)
  return if pid.nil?

  begin
    Process.kill("INT", -pid)
  rescue Errno::ESRCH
    nil
  end

  deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 15.0
  loop do
    begin
      return if Process.waitpid(pid, Process::WNOHANG)
    rescue Errno::ECHILD
      return
    end
    break if Process.clock_gettime(Process::CLOCK_MONOTONIC) >= deadline
    sleep 0.05
  end

  begin
    Process.kill("KILL", -pid)
  rescue Errno::ESRCH
    nil
  end
  begin
    Process.waitpid(pid)
  rescue Errno::ECHILD
    nil
  end
end

repo_lib = File.expand_path("../lib", __dir__)
expanded_load_path = $LOAD_PATH.filter_map do |path|
  File.expand_path(path)
rescue StandardError
  nil
end
check(!expanded_load_path.include?(repo_lib), "package-mode E2E must not add clients/ruby/lib to $LOAD_PATH")

gem_spec = Gem.loaded_specs.fetch("searchkernel")
loaded_entrypoint = $LOADED_FEATURES.find { |path| File.basename(path) == "searchkernel.rb" }
check(!loaded_entrypoint.nil?, "installed searchkernel gem entrypoint was not loaded")
gem_root = File.expand_path(gem_spec.full_gem_path)
loaded_path = File.expand_path(loaded_entrypoint)
check(
  loaded_path == File.join(gem_root, "lib", "searchkernel.rb"),
  "searchkernel loaded outside the installed gem: #{loaded_path}"
)
check(
  !loaded_path.start_with?("#{repo_lib}#{File::SEPARATOR}"),
  "searchkernel unexpectedly loaded from the repository Ruby lib directory"
)

host = ENV.fetch("SEARCHKERNEL_LOCAL_HOST")
check(File.file?(host) && File.executable?(host), "SEARCHKERNEL_LOCAL_HOST is not an executable daemon: #{host}")

Dir.mktmpdir("searchkernel-ruby-package-daemon-") do |work|
  data_root = File.join(work, "data-root")
  socket_path = File.join(work, "searchkerneld.sock")
  stderr_path = File.join(work, "searchkerneld.stderr")

  fields = {
    "title" => SearchKernel::Text.new(stored: true, searchable: true),
    "inventory" => SearchKernel::IntegerField.new(stored: true, filterable: true, sortable: true)
  }
  probe = TCPServer.new("127.0.0.1", 0)
  port = probe.addr[1]
  probe.close

  pid = Process.spawn(
    host,
    "--data-root", data_root,
    "--listen", "unix://#{socket_path}",
    "--web-listen", "127.0.0.1:#{port}",
    pgroup: true,
    out: File::NULL,
    err: stderr_path
  )

  begin
    wait_until_ready(pid, port, stderr_path)
    base_url = "http://127.0.0.1:#{port}"

    begin
      SearchKernel::Index.open(base_url, target: "Products")
      raise "domain-invalid target unexpectedly opened"
    rescue SearchKernel::SearchKernelError => error
      same("InvalidArgument", error.code, "invalid target reaches canonical portable validation")
      same("invalid_index_target", error.details.first.fetch("code"), "invalid target canonical detail code")
      same("target.name", error.details.first.fetch("path"), "invalid target canonical semantic path")
    end

    index = SearchKernel::Index.create(
      base_url,
      target: "products",
      fields: fields
    )
    sibling_index = SearchKernel::Index.create(
      base_url,
      target: "products-b",
      fields: fields
    )
    begin
      same(
        "sibling-1",
        sibling_index.upsert("sibling-1", {"title" => "Sibling Ruby Client", "inventory" => 7}),
        "sibling lifecycle target mutation id"
      )
    ensure
      sibling_index.close
    end

    begin
      SearchKernel::Index.create(base_url, target: "products", fields: fields)
      raise "duplicate logical index unexpectedly created"
    rescue SearchKernel::SearchKernelError => error
      same("AlreadyExists", error.code, "duplicate create has canonical AlreadyExists semantics")
      same("index_already_exists", error.details.first.fetch("code"), "duplicate create canonical detail")
    end
    begin
      described = index.describe
      same(%w[inventory title], described.fetch("fields").map { |field| field.fetch("name") }.sort,
           "installed gem opens externally owned daemon index")

      same("package-1", index.upsert("package-1", {"title" => "Packaged Ruby Client", "inventory" => 1}),
           "installed gem mutation id")
      same(["package-1"], index.search("packaged").limit(10).execute.hits.map(&:id),
           "installed gem search through external daemon")

      index.patch("package-1").set("inventory", 2).set("title", "Updated Package Client").execute
      same(2, index.get("package-1").fetch("inventory"), "installed gem patch through external daemon")
      same(["package-1"], index.search("updated").limit(10).execute.hits.map(&:id),
           "updated document is searchable through external daemon")

      sibling = SearchKernel::Index.open(base_url, target: "products")
      index.close
      same(2, sibling.get("package-1").fetch("inventory"),
           "closing one package handle leaves an already-open sibling usable")
      same(["package-1"], sibling.search("updated").limit(10).execute.hits.map(&:id),
           "sibling handle can search after the first handle closes")
      sibling.close
    ensure
      index.close
    end

    check(process_alive?(pid), "Index#close terminated an externally owned daemon")

    begin
      index.get("package-1")
      raise "closed connected index unexpectedly accepted an operation"
    rescue SearchKernel::TransportError => error
      check(error.message.include?("Search Kernel client is closed"),
            "closed-client diagnostic lost its lifecycle error: #{error.message}")
    end

    reopened = SearchKernel::Index.open(base_url, target: "products")
    begin
      same(2, reopened.get("package-1").fetch("inventory"),
           "external daemon remains usable after first client close")
      same(["package-1"], reopened.search("updated").limit(10).execute.hits.map(&:id),
           "reconnected installed gem can search the same external daemon")
    ensure
      reopened.close
    end
    check(process_alive?(pid), "second client close terminated an externally owned daemon")

    check(SearchKernel::Index.delete(base_url, target: "products"), "delete logical index")
    begin
      SearchKernel::Index.open(base_url, target: "products")
      raise "deleted logical index unexpectedly reopened"
    rescue SearchKernel::SearchKernelError => error
      same("NotFound", error.code, "deleted logical index returns canonical NotFound")
      same("index_not_found", error.details.first.fetch("code"), "deleted logical index canonical detail")
    end
    begin
      SearchKernel::Index.delete(base_url, target: "products")
      raise "missing logical index unexpectedly deleted"
    rescue SearchKernel::SearchKernelError => error
      same("NotFound", error.code, "missing delete has canonical NotFound semantics")
      same("index_not_found", error.details.first.fetch("code"), "missing delete canonical detail")
    end

    surviving_sibling = SearchKernel::Index.open(base_url, target: "products-b")
    begin
      same(
        7,
        surviving_sibling.get("sibling-1").fetch("inventory"),
        "deleting one lifecycle target leaves sibling target usable"
      )
    ensure
      surviving_sibling.close
    end
    check(
      SearchKernel::Index.delete(base_url, target: "products-b"),
      "delete surviving sibling logical index"
    )

    stop_daemon(pid)
    pid = nil

    begin
      SearchKernel::Index.connect(base_url, target: "products", timeout: 0.25)
      raise "connecting to a stopped external daemon unexpectedly succeeded"
    rescue SearchKernel::TransportError => error
      check(
        error.message.include?("Search Kernel HTTP transport failed:"),
        "unavailable-daemon error lost the transport diagnostic domain: #{error.message}"
      )
      check(
        error.message.include?("#{base_url}/v1/execute"),
        "unavailable-daemon diagnostic does not identify the target endpoint: #{error.message}"
      )
    end
  ensure
    stop_daemon(pid)
  end
end

# SKC: query.text_weak_and/basic kinds=ergonomic,e2e runtime=http
# SKC: update.document_crud/basic kinds=ergonomic,e2e runtime=http

puts "Ruby installed-gem external-daemon E2E PASS"
