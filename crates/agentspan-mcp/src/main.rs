//! AgentSpan MCP server binary.
//!
//! Defaults to the stdio transport (for Claude Code, Cursor, etc.). Pass
//! `--http [addr]` to serve the HTTP/SSE transport instead — useful for remote
//! MCP clients, web-based consumers, and multi-tenant gateways.
//!
//! Examples:
//!   agentspan-mcp                           # stdio (default)
//!   agentspan-mcp --http                    # HTTP on 0.0.0.0:9000
//!   agentspan-mcp --http 127.0.0.1:8080     # HTTP on a specific address

use agentspan_mcp::McpServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Simple arg parsing: `--http [addr]` switches to HTTP transport.
    let http_idx = args.iter().position(|a| a == "--http");

    let server = McpServer::new();

    if let Some(_idx) = http_idx {
        #[cfg(feature = "http")]
        {
            let addr = args
                .get(_idx + 1)
                .map(|s| s.as_str())
                .unwrap_or("0.0.0.0:9000");
            agentspan_mcp::http::run_http(server, addr).await
        }
        #[cfg(not(feature = "http"))]
        {
            eprintln!("error: this binary was built without the `http` feature.");
            eprintln!("rebuild with: cargo build -p agentspan-mcp --features http");
            std::process::exit(1);
        }
    } else {
        server.run_stdio().await
    }
}
