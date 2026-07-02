# AGENTS.md

AgentSpan is an async-Rust web access gateway for AI agents. This file is for
AI coding assistants (Claude Code, Cursor, Codex) working **in** this repo.

## Build & test

```bash
cargo test --workspace                    # run all tests
cargo clippy --workspace --all-targets -- -D warnings   # lint
cargo fmt --all -- --check                # format check
cargo run --bin agentspan -- serve         # start the API on :8080
cargo run --bin agentspan-mcp              # start the MCP server (stdio)
```

Python SDK: `pip install -e sdk/python[dev]` then `pytest -q` in `sdk/python/`.

## Crate layout

| Crate | What lives here |
|-------|----------------|
| `agentspan-core` | `Channel`/`Backend` traits, types, config, errors |
| `agentspan-probe` | Command execution + 6-state health classification |
| `agentspan-router` | Backend selection, retry, circuit breaker, adaptive routing |
| `agentspan-cache` | 3-tier cache (memory/disk/Redis), singleflight, optimizer |
| `agentspan-auth` | API keys (SHA-256), tenants, RBAC, rate limiting, audit |
| `agentspan-channels` | All 52 channel implementations + registry |
| `agentspan-api` | Axum REST server, SSE, OpenAPI spec, middleware |
| `agentspan-mcp` | MCP server (stdio + HTTP), 91 tools |
| `agentspan-cli` | The `agentspan` binary — install, doctor, serve, skill, etc. |

## Adding a channel

1. Copy `crates/agentspan-channels/src/hackernews.rs` (free API) or
   `spotify.rs` (OAuth) as a template.
2. Implement `Channel` + `Backend` traits. Use `BackendRouter` for fallback.
3. Register in `src/lib.rs` (module + re-export) and `src/registry.rs`.
4. Add MCP tools in `crates/agentspan-mcp/src/tools.rs`.
5. Add a format rule in `crates/agentspan-cli/src/commands/format.rs`.
6. Run `cargo test -p agentspan-channels` — the registry count test will tell
   you the expected count.

## Adding an MCP tool

Add a `ToolDef` entry in `crates/agentspan-mcp/src/tools.rs`. The `channel`
field must match a registered channel name. Use `Op::Read` for URL-based
fetch, `Op::Search` for keyword queries, `Op::Doctor` for health.

## Conventions

- Edition 2021, async/await, tokio runtime.
- `thiserror` for library errors, `anyhow` at binary boundaries.
- `tracing` for structured logging (never `println!` in library code).
- Tests live in `#[cfg(test)] mod tests` at the bottom of each file.
- No `todo!()` or `unimplemented!()` in production code paths.

## Known constraints

See [KNOWN_ISSUES.md](KNOWN_ISSUES.md) for build quirks (windows-gnu slow
builds, WebSocket feature-gate) and feature gaps.
