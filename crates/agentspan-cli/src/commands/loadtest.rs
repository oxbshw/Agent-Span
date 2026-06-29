//! `agentspan loadtest` — hammer an HTTP endpoint and report latency/throughput.
//!
//! A small concurrent load generator for the gateway (or any URL): N workers
//! issue requests, and it reports throughput plus p50/p99/p999 latency and the
//! error rate. Complements `benchmark` (which times the in-process router); this
//! one drives a *running* server over real HTTP.

use std::time::{Duration, Instant};

use clap::Args;

use crate::commands::benchmark::percentile;

#[derive(Args)]
pub struct LoadTestArgs {
    /// URL to hit (e.g. http://localhost:8080/health).
    pub url: String,

    /// Number of concurrent workers.
    #[arg(long, default_value_t = 10)]
    pub concurrency: usize,

    /// Total number of requests to send.
    #[arg(long, default_value_t = 100)]
    pub requests: usize,

    /// Optional API key, sent as `X-API-Key`.
    #[arg(long)]
    pub api_key: Option<String>,
}

/// Aggregated results of a load test.
#[derive(Debug, PartialEq)]
pub struct Summary {
    pub total: usize,
    pub ok: usize,
    pub errors: usize,
    pub elapsed_ms: u128,
    pub rps: f64,
    pub p50_us: u64,
    pub p99_us: u64,
    pub p999_us: u64,
}

/// Build a [`Summary`] from per-request latencies (µs), an error count, and the
/// wall-clock duration. Pure so it can be unit-tested without a server.
pub fn summarize(mut latencies_us: Vec<u64>, errors: usize, elapsed: Duration) -> Summary {
    latencies_us.sort_unstable();
    let ok = latencies_us.len();
    let total = ok + errors;
    let secs = elapsed.as_secs_f64().max(1e-9);
    Summary {
        total,
        ok,
        errors,
        elapsed_ms: elapsed.as_millis(),
        rps: total as f64 / secs,
        p50_us: percentile(&latencies_us, 50.0),
        p99_us: percentile(&latencies_us, 99.0),
        p999_us: percentile(&latencies_us, 99.9),
    }
}

pub async fn run(args: LoadTestArgs) -> anyhow::Result<()> {
    let workers = args.concurrency.max(1);
    let per_worker = args.requests.div_ceil(workers);
    let client = reqwest::Client::new();

    println!(
        "Load testing {} — {} workers × {} requests…",
        args.url, workers, per_worker
    );
    let started = Instant::now();

    let mut handles = Vec::with_capacity(workers);
    for _ in 0..workers {
        let client = client.clone();
        let url = args.url.clone();
        let api_key = args.api_key.clone();
        handles.push(tokio::spawn(async move {
            let mut latencies = Vec::with_capacity(per_worker);
            let mut errors = 0usize;
            for _ in 0..per_worker {
                let mut req = client.get(&url);
                if let Some(key) = &api_key {
                    req = req.header("X-API-Key", key);
                }
                let t = Instant::now();
                match req.send().await {
                    Ok(resp) if resp.status().is_success() => {
                        latencies.push(t.elapsed().as_micros() as u64);
                    }
                    _ => errors += 1,
                }
            }
            (latencies, errors)
        }));
    }

    let mut latencies = Vec::new();
    let mut errors = 0usize;
    for handle in handles {
        let (lats, errs) = handle.await?;
        latencies.extend(lats);
        errors += errs;
    }

    let s = summarize(latencies, errors, started.elapsed());
    println!(
        "\nrequests: {}   ok: {}   errors: {}",
        s.total, s.ok, s.errors
    );
    println!(
        "elapsed: {} ms   throughput: {:.0} req/s",
        s.elapsed_ms, s.rps
    );
    println!(
        "latency   p50: {} µs   p99: {} µs   p999: {} µs",
        s.p50_us, s.p99_us, s.p999_us
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_computes_rps_and_percentiles() {
        let latencies = vec![4000u64, 1000, 3000, 2000]; // µs, unsorted
        let s = summarize(latencies, 1, Duration::from_secs(1));
        assert_eq!(s.ok, 4);
        assert_eq!(s.errors, 1);
        assert_eq!(s.total, 5);
        assert!((s.rps - 5.0).abs() < 1e-6);
        assert!(s.p99_us >= s.p50_us);
        assert!(s.p50_us >= 1000 && s.p50_us <= 4000);
    }

    #[test]
    fn summarize_handles_all_errors() {
        let s = summarize(vec![], 3, Duration::from_secs(1));
        assert_eq!(s.ok, 0);
        assert_eq!(s.errors, 3);
        assert_eq!(s.total, 3);
        assert_eq!(s.p50_us, 0);
    }
}
