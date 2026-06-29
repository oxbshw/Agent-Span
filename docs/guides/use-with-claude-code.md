# Use AgentSpan with Claude Code

AgentSpan gives Claude Code web access through 92 MCP tools — read any URL,
search 52 platforms, check channel health.

## Setup (one command)

```bash
agentspan mcp install --client claude-code
```

This writes the MCP server config to `~/.claude.json`. Restart Claude Code to
pick up the new server.

## Manual setup

If you prefer to edit the config yourself:

```bash
agentspan mcp print-config --client claude-code
```

Paste the output into the `mcpServers` section of `~/.claude.json`:

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

## Install the skill file

```bash
agentspan skill install
```

This writes a skill file to `~/.claude/skills/agentspan.md` that teaches
Claude Code when to use each channel.

## Verify it works

```bash
agentspan doctor
```

Then in Claude Code, ask:

> Read https://news.ycombinator.com and summarize the top 3 stories

Claude Code will call `web_read` or `hackernews_read` via AgentSpan.

## Available tools

```bash
agentspan mcp tools
```

92 tools across 52 channels: `web_read`, `github_read`, `youtube_subtitles`,
`hackernews_search`, `reddit_search`, `arxiv_search`, `wikipedia_read`,
`spotify_read`, and more.

## Troubleshooting

- **"command not found: agentspan-mcp"** — install with `cargo install
  --path crates/agentspan-mcp` or download a release binary.
- **Tools not showing up** — restart Claude Code after installing the config.
- **Channel returns error** — run `agentspan doctor` to see which channels
  need API keys or CLI tools installed.
