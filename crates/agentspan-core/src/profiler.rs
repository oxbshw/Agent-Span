//! Latency profiling per channel + backend.
//!
//! Each `(channel, backend)` pair gets a [`BackendProfile`] holding an
//! exponentially-weighted moving average (so recent requests count more than old
//! ones) plus a small window of recent samples used to estimate p99. The point is
//! to spot a backend that has quietly gotten slow and, when a faster sibling
//! exists on the same channel, recommend switching to it.

use std::collections::VecDeque;
use std::sync::Arc;

use dashmap::DashMap;
use serde::Serialize;

/// Smoothing factor for the EWMA: higher reacts faster to recent latency.
const DEFAULT_ALPHA: f64 = 0.2;

/// How many recent samples to keep per backend for the p99 estimate.
const DEFAULT_WINDOW: usize = 256;

/// Latency above which a backend is considered slow (p99, milliseconds).
pub const SLOW_P99_MS: u64 = 1_000;

/// Rolling latency stats for one backend of one channel.
#[derive(Debug, Clone)]
pub struct BackendProfile {
    pub channel: String,
    pub backend: String,
    pub samples: u64,
    pub ewma_ms: f64,
    recent: VecDeque<u64>,
    window: usize,
}

impl BackendProfile {
    fn new(channel: &str, backend: &str, window: usize) -> Self {
        Self {
            channel: channel.to_string(),
            backend: backend.to_string(),
            samples: 0,
            ewma_ms: 0.0,
            recent: VecDeque::with_capacity(window),
            window,
        }
    }

    fn record(&mut self, latency_ms: u64, alpha: f64) {
        self.ewma_ms = if self.samples == 0 {
            latency_ms as f64
        } else {
            alpha * latency_ms as f64 + (1.0 - alpha) * self.ewma_ms
        };
        self.samples += 1;
        if self.recent.len() == self.window {
            self.recent.pop_front();
        }
        self.recent.push_back(latency_ms);
    }

    /// Estimated 99th-percentile latency over the recent window.
    pub fn p99_ms(&self) -> u64 {
        percentile(&self.recent, 99.0)
    }

    /// Whether this backend's p99 exceeds [`SLOW_P99_MS`].
    pub fn is_slow(&self) -> bool {
        self.p99_ms() > SLOW_P99_MS
    }
}

/// A serializable view of a [`BackendProfile`].
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BackendProfileView {
    pub channel: String,
    pub backend: String,
    pub samples: u64,
    pub ewma_ms: u64,
    pub p99_ms: u64,
    pub slow: bool,
}

impl From<&BackendProfile> for BackendProfileView {
    fn from(p: &BackendProfile) -> Self {
        Self {
            channel: p.channel.clone(),
            backend: p.backend.clone(),
            samples: p.samples,
            ewma_ms: p.ewma_ms.round() as u64,
            p99_ms: p.p99_ms(),
            slow: p.is_slow(),
        }
    }
}

/// A recommendation to swap a slow backend for a faster sibling.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BackendSwap {
    pub channel: String,
    pub slow_backend: String,
    pub slow_ms: u64,
    pub fast_backend: String,
    pub fast_ms: u64,
    pub reason: String,
}

/// The full performance picture: every profile plus derived flags/suggestions.
#[derive(Debug, Clone, Serialize)]
pub struct PerformanceReport {
    pub profiles: Vec<BackendProfileView>,
    pub slow_backends: Vec<BackendProfileView>,
    pub suggestions: Vec<BackendSwap>,
}

/// Tracks latency per `(channel, backend)`.
#[derive(Debug, Clone)]
pub struct Profiler {
    profiles: Arc<DashMap<(String, String), BackendProfile>>,
    alpha: f64,
    window: usize,
}

impl Default for Profiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Profiler {
    /// Create a profiler with default smoothing and window.
    pub fn new() -> Self {
        Self {
            profiles: Arc::new(DashMap::new()),
            alpha: DEFAULT_ALPHA,
            window: DEFAULT_WINDOW,
        }
    }

    /// Record one latency sample for `channel`/`backend`.
    pub fn record(&self, channel: &str, backend: &str, latency_ms: u64) {
        let key = (channel.to_string(), backend.to_string());
        let mut entry = self
            .profiles
            .entry(key)
            .or_insert_with(|| BackendProfile::new(channel, backend, self.window));
        entry.record(latency_ms, self.alpha);
    }

    /// Snapshot of one backend's profile.
    pub fn profile(&self, channel: &str, backend: &str) -> Option<BackendProfileView> {
        self.profiles
            .get(&(channel.to_string(), backend.to_string()))
            .map(|p| BackendProfileView::from(p.value()))
    }

    /// All profiles, sorted by channel then backend.
    pub fn all(&self) -> Vec<BackendProfileView> {
        let mut v: Vec<BackendProfileView> =
            self.profiles.iter().map(|e| e.value().into()).collect();
        v.sort_by(|a, b| {
            a.channel
                .cmp(&b.channel)
                .then_with(|| a.backend.cmp(&b.backend))
        });
        v
    }

    /// Backends whose p99 exceeds [`SLOW_P99_MS`].
    pub fn slow_backends(&self) -> Vec<BackendProfileView> {
        let mut v: Vec<BackendProfileView> = self
            .profiles
            .iter()
            .filter(|e| e.value().is_slow())
            .map(|e| e.value().into())
            .collect();
        v.sort_by_key(|b| std::cmp::Reverse(b.p99_ms));
        v
    }

    /// For each channel with a slow backend, suggest the fastest sibling backend
    /// (by EWMA) if it is meaningfully quicker.
    pub fn suggestions(&self) -> Vec<BackendSwap> {
        // Group profiles by channel.
        let mut by_channel: std::collections::HashMap<String, Vec<BackendProfile>> =
            std::collections::HashMap::new();
        for e in self.profiles.iter() {
            by_channel
                .entry(e.value().channel.clone())
                .or_default()
                .push(e.value().clone());
        }

        let mut swaps = Vec::new();
        for (channel, mut backends) in by_channel {
            if backends.len() < 2 {
                continue;
            }
            backends.sort_by(|a, b| a.ewma_ms.total_cmp(&b.ewma_ms));
            let fastest = &backends[0];
            let slowest = backends.last().unwrap();
            // Only suggest if the slow one is actually slow and the fast one is
            // at least 2x quicker — avoids churning on noise.
            if slowest.is_slow() && slowest.ewma_ms >= fastest.ewma_ms * 2.0 {
                let slow_ms = slowest.ewma_ms.round() as u64;
                let fast_ms = fastest.ewma_ms.round() as u64;
                swaps.push(BackendSwap {
                    channel: channel.clone(),
                    slow_backend: slowest.backend.clone(),
                    slow_ms,
                    fast_backend: fastest.backend.clone(),
                    fast_ms,
                    reason: format!(
                        "{} slow (avg {}ms), try {} (avg {}ms)",
                        slowest.backend, slow_ms, fastest.backend, fast_ms
                    ),
                });
            }
        }
        swaps.sort_by(|a, b| a.channel.cmp(&b.channel));
        swaps
    }

    /// Assemble the full performance report.
    pub fn report(&self) -> PerformanceReport {
        PerformanceReport {
            profiles: self.all(),
            slow_backends: self.slow_backends(),
            suggestions: self.suggestions(),
        }
    }
}

/// Nearest-rank percentile over a set of samples. Returns `0` when empty.
fn percentile(samples: &VecDeque<u64>, pct: f64) -> u64 {
    if samples.is_empty() {
        return 0;
    }
    let mut sorted: Vec<u64> = samples.iter().copied().collect();
    sorted.sort_unstable();
    let rank = (pct / 100.0 * sorted.len() as f64).ceil() as usize;
    let idx = rank.saturating_sub(1).min(sorted.len() - 1);
    sorted[idx]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ewma_tracks_recent_latency() {
        let p = Profiler::new();
        for _ in 0..10 {
            p.record("twitter", "twitter-cli", 100);
        }
        let view = p.profile("twitter", "twitter-cli").unwrap();
        assert_eq!(view.samples, 10);
        // All samples equal -> EWMA equals the value.
        assert_eq!(view.ewma_ms, 100);
    }

    #[test]
    fn p99_reflects_tail_latency() {
        // A backend that is slow on a meaningful fraction of requests should show
        // an elevated p99 (nearest-rank p99 is, by design, robust to a single
        // one-in-a-hundred spike — it tracks sustained tail latency).
        let p = Profiler::new();
        for _ in 0..80 {
            p.record("c", "b", 50);
        }
        for _ in 0..20 {
            p.record("c", "b", 5_000);
        }
        let view = p.profile("c", "b").unwrap();
        assert!(view.p99_ms >= 5_000, "p99 was {}", view.p99_ms);
        assert!(view.slow);
    }

    #[test]
    fn slow_backends_are_flagged() {
        let p = Profiler::new();
        p.record("fast", "a", 50);
        p.record("slow", "b", 3_000);
        let slow = p.slow_backends();
        assert_eq!(slow.len(), 1);
        assert_eq!(slow[0].backend, "b");
    }

    #[test]
    fn suggests_faster_sibling_backend() {
        let p = Profiler::new();
        for _ in 0..20 {
            p.record("twitter", "twitter-cli", 800);
            p.record("twitter", "opencli", 200);
        }
        // Force the slow one over the slow threshold via a tail sample.
        p.record("twitter", "twitter-cli", 2_000);
        let swaps = p.suggestions();
        assert_eq!(swaps.len(), 1);
        assert_eq!(swaps[0].slow_backend, "twitter-cli");
        assert_eq!(swaps[0].fast_backend, "opencli");
        assert!(swaps[0].reason.contains("try opencli"));
    }

    #[test]
    fn no_suggestion_for_single_backend() {
        let p = Profiler::new();
        for _ in 0..10 {
            p.record("solo", "only", 5_000);
        }
        assert!(p.suggestions().is_empty());
    }

    #[test]
    fn no_suggestion_when_all_fast() {
        let p = Profiler::new();
        for _ in 0..10 {
            p.record("c", "a", 100);
            p.record("c", "b", 120);
        }
        assert!(p.suggestions().is_empty());
    }

    #[test]
    fn report_bundles_everything() {
        let p = Profiler::new();
        p.record("c", "a", 50);
        let report = p.report();
        assert_eq!(report.profiles.len(), 1);
    }

    #[test]
    fn percentile_basic() {
        let mut q = VecDeque::new();
        for i in 1..=100 {
            q.push_back(i);
        }
        assert_eq!(percentile(&q, 99.0), 99);
        assert_eq!(percentile(&q, 100.0), 100);
        assert_eq!(percentile(&VecDeque::new(), 99.0), 0);
    }
}
