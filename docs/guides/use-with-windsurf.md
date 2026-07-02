# Use AgentSpan with Windsurf

AgentSpan gives Windsurf web access through 91 MCP tools.

## Setup (one command)

```bash
agentspan mcp install --client windsurf
```

This writes the MCP server config to `~/.codeium/windsurf/mcp_config.json`.
Restart Windsurf to pick up the new server.

## Manual setup

```bash
agentspan mcp print-config --client windsurf
```

Paste the output into `~/.codeium/windsurf/mcp_config.json`:

```json
{
  "mcpServers": {
    "agentspan": {
      "command": "agentspan-mcp",
      "args": []
    }
  }
}
```

## Verify it works

In Windsurf's chat, ask:

> Read https://github.com/tokio-rs/tokio and summarize what this repo does

Windsurf will call `github_read` via AgentSpan.

## Available tools

```bash
agentspan mcp tools
```
