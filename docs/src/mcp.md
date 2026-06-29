# MCP Integration

AgentSpan ships a native **Model Context Protocol** server (`agentspan-mcp`) that
exposes **92 tools** — read + search for every channel, plus `doctor` — dispatched
through the channel registry.

## Quick setup

```bash
# One command — writes the config to the right file for your client:
agentspan mcp install --client claude-code    # or: cursor, windsurf, cline

# Or print the config and paste it yourself:
agentspan mcp print-config --client cursor

# See all 92 tools:
agentspan mcp tools
```

See the full guides: [Claude Code](../guides/use-with-claude-code.md) ·
[Cursor](../guides/use-with-cursor.md) · [Windsurf](../guides/use-with-windsurf.md).

## Manual config

Add this to your MCP client config:

```json
{
  "mcpServers": {
    "agentspan": { "command": "agentspan-mcp" }
  }
}
```

Pre-built configs are in [`integrations/mcp-clients/`](../../integrations/mcp-clients/).

## HTTP transport (remote / multi-tenant)

For remote or multi-tenant setups, run the MCP server over HTTP:

```bash
agentspan-mcp --http 0.0.0.0:9000
```

Then point your client at `http://your-server:9000/mcp`.

## Skill file

Generate and install the skill so the agent knows when to use each channel:

```bash
agentspan skill install
```

## Protocol

JSON-RPC 2.0 (protocol version `2024-11-05`). Methods: `initialize`,
`tools/list`, `tools/call`, `ping`. Transport: stdio (default) or HTTP/SSE
(behind the `http` cargo feature).
