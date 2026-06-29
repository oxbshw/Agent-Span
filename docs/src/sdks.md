# SDKs

Nine official client libraries, all thin typed wrappers over the same
[REST API](api-reference.md) with identical method names and error mapping.

| Language | Package | Install | Tests |
|---|---|---|---|
| Python | `agentspan` | `pip install -e sdk/python` | pytest |
| JavaScript / TypeScript | `@agentspan/sdk` | `npm install` | node --test |
| Rust | `agentspan-sdk` | `cargo add agentspan-sdk` | cargo + wiremock |
| Go | `github.com/oxbshw/Agent-Span-go` | `go get` | go test |
| Ruby | `agentspan` | `gem install` | minitest |
| Java | `io.agentspan:agentspan-sdk` | Maven | JUnit |
| PHP | `agentspan/sdk` | Composer | PHPUnit |
| C# / .NET | `AgentSpan.Sdk` | NuGet | xUnit |
| Swift | `AgentSpan` | SwiftPM | XCTest |

Every SDK exposes: `read`, `search`, `listChannels`, `doctor`, `getConfig`,
`batchRead`, `batchSearch`, `health`, `createKey`, `revokeKey` (+ SSE
`streamEvents` where natural). Error mapping is uniform:
`401 → AuthenticationError`, `429 → RateLimitError(retryAfter)`, other non-2xx →
`APIError`, embedded `{"error"}` → `ChannelError`.

```python
from agentspan import AgentSpanClient
client = AgentSpanClient(base_url="http://localhost:8080", api_key="as_...")
content = await client.read("https://example.com")
results = await client.search("hackernews", "rust", limit=10)
```

See [`sdk/`](https://github.com/oxbshw/Agent-Span/tree/main/sdk) for per-language READMEs.
