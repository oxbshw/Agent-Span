# MCP Client Configs

Ready-to-paste MCP server configs for popular AI coding clients.

## Quick install (recommended)

```bash
agentspan mcp install --client claude-code    # writes to ~/.claude.json
agentspan mcp install --client cursor         # writes to ~/.cursor/mcp.json
agentspan mcp install --client windsurf       # writes to ~/.codeium/windsurf/mcp_config.json
```

Or print the config and paste it yourself:

```bash
agentspan mcp print-config --client claude-code
```

## Manual config files

| Client | File | Config |
|--------|------|--------|
| Claude Code | `~/.claude.json` | [`claude-code.json`](claude-code.json) |
| Cursor | `~/.cursor/mcp.json` | [`cursor.json`](cursor.json) |
| Windsurf | `~/.codeium/windsurf/mcp_config.json` | [`windsurf.json`](windsurf.json) |
| Cline | `~/.cline/mcp_config.json` | [`cline.json`](cline.json) |
| Remote (HTTP) | any client | [`http-remote.json`](http-remote.json) |

## HTTP transport (remote / multi-tenant)

For remote or multi-tenant setups, run the MCP server over HTTP:

```bash
agentspan-mcp --http 0.0.0.0:9000
```

Then point your client at `http://your-server:9000/mcp` using the
[`http-remote.json`](http-remote.json) config.

## See all available tools

```bash
agentspan mcp tools
```

This prints all 92 MCP tools with their channel, operation, and description.
