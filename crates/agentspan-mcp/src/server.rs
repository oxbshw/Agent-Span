//! Newline-delimited JSON-RPC 2.0 MCP server over stdio.

use agentspan_channels::ChannelRegistry;
use agentspan_core::types::{ProbeStatus, ReadOptions, SearchOptions};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::tools::{self, Op};

const PROTOCOL_VERSION: &str = "2024-11-05";

/// The AgentSpan MCP server.
#[derive(Clone)]
pub struct McpServer {
    registry: ChannelRegistry,
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

fn ok(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn err(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn text_content(text: String, is_error: bool) -> Value {
    json!({ "content": [{ "type": "text", "text": text }], "isError": is_error })
}

impl McpServer {
    /// Build a server over the default channel registry.
    pub fn new() -> Self {
        Self {
            registry: ChannelRegistry::default_channels(),
        }
    }

    /// Handle one JSON-RPC request, returning the response (None for notifications).
    pub async fn handle_request(&self, req: &Value) -> Option<Value> {
        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");

        match method {
            "initialize" => Some(ok(
                id,
                json!({
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "agentspan", "version": env!("CARGO_PKG_VERSION") }
                }),
            )),
            "notifications/initialized" => None,
            "ping" => Some(ok(id, json!({}))),
            "tools/list" => Some(ok(id, json!({ "tools": tools::tool_schemas() }))),
            "tools/call" => {
                let name = req["params"]["name"].as_str().unwrap_or("");
                let args = &req["params"]["arguments"];
                match self.call_tool(name, args).await {
                    Ok(text) => Some(ok(id, text_content(text, false))),
                    Err(e) => Some(ok(id, text_content(e, true))),
                }
            }
            "" => Some(err(id, -32600, "invalid request: missing method")),
            other => Some(err(id, -32601, &format!("method not found: {other}"))),
        }
    }

    /// Dispatch a tool call to the channel registry.
    pub async fn call_tool(&self, name: &str, args: &Value) -> Result<String, String> {
        let tool = tools::find(name).ok_or_else(|| format!("unknown tool: {name}"))?;

        if tool.op == Op::Doctor {
            return Ok(self.doctor_summary().await);
        }

        let channel = self
            .registry
            .by_name(tool.channel)
            .ok_or_else(|| format!("channel not available: {}", tool.channel))?;

        match tool.op {
            Op::Read => {
                let url = args["url"]
                    .as_str()
                    .ok_or_else(|| "missing required argument: url".to_string())?;
                let content = channel
                    .read(url, ReadOptions::default())
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(channel.format_for_llm(&content.body))
            }
            Op::Search => {
                let query = args["query"]
                    .as_str()
                    .ok_or_else(|| "missing required argument: query".to_string())?;
                let limit = args["limit"].as_u64().unwrap_or(10) as usize;
                let opts = SearchOptions {
                    limit,
                    ..Default::default()
                };
                let results = channel
                    .search(query, opts)
                    .await
                    .map_err(|e| e.to_string())?;
                let trimmed: Vec<Value> = results
                    .iter()
                    .map(|r| {
                        json!({
                            "title": r.title,
                            "url": r.url,
                            "snippet": r.snippet,
                            "author": r.author,
                        })
                    })
                    .collect();
                Ok(serde_json::to_string_pretty(&trimmed).unwrap_or_default())
            }
            Op::Doctor => unreachable!(),
        }
    }

    async fn doctor_summary(&self) -> String {
        let mut lines = Vec::new();
        for ch in self.registry.list() {
            let healths = ch.check_health().await;
            let active = healths
                .iter()
                .find(|h| h.probe.status == ProbeStatus::Ok)
                .or_else(|| healths.iter().find(|h| h.probe.status == ProbeStatus::Warn));
            let status = match active {
                Some(h) => format!("{:?} via {}", h.probe.status, h.backend_name),
                None => "unavailable".to_string(),
            };
            lines.push(format!("{}: {}", ch.name(), status));
        }
        lines.join("\n")
    }

    /// Run the stdio transport loop (newline-delimited JSON-RPC).
    pub async fn run_stdio(&self) -> anyhow::Result<()> {
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin).lines();
        let mut stdout = tokio::io::stdout();

        while let Some(line) = reader.next_line().await? {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let req: Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(e) => {
                    let resp = err(Value::Null, -32700, &format!("parse error: {e}"));
                    stdout.write_all(format!("{resp}\n").as_bytes()).await?;
                    stdout.flush().await?;
                    continue;
                }
            };
            if let Some(resp) = self.handle_request(&req).await {
                stdout.write_all(format!("{resp}\n").as_bytes()).await?;
                stdout.flush().await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn initialize_returns_server_info() {
        let server = McpServer::new();
        let req = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}});
        let resp = server.handle_request(&req).await.unwrap();
        assert_eq!(resp["result"]["serverInfo"]["name"], "agentspan");
        assert_eq!(resp["result"]["protocolVersion"], PROTOCOL_VERSION);
    }

    #[tokio::test]
    async fn tools_list_returns_all_tools() {
        let server = McpServer::new();
        let req = json!({"jsonrpc":"2.0","id":2,"method":"tools/list"});
        let resp = server.handle_request(&req).await.unwrap();
        let tools = resp["result"]["tools"].as_array().unwrap();
        assert!(tools.len() >= 15);
    }

    #[tokio::test]
    async fn initialized_notification_has_no_response() {
        let server = McpServer::new();
        let req = json!({"jsonrpc":"2.0","method":"notifications/initialized"});
        assert!(server.handle_request(&req).await.is_none());
    }

    #[tokio::test]
    async fn unknown_method_is_error() {
        let server = McpServer::new();
        let req = json!({"jsonrpc":"2.0","id":3,"method":"does/not/exist"});
        let resp = server.handle_request(&req).await.unwrap();
        assert_eq!(resp["error"]["code"], -32601);
    }

    #[tokio::test]
    async fn call_unknown_tool_is_error_content() {
        let server = McpServer::new();
        let req = json!({"jsonrpc":"2.0","id":4,"method":"tools/call",
            "params":{"name":"nope","arguments":{}}});
        let resp = server.handle_request(&req).await.unwrap();
        assert_eq!(resp["result"]["isError"], true);
    }

    #[tokio::test]
    async fn call_doctor_returns_summary() {
        let server = McpServer::new();
        let req = json!({"jsonrpc":"2.0","id":5,"method":"tools/call",
            "params":{"name":"doctor","arguments":{}}});
        let resp = server.handle_request(&req).await.unwrap();
        assert_eq!(resp["result"]["isError"], false);
        let text = resp["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("web"));
        assert!(text.contains("exa"));
    }

    #[tokio::test]
    async fn read_without_url_is_error() {
        let server = McpServer::new();
        let result = server.call_tool("web_read", &json!({})).await;
        assert!(result.is_err());
    }

    #[test]
    fn every_tool_channel_resolves_in_registry() {
        let registry = ChannelRegistry::default_channels();
        for tool in tools::TOOLS {
            if tool.op == Op::Doctor {
                continue;
            }
            assert!(
                registry.by_name(tool.channel).is_some(),
                "tool {} maps to unknown channel {}",
                tool.name,
                tool.channel
            );
        }
    }
}
