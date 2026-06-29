//! `agentspan mcp` — MCP server configuration and tool discovery.
//!
//! Lets an agent (or human) self-configure MCP clients in one command:
//!   agentspan mcp print-config --client claude-code
//!   agentspan mcp install --client cursor
//!   agentspan mcp tools

use std::path::PathBuf;

use clap::Args;
use serde_json::{json, Value};

#[derive(Args)]
pub struct McpArgs {
    #[command(subcommand)]
    command: McpSub,
}

#[derive(clap::Subcommand)]
enum McpSub {
    /// Print ready-to-paste MCP server config JSON for a specific client.
    PrintConfig(PrintConfigArgs),
    /// Write the MCP config directly to the right file for a client.
    Install(InstallArgs),
    /// List all MCP tools (name, channel, operation, description).
    Tools,
}

#[derive(Args)]
struct PrintConfigArgs {
    /// Which MCP client to generate config for.
    #[arg(long, short)]
    client: Client,
    /// Transport: stdio (default) or http (remote server).
    #[arg(long, short, default_value = "stdio")]
    transport: String,
    /// HTTP server address (only used with --transport http).
    #[arg(long, short)]
    addr: Option<String>,
}

#[derive(Args)]
struct InstallArgs {
    /// Which MCP client to install the config for.
    #[arg(long, short)]
    client: Client,
    /// Transport: stdio (default) or http (remote server).
    #[arg(long, short, default_value = "stdio")]
    transport: String,
    /// HTTP server address (only used with --transport http).
    #[arg(long, short)]
    addr: Option<String>,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum Client {
    /// Claude Code / Claude Desktop
    #[value(name = "claude-code")]
    ClaudeCode,
    /// Cursor IDE
    #[value(name = "cursor")]
    Cursor,
    /// Windsurf (Codeium)
    #[value(name = "windsurf")]
    Windsurf,
    /// Cline (VS Code extension)
    #[value(name = "cline")]
    Cline,
    /// Zed editor
    #[value(name = "zed")]
    Zed,
    /// Generic JSON (print only, no known install path)
    #[value(name = "json")]
    Json,
}

/// Build the MCP server config JSON for a client + transport.
fn server_config(client: &Client, transport: &str, addr: &Option<String>) -> Value {
    let bin = "agentspan-mcp";
    match transport {
        "http" => {
            let url = format!("http://{}", addr.as_deref().unwrap_or("localhost:9000"));
            json!({
                "mcpServers": {
                    "agentspan": {
                        "url": url,
                        "transport": "http"
                    }
                }
            })
        }
        _ => {
            // stdio: all clients use the same shape, just different file paths.
            let _ = client; // suppress unused warning
            json!({
                "mcpServers": {
                    "agentspan": {
                        "command": bin,
                        "args": []
                    }
                }
            })
        }
    }
}

/// Where each client stores its MCP config.
fn config_path(client: &Client) -> Option<PathBuf> {
    let home = dirs_home()?;
    match client {
        Client::ClaudeCode => Some(home.join(".claude.json")),
        Client::Cursor => Some(home.join(".cursor").join("mcp.json")),
        Client::Windsurf => Some(
            home.join(".codeium")
                .join("windsurf")
                .join("mcp_config.json"),
        ),
        Client::Cline => Some(home.join(".cline").join("mcp_config.json")),
        Client::Zed => Some(home.join(".zed").join("settings.json")),
        Client::Json => None,
    }
}

/// Best-effort home directory (works on Windows, macOS, Linux).
fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Print the config JSON to stdout.
fn print_config(client: &Client, transport: &str, addr: &Option<String>) {
    let config = server_config(client, transport, addr);
    println!("{}", serde_json::to_string_pretty(&config).unwrap());
}

/// Write the config to the client's config file. Creates parent dirs.
fn install_config(client: &Client, transport: &str, addr: &Option<String>) -> anyhow::Result<()> {
    let path = config_path(client).ok_or_else(|| {
        anyhow::anyhow!("no known config path for this client; use 'print-config' instead")
    })?;

    // Read existing config if present, merge our server entry in.
    let mut existing = if path.exists() {
        let raw = std::fs::read_to_string(&path)?;
        serde_json::from_str::<Value>(&raw).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };

    let new_config = server_config(client, transport, addr);
    if let Some(servers) = new_config["mcpServers"].as_object() {
        let existing_obj = existing
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("existing config is not a JSON object"))?;
        let entry = existing_obj
            .entry("mcpServers")
            .or_insert_with(|| json!({}));
        if let Some(entry_obj) = entry.as_object_mut() {
            for (k, v) in servers {
                entry_obj.insert(k.clone(), v.clone());
            }
        }
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(&existing)?)?;
    println!("Installed agentspan MCP config to: {}", path.display());
    if let Some(parent) = path.parent() {
        println!("Restart your client to pick up the new server.");
        let _ = parent; // suppress unused
    }
    Ok(())
}

/// Print all MCP tools as a table.
fn print_tools() {
    use agentspan_mcp::{Op, TOOLS};
    println!("{:<24} {:<16} {:<8} DESCRIPTION", "TOOL", "CHANNEL", "OP");
    println!("{}", "-".repeat(80));
    for tool in TOOLS {
        let op = match tool.op {
            Op::Read => "read",
            Op::Search => "search",
            Op::Doctor => "doctor",
        };
        println!(
            "{:<24} {:<16} {:<8} {}",
            tool.name, tool.channel, op, tool.description
        );
    }
    println!("\n{} tools total", TOOLS.len());
}

pub async fn run(args: McpArgs) -> anyhow::Result<()> {
    match args.command {
        McpSub::PrintConfig(a) => {
            print_config(&a.client, &a.transport, &a.addr);
            Ok(())
        }
        McpSub::Install(a) => install_config(&a.client, &a.transport, &a.addr),
        McpSub::Tools => {
            print_tools();
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stdio_config_has_correct_shape() {
        let config = server_config(&Client::ClaudeCode, "stdio", &None);
        assert_eq!(
            config["mcpServers"]["agentspan"]["command"],
            "agentspan-mcp"
        );
    }

    #[test]
    fn http_config_includes_url() {
        let config = server_config(&Client::Cursor, "http", &Some("localhost:9000".to_string()));
        assert_eq!(
            config["mcpServers"]["agentspan"]["url"],
            "http://localhost:9000"
        );
    }

    #[test]
    fn config_path_returns_path_for_known_clients() {
        // We can't assert the exact path without a home dir, but it should be Some.
        if dirs_home().is_some() {
            assert!(config_path(&Client::ClaudeCode).is_some());
            assert!(config_path(&Client::Cursor).is_some());
        }
    }

    #[test]
    fn config_path_is_none_for_json_client() {
        assert!(config_path(&Client::Json).is_none());
    }
}
