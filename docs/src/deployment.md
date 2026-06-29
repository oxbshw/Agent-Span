# Deployment

## Docker (recommended)

```bash
docker compose up --build
```

This starts the full stack:

| Service | Port | Purpose |
|---------|------|---------|
| api | 8080 | AgentSpan REST API + SSE + OpenAPI |
| ui | 3000 | React marketing site + dashboard |
| redis | 6379 | L3 cache |
| postgres | 5432 | (reserved for future use) |
| prometheus | 9090 | Metrics scraper |
| grafana | 3001 | Dashboard (admin/admin) |

The API image is also published to GHCR as `ghcr.io/oxbshw/Agent-Span`.

## Bare binary

```bash
cargo install --path crates/agentspan-cli
agentspan serve
```

## Production checklist

- **Auth**: set `auth.require_api_key=true` and mint scoped keys
  (`POST /api/v1/auth/keys`). Admin routes return `403` until this is enabled.
- **TLS**: terminate TLS in front of the gateway (reverse proxy).
- **Cache L3**: point `cache.l3_url` at Redis for a shared, durable cache tier.
- **Multi-tenant**: create tenants with their own channel allow-lists, quotas, and
  cache policy.
- **Proxy**: `agentspan config set proxy.url http://user:pass@host:port` for
  restricted networks; all HTTP backends honor it.
- **Observability**: every request is audited and streamed over
  `/api/v1/events/stream` (SSE). Prometheus scrapes `/metrics`.
- **OpenAPI**: the spec is at `/openapi.json`; Swagger UI at `/docs`.
- **WebSocket**: build with `--features websocket` if you need bidirectional
  streaming (SSE is the default and works everywhere).

## Configuration

Layered via `~/.agentspan/config.yaml` and `AGENTSPAN_*` env vars (see
`.env.example` for all supported variables). The file is written `0600`; secrets
are masked in `agentspan config` output. View the non-secret config at
`GET /api/v1/config`.

## MCP server (for AI agents)

### stdio (local)

```bash
agentspan-mcp                # stdio transport (default)
```

### HTTP (remote / multi-tenant)

```bash
agentspan-mcp --http 0.0.0.0:9000
```

Build with: `cargo build -p agentspan-mcp --features http`

### Auto-install for AI clients

```bash
agentspan mcp install --client claude-code    # writes to ~/.claude.json
agentspan mcp install --client cursor         # writes to ~/.cursor/mcp.json
agentspan mcp install --client windsurf       # writes to ~/.codeium/windsurf/mcp_config.json
```
