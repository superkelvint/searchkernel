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

      def prepare_data_root(data_root)
        parent = File.dirname(data_root)
        FileUtils.mkdir_p(parent)
        marker = File.join(data_root, MANAGED_ROOT_MARKER_NAME)
        root_stat = lstat_optional(data_root)
        if root_stat
          validate_data_root(data_root, marker, root_stat)
          return
        end

        staging = Dir.mktmpdir(".searchkernel-managed-bootstrap-", parent)
        begin
          File.write(File.join(staging, MANAGED_ROOT_MARKER_NAME), "searchkernel-managed-v1\n", mode: "wx")
          begin
            File.rename(staging, data_root)
          rescue SystemCallError
            winner = lstat_optional(data_root)
            raise unless winner

            validate_data_root(data_root, marker, winner)
          end
        ensure
          FileUtils.remove_entry(staging) if File.exist?(staging)
        end
      end

      def lstat_optional(path)
        File.lstat(path)
      rescue Errno::ENOENT
        nil
      end

      def validate_data_root(data_root, marker, root_stat)
        unless root_stat.directory? && !root_stat.symlink?
          raise ClientError, "managed-local data root is not owned by Search Kernel: #{data_root}"
        end
        marker_stat = lstat_optional(marker)
        unless marker_stat&.file? && !marker_stat.symlink? &&
               File.read(marker) == "searchkernel-managed-v1\n"
          raise ClientError, "managed-local data root is not owned by Search Kernel: #{data_root}"
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

      def ensure_marker(index_path, target)
        return if File.exist?(index_path)

        Dir.mkdir(index_path)
        File.write(marker_path(index_path), "#{target}\n", mode: "wx")
      end

      def migrate_legacy(index_path, data_root, target)
        destination = File.join(data_root, target)
        require_real_directory(data_root, "managed data root")
        root_marker = File.join(data_root, MANAGED_ROOT_MARKER_NAME)
        require_regular_file(root_marker, "managed data-root ownership marker")
        unless File.read(root_marker) == "searchkernel-managed-v1\n"
          raise ClientError, "managed data-root ownership marker is invalid: #{data_root}"
        end
        destination_stat = lstat_optional(destination)
        if destination_stat
          require_index_directory(destination)
          return
        end
        source_stat = lstat_optional(index_path)
        if source_stat && (!source_stat.directory? || source_stat.symlink?)
          raise ClientError, "managed-local legacy index path is not a real directory: #{index_path}"
        end
        marker_stat = source_stat && lstat_optional(marker_path(index_path))
        if marker_stat
          unless marker_stat.file? && !marker_stat.symlink?
            raise ClientError, "managed-local index marker is not a regular file: #{marker_path(index_path)}"
          end
          return
        end
        return unless source_stat

        env = {}
        sdk_lib = ENV["SEARCHKERNEL_SDK_LIB"]
        if sdk_lib && !sdk_lib.empty?
          existing = ENV["LD_LIBRARY_PATH"]
          env["LD_LIBRARY_PATH"] = existing && !existing.empty? ? "#{sdk_lib}:#{existing}" : sdk_lib
        end
        stdout, stderr, status = Open3.capture3(
          env,
          host_binary,
          "--migrate-index", index_path,
          "--data-root", data_root,
          "--target", target
        )
        unless status.success?
          output = [stdout, stderr].reject(&:empty?).join("\n").strip
          suffix = output.empty? ? "" : "\nsearchkerneld output:\n#{output}"
          raise ClientError, "managed-local legacy index migration failed for #{index_path}#{suffix}"
        end
        ensure_marker(index_path, target)
      end

      def lstat_optional(path)
        File.lstat(path)
      rescue Errno::ENOENT
        nil
      end

      def require_real_directory(path, label)
        stat = lstat_optional(path)
        unless stat&.directory? && !stat.symlink?
          raise ClientError, "#{label} must be a real directory: #{path}"
        end
      end

      def require_regular_file(path, label)
        stat = lstat_optional(path)
        unless stat&.file? && !stat.symlink?
          raise ClientError, "#{label} must be a regular file: #{path}"
        end
      end

      def require_index_directory(path)
        require_real_directory(path, "managed index target")
        %w[data config manifest].each do |component|
          require_real_directory(File.join(path, component), "managed index component")
        end
        require_regular_file(File.join(path, "manifest", "index.json"), "managed index manifest")
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
      env = {}
      sdk_lib = ENV["SEARCHKERNEL_SDK_LIB"]
      if sdk_lib && !sdk_lib.empty?
        existing = ENV["LD_LIBRARY_PATH"]
        env["LD_LIBRARY_PATH"] = existing && !existing.empty? ? "#{sdk_lib}:#{existing}" : sdk_lib
      end

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
