# AgentSpan REST API Reference

The canonical contract for every AgentSpan SDK. All SDKs (Python, TypeScript, Go,
Rust, Ruby, Java, PHP, C#, Swift) implement the same method set against these
endpoints. Base URL defaults to `http://localhost:8080`.

## Authentication

- Single-user mode (default): no key required.
- Multi-tenant mode (`auth.require_api_key = true`): send `X-API-Key: <key>`.
- `401` on missing/invalid key; `429` with `Retry-After` when rate-limited.

## Conventions

- Request/response bodies are JSON.
- Errors return `{ "error": "<message>", ... }` with a non-2xx status (handlers
  that embed `{"error":...}` in a 200 body are noted).
- Status mapping for SDK exceptions: `401 → AuthenticationError`,
  `429 → RateLimitError(retry_after)`, other `4xx/5xx → APIError(status, message)`,
  embedded `{"error"}` → `ChannelError`.
- Every response carries an `x-trace-id` header. Send your own
  `x-trace-id` (or `x-request-id`) to correlate a request across logs and
  audit events; otherwise the server generates one.
- The server applies a 2 MiB request-body limit and a global concurrency
  limit, shedding excess load with `503` (`{ "error": "server at capacity" }`).

## Endpoints

### Health
`GET /health` → `{ "status": "ok" }` (public, no auth).

### Metrics
`GET /metrics` → Prometheus text exposition (`text/plain; version=0.0.4`),
public, no auth. Exposes `agentspan_requests_total`,
`agentspan_request_errors_total`, `agentspan_requests_rejected_total`,
`agentspan_request_latency_ms_sum`, and the `agentspan_channels` gauge.

### Read (smart, auto-detect channel)
`GET /api/v1/read?url=<url>&force_refresh=<bool>`
→ `{ "channel": "...", "content": { url, title, body, metadata, cached } }`
or `{ "error": "no channel can handle this URL", "url": ... }` (200 body).

`POST /api/v1/read` body `{ "url": "...", "force_refresh": false }`
→ same content shape; `422` if no channel matches; `502` on backend failure.

The `web` channel handles any `http(s)` URL, so unknown hosts fall back to it.

### Per-channel operations
- `GET /api/v1/channels` → `{ "channels": [ { name, description, tier } ] }`
- `GET /api/v1/channels/{name}` → `{ name, description, tier, backends:[{backend,status,message,latency_ms}] }`
- `GET /api/v1/channels/{name}/read?url=&force_refresh=` → `{ channel, content }`
- `GET /api/v1/channels/{name}/search?q=&limit=` → `{ channel, results:[{title,url,snippet,author,timestamp,metadata}] }`

### Batch (parallel)
- `POST /api/v1/batch/read` body `{ "urls": [...], "force_refresh": false }`
  → `{ "count": N, "results": [ { url, ok, channel?, content?|error? } ] }`.
  Max 50 URLs (`422` otherwise). One failure never sinks the batch.
- `POST /api/v1/batch/search` body `{ "channel": "...", "queries": [...], "limit": 10 }`
  → `{ channel, count, results:[ { query, ok, results?|error? } ] }`. `422` if the
  channel is unknown or the batch exceeds 50 queries.

### Federated search (many channels at once)
- `POST /api/v1/search/federated` body `{ "query": "...", "channels": ["web","reddit"]?, "limit": 10 }`
  → `{ query, searched:[names], errors:[{channel,error}], results:[{channels:[names],title,url,snippet}] }`.
  Channels are queried concurrently; identical URLs are de-duplicated (sources
  merged) and results found by more channels rank first. Omit `channels` to
  search them all; a single channel's failure is reported in `errors`, not fatal.

### Diagnostics
- `GET /api/v1/doctor` → full health report across all channels.
- `GET /api/v1/doctor/{channel}` → health report for one channel.
- `GET /api/v1/stats` → `{ channels, audit_entries, tenants, recent }`.
- `GET /api/v1/config` → non-secret configuration view.

### Live events (SSE)
`GET /api/v1/events/stream` → `text/event-stream`. First frame:
`{ "type": "hello", "channels": N }`, then one frame per server event, e.g.
`{ "type":"request", "method", "path", "channel", "status", "latency_ms" }`.
Browser clients use `EventSource` (auto-reconnect). Non-browser SDKs read the
chunked stream line-by-line (`data: <json>`).

### Admin (requires `auth.require_api_key=true` + Admin scope)
- `POST /api/v1/auth/keys` body `{ name, scopes:[...], tenant_id }` → `{ id, secret, ... }`
- `GET /api/v1/auth/keys` → `[ { id, name, scopes, ... } ]`
- `DELETE /api/v1/auth/keys/{id}` → `204`
- `GET /api/v1/admin/audit-log` → recent audit entries.
- `GET /api/v1/admin/healing-report` → self-healing status.
- `GET /api/v1/admin/auto-switches` → backend auto-switch log.
- `POST /api/v1/admin/repair-channel` body `{ tool, kind? }` → repair attempt.
- `GET /api/v1/admin/performance-report` → per-channel/backend latency.
- `GET /api/v1/admin/analytics` → usage totals + per-channel stats.

### Agent memory
- `PUT /api/v1/memory/{namespace}/{key}` body `{ "value": "...", "ttl_seconds": 300 }`
- `GET /api/v1/memory/{namespace}/{key}` → stored value
- `DELETE /api/v1/memory/{namespace}/{key}` → `204`
- `GET /api/v1/memory/{namespace}` → list of live keys

### Suggestions
`GET /api/v1/suggestions` → actionable recommendations derived from usage
(cache TTL tweaks, faster backends, platforms worth adding).

### OpenAPI
- `GET /openapi.json` → OpenAPI 3.0.3 spec (machine-readable).
- `GET /docs` → Swagger UI (interactive).

### WebSocket (feature-gated)
`GET /ws/v1/stream` — bidirectional event stream. Auth via `?token=<api-key>`
query parameter. Available when built with `--features websocket`. SSE at
`/api/v1/events/stream` is the default and works everywhere.

## Canonical SDK method set

Every SDK exposes (names adapted to language idiom):

| Method | Endpoint |
|---|---|
| `read(url, force_refresh=false)` | `GET /api/v1/read` |
| `search(channel, query, limit=10)` | `GET /api/v1/channels/{c}/search` |
| `list_channels()` | `GET /api/v1/channels` |
| `doctor()` | `GET /api/v1/doctor` |
| `get_config()` | `GET /api/v1/config` |
| `batch_read(urls, force_refresh=false)` | `POST /api/v1/batch/read` |
| `batch_search(channel, queries, limit=10)` | `POST /api/v1/batch/search` |
| `health()` | `GET /health` |
| `create_key(name, scopes, tenant_id)` | `POST /api/v1/auth/keys` |
| `revoke_key(id)` | `DELETE /api/v1/auth/keys/{id}` |
| `stream_events(on_event)` | `GET /api/v1/events/stream` (SSE) |
