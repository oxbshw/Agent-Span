# Install AgentSpan

Paste this whole file's URL to your AI agent (Claude Code, Cursor, OpenClaw,
Windsurf…) and it will install AgentSpan for you:

```
Install AgentSpan for me: https://raw.githubusercontent.com/oxbshw/Agent-Span/main/docs/install.md
```

## What the agent will do

1. **Install the CLI** — build from source (`cargo install --path crates/agentspan-cli`)
   or grab a release binary from the [Releases](https://github.com/oxbshw/Agent-Span/releases) page.
2. **Install upstream tools** — `agentspan install --env auto` detects local vs.
   server and installs what the zero-config channels need (gh, node, yt-dlp, mcporter).
   - Safe preview: `agentspan install --safe` (shows what's needed, changes nothing)
   - Dry run: `agentspan install --dry-run`
   - Behind a proxy: `agentspan install --proxy http://user:pass@host:port`
   - More channels: `agentspan install --channels twitter,reddit,bilibili` (or `all`)
3. **Register the agent skill** — `agentspan skill install` writes `SKILL.md` to your
   agent's skills directory so it knows when to call which channel.
4. **Verify** — `agentspan doctor` prints every channel's status and active backend.

## Run the gateway

```bash
agentspan serve          # REST API + SSE on http://localhost:8080
# or
docker compose up        # API + Redis + PostgreSQL
```

## Add credentials (only for Tier-1 channels)

Tell your agent "set up Discord/Spotify/Twitter/…" — it will follow the matching
[setup guide](guides/). Most platforms use the Cookie-Editor flow or a free API key.

See the [README](../README.md) and [API reference](api-reference.md) for everything else.
