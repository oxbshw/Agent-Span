# AgentSpan (Swift)

Official Swift SDK for the [AgentSpan](https://github.com/oxbshw/Agent-Span) gateway.

```swift
.package(url: "https://github.com/oxbshw/Agent-Span-swift", from: "0.4.0")
```

```swift
import AgentSpan

let client = AgentSpanClient(baseURL: "http://localhost:8080", apiKey: "as_...")
let content = try await client.read("https://example.com")
print(content["body"] as? String ?? "")

let results = try await client.search("hackernews", "rust", limit: 10)
let batch = try await client.batchRead(["https://a", "https://b"])
```

Built on `URLSession` async/await (macOS 12+ / iOS 15+). See
[docs/api-reference.md](../../docs/api-reference.md). Tests: `swift test`
(XCTest + `URLProtocol` stub).
