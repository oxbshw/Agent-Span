//! AgentSpan MCP server — exposes channels as Model Context Protocol tools.

pub mod server;
pub mod tools;

#[cfg(feature = "http")]
pub mod http;

pub use server::McpServer;
pub use tools::{tool_schemas, Op, ToolDef, TOOLS};
