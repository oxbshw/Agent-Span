# Load test baselines

Every release should append a row here so we track p99/error drift over time.
The numbers are from `k6_smoke.js` run against a single API instance on the
noted host. Run on the same hardware for comparable results.

| Date       | Version | Host                              | RPS achieved | p99 (ms) | Failed rate | Pass |
|------------|---------|-----------------------------------|--------------|----------|-------------|------|
| 2026-06-28 | 0.4.0   | (pending — capture on Linux CI)   | —            | —        | —           | —    |

## How to capture a baseline

1. Start the API on a quiet host: `cargo run --release --bin agentspan -- serve`.
2. From another machine (or the same, if CPU is idle): `k6 run tests/load/k6_smoke.js`.
3. Copy the summary line from k6 output into the table above.
4. Commit `BASELINE.md` with the release.

## Notes

- **Windows hosts are not suitable baselines.** `KNOWN_ISSUES.md:42-44` flags
  that timer granularity on Windows inflates sub-ms latency figures. Always
  capture the canonical baseline on Linux.
- The smoke profile hits `/health`, `/api/v1/channels`, and `/api/v1/stats` —
  not upstream platforms. A p99 regression here means the gateway itself
  (middleware, router, serde) got slower, not a third-party hiccup.
