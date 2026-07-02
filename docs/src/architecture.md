# Architecture

AgentSpan is an async-Rust workspace: nine crates, each with one
responsibility, tied together by two core traits (`Channel` and `Backend`).

## Crate map

```
┌─────────────────────────────────────────────────────────────┐
│                      agentspan-cli                          │
│   The `agentspan` binary: serve, doctor, mcp, skill, ...    │
├─────────────────────────────────────────────────────────────┤
│  agentspan-api            │  agentspan-mcp                  │
│  Axum REST + SSE +        │  JSON-RPC 2.0 over stdio/HTTP   │
│  OpenAPI + WebSocket      │  91 tools → channel registry     │
├─────────────────────────────────────────────────────────────┤
│  agentspan-auth           │  agentspan-channels              │
│  SHA-256 keys, tenants,   │  52 channel impls + registry +   │
│  RBAC, rate limit, audit  │  healer + content intelligence   │
├─────────────────────────────────────────────────────────────┤
│  agentspan-router         │  agentspan-cache                 │
│  Parallel probe, retry,   │  L1 memory / L2 disk / L3 Redis  │
│  circuit breaker, scoring │  + singleflight + optimizer      │
├─────────────────────────────────────────────────────────────┤
│  agentspan-probe          │  agentspan-core                  │
│  6-state health classify  │  Channel/Backend traits, types,  │
│  via tokio::process        │  config, errors                  │
└─────────────────────────────────────────────────────────────┘
```

## Core traits

### Channel

A web platform (Hacker News, GitHub, YouTube, ...). Each channel implements:

- `name()` — stable identifier (cache namespace, MCP tool prefix)
- `description()` — human/LLM-readable
- `can_handle(url)` — does this URL belong to this channel?
- `tier()` — Zero (no config) or One (needs API key/cookies)
- `backends()` — ordered list of backends to try
- `read(url, opts)` — fetch content from a URL
- `search(query, opts)` — keyword search on the platform
- `check_health()` — probe all backends (default impl probes each and
  collects latency + status)
- `format_for_llm(raw)` — reduce tokens before handing text to an LLM
  (default: pass through)

### Backend

A concrete access method for a channel (CLI tool, HTTP API, browser session).
Each backend implements:

- `name()` — e.g. "gh-cli", "jina", "opencli-reddit"
- `probe()` — is this backend healthy right now? Returns one of 6 states:
  `Ok`, `Warn`, `Missing`, `Broken`, `Timeout`, `Error`
- `read(url, opts)` — fetch via this backend
- `search(query, opts)` — search via this backend

## Request flow

```
Client (SDK / MCP / curl / dashboard)
  │
  ▼
┌──────────────┐
│  Auth        │  API key check → tenant → rate limit → audit
│  middleware  │
└──────┬───────┘
       │
       ▼
┌──────────────┐
│  Observe     │  Trace ID, metrics, concurrency limit (1024)
│  middleware  │
└──────┬───────┘
       │
       ▼
┌──────────────┐
│  Handler     │  e.g. POST /api/v1/read → smart_read()
└──────┬───────┘
       │
       ▼
┌──────────────┐
│  Router      │  1. Check cache (L1 → L2 → L3)
│              │  2. If miss: select_ordered() — probe all
│              │     backends in parallel, Ok before Warn
│              │  3. Skip backends with open circuit breaker
│              │  4. Retry with exponential backoff + jitter
│              │  5. Write result back to cache
│              │  6. Record outcome (scorer + circuit breaker)
└──────┬───────┘
       │
       ▼
┌──────────────┐
│  Backend     │  HTTP API / CLI tool / Jina Reader
└──────────────┘
```

## Health model

The probe engine runs a command (default: `--version`) via
`tokio::process::Command` with a timeout and classifies the result into one of
six states:

| State | Meaning | Example |
|-------|---------|---------|
| Ok | Healthy, responsive | `gh --version` exits 0, prints version |
| Warn | Works but degraded | OpenCLI installed but Chrome extension not connected |
| Missing | Binary not on PATH | `yt-dlp` not installed |
| Broken | Binary exists but can't run | Stale pipx venv shebang |
| Timeout | Command exceeded timeout | `gh --version` hung for >5s |
| Error | Other failure | Non-zero exit, parse error |

The router prefers `Ok` backends, falls back to `Warn`, and skips
`Missing`/`Broken`/`Timeout`/`Error`. When a circuit breaker is enabled,
backends that fail repeatedly are short-circuited (Open) and skipped without
even probing — they recover to Half-Open after a cooldown and to Closed after
consecutive successes.

## Cache tiers

| Tier | Storage | Latency | Survives | Use case |
|------|---------|---------|----------|----------|
| L1 | In-process memory (DashMap) | ~0ms | Request | Hot reads — same URL hit many times |
| L2 | Disk (file-per-key, async fs) | ~1ms | Restart | Warm reads — persistent across restarts |
| L3 | Redis (feature-gated) | ~1ms | Restart | Multi-instance — shared cache across nodes |

The `CacheManager` orchestrates all three: check L1 → L2 → L3, write back on
a miss. TTLs are per-tier (Hot/Warm/Cold presets or custom). A background
sweeper evicts expired entries. `SingleFlight` coalesces concurrent identical
reads into one upstream fetch.

## Self-healing

A background `HealthMonitor` probes every channel on an interval. When a
channel's active backend fails repeatedly:

1. **Auto-switch**: the `BackendSwitcher` moves the channel to the next
   healthy backend and logs the switch.
2. **Repair**: the `RepairManager` can reinstall broken CLI tools via
   `pip`/`npm`/`cargo` (rate-limited to 3 attempts/hour/tool).
3. **Alert**: the `AlertManager` fires a webhook (Discord/Slack) if a channel
   stays down past a grace window (1 per channel per hour).

Surfaced at `GET /api/v1/admin/healing-report`, `GET /api/v1/admin/auto-switches`,
and `POST /api/v1/admin/repair-channel`.

## Self-improving analytics

The gateway observes its own traffic and makes recommendations:

- **Profiler**: EWMA + p99 latency per channel/backend. Flags backends over
  1s p99 and suggests a faster sibling.
- **Cache optimizer**: tunes TTL per channel from observed hit rates.
- **Adaptive rate limiter**: learns per-platform limits from observed 429s.
- **Missing-channel detector**: ranks requested-but-unsupported platforms by
  frequency.

All feed `GET /api/v1/suggestions` — actionable recommendations an operator
or agent can act on.

## MCP server

The MCP server (`agentspan-mcp`) exposes 91 tools over JSON-RPC 2.0
(protocol version `2024-11-05`):

- **stdio** (default): newline-delimited JSON-RPC over stdin/stdout. For
  Claude Code, Cursor, Windsurf, Cline, Zed.
- **HTTP** (feature-gated): `POST /mcp` for request/response, `GET /mcp/sse`
  for SSE. For remote/multi-tenant setups.

Each tool maps to a channel + operation (`Read`, `Search`, or `Doctor`).
The dispatcher calls through the same `ChannelRegistry` the REST API uses,
so MCP and REST always see the same channels and backends.

## Configuration

Layered (highest precedence first):

1. Code overrides (`Config { ... }` in Rust)
2. Profile YAML (`~/.agentspan/profiles/production.yaml`)
3. Base YAML (`~/.agentspan/config.yaml`)
4. Environment variables (`AGENTSPAN_SERVER__PORT=8080`)
5. Defaults

The file is written `0600`; secrets are masked in `/api/v1/config` output.
See `.env.example` for all supported environment variables.
