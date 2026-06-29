# Integrations

AgentSpan integrates with AI coding tools, IDEs, and CI pipelines.

## MCP client configs

Ready-to-paste MCP server configs for popular AI clients:

| Client | Config file | Status |
|--------|------------|--------|
| Claude Code | `~/.claude.json` | ✅ Ready |
| Cursor | `~/.cursor/mcp.json` | ✅ Ready |
| Windsurf | `~/.codeium/windsurf/mcp_config.json` | ✅ Ready |
| Cline | `~/.cline/mcp_config.json` | ✅ Ready |
| Remote (HTTP) | any client | ✅ Ready |

Quick install: `agentspan mcp install --client claude-code`

See [`mcp-clients/`](mcp-clients/) for the JSON files and
[README](mcp-clients/README.md) for details.

## IDE plugins

| Plugin | Dir | Status |
|--------|-----|--------|
| VS Code | `vscode/` | 🚧 Scaffold |
| JetBrains | `jetbrains/` | 🚧 Scaffold |
| Neovim | `nvim/` | 🚧 Scaffold |

IDE plugins are source scaffolds — they work against a running AgentSpan API
but are not yet published to marketplaces.

## CI integrations

| Integration | Dir | Status |
|-------------|-----|--------|
| GitHub Action | `github-action/` | ✅ Usable |
| pre-commit hook | `pre-commit/` | ✅ Usable |

The GitHub Action (`agentspan-read`) reads a URL via a running AgentSpan
instance. The pre-commit hook checks Markdown links.
