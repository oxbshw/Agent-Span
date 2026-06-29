# Contributing to AgentSpan

Thanks for your interest in improving AgentSpan!

## Development setup

```bash
git clone https://github.com/oxbshw/Agent-Span
cd agentspan
cargo build --workspace
cargo test --workspace
```

Or use `just` (install with `cargo install just`):

```bash
just build     # cargo build --workspace
just test      # cargo test --workspace
just ci        # fmt + clippy + test (everything a PR must pass)
```

Python SDK:

```bash
cd sdk/python
pip install -e ".[dev]"
pytest
```

Web dashboard:

```bash
cd web
npm install
npm run dev      # http://localhost:5173
```

## Verification gates

Every change must pass all of these before review:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
pytest                           # if the Python SDK changed
```

Or simply: `just ci`

## Adding a channel

AgentSpan channels follow one consistent template. Copy
`crates/agentspan-channels/src/hackernews.rs` (free API) or
`spotify.rs` (OAuth) as a starting point.

1. Implement `Backend` (probe/read/search) and `Channel` (name/description/
   can_handle/tier/backends/read/search). The `check_health` method has a
   default implementation — you only need to override it if your channel needs
   custom probe logic.
2. Use `BackendRouter` to tie backends together with retry and fallback.
3. Register in `src/lib.rs` (module + re-export) and `src/registry.rs`.
   The registry test will tell you the expected count.
4. Add tests: `can_handle`, `tier`, URL parsing, and wiremock read/search.
5. Add MCP tools in `crates/agentspan-mcp/src/tools.rs`.
6. Add a format rule in `crates/agentspan-cli/src/commands/format.rs`.

## Adding an MCP tool

Add a `ToolDef` entry in `crates/agentspan-mcp/src/tools.rs`. The `channel`
field must match a registered channel name. Use `Op::Read` for URL fetch,
`Op::Search` for keyword queries, `Op::Doctor` for health.

## Commit / PR conventions

- Keep PRs focused; one logical change per PR.
- Use Conventional Commit prefixes (`feat:`, `fix:`, `docs:`, `chore:`).
- Update `CHANGELOG.md` under `[Unreleased]`.
- Don't commit secrets. Keys/cookies belong in `~/.agentspan/config.yaml` or env.

## License

By contributing you agree your contributions are licensed under the MIT license.
