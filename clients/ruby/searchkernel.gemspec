# frozen_string_literal: true

require_relative "lib/searchkernel/version"

Gem::Specification.new do |spec|
  spec.name = "searchkernel"
  spec.version = SearchKernel::VERSION
  spec.summary = "Ruby client for Search Kernel"
  spec.description = "Idiomatic Ruby client plumbing for the canonical Search Kernel protocol."
  spec.authors = ["Search Kernel contributors"]
  spec.homepage = "https://github.com/superkelvint/searchkernel"
  spec.files = Dir.chdir(__dir__) { Dir["lib/**/*.rb", "lib/**/*.json", "README.md"] }
  spec.require_paths = ["lib"]
  spec.required_ruby_version = ">= 3.1"
  spec.license = "Apache-2.0"
end
