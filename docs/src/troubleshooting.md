# Troubleshooting

## Build issues

### Slow builds on Windows (windows-gnu)

A clean `cargo build` can take ~10 minutes on `x86_64-pc-windows-gnu`. Use
`cargo check` or `cargo clippy` during development — they're much faster once
dependencies are built. On Linux/macOS, builds are typically under 2 minutes.

### WebSocket feature doesn't build on windows-gnu

axum's `ws` feature pulls `tungstenite → rand 0.9 → getrandom 0.3`, which fails
to link on windows-gnu. WebSocket is gated behind the `websocket` cargo feature
(off by default). SSE (`/api/v1/events/stream`) works everywhere and is the
default real-time transport.

```bash
# Build with WebSocket on Linux/macOS:
cargo build -p agentspan-api --features websocket
```

## Channel issues

### A channel shows `missing` in doctor

The channel's backend CLI tool isn't installed. Run:

```bash
agentspan install --channels <name>
agentspan doctor
```

### A channel shows `broken` in doctor

The CLI tool is installed but can't execute (stale venv, missing runtime, etc).
Run `agentspan install --channels <name>` to reinstall, or check
`agentspan doctor --json` for the specific error message.

### Tier-1 channels need API keys

Discord, Telegram, Spotify, Twitch, Podcast Index, OpenAI, Anthropic, Brave,
Bing, Google, Notion, Slack, and Scholar all require API keys. Set them via:

```bash
agentspan config set openai_api_key sk-...
```

Or via environment variables (see `.env.example`).

### Reddit has no zero-config path

Reddit blocks anonymous `.json` endpoints (403). Every backend needs a logged-in
session. Use OpenCLI (desktop + Chrome) or rdt-cli with cookies.

### Bilibili yt-dlp is blocked

Bilibili's risk-control system blocks yt-dlp (412 error). Use `bili-cli`
instead: `agentspan install --channels bilibili`.

## Runtime issues

### `503 Service Unavailable`

The gateway is at capacity (1024 concurrent requests). Scale horizontally or
increase `MAX_CONCURRENT_REQUESTS` in `agentspan-api/src/lib.rs`.

### `429 Too Many Requests`

You've hit your tenant's rate limit. The response includes a `Retry-After` header.
Wait and retry, or ask an admin to raise your tenant's quota.

### MCP tools not showing up in Claude Code / Cursor

1. Run `agentspan mcp install --client claude-code` (or `cursor`).
2. Restart the client.
3. Verify with `agentspan mcp tools` that the binary works.

### SSE stream connects but no events

Events are only published when requests hit the API. Make a request
(`curl localhost:8080/api/v1/stats`) and you should see an event.

## Docker issues

### `docker compose up` fails

Make sure nothing is already using ports 8080, 3000, 9090, or 3001. The
compose file runs: API (8080), UI (3000), Prometheus (9090), Grafana (3001),
Redis (6379), PostgreSQL (5432).

### Dashboard not accessible

The UI service runs on port 3000. If you built only the API, run the dashboard
separately:

```bash
cd web && npm install && npm run dev
```
