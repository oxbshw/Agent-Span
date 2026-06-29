//! `agentspan benchmark` — synthetic throughput/latency benchmark.
//!
//! Drives the real [`BackendRouter`] with a mock backend so results are
//! deterministic (no network). It compares the cache-miss path (every read hits
//! the simulated backend) against the cache-hit path (served from the in-memory
//! L1 cache), reporting p50/p99 latency and requests/sec for each.

use std::io::Write;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::style;
use agentspan_cache::{CacheManager, MemoryCache};
use agentspan_core::backend::Backend;
use agentspan_core::error::BackendError;
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;
use async_trait::async_trait;
use clap::Args;

#[derive(Args)]
pub struct BenchmarkArgs {
    /// Number of read iterations per scenario.
    #[arg(long, default_value_t = 1000)]
    pub iterations: usize,
    /// Simulated backend latency in microseconds (the cache-miss cost).
    #[arg(long, default_value_t = 500)]
    pub backend_us: u64,
}

/// A backend with a configurable artificial latency (no network).
#[derive(Debug, Clone)]
struct MockBackend {
    delay: Duration,
}

#[async_trait]
impl Backend for MockBackend {
    fn name(&self) -> &str {
        "mock"
    }

    async fn probe(&self) -> ProbeResult {
        ProbeResult::ok("mock", "1")
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        if !self.delay.is_zero() {
            tokio::time::sleep(self.delay).await;
        }
        Ok(Content {
            url: url.to_string(),
            title: None,
            body: "ok".to_string(),
            metadata: serde_json::Value::Null,
            cached: false,
        })
    }

    async fn search(
        &self,
        _query: &str,
        _opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        Ok(vec![])
    }
}

/// Measured results for one benchmark scenario.
#[derive(Debug, Clone, PartialEq)]
pub struct Metrics {
    pub label: String,
    pub iterations: usize,
    pub p50_us: u64,
    pub p99_us: u64,
    pub rps: f64,
}

/// Nearest-rank percentile over a pre-sorted slice of microsecond latencies.
pub fn percentile(sorted_us: &[u64], pct: f64) -> u64 {
    if sorted_us.is_empty() {
        return 0;
    }
    let rank = (pct / 100.0) * (sorted_us.len() - 1) as f64;
    let idx = rank.round() as usize;
    sorted_us[idx.min(sorted_us.len() - 1)]
}

async fn bench_reads(
    label: &str,
    router: &BackendRouter,
    iterations: usize,
    force_refresh: bool,
    progress: bool,
) -> Metrics {
    let mut latencies = Vec::with_capacity(iterations);
    let mut bar = style::ProgressBar::new(iterations);
    // Refresh the bar at most ~100 times to avoid flooding the terminal.
    let step = (iterations / 100).max(1);
    let start = Instant::now();
    for i in 0..iterations {
        let opts = ReadOptions {
            force_refresh,
            ..Default::default()
        };
        let t = Instant::now();
        let _ = router.read("https://bench.local/item", opts).await;
        latencies.push(t.elapsed().as_micros() as u64);
        bar.inc();
        if progress && (i % step == 0 || i + 1 == iterations) {
            print!("\r  {label:<22} {}", bar.render());
            let _ = std::io::stdout().flush();
        }
    }
    if progress {
        println!();
    }
    let total = start.elapsed();
    latencies.sort_unstable();
    Metrics {
        label: label.to_string(),
        iterations,
        p50_us: percentile(&latencies, 50.0),
        p99_us: percentile(&latencies, 99.0),
        rps: if total.as_secs_f64() > 0.0 {
            iterations as f64 / total.as_secs_f64()
        } else {
            0.0
        },
    }
}

fn memory_cache() -> CacheManager {
    CacheManager::new(
        Some(Arc::new(MemoryCache::new())),
        None,
        None,
        Duration::from_secs(60),
        Duration::from_secs(3600),
        Duration::from_secs(86_400),
    )
}

fn mock_router(delay: Duration) -> BackendRouter {
    BackendRouter::new(
        vec![Arc::new(MockBackend { delay }) as Arc<dyn Backend>],
        ProbeEngine::new(Duration::from_secs(1)),
        RetryConfig::default(),
    )
}

fn render_results(rows: &[Metrics]) {
    style::section("Benchmark Results");
    let iters = rows.first().map(|m| m.iterations).unwrap_or(0);
    println!("{iters} iterations per scenario");
    let table: Vec<Vec<String>> = rows
        .iter()
        .map(|m| {
            vec![
                m.label.clone(),
                format!("{}µs", m.p50_us),
                format!("{}µs", m.p99_us),
                format!("{:.0}", m.rps),
            ]
        })
        .collect();
    style::print_table(&["Scenario", "p50", "p99", "req/s"], &table);
    if let (Some(miss), Some(hit)) = (rows.first(), rows.get(1)) {
        if miss.rps > 0.0 {
            println!(
                "\nCache speedup: {:.1}x more req/s on hits vs. backend",
                hit.rps / miss.rps
            );
        }
    }
}

pub async fn run(args: BenchmarkArgs) -> anyhow::Result<()> {
    let n = args.iterations.max(1);
    let delay = Duration::from_micros(args.backend_us);

    style::section("Running Benchmark");

    // Cache miss: force_refresh on every call → always hits the simulated backend.
    let miss_router = mock_router(delay);
    let miss = bench_reads("cache miss (backend)", &miss_router, n, true, true).await;

    // Cache hit: warm once, then serve from the in-memory L1 cache.
    let hit_router = mock_router(delay).with_cache(memory_cache(), "bench");
    let _ = hit_router
        .read("https://bench.local/item", ReadOptions::default())
        .await;
    let hit = bench_reads("cache hit (L1)", &hit_router, n, false, true).await;

    render_results(&[miss, hit]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentile_handles_edges() {
        assert_eq!(percentile(&[], 50.0), 0);
        let data = [10, 20, 30, 40, 50];
        assert_eq!(percentile(&data, 0.0), 10);
        assert_eq!(percentile(&data, 50.0), 30);
        assert_eq!(percentile(&data, 100.0), 50);
    }

    #[tokio::test]
    async fn bench_reads_produces_metrics() {
        let router = mock_router(Duration::ZERO);
        let m = bench_reads("test", &router, 20, true, false).await;
        assert_eq!(m.iterations, 20);
        assert!(m.rps > 0.0);
        assert!(m.p99_us >= m.p50_us);
    }

    #[tokio::test]
    async fn cache_hit_is_not_slower_than_miss() {
        let delay = Duration::from_micros(2000);
        let miss = bench_reads("miss", &mock_router(delay), 30, true, false).await;
        let hit_router = mock_router(delay).with_cache(memory_cache(), "bench");
        let _ = hit_router
            .read("https://bench.local/item", ReadOptions::default())
            .await;
        let hit = bench_reads("hit", &hit_router, 30, false, false).await;
        // Cache hits bypass the 2ms backend sleep entirely.
        assert!(hit.p50_us <= miss.p50_us);
    }
}
