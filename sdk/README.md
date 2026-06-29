# AgentSpan SDKs

Official client libraries for the [AgentSpan](https://github.com/oxbshw/Agent-Span)
gateway. Every SDK is a thin, typed client over the same REST API
([docs/api-reference.md](../docs/api-reference.md)) and exposes the same method set:
`read`, `search`, `listChannels`, `doctor`, `getConfig`, `batchRead`,
`batchSearch`, `health`, `createKey`, `revokeKey` (+ SSE `streamEvents` where the
language makes it natural).

| Language | Package | Dir | HTTP stack | Tests |
|---|---|---|---|---|
| Python | `agentspan` (PyPI) | [python/](python) | httpx | pytest + MockTransport |
| JavaScript / TypeScript | `@agentspan/sdk` (npm) | [js/](js) | `fetch` | `node --test` |
| Rust | `agentspan-sdk` (crates.io) | [rust/](rust) | reqwest | cargo + wiremock |
| Go | `github.com/oxbshw/Agent-Span-go` | [go/](go) | net/http | `go test` + httptest |
| Ruby | `agentspan` (RubyGems) | [ruby/](ruby) | net/http | minitest + webmock |
| Java | `io.agentspan:agentspan-sdk` (Maven) | [java/](java) | java.net.http + Jackson | JUnit 5 + MockWebServer |
| PHP | `agentspan/sdk` (Packagist) | [php/](php) | Guzzle | PHPUnit + MockHandler |
| C# / .NET | `AgentSpan.Sdk` (NuGet) | [csharp/](csharp) | HttpClient | xUnit + stub handler |
| Swift | `AgentSpan` (SwiftPM) | [swift/](swift) | URLSession | XCTest + URLProtocol |

**9 SDKs** — every mainstream agent/runtime ecosystem. Error mapping is uniform
across all of them: `401 → AuthenticationError`, `429 → RateLimitError(retryAfter)`,
other non-2xx → `APIError(status, message)`, embedded `{"error"}` → `ChannelError`.

## Quick start (any language)

```bash
# 1. run the gateway
agentspan serve            # or: docker run -p 8080:8080 ghcr.io/oxbshw/Agent-Span

# 2. install your SDK (see the per-language READMEs)
# 3. read & search 24+ platforms behind one API
```
