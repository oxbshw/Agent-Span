// AgentSpan Quickstart — Rust SDK
//
// Prereq: agentspan serve --port 8080
// Run:    cargo run --example quickstart

use agentspan_sdk::AgentSpanClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = AgentSpanClient::new("http://localhost:8080");

    // Read any URL.
    let content = client.read("https://example.com").await?;
    println!("Title: {:?}", content.title);
    println!("Body (first 200 chars): {}", &content.body[..200.min(content.body.len())]);
    println!("Cached: {}", content.cached);

    // Search Hacker News.
    let results = client.search("hackernews", "rust async", 10).await?;
    println!("\nHN search results ({}):", results.len());
    for r in &results {
        println!("  - {}", r.title);
        println!("    {}", r.url);
    }

    // List all channels.
    let channels = client.list_channels().await?;
    println!("\n{} channels available", channels.len());

    Ok(())
}
