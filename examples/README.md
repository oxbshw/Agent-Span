# Examples

Runnable examples for each integration path. All assume a running AgentSpan
gateway (`agentspan serve` or `docker compose up api`).

## Quick start

| Example | Language | What it does |
|---------|----------|-------------|
| `curl/quickstart.sh` | Bash | Health, read, search, doctor, federated search, OpenAPI |
| `python/batch_read.py` | Python | Read 3 URLs in parallel, report token counts |
| `python/federated_search.py` | Python | Federated search across HN + Reddit + Lobsters |
| `python/auth_flow.py` | Python | Create/use/revoke an API key |
| `javascript/quickstart.mjs` | JS | Read a URL + search HN |
| `rust/quickstart.rs` | Rust | Read + search + list channels via the Rust SDK |

## Running

```bash
# Start the gateway in one terminal:
agentspan serve

# In another terminal:
bash examples/curl/quickstart.sh
python examples/python/batch_read.py
node examples/javascript/quickstart.mjs
```

## MCP examples

See `agentspan mcp tools` for the full tool list, and the
[agent integration guides](../docs/guides/) for Claude Code / Cursor / Windsurf
setup.
