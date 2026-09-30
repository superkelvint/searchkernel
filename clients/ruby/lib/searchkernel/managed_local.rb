# frozen_string_literal: true

require "digest"
require "fileutils"
require "open3"
require "socket"
require "tmpdir"

require_relative "errors"

module SearchKernel
  class ManagedLocalHost
    STARTUP_TIMEOUT = 60.0
    SHUTDOWN_TIMEOUT = 15.0
    KILL_TIMEOUT = 5.0
    MANAGED_DATA_ROOT_NAME = ".searchkernel-managed"
    MANAGED_MARKER_NAME = ".searchkernel-managed-target"
    MANAGED_ROOT_MARKER_NAME = ".searchkernel-data-root"
    MANAGED_ROOT_MARKER_VALUE = "searchkernel-managed-v1"
    LEGACY_HOST_ENV_KEYS = %w[
      SEARCHKERNEL_INDEX_PATH
      SEARCHKERNEL_INDEX_DEFINITION
      SEARCHKERNEL_INDEX_DEFINITION_JSON
    ].freeze

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

      def pool
        @pool ||= {}
      end

      def pool_mutex
        @pool_mutex ||= Mutex.new
      end

      def acquire(data_root)
        key = File.expand_path(data_root.to_s)
        stale = nil
        pool_mutex.synchronize do
          entry = pool[key]
          if entry && entry[:host].alive?
            entry[:refs] += 1
            return entry[:host]
          end
          if entry
            pool.delete(key)
            stale = entry[:host]
          end
        end
        stale&.close(force: true)

        pool_mutex.synchronize do
          entry = pool[key]
          if entry && entry[:host].alive?
            entry[:refs] += 1
            return entry[:host]
          end

          host = new(key)
          host.instance_variable_set(:@pool_key, key)
          pool[key] = { host: host, refs: 1 }
          host
        end
      end

      def location(index_path)
        path = File.expand_path(index_path.to_s)
        target = "local-#{Digest::SHA256.hexdigest(path)[0, 32]}"
        [path, File.join(File.dirname(path), MANAGED_DATA_ROOT_NAME), target]
      end

      def marker_path(index_path)
        File.join(index_path, MANAGED_MARKER_NAME)
      end

      def lstat_or_nil(path)
        File.lstat(path)
      rescue Errno::ENOENT
        nil
      end

      def validate_data_root(data_root)
        marker = File.join(data_root, MANAGED_ROOT_MARKER_NAME)
        root_stat = lstat_or_nil(data_root)
        marker_stat = lstat_or_nil(marker)
        unless root_stat&.directory? && !root_stat.symlink? && marker_stat&.file? && !marker_stat.symlink?
          raise ClientError, "managed-local data root is not owned by Search Kernel: #{data_root}"
        end
        value = File.read(marker)
        unless [
          MANAGED_ROOT_MARKER_VALUE,
          "#{MANAGED_ROOT_MARKER_VALUE}\n",
          "#{MANAGED_ROOT_MARKER_VALUE}\r\n"
        ].include?(value)
          raise ClientError, "managed-local data root is not owned by Search Kernel: #{data_root}"
        end
      rescue SystemCallError => error
        raise ClientError, "managed-local data root is not owned by Search Kernel: #{data_root}: #{error.message}"
      end

      def prepare_data_root(data_root)
        parent = File.dirname(data_root)
        FileUtils.mkdir_p(parent)
        marker = File.join(data_root, MANAGED_ROOT_MARKER_NAME)
        created = false
        begin
          Dir.mkdir(data_root)
          created = true
        rescue Errno::EEXIST
          nil
        end

        if created
          begin
            File.write(marker, "#{MANAGED_ROOT_MARKER_VALUE}\n", mode: "wx")
            validate_data_root(data_root)
            return
          rescue StandardError
            File.delete(marker) if File.file?(marker)
            Dir.rmdir(data_root) if File.directory?(data_root) && Dir.empty?(data_root)
            raise
          end
        end

        deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 2.0
        loop do
          root_stat = lstat_or_nil(data_root)
          unless root_stat&.directory? && !root_stat.symlink?
            raise ClientError, "managed-local data root is not owned by Search Kernel: #{data_root}"
          end
          marker_stat = lstat_or_nil(marker)
          if marker_stat.nil?
            if Process.clock_gettime(Process::CLOCK_MONOTONIC) >= deadline
              raise ClientError, "managed-local data root is not owned by Search Kernel: #{data_root}"
            end
            sleep 0.025
            next
          end
          validate_data_root(data_root)
          return
        end
      end

      def reserve_path(index_path, target)
        FileUtils.mkdir_p(File.dirname(index_path))
        created = false
        begin
          Dir.mkdir(index_path)
          created = true
          File.write(marker_path(index_path), "#{target}\n", mode: "wx")
        rescue StandardError
          if created && File.directory?(index_path)
            marker = marker_path(index_path)
            File.delete(marker) if File.file?(marker)
            Dir.rmdir(index_path) if Dir.empty?(index_path)
          end
          raise
        end
      rescue Errno::EEXIST => error
        raise ClientError, "managed-local index path already exists: #{index_path}"
      end

      def cleanup_reservation(index_path, target)
        marker = marker_path(index_path)
        return unless File.file?(marker)
        return unless File.read(marker).strip == target
        return unless Dir.children(index_path) == [MANAGED_MARKER_NAME]

        File.delete(marker)
        Dir.rmdir(index_path)
      rescue StandardError
        nil
      end

      def marker_matches?(index_path, target)
        marker = marker_path(index_path)
        marker_stat = lstat_or_nil(marker)
        return false unless marker_stat
        if marker_stat.symlink? || !marker_stat.file?
          raise ClientError, "unsafe managed-local marker: #{marker}"
        end
        value = File.read(marker).strip
        raise ClientError, "managed-local marker target mismatch at #{marker}" unless value == target
        true
      end

      def ensure_marker(index_path, target)
        begin
          Dir.mkdir(index_path)
        rescue Errno::EEXIST
          path_stat = lstat_or_nil(index_path)
          if !path_stat || path_stat.symlink? || !path_stat.directory?
            raise ClientError, "managed-local identity path is not a real directory: #{index_path}"
          end
        end

        marker = marker_path(index_path)
        begin
          File.write(marker, "#{target}\n", mode: "wx")
        rescue Errno::EEXIST
          nil
        end
        unless marker_matches?(index_path, target)
          raise ClientError, "managed-local identity path is not owned by Search Kernel: #{index_path}"
        end
      end

      def host_environment
        env = LEGACY_HOST_ENV_KEYS.to_h { |key| [key, nil] }
        sdk_lib = ENV["SEARCHKERNEL_SDK_LIB"]
        if sdk_lib && !sdk_lib.empty?
          existing = ENV["LD_LIBRARY_PATH"]
          env["LD_LIBRARY_PATH"] = existing && !existing.empty? ? "#{sdk_lib}:#{existing}" : sdk_lib
        end
        env
      end

      def migrate_legacy(index_path, data_root, target)
        destination = File.join(data_root, target)
        source_stat = lstat_or_nil(index_path)
        if source_stat && (source_stat.symlink? || !source_stat.directory?)
          raise ClientError, "legacy managed-local index must be a real directory: #{index_path}"
        end

        marker_matches = source_stat ? marker_matches?(index_path, target) : false
        destination_stat = lstat_or_nil(destination)
        if destination_stat
          if destination_stat.symlink? || !destination_stat.directory?
            raise ClientError, "unsafe managed-local destination: #{destination}"
          end
          return if source_stat.nil? || marker_matches
          raise ClientError, "managed-local destination already exists while legacy source remains: #{destination}"
        end
        return if marker_matches || source_stat.nil?

        env = host_environment
        stdout, stderr, status = Open3.capture3(
          env,
          host_binary,
          "--migrate-index", index_path,
          "--data-root", data_root,
          "--target", target
        )
        unless status.success?
          final_destination = lstat_or_nil(destination)
          final_source = lstat_or_nil(index_path)
          if final_destination&.directory? && !final_destination.symlink? && final_source.nil?
            ensure_marker(index_path, target)
            return
          end
          output = [stdout, stderr].reject(&:empty?).join("\n").strip
          suffix = output.empty? ? "" : "\nsearchkerneld output:\n#{output}"
          raise ClientError, "managed-local legacy index migration failed for #{index_path}#{suffix}"
        end
        ensure_marker(index_path, target)
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

      def release(host, force:)
        key = host.instance_variable_get(:@pool_key)
        return true unless key

        pool_mutex.synchronize do
          entry = pool[key]
          return true unless entry && entry[:host].equal?(host)

          if !force && entry[:refs] > 1
            entry[:refs] -= 1
            return false
          end

          pool.delete(key)
          host.instance_variable_set(:@pool_key, nil)
          true
        end
      end

      def close_all
        registry.values.each do |host|
          begin
            host.close(force: true)
          rescue StandardError
            nil
          end
        end
      end
    end

    at_exit { close_all }

    def initialize(data_root)
      @data_root = File.expand_path(data_root.to_s)
      @closed = false
      @pid = nil
      @tmpdir = Dir.mktmpdir("searchkernel-ruby-")
      @socket_path = File.join(@tmpdir, "searchkerneld.sock")
      @stderr_path = File.join(@tmpdir, "searchkerneld.stderr")
      @stderr_file = File.open(@stderr_path, "wb")
      start
      self.class.register(self)
    rescue StandardError
      close(force: true)
      raise
    end

    def alive?
      return false if @closed || !@pid
      process_alive?(@pid)
    end

    def close(force: false)
      return if @closed
      return nil unless self.class.release(self, force: force)

      @closed = true
      self.class.unregister(self)
      stop_process(force: force)
      close_stderr
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
      env = self.class.host_environment

      command = [
        binary,
        "--data-root", @data_root,
        "--listen", "unix://#{@socket_path}",
        "--web-listen", "127.0.0.1:#{port}"
      ]

      @pid = Process.spawn(
        env,
        *command,
        pgroup: true,
        out: File::NULL,
        err: @stderr_file
      )
      wait_until_ready(port)
      @base_url = "http://127.0.0.1:#{port}"
    rescue SystemCallError => error
      raise ClientError, "failed to start managed local host: #{error.message}"
    end

    def host_binary
      self.class.host_binary
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
