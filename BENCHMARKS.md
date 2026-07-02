# Benchmarks

Numbers below are **measured, not estimated** — the load test runs in CI on
every pull request (`.github/workflows/load.yml`), so they're reproduced
continuously on stock GitHub-hosted runners (2-core `ubuntu-latest`).

## Gateway smoke test — 1000 RPS sustained

k6 drives a release build of `agentspan-api` (started with `--port 18080`) at
1000 requests/second for 30 seconds, mixing `/health` (70%),
`/api/v1/channels` (20%), and `/api/v1/stats` (10%) — i.e. routing, auth
middleware, registry serialization, and stats, without external egress.

Latest verified run (CI, 2026-07-02):

| Metric | Result | Threshold |
|---|---|---|
| Sustained rate | **1000 RPS** (30 s hold; 874.9 avg incl. ramps) | 1000 RPS |
| p99 latency | **0.4 ms** | < 200 ms |
| Failed requests | **0.0000** | < 1% |

The thresholds fail the workflow if breached, so a performance regression on
these paths cannot merge silently.

Reproduce locally:

```bash
cargo build --release --bin agentspan-api
./target/release/agentspan-api --port 18080 &
k6 run --env BASE_URL=http://127.0.0.1:18080 tests/load/k6_smoke.js
```

## Built-in benchmark tooling

The CLI ships two measurement commands (no external tools needed):

```bash
# Synthetic router benchmark: cache-hit vs backend-miss latency (p50/p99), RPS
agentspan benchmark

# HTTP load generator against any endpoint: concurrency, p50/p99/p999
agentspan loadtest --url http://localhost:8080/health --requests 10000 --concurrency 64
```

`agentspan benchmark` exercises the real `BackendRouter` with an in-memory L1
cache, so it isolates gateway overhead from network noise.

## Notes on method

- CI runners are shared 2-core VMs; treat absolute numbers as a floor, not a
  ceiling — dedicated hardware does better.
- The k6 mix intentionally avoids external platform calls: it measures the
  gateway, not third-party APIs.
- The release profile builds with thin LTO, a single codegen unit, and
  stripped symbols (see `[profile.release]` in `Cargo.toml`).
- Windows note: timer granularity (~15.6 ms) inflates sub-millisecond sleeps
  in `agentspan benchmark`'s miss simulation — a known, documented artifact,
  not gateway latency.
