# Changelog

All notable changes to AgentSpan are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.0] - 2026-06-29

### Added

- **TikTok and Instagram channels** (registry now serves **52**): tiktok uses
  yt-dlp for video metadata (Tier 0); instagram uses OpenCLI + instaloader
  (Tier 1). MCP tools: tiktok_read, instagram_read, instagram_search.
- **OpenAPI 3.0 spec** via utoipa: `GET /openapi.json` serves a machine-readable
  spec; `GET /docs` serves Swagger UI. 16 handlers annotated, 7 component
  schemas registered.
- **WebSocket transport** (feature-gated): `/ws/v1/stream` mirrors the SSE event
  fan-out. Gated behind the `websocket` cargo feature (off by default so the
  build stays clean on windows-gnu). SSE remains the default.
- **MCP HTTP/SSE transport** (feature-gated): `POST /mcp` handles JSON-RPC over
  HTTP; `GET /mcp/sse` streams the SSE endpoint. Run with
  `agentspan-mcp --http [addr]`. Gated behind the `http` cargo feature.
- **`agentspan mcp` CLI command**: one-command MCP client setup.
  `agentspan mcp install --client claude-code` writes the config to the right
  file automatically. `agentspan mcp print-config` prints ready-to-paste JSON.
  `agentspan mcp tools` lists all 91 tools. Supports Claude Code, Cursor,
  Windsurf, Cline, Zed.
- **MCP client configs**: `integrations/mcp-clients/` ships 5 ready-to-paste
  JSON files (claude-code, cursor, windsurf, cline, http-remote) + README.
- **Agent integration guides**: `docs/guides/use-with-claude-code.md`,
  `use-with-cursor.md`, `use-with-windsurf.md` — step-by-step setup, verify,
  and troubleshooting.
- **Integration test suite** (`tests/integration/`): 17 tests that boot the real
  API on an ephemeral port and exercise /health, /metrics, /openapi.json, /docs,
  /channels, /doctor, /read, /stats, /config, auth flow (create/use/revoke key),
  SSE stream.
- **Load test suite** (`tests/load/`): k6 script targeting 1000 RPS / 30s /
  p99<200ms / <1% errors. CI workflow runs it against a containerized API.
- **Coverage gate**: `cargo tarpaulin --fail-under 80` in CI (Linux).
- **Circuit breaker wired into router hot path**: `BackendRouter::with_circuit_breaker()`
  skips backends whose circuit is open, records success/failure, and reopens
  after a cooldown. 3 new tests.
- **Prometheus + Grafana in docker-compose**: prometheus service scrapes
  `api:8080/metrics`; grafana service with auto-provisioned datasource +
  AgentSpan dashboard (request rate, p99, cache hit ratio).
- **Dockerfile.ui**: multi-stage build for the React dashboard (node build ->
  nginx serve). `ui` service in docker-compose on port 3000.
- **DX files**: `.env.example` (all channel env vars), `justfile` (15 dev tasks),
  `.github/dependabot.yml` (cargo/npm/actions/pip weekly), filled
  `scripts/setup-dev.sh`, `test-all.sh`, `release.sh`.
- **Marketing site** (`web/`, route `/`): React 18 + Vite + GSAP + Lenis, pure CSS.
- **Status dashboard** (route `/status`): channel grid, health table, performance
  charts, install snippet.

### Changed

- `check_health` is now a default trait method on `Channel` — removed 49x
  duplicated implementations across channel files.
- AGENTS.md replaced: 37KB build prompt -> 1.5KB consumption guide for AI
  coding assistants.
- README: live CI badge, "What AgentSpan is not" section, "Use with AI agents"
  quickstart, Community section, KNOWN_ISSUES link.
- docs/src/ updated: channels.md (24 -> 52), mcp.md (36 -> 91), introduction.md,
  architecture.md, getting-started.md, deployment.md, troubleshooting.md.
- .gitignore: added `__pycache__/`, `*.pyc`, `.pytest_cache/`, `*.tsbuildinfo`;
  removed `Cargo.lock` (binaries should commit it).
- Probe tests made cross-platform (echo works on Windows via `cmd /C echo`).

### Removed

- Deleted 25+ AI-generated research/report files from root.
- Deleted internal planning docs (AGENT_WORKFLOW.md, PROJECT_BRIEF.md, plan.md,
  docs/launch-plan.md, docs/implementation-report.md, docs/innovation-proposals.md,
  docs/competitive-analysis.md).
- Deleted 13 PNG charts and 2 .docx files from root.
- Deleted web/DESIGN_SYSTEM.md and web/HTML_ANALYSIS.md (scraped reference analysis).
- Removed `Cargo.lock` from .gitignore (binaries should commit it).
- `git rm --cached` tracked .pyc and .tsbuildinfo files.

## [0.4.0] - 2026-06-25

### Added

- 26 new channels (registry: 24 -> 50): npm, crates, pypi, gitlab, dockerhub,
  wayback, maps, weather, coinbase, duckduckgo, gnews, statuspage, huggingface,
  openai, anthropic, brave, bing, google, notion, slack, flight, devto,
  openlibrary, gutenberg, lobsters, wikidata.
- Federated search across multiple channels with reranking and deduplication.
- Request coalescing (SingleFlight) to collapse dogpiles.
- Agent memory (namespaced KV with TTL).
- Content intelligence (type detection, key facts, smart truncation).
- Self-healing channels (health monitor, auto-switch, repair, alerts).
- Self-improving analytics (profiler, cache optimizer, adaptive rate limiter).
- Adaptive routing (EWMA latency + success rate scoring).
- Conditional-request cache revalidation (ETag/Last-Modified).
- MCP tools: 36 -> 88.
- 8 more SDKs (JS, Go, Ruby, Java, PHP, C#, Swift, Rust) -> 9 total.
- IDE integrations (VS Code, JetBrains, Neovim) and GitHub Action.
- Load-testing CLI (`agentspan loadtest`).
- Shell completions, config backup/restore.

## [0.3.0] - 2026-06-22

### Added

- 3-tier cache (L1 memory / L2 disk / L3 Redis) with metrics and TTL sweeper.
- Authentication (SHA-256 keys, tenants, RBAC, rate limiting, audit).
- MCP server (36 tools, JSON-RPC over stdio).
- Python SDK (async httpx).
- Docker (multi-stage Dockerfile + docker-compose).
- CI (fmt, clippy, test, cargo-audit, Python SDK).
- 9 more channels (24 total): wikipedia, arxiv, discord, telegram, spotify,
  twitch, scholar, podcasts, quora, pinterest.
- Prometheus metrics at /metrics.
- Request tracing, body limits, graceful shutdown.
- Background health monitor with alerts.

## [0.1.0]

### Added

- Initial workspace: core, probe, router, channels, api, cli.
- Tier-0 channels: Web (Jina Reader), GitHub (gh CLI + REST), RSS.
- REST API: /health, /api/v1/channels, /api/v1/doctor, /api/v1/read.
- CLI: doctor, serve, version.

[Unreleased]: https://github.com/oxbshw/Agent-Span/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/oxbshw/Agent-Span/releases/tag/v0.5.0
[0.4.0]: https://github.com/oxbshw/Agent-Span/releases/tag/v0.4.0
[0.3.0]: https://github.com/oxbshw/Agent-Span/releases/tag/v0.3.0
