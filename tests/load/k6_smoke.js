import http from 'k6/http';
import { check, sleep } from 'k6';
import { Trend, Rate } from 'k6/metrics';

// AgentSpan load test — exercises the gateway at 1000 RPS for 30s and asserts
// the SLA: p99 < 200ms, error rate < 1%.
//
// Run:
//   k6 run --env BASE_URL=http://localhost:8080 tests/load/k6_smoke.js
//
// Prereq: a running AgentSpan API (`cargo run --bin agentspan -- serve` or
// `docker compose up api`).

const BASE = __ENV.BASE_URL || 'http://localhost:8080';

// Custom metrics for clearer reporting.
const readLatency = new Trend('read_latency', true);
const errorRate = new Rate('errors');

export const options = {
  // Default trend stats stop at p(95); include p(99) so handleSummary can
  // report the number the threshold is actually judged on.
  summaryTrendStats: ['avg', 'min', 'med', 'max', 'p(90)', 'p(95)', 'p(99)'],
  // Target the spec SLA: 1000 req/sec for 30 seconds.
  scenarios: {
    smoke_read: {
      executor: 'ramping-arrival-rate',
      startRate: 0,
      timeUnit: '1s',
      preAllocatedVUs: 500,
      maxVUs: 2000,
      stages: [
        { duration: '5s', target: 1000 }, // ramp to 1000 RPS
        { duration: '30s', target: 1000 }, // hold 1000 RPS for 30s
        { duration: '5s', target: 0 }, // ramp down
      ],
    },
  },
  thresholds: {
    // The spec mandate (AGENTS.md §8): p99 < 200ms, error rate < 1%.
    http_req_duration: ['p(99)<200'],
    http_req_failed: ['rate<0.01'],
    errors: ['rate<0.01'],
  },
};

// Sample URLs that exercise different channels without heavy egress.
// `/health` is the cheapest path (no channel, no auth, no upstream).
// `/api/v1/channels` exercises auth middleware + registry.
// `/api/v1/doctor` exercises a full probe sweep (heavier — sample it).
const URLS = [
  `${BASE}/health`,
  `${BASE}/api/v1/channels`,
  `${BASE}/api/v1/stats`,
];

export default function () {
  // Weighted pick: 70% health, 20% channels, 10% stats.
  const r = Math.random();
  let url;
  if (r < 0.7) {
    url = URLS[0];
  } else if (r < 0.9) {
    url = URLS[1];
  } else {
    url = URLS[2];
  }

  const res = http.get(url, { timeout: '5s' });
  readLatency.add(res.timings.duration);

  const ok = res.status >= 200 && res.status < 300;
  errorRate.add(!ok);

  check(res, {
    'status is 2xx': (r) => r.status >= 200 && r.status < 300,
    'body present': (r) => r.body && r.body.length > 0,
  });

  // Small jitter to avoid synchronized-wave artifacts.
  sleep(Math.random() * 0.01);
}

export function handleSummary(data) {
  // Print a concise summary line so CI logs surface the headline numbers.
  const p99 = data.metrics.http_req_duration?.values?.['p(99)'];
  const failed = data.metrics.http_req_failed?.values?.rate;
  const rps = data.metrics.http_reqs?.values?.rate;
  const fmt = (v, digits) => (v === undefined ? 'n/a' : v.toFixed(digits));
  const pass = p99 !== undefined && p99 < 200 && failed !== undefined && failed < 0.01;
  console.log(
    `\n--- AgentSpan load summary ---\n` +
      `RPS: ${fmt(rps, 1)}\n` +
      `p99: ${fmt(p99, 1)} ms  (threshold: <200)\n` +
      `failed rate: ${fmt(failed, 4)}  (threshold: <0.01)\n` +
      `pass: ${pass}\n`
  );
  return {};
}
