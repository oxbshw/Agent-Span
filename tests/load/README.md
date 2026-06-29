# AgentSpan load tests

Reproducible load tests that prove the gateway meets the spec SLA:
**1000 req/sec for 30 seconds, p99 < 200ms, error rate < 1%**
(`AGENTS.md` §8).

## Prerequisites

- A running AgentSpan API:
  ```bash
  cargo run --bin agentspan -- serve            # local dev
  # or
  docker compose up api                          # container
  ```
- [`k6`](https://k6.io) installed (`brew install k6`, `choco install k6`, or
  `docker run grafana/k6 run -`).

## Running

```bash
# Against a local server:
k6 run tests/load/k6_smoke.js

# Against a remote server with a custom base URL:
k6 run --env BASE_URL=https://agentspan.example.com tests/load/k6_smoke.js

# Via Docker (no local k6 install):
docker run --rm --network host -i grafana/k6 run - < tests/load/k6_smoke.js
```

k6 will:
1. Ramp from 0 to 1000 RPS over 5 seconds.
2. Hold 1000 RPS for 30 seconds.
3. Ramp down to 0 over 5 seconds.
4. Fail the run if `p(99) >= 200ms` or `http_req_failed >= 1%`.

## Workload

The smoke script exercises three endpoints with realistic weighting:

| Endpoint            | Weight | Why |
|---------------------|--------|-----|
| `GET /health`       | 70%    | Liveness probe — cheapest path, no auth, no upstream. Bulk of any real load balancer traffic. |
| `GET /api/v1/channels` | 20% | Auth middleware + registry lookup — middle-tier cost. |
| `GET /api/v1/stats`    | 10% | Audit ring read + analytics — slightly heavier. |

Per-channel `/read` and `/search` are intentionally excluded from the smoke
profile because they hit upstream platforms (network egress, rate limits,
flaky third parties). A `k6_channels.js` profile that exercises the `web`
channel (Jina Reader) against a mock is planned.

## CI integration

`.github/workflows/load.yml` runs this script against a containerized API on
release branches. The job is **non-fatal on PRs** (informational) and
**fatal on `main`/tags** so a regression in p99 blocks the release.

## Captured baseline

See `BASELINE.md` for the last recorded numbers from a reference run.
Add new entries there after every release so we track drift.
