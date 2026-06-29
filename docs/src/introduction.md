# Introduction

**AgentSpan** is the Web Access Gateway for AI Agents — an async-Rust service
that gives agents persistent, cached access to **52 internet platforms** through
one REST API, an SSE event stream, a native **MCP server (92 tools)**, **9
language SDKs**, a CLI, and a React dashboard.

## The problem

AI coding agents (Claude Code, Cursor, Windsurf) need to read web pages, search
platforms, and fetch content — but they face three problems:

1. **Every agent re-fetches the same URLs.** No caching means redundant network
   calls, slower responses, and higher API costs.
2. **Platforms break.** A CLI tool gets stale, an API changes, a cookie expires.
   Agents have no way to detect this and fall back gracefully.
3. **No multi-tenancy.** If you run agents for multiple users, there's no way to
   scope API keys, rate limits, or audit trails per tenant.

## The solution

AgentSpan sits between your agents and the internet:

- **Gateway, not wrapper**: AgentSpan performs the reads itself, behind one
  stable API. Agents don't need to know which CLI tool to use or which API to
  call — they call `web_read` or `hackernews_search` and AgentSpan handles the
  rest.
- **3-tier cache**: L1 (memory) → L2 (disk) → L3 (Redis). Identical reads are
  coalesced (single-flight), so 10 agents asking for the same URL hit upstream
  once.
- **Self-healing**: a background monitor probes every channel, auto-switches
  failing backends, reinstalls broken CLI tools, and alerts on sustained
  outages.
- **Self-improving**: the gateway observes its own traffic and suggests cache-TTL
  tweaks, faster backends, and platforms worth adding.
- **Multi-tenant**: SHA-256-hashed API keys, per-tenant rate limiting, RBAC
  scopes, and an audit log.

## Highlights

- **52 channels** — web, github, youtube, tiktok, instagram, reddit, twitter,
  hackernews, arxiv, wikipedia, spotify, discord, telegram, and 39 more.
- **92 MCP tools** — read + search per channel, plus `doctor`. stdio and HTTP
  transport.
- **9 SDKs** — Python, JS/TS, Rust, Go, Ruby, Java, PHP, C#, Swift.
- **OpenAPI 3.0 spec** at `/openapi.json` + Swagger UI at `/docs`.
- **Docker Compose** — one command for API + UI + Redis + Prometheus + Grafana.
- **`agentspan mcp install`** — one command to wire up Claude Code, Cursor, or
  Windsurf.

## Where AgentSpan fits

AgentSpan is the **read layer** underneath your agents. It is not:

- A browser-automation agent (can't click, log in, fill forms — use
  Browser-Use or Playwright for that).
- A generic crawler competing with Firecrawl on arbitrary-site quality.
- A hosted SaaS. It's a self-hosted binary you run on your own infra.

Continue to [Installation](getting-started.md).
