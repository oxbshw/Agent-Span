# agentspan (Ruby)

Official Ruby SDK for the [AgentSpan](https://github.com/oxbshw/Agent-Span) gateway.

```bash
gem install agentspan
```

```ruby
require "agentspan"

client = AgentSpan::Client.new(base_url: "http://localhost:8080", api_key: "as_...")
content = client.read("https://example.com")
puts content["body"]

results = client.search("hackernews", "rust", limit: 10)
batch = client.batch_read(["https://a", "https://b"])
```

Standard-library HTTP (`net/http`), no runtime dependencies. See
[docs/api-reference.md](../../docs/api-reference.md). Tests: `bundle install && rake test`
(minitest + webmock).
