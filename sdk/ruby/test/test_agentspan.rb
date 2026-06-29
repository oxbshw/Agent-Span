# frozen_string_literal: true

require "minitest/autorun"
require "webmock/minitest"
require "agentspan"

class AgentSpanTest < Minitest::Test
  BASE = "http://test"

  def client
    AgentSpan::Client.new(base_url: BASE, api_key: "k")
  end

  def test_read_returns_content
    stub_request(:get, "#{BASE}/api/v1/read")
      .with(query: hash_including("url" => "https://x"))
      .to_return(body: { channel: "web", content: { url: "https://x", title: "T", body: "hi" } }.to_json)
    content = client.read("https://x")
    assert_equal "hi", content["body"]
  end

  def test_read_channel_error
    stub_request(:get, "#{BASE}/api/v1/read")
      .with(query: hash_including("url" => "ftp://x"))
      .to_return(body: { error: "no channel" }.to_json)
    assert_raises(AgentSpan::ChannelError) { client.read("ftp://x") }
  end

  def test_search_passes_limit
    stub_request(:get, "#{BASE}/api/v1/channels/hackernews/search")
      .with(query: hash_including("limit" => "7"))
      .to_return(body: { results: [{ title: "Rust" }] }.to_json)
    results = client.search("hackernews", "rust", limit: 7)
    assert_equal "Rust", results.first["title"]
  end

  def test_authentication_error
    stub_request(:get, "#{BASE}/api/v1/channels").to_return(status: 401, body: { error: "bad" }.to_json)
    assert_raises(AgentSpan::AuthenticationError) { client.list_channels }
  end

  def test_rate_limit_error
    stub_request(:get, "#{BASE}/api/v1/read")
      .with(query: hash_including("url" => "https://x"))
      .to_return(status: 429, headers: { "Retry-After" => "12" }, body: { error: "slow" }.to_json)
    err = assert_raises(AgentSpan::RateLimitError) { client.read("https://x") }
    assert_equal 12, err.retry_after
  end

  def test_batch_read
    stub_request(:post, "#{BASE}/api/v1/batch/read")
      .to_return(body: { count: 2, results: [{ ok: true }, { ok: false }] }.to_json)
    results = client.batch_read(%w[https://a https://b])
    assert_equal 2, results.length
  end

  def test_health
    stub_request(:get, "#{BASE}/health").to_return(status: 200)
    assert client.health
  end

  def test_sends_api_key
    stub_request(:get, "#{BASE}/api/v1/channels")
      .with(headers: { "X-API-Key" => "k" })
      .to_return(body: { channels: [] }.to_json)
    assert_equal [], client.list_channels
  end
end
