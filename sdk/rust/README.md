# agentspan-sdk (Rust)

Official Rust SDK for the [AgentSpan](https://github.com/oxbshw/Agent-Span) gateway.

```toml
[dependencies]
agentspan-sdk = "0.3"
tokio = { version = "1", features = ["full"] }
```

```rust
use agentspan_sdk::AgentSpanClient;

#[tokio::main]
async fn main() -> Result<(), agentspan_sdk::Error> {
    let client = AgentSpanClient::new("http://localhost:8080").with_api_key("as_...");
    let content = client.read("https://example.com", false).await?;
    println!("{}", content.body);

    let results = client.search("hackernews", "rust", 10).await?;
    let batch = client.batch_read(&["https://a".into(), "https://b".into()], false).await?;
    Ok(())
}
```

Async (Tokio + reqwest). See [docs/api-reference.md](../../docs/api-reference.md)
for the full method set. Tests: `cargo test -p agentspan-sdk` (uses `wiremock`).
