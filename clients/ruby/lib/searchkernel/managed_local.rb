# frozen_string_literal: true

require "digest"
require "fileutils"
require "json"
require "open-uri"
require "uri"
require "socket"
require "tempfile"
require "tmpdir"

require_relative "errors"

module SearchKernel
  class ManagedLocalHost
    STARTUP_TIMEOUT = 60.0
    SHUTDOWN_TIMEOUT = 15.0
    KILL_TIMEOUT = 5.0

    attr_reader :base_url

    class << self
      def registry
        @registry ||= {}
      end

      def register(host)
        registry[host.object_id] = host
      end

      def unregister(host)
        registry.delete(host.object_id)
      end

      def close_all
        registry.values.each do |host|
          begin
            host.close
          rescue StandardError
            nil
          end
        end
      end
    end

    at_exit { close_all }

    def initialize(path, definition_json: nil)
      @path = File.expand_path(path.to_s)
      @closed = false
      @pid = nil
      @definition_file = nil
      @tmpdir = Dir.mktmpdir("searchkernel-ruby-")
      @socket_path = File.join(@tmpdir, "searchkerneld.sock")
      @stderr_path = File.join(@tmpdir, "searchkerneld.stderr")
      @stderr_file = File.open(@stderr_path, "wb")
      @definition_file = write_definition(definition_json) unless definition_json.nil?
      start
      self.class.register(self)
    rescue StandardError
      close(force: true)
      raise
    end

    def close(force: false)
      return if @closed

      @closed = true
      self.class.unregister(self)
      stop_process(force: force)
      close_stderr
      cleanup_definition
      cleanup_tmpdir
      nil
    end

    private

    def start
      binary = host_binary
      unless File.file?(binary) && File.executable?(binary)
        raise ClientError, "managed local host binary not found or executable at #{binary}"
      end

      port = free_port
      env = {}
      sdk_lib = ENV["SEARCHKERNEL_SDK_LIB"]
      if sdk_lib && !sdk_lib.empty?
        existing = ENV["LD_LIBRARY_PATH"]
        env["LD_LIBRARY_PATH"] = existing && !existing.empty? ? "#{sdk_lib}:#{existing}" : sdk_lib
      end

      command = [
        binary,
        "--index", @path,
        "--listen", "unix://#{@socket_path}",
        "--web-listen", "127.0.0.1:#{port}"
      ]
      command.concat(["--definition-json", @definition_file.path]) if @definition_file

      @pid = Process.spawn(
        env,
        *command,
        pgroup: true,
        out: File::NULL,
        err: @stderr_file
      )
      wait_until_ready(port)
      @base_url = "http://127.0.0.1:#{port}"
      cleanup_definition
    rescue SystemCallError => error
      raise ClientError, "failed to start managed local host: #{error.message}"
    end

    def host_binary
      configured = ENV["SEARCHKERNEL_LOCAL_HOST"]
      return File.expand_path(configured) if configured && !configured.empty?

      descriptor_path = File.expand_path("runtime.json", __dir__)
      return File.join(resolve_runtime(descriptor_path), "bin", "searchkerneld") if File.file?(descriptor_path)

      root = File.expand_path("../../../..", __dir__)
      target_dir = ENV["CARGO_TARGET_DIR"]
      if target_dir && !target_dir.empty?
        target_dir = File.expand_path(target_dir, root) unless target_dir.start_with?("/")
        candidate = File.join(target_dir, "debug", "searchkerneld")
        return candidate if File.file?(candidate)
      end
      File.join(root, "target", "debug", "searchkerneld")
    end

    def resolve_runtime(descriptor_path)
      descriptor = JSON.parse(File.read(descriptor_path))
      identity_keys = %w[searchkernel_version searchkernel_revision runtime_package_generation platform native_target build_environment native_abi_version vespa_commit]
      material = identity_keys.sort.to_h { |key| [key, descriptor.fetch(key)] }
      derived = "skrt-v1-#{Digest::SHA256.hexdigest(JSON.generate(material))}"
      raise ClientError, "runtime descriptor identity mismatch" unless descriptor["runtime_identity"] == derived
      unless RUBY_PLATFORM.include?("linux") && RUBY_PLATFORM.match?(/x86_64|amd64/) && descriptor["platform"] == "linux-x86_64-glibc" && descriptor["native_target"] == "x86_64-unknown-linux-gnu"
        raise ClientError, "runtime descriptor platform is unsupported on this host"
      end
      artifact_uri = URI.parse(descriptor.fetch("artifact_url"))
      expected_artifact = "searchkernel-runtime-#{descriptor.fetch('runtime_identity')}-#{descriptor.fetch('platform')}.tar.gz"
      unless %w[https file].include?(artifact_uri.scheme) && File.basename(artifact_uri.path) == expected_artifact
        raise ClientError, "runtime descriptor artifact URL is not immutable or supported"
      end
      override = ENV["SEARCHKERNEL_RUNTIME_DIR"]
      return validate_runtime(override, descriptor) if override && !override.empty?
      cache = ENV["SEARCHKERNEL_RUNTIME_CACHE"] || File.join(ENV["XDG_CACHE_HOME"] || File.join(Dir.home, ".cache"), "searchkernel", "runtime")
      final = File.join(cache, "objects", descriptor.fetch("runtime_identity"))
      begin; return validate_runtime(final, descriptor); rescue ClientError; end
      FileUtils.mkdir_p(File.dirname(final)); FileUtils.mkdir_p(File.join(cache, "locks"))
      File.open(File.join(cache, "locks", "#{descriptor.fetch('runtime_identity')}.lock"), "w") do |lock|
        lock.flock(File::LOCK_EX)
        begin; return validate_runtime(final, descriptor); rescue ClientError; FileUtils.rm_rf(final); end
        Dir.mktmpdir("searchkernel-runtime-", cache) do |tmp|
          archive = File.join(tmp, "runtime.tar.gz")
          url = descriptor.fetch("artifact_url")
          if (origin = ENV["SEARCHKERNEL_RUNTIME_ORIGIN"]) && !origin.empty?
            url = origin.sub(%r{/+$}, "") + "/" + File.basename(URI.parse(url).path)
          end
          parsed_url = URI.parse(url)
          if parsed_url.scheme == "file"
            File.binwrite(archive, File.binread(parsed_url.path))
          else
            URI.open(url) { |input| File.binwrite(archive, input.read) }
          end
          raise ClientError, "runtime artifact digest mismatch" unless Digest::SHA256.file(archive).hexdigest == descriptor.fetch("artifact_sha256")
          unpack = File.join(tmp, "unpack"); Dir.mkdir(unpack)
          system("tar", "-xzf", archive, "-C", unpack, "--no-same-owner", "--no-same-permissions", exception: true)
          validate_runtime(unpack, descriptor); File.rename(unpack, final)
        end
      end
      validate_runtime(final, descriptor)
      rescue StandardError => error
      raise error if error.is_a?(ClientError)
      raise ClientError, "managed-local runtime resolution failed: #{error.message}"
    end

    def validate_runtime(root, descriptor)
      root = File.expand_path(root)
      manifest = JSON.parse(File.read(File.join(root, "manifest.json")))
      %w[searchkernel_version searchkernel_revision runtime_package_generation platform native_target build_environment native_abi_version vespa_commit runtime_identity].each do |key|
        raise ClientError, "runtime identity mismatch for #{key}" unless manifest[key] == descriptor[key]
      end
      %w[bin/searchkerneld bin/searchkerneld.real lib/libsearchkernel_vespa.so.2].each do |member|
        raise ClientError, "runtime required member missing: #{member}" unless File.file?(File.join(root, member))
      end
      digest = Digest::SHA256.new
      Dir.glob(File.join(root, "**", "*"), File::FNM_DOTMATCH).select { |path| File.file?(path) && File.basename(path) != "manifest.json" }.sort.each do |path|
        relative = path.delete_prefix(File.expand_path(root) + "/")
        data = File.binread(path)
        digest.update([relative.bytesize].pack("Q>")); digest.update(relative); digest.update([data.bytesize].pack("Q>")); digest.update(data)
      end
      raise ClientError, "runtime payload digest mismatch" unless manifest["payload_sha256"] == digest.hexdigest
      File.expand_path(root)
      rescue Errno::ENOENT, JSON::ParserError => error
      raise ClientError, "runtime invalid: #{error.message}"
    end


    def write_definition(definition)
      raise ArgumentError, "index definition must be a Hash" unless definition.is_a?(Hash)

      file = Tempfile.new(["searchkernel-definition-", ".json"])
      file.binmode
      file.write(JSON.generate(definition, allow_nan: false))
      file.flush
      file
    rescue JSON::GeneratorError => error
      raise ArgumentError, "index definition is not valid canonical JSON: #{error.message}"
    end

    def free_port
      server = TCPServer.new("127.0.0.1", 0)
      server.addr[1]
    ensure
      server&.close
    end

    def wait_until_ready(port)
      deadline = monotonic + STARTUP_TIMEOUT
      loop do
        reap = Process.waitpid(@pid, Process::WNOHANG)
        if reap
          @pid = nil
          raise ClientError, "managed local host exited during startup#{diagnostic_suffix}"
        end

        begin
          socket = TCPSocket.new("127.0.0.1", port)
          socket.close
          return
        rescue Errno::ECONNREFUSED, Errno::EHOSTUNREACH, Errno::ENOENT
          nil
        end

        break if monotonic >= deadline
        sleep 0.05
      end
      raise ClientError, "managed local host did not open its HTTP listener#{diagnostic_suffix}"
    end

    def stop_process(force:)
      pid = @pid
      @pid = nil
      return unless pid

      if process_alive?(pid)
        signal_group(force ? "KILL" : "INT", pid)
        unless wait_for_exit(pid, force ? KILL_TIMEOUT : SHUTDOWN_TIMEOUT)
          signal_group("KILL", pid)
          wait_for_exit(pid, KILL_TIMEOUT)
        end
      else
        begin
          Process.waitpid(pid, Process::WNOHANG)
        rescue Errno::ECHILD
          nil
        end
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

    def signal_group(signal, pid)
      Process.kill(signal, -pid)
    rescue Errno::ESRCH
      nil
    end

    def wait_for_exit(pid, timeout)
      deadline = monotonic + timeout
      loop do
        begin
          return true if Process.waitpid(pid, Process::WNOHANG)
        rescue Errno::ECHILD
          return true
        end
        return false if monotonic >= deadline
        sleep 0.05
      end
    end

    def diagnostic_suffix
      @stderr_file&.flush
      return "" unless File.file?(@stderr_path)

      text = File.binread(@stderr_path)
      text = text.byteslice(-4096, 4096) if text.bytesize > 4096
      text = text.to_s.force_encoding(Encoding::UTF_8).scrub.gsub(/[\x00-\x1f\x7f]+/, " ").strip
      text.empty? ? "" : ": #{text}"
    rescue StandardError
      ""
    end

    def cleanup_definition
      file = @definition_file
      @definition_file = nil
      return unless file

      path = file.path
      file.close
      File.unlink(path) if File.exist?(path)
    rescue StandardError
      nil
    end

    def close_stderr
      @stderr_file&.close unless @stderr_file&.closed?
      @stderr_file = nil
    rescue StandardError
      nil
    end

    def cleanup_tmpdir
      return unless @tmpdir
      FileUtils.remove_entry_secure(@tmpdir) if File.exist?(@tmpdir)
      @tmpdir = nil
    rescue StandardError
      nil
    end

    def monotonic
      Process.clock_gettime(Process::CLOCK_MONOTONIC)
    end
  end
end
