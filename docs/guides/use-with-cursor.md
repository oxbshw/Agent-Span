# Use AgentSpan with Cursor

AgentSpan gives Cursor web access through 91 MCP tools.

## Setup (one command)

```bash
agentspan mcp install --client cursor
```

This writes the MCP server config to `~/.cursor/mcp.json`. Restart Cursor to
pick up the new server.

## Manual setup

```bash
agentspan mcp print-config --client cursor
```

Paste the output into `~/.cursor/mcp.json`:

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

In Cursor's chat, ask:

> Search Hacker News for "rust async" and summarize the top results

Cursor will call `hackernews_search` via AgentSpan.

## Available tools

```bash
agentspan mcp tools
```

## Troubleshooting

- **Tools not showing up** — restart Cursor after installing the config.
- **MCP panel empty** — check `~/.cursor/mcp.json` is valid JSON.
- **Channel returns error** — run `agentspan doctor`.
