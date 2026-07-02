# Getting Started

## Install

Build from source (the quickest path while pre-1.0):

```bash
cargo install --path crates/agentspan-cli
cargo install --path crates/agentspan-mcp
agentspan install --env auto      # installs upstream tools for zero-config channels
agentspan doctor                  # shows every channel's status + active backend
```

Or run with Docker (see [Deployment](deployment.md)).

## Run the gateway

```bash
agentspan serve                   # REST API + SSE on http://localhost:8080
# or
docker compose up --build         # full stack: API + UI + Redis + Prometheus + Grafana
```

## Wire up your AI agent

```bash
# One command — writes the MCP config to the right file automatically:
agentspan mcp install --client claude-code    # or: cursor, windsurf, cline

# See all 91 tools:
agentspan mcp tools
```

See the full guides: [Claude Code](../guides/use-with-claude-code.md) ·
[Cursor](../guides/use-with-cursor.md) · [Windsurf](../guides/use-with-windsurf.md).

## First calls

```bash
# smart read (auto-detects the channel; falls back to the web reader)
curl "localhost:8080/api/v1/read?url=https://example.com"

# search a platform
curl "localhost:8080/api/v1/channels/hackernews/search?q=rust"

# batch read in parallel
curl -X POST localhost:8080/api/v1/batch/read \
  -H 'content-type: application/json' \
  -d '{"urls":["https://a.com","https://b.com"]}'

# federated search across multiple channels
curl -X POST localhost:8080/api/v1/search/federated \
  -H 'content-type: application/json' \
  -d '{"query":"rust async","channels":["hackernews","reddit","lobsters"]}'

# OpenAPI spec + Swagger UI
open http://localhost:8080/docs
```

## Next steps

- Add credentials for Tier-1 channels — see the [setup guides](../guides/).
- Pick an [SDK](sdks.md) for your language.
- Read the [API reference](api-reference.md).
- Browse the [channel list](channels.md).
