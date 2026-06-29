# frozen_string_literal: true

require_relative "lib/agentspan"

Gem::Specification.new do |spec|
  spec.name = "agentspan"
  spec.version = AgentSpan::VERSION
  spec.authors = ["AgentSpan Contributors"]
  spec.summary = "Official Ruby SDK for the AgentSpan gateway"
  spec.description = "Thin Ruby client for the AgentSpan REST API (read/search 24+ platforms)."
  spec.homepage = "https://github.com/oxbshw/Agent-Span"
  spec.license = "MIT"
  spec.required_ruby_version = ">= 3.0"

  spec.files = Dir["lib/**/*.rb", "README.md"]
  spec.require_paths = ["lib"]

  spec.add_development_dependency "minitest", "~> 5.0"
  spec.add_development_dependency "webmock", "~> 3.0"
end
