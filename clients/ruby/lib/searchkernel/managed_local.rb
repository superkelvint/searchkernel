# frozen_string_literal: true

require "fileutils"
require "json"
require "socket"
require "tempfile"
require "tmpdir"

require_relative "errors"

module SearchKernel
  class ManagedLocalHost
    STARTUP_TIMEOUT = 15.0
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

      packaged = File.expand_path("bin/searchkerneld", __dir__)
      return packaged if File.file?(packaged)

      root = File.expand_path("../../../..", __dir__)
      target_dir = ENV["CARGO_TARGET_DIR"]
      if target_dir && !target_dir.empty?
        target_dir = File.expand_path(target_dir, root) unless target_dir.start_with?("/")
        candidate = File.join(target_dir, "debug", "searchkerneld")
        return candidate if File.file?(candidate)
      end
      File.join(root, "target", "debug", "searchkerneld")
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
