# frozen_string_literal: true

# Official Ruby SDK for the AgentSpan gateway.
# Thin client over the AgentSpan REST API (see docs/api-reference.md).

require "net/http"
require "json"
require "uri"

module AgentSpan
  VERSION = "0.3.0"

  class Error < StandardError; end
  class AuthenticationError < Error; end

  class RateLimitError < Error
    attr_reader :retry_after
    def initialize(message, retry_after = nil)
      super(message)
      @retry_after = retry_after
    end
  end

  class APIError < Error
    attr_reader :status
    def initialize(status, message)
      super(message)
      @status = status
    end
  end

  class ChannelError < Error; end

  # Synchronous client for the AgentSpan gateway.
  class Client
    def initialize(base_url: "http://localhost:8080", api_key: nil)
      @base_url = base_url.sub(%r{/+\z}, "")
      @api_key = api_key
    end

    # Read a URL via the best matching channel.
    def read(url, force_refresh: false)
      data = request(:get, "/api/v1/read", query: { url: url, force_refresh: force_refresh })
      raise ChannelError, data["error"] if data["error"]

      data["content"]
    end

    # Search a platform via a named channel.
    def search(channel, query, limit: 10)
      data = request(:get, "/api/v1/channels/#{channel}/search", query: { q: query, limit: limit })
      raise ChannelError, data["error"] if data["error"]

      data["results"] || []
    end

    # List available channels.
    def list_channels
      request(:get, "/api/v1/channels")["channels"] || []
    end

    # Run health diagnostics across all channels.
    def doctor
      request(:get, "/api/v1/doctor")
    end

    # Fetch the non-secret configuration view.
    def get_config
      request(:get, "/api/v1/config")
    end

    # Read many URLs in parallel (server-side batch).
    def batch_read(urls, force_refresh: false)
      request(:post, "/api/v1/batch/read", body: { urls: urls, force_refresh: force_refresh })["results"] || []
    end

    # Run many queries against one channel in parallel.
    def batch_search(channel, queries, limit: 10)
      body = { channel: channel, queries: queries, limit: limit }
      request(:post, "/api/v1/batch/search", body: body)["results"] || []
    end

    # Return true when the server's /health endpoint is OK.
    def health
      uri = URI("#{@base_url}/health")
      res = http(uri).get(uri.request_uri, headers)
      res.code.to_i == 200
    rescue StandardError
      false
    end

    # Create a new API key (requires admin scope).
    def create_key(name, scopes: ["read"], tenant_id: "default")
      request(:post, "/api/v1/auth/keys", body: { name: name, scopes: scopes, tenant_id: tenant_id })
    end

    # Revoke an API key by id (requires admin scope).
    def revoke_key(id)
      request(:delete, "/api/v1/auth/keys/#{id}")
      nil
    end

    private

    def headers
      h = { "Accept" => "application/json" }
      h["X-API-Key"] = @api_key if @api_key
      h
    end

    def http(uri)
      client = Net::HTTP.new(uri.host, uri.port)
      client.use_ssl = uri.scheme == "https"
      client
    end

    def request(method, path, query: nil, body: nil)
      uri = URI("#{@base_url}#{path}")
      uri.query = URI.encode_www_form(query) if query
      req = build_request(method, uri, body)
      res = http(uri).request(req)
      check(res)
      return {} if res.body.nil? || res.body.empty?

      JSON.parse(res.body)
    end

    def build_request(method, uri, body)
      klass = { get: Net::HTTP::Get, post: Net::HTTP::Post, delete: Net::HTTP::Delete }[method]
      req = klass.new(uri.request_uri)
      headers.each { |k, v| req[k] = v }
      if body
        req["Content-Type"] = "application/json"
        req.body = JSON.dump(body)
      end
      req
    end

    def check(res)
      status = res.code.to_i
      return if status < 400

      message = (JSON.parse(res.body)["error"] rescue nil) || res.message
      case status
      when 401 then raise AuthenticationError, message
      when 429 then raise RateLimitError.new(message, res["Retry-After"]&.to_i)
      else raise APIError.new(status, message)
      end
    end
  end
end
