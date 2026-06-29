//! Usage analytics — a lightweight record of what the gateway actually serves.
//!
//! Every request can be fed to [`Analytics::record`], which keeps two things: a
//! bounded ring buffer of the most recent [`RequestRecord`]s (for "show me the
//! last N requests" style queries) and a set of always-on aggregate counters per
//! channel plus a global total. Counters are atomic and the ring is behind a
//! short-lived mutex, so the whole thing is cheap to share across request
//! handlers behind an `Arc`.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::Serialize;

/// How many recent requests we keep in the ring buffer by default.
pub const DEFAULT_CAPACITY: usize = 100_000;

/// A single served request.
#[derive(Debug, Clone, Serialize)]
pub struct RequestRecord {
    /// Channel that served the request, if one was resolved.
    pub channel: Option<String>,
    /// Backend within the channel that did the work, if known.
    pub backend: Option<String>,
    /// End-to-end latency in milliseconds.
    pub latency_ms: u64,
    /// Whether the response came from cache.
    pub cache_hit: bool,
    /// Estimated input tokens.
    pub tokens_in: u64,
    /// Estimated output tokens.
    pub tokens_out: u64,
    /// HTTP status code.
    pub status: u16,
    /// When the request completed.
    pub at: DateTime<Utc>,
}

impl RequestRecord {
    /// Start a record for `channel` with sensible defaults; chain the setters to
    /// fill in what you know.
    pub fn new(channel: Option<String>) -> Self {
        Self {
            channel,
            backend: None,
            latency_ms: 0,
            cache_hit: false,
            tokens_in: 0,
            tokens_out: 0,
            status: 200,
            at: Utc::now(),
        }
    }

    /// Set the backend that handled the request.
    pub fn backend(mut self, backend: impl Into<String>) -> Self {
        self.backend = Some(backend.into());
        self
    }

    /// Set the measured latency.
    pub fn latency(mut self, ms: u64) -> Self {
        self.latency_ms = ms;
        self
    }

    /// Mark whether the response was a cache hit.
    pub fn cache(mut self, hit: bool) -> Self {
        self.cache_hit = hit;
        self
    }

    /// Set the estimated token counts.
    pub fn tokens(mut self, input: u64, output: u64) -> Self {
        self.tokens_in = input;
        self.tokens_out = output;
        self
    }

    /// Set the response status code.
    pub fn status(mut self, status: u16) -> Self {
        self.status = status;
        self
    }
}

/// Atomic per-channel counters.
#[derive(Debug, Default)]
struct ChannelCounters {
    requests: AtomicU64,
    errors: AtomicU64,
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
    latency_sum_ms: AtomicU64,
    tokens_in: AtomicU64,
    tokens_out: AtomicU64,
}

/// A serializable snapshot of a channel's aggregate usage.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ChannelStats {
    pub channel: String,
    pub requests: u64,
    pub errors: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub avg_latency_ms: f64,
    pub cache_hit_rate: f64,
    pub tokens_in: u64,
    pub tokens_out: u64,
}

/// A serializable snapshot of the global totals.
#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct Totals {
    pub requests: u64,
    pub errors: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
}

/// Records every request into a ring buffer plus per-channel aggregates.
#[derive(Clone)]
pub struct Analytics {
    ring: Arc<Mutex<VecDeque<RequestRecord>>>,
    per_channel: Arc<DashMap<String, ChannelCounters>>,
    totals: Arc<ChannelCounters>,
    capacity: usize,
}

impl std::fmt::Debug for Analytics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Analytics")
            .field("capacity", &self.capacity)
            .field("channels", &self.per_channel.len())
            .finish_non_exhaustive()
    }
}

impl Default for Analytics {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }
}

impl Analytics {
    /// Create an analytics store keeping the last [`DEFAULT_CAPACITY`] requests.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an analytics store with a custom ring-buffer capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            ring: Arc::new(Mutex::new(VecDeque::with_capacity(capacity.min(4096)))),
            per_channel: Arc::new(DashMap::new()),
            totals: Arc::new(ChannelCounters::default()),
            capacity: capacity.max(1),
        }
    }

    /// Record one served request.
    pub fn record(&self, record: RequestRecord) {
        let is_error = record.status >= 400;
        if let Some(channel) = record.channel.clone() {
            let counters = self.per_channel.entry(channel).or_default();
            bump(&counters, &record, is_error);
        }
        bump(&self.totals, &record, is_error);

        let mut ring = self.ring.lock().expect("analytics ring poisoned");
        if ring.len() == self.capacity {
            ring.pop_front();
        }
        ring.push_back(record);
    }

    /// The most recent `n` requests, newest last.
    pub fn recent(&self, n: usize) -> Vec<RequestRecord> {
        let ring = self.ring.lock().expect("analytics ring poisoned");
        ring.iter().rev().take(n).rev().cloned().collect()
    }

    /// Number of requests currently held in the ring buffer.
    pub fn len(&self) -> usize {
        self.ring.lock().expect("analytics ring poisoned").len()
    }

    /// Whether any request has been recorded.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Aggregate stats for one channel.
    pub fn channel_stats(&self, channel: &str) -> Option<ChannelStats> {
        self.per_channel
            .get(channel)
            .map(|c| snapshot(channel, c.value()))
    }

    /// Aggregate stats for every channel, sorted by request volume (desc).
    pub fn all_channel_stats(&self) -> Vec<ChannelStats> {
        let mut stats: Vec<ChannelStats> = self
            .per_channel
            .iter()
            .map(|e| snapshot(e.key(), e.value()))
            .collect();
        stats.sort_by(|a, b| {
            b.requests
                .cmp(&a.requests)
                .then_with(|| a.channel.cmp(&b.channel))
        });
        stats
    }

    /// Global totals across all channels.
    pub fn totals(&self) -> Totals {
        let t = &self.totals;
        Totals {
            requests: t.requests.load(Ordering::Relaxed),
            errors: t.errors.load(Ordering::Relaxed),
            cache_hits: t.cache_hits.load(Ordering::Relaxed),
            cache_misses: t.cache_misses.load(Ordering::Relaxed),
            tokens_in: t.tokens_in.load(Ordering::Relaxed),
            tokens_out: t.tokens_out.load(Ordering::Relaxed),
        }
    }
}

fn bump(c: &ChannelCounters, r: &RequestRecord, is_error: bool) {
    c.requests.fetch_add(1, Ordering::Relaxed);
    if is_error {
        c.errors.fetch_add(1, Ordering::Relaxed);
    }
    if r.cache_hit {
        c.cache_hits.fetch_add(1, Ordering::Relaxed);
    } else {
        c.cache_misses.fetch_add(1, Ordering::Relaxed);
    }
    c.latency_sum_ms.fetch_add(r.latency_ms, Ordering::Relaxed);
    c.tokens_in.fetch_add(r.tokens_in, Ordering::Relaxed);
    c.tokens_out.fetch_add(r.tokens_out, Ordering::Relaxed);
}

fn snapshot(channel: &str, c: &ChannelCounters) -> ChannelStats {
    let requests = c.requests.load(Ordering::Relaxed);
    let cache_hits = c.cache_hits.load(Ordering::Relaxed);
    let cache_misses = c.cache_misses.load(Ordering::Relaxed);
    let latency_sum = c.latency_sum_ms.load(Ordering::Relaxed);
    let lookups = cache_hits + cache_misses;
    ChannelStats {
        channel: channel.to_string(),
        requests,
        errors: c.errors.load(Ordering::Relaxed),
        cache_hits,
        cache_misses,
        avg_latency_ms: if requests == 0 {
            0.0
        } else {
            latency_sum as f64 / requests as f64
        },
        cache_hit_rate: if lookups == 0 {
            0.0
        } else {
            cache_hits as f64 / lookups as f64
        },
        tokens_in: c.tokens_in.load(Ordering::Relaxed),
        tokens_out: c.tokens_out.load(Ordering::Relaxed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(channel: &str, latency: u64, hit: bool, status: u16) -> RequestRecord {
        RequestRecord::new(Some(channel.to_string()))
            .latency(latency)
            .cache(hit)
            .status(status)
            .tokens(10, 40)
    }

    #[test]
    fn records_accumulate_per_channel() {
        let a = Analytics::new();
        a.record(rec("github", 100, false, 200));
        a.record(rec("github", 200, true, 200));
        let s = a.channel_stats("github").unwrap();
        assert_eq!(s.requests, 2);
        assert_eq!(s.avg_latency_ms, 150.0);
        assert_eq!(s.cache_hits, 1);
        assert_eq!(s.cache_misses, 1);
        assert_eq!(s.cache_hit_rate, 0.5);
        assert_eq!(s.tokens_in, 20);
        assert_eq!(s.tokens_out, 80);
    }

    #[test]
    fn errors_are_counted() {
        let a = Analytics::new();
        a.record(rec("reddit", 10, false, 200));
        a.record(rec("reddit", 10, false, 502));
        a.record(rec("reddit", 10, false, 429));
        let s = a.channel_stats("reddit").unwrap();
        assert_eq!(s.requests, 3);
        assert_eq!(s.errors, 2);
    }

    #[test]
    fn totals_aggregate_across_channels() {
        let a = Analytics::new();
        a.record(rec("a", 10, true, 200));
        a.record(rec("b", 20, false, 500));
        let t = a.totals();
        assert_eq!(t.requests, 2);
        assert_eq!(t.errors, 1);
        assert_eq!(t.cache_hits, 1);
        assert_eq!(t.cache_misses, 1);
    }

    #[test]
    fn ring_buffer_is_bounded() {
        let a = Analytics::with_capacity(3);
        for i in 0..10 {
            a.record(rec("x", i, false, 200));
        }
        assert_eq!(a.len(), 3);
        // Counters keep counting even though the ring drops old records.
        assert_eq!(a.channel_stats("x").unwrap().requests, 10);
        // The retained records are the three most recent (latencies 7, 8, 9).
        let recent = a.recent(3);
        assert_eq!(
            recent.iter().map(|r| r.latency_ms).collect::<Vec<_>>(),
            vec![7, 8, 9]
        );
    }

    #[test]
    fn recent_returns_newest_in_order() {
        let a = Analytics::new();
        a.record(rec("x", 1, false, 200));
        a.record(rec("x", 2, false, 200));
        a.record(rec("x", 3, false, 200));
        let recent = a.recent(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].latency_ms, 2);
        assert_eq!(recent[1].latency_ms, 3);
    }

    #[test]
    fn all_channel_stats_sorted_by_volume() {
        let a = Analytics::new();
        a.record(rec("low", 1, false, 200));
        for _ in 0..5 {
            a.record(rec("high", 1, false, 200));
        }
        let stats = a.all_channel_stats();
        assert_eq!(stats[0].channel, "high");
        assert_eq!(stats[1].channel, "low");
    }

    #[test]
    fn untracked_channel_has_no_stats() {
        let a = Analytics::new();
        assert!(a.channel_stats("nope").is_none());
        assert!(a.is_empty());
    }
}
