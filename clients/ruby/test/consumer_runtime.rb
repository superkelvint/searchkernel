# frozen_string_literal: true

require "digest"
require "json"
require "rubygems/package"
require "tmpdir"
require "zlib"

ROOT = File.expand_path("../../..", __dir__) unless defined?(ROOT)
CONFORMANCE = JSON.parse(File.read(File.join(ROOT, "build/consumer-runtime/conformance.json")))
IDENTITY_VECTOR = CONFORMANCE.fetch("vectors").first
IDENTITY_MATERIAL = IDENTITY_VECTOR.fetch("identity_material")
RUNTIME_IDENTITY = IDENTITY_VECTOR.fetch("runtime_identity")

same("objects/#{RUNTIME_IDENTITY}", IDENTITY_VECTOR.fetch("cache_relative_path"), "Ruby runtime conformance cache path")
same("locks/#{RUNTIME_IDENTITY}.lock", IDENTITY_VECTOR.fetch("lock_relative_path"), "Ruby runtime conformance lock path")
same(
  "skrt-v1-#{Digest::SHA256.hexdigest(JSON.generate(IDENTITY_MATERIAL.sort.to_h))}",
  RUNTIME_IDENTITY,
  "Ruby runtime conformance identity"
)

def runtime_payload_digest(root)
  digest = Digest::SHA256.new
  Dir.glob(File.join(root, "**", "*"), File::FNM_DOTMATCH)
    .select { |path| File.file?(path) && File.basename(path) != "manifest.json" }
    .sort.each do |path|
      relative = path.delete_prefix(File.expand_path(root) + "/")
      data = File.binread(path)
      digest.update([relative.bytesize].pack("Q>"))
      digest.update(relative)
      digest.update([data.bytesize].pack("Q>"))
      digest.update(data)
    end
  digest.hexdigest
end

def create_runtime_fixture(root)
  FileUtils.mkdir_p(File.join(root, "bin"))
  FileUtils.mkdir_p(File.join(root, "lib"))
  File.write(File.join(root, "bin/searchkerneld"), "launcher")
  File.write(File.join(root, "bin/searchkerneld.real"), "daemon")
  File.write(File.join(root, "lib/libsearchkernel_vespa.so.2"), "adapter")
  File.chmod(0o755, File.join(root, "bin/searchkerneld"))
  File.chmod(0o755, File.join(root, "bin/searchkerneld.real"))
  File.write(File.join(root, "manifest.json"), JSON.generate(
    {
      "format_version" => 1,
      **IDENTITY_MATERIAL,
      "runtime_identity" => RUNTIME_IDENTITY,
      "payload_sha256" => runtime_payload_digest(root)
    }
  ))
end

def write_runtime_manifest(root, changes = {})
  path = File.join(root, "manifest.json")
  manifest = JSON.parse(File.read(path))
  manifest.merge!(changes)
  File.write(path, JSON.generate(manifest))
end

def runtime_descriptor
  {
    "format_version" => 1,
    **IDENTITY_MATERIAL,
    "runtime_identity" => RUNTIME_IDENTITY,
    "artifact_url" => "https://example.invalid/searchkernel-runtime-#{RUNTIME_IDENTITY}-linux-x86_64-glibc.tar.gz",
    "artifact_sha256" => "a" * 64
  }
end

assert(
  !SearchKernel::ManagedLocalHost.glibc_x86_64?("x86_64-linux-musl", "musl libc"),
  "x86-64 musl is not a qualified consumer-runtime host"
)
assert(
  SearchKernel::ManagedLocalHost.glibc_x86_64?("x86_64-linux-gnu", "glibc 2.39"),
  "x86-64 glibc is a qualified consumer-runtime host"
)
SearchKernel::ManagedLocalHost.validate_runtime_descriptor(runtime_descriptor)

CONFORMANCE.fetch("runtime_validation_cases").each do |fixture|
  Dir.mktmpdir("searchkernel-ruby-runtime-conformance-") do |base|
    runtime = File.join(base, "runtime")
    create_runtime_fixture(runtime)
    case fixture.fetch("mutation")
    when "manifest-format"
      write_runtime_manifest(runtime, "format_version" => 99)
    when "non-executable"
      File.chmod(0o644, File.join(runtime, "bin/searchkerneld.real"))
    when "escaping-symlink"
      File.unlink(File.join(runtime, "lib/libsearchkernel_vespa.so.2"))
      outside = File.join(base, "outside.so")
      File.write(outside, "outside")
      File.symlink(outside, File.join(runtime, "lib/libsearchkernel_vespa.so.2"))
    when "identity-mismatch"
      write_runtime_manifest(runtime, "searchkernel_revision" => "different")
    when "payload-digest-mismatch"
      write_runtime_manifest(runtime, "payload_sha256" => "0" * 64)
    else
      raise "unknown shared runtime fixture mutation #{fixture.fetch('mutation').inspect}"
    end

    error = raises(SearchKernel::ClientError, "#{fixture.fetch('name')} fails closed") do
      SearchKernel::ManagedLocalHost.validate_runtime(runtime, runtime_descriptor)
    end
    assert(
      error.message.include?(fixture.fetch("expected_error")),
      "#{fixture.fetch('name')} uses canonical #{fixture.fetch('expected_error')} taxonomy"
    )
  end
end

{
  "archive path traversal" => lambda do |tar|
    tar.add_file_simple("../outside", 0o644, 7) { |file| file.write("outside") }
  end,
  "archive symlink escape" => lambda do |tar|
    tar.add_symlink("lib/escape.so", "../../outside.so", 0o777)
  end
}.each do |label, add_entry|
  Dir.mktmpdir("searchkernel-ruby-archive-conformance-") do |root|
    archive_path = File.join(root, "runtime.tar.gz")
    Zlib::GzipWriter.open(archive_path) do |gzip|
      Gem::Package::TarWriter.new(gzip) do |tar|
        add_entry.call(tar)
      end
    end
    error = raises(SearchKernel::ClientError, "#{label} is rejected before extraction") do
      SearchKernel::ManagedLocalHost.validate_runtime_archive(archive_path)
    end
    assert(error.message.include?("artifact-invalid"), "#{label} uses canonical artifact-invalid taxonomy")
  end
end

puts "Ruby consumer-runtime conformance PASS"
