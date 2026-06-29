//! Per-channel TTL tuning driven by observed hit rates.
//!
//! A channel whose cache almost never hits (content changes faster than we keep
//! it) is wasting memory holding stale entries, so we shorten its TTL. A channel
//! that almost always hits (stable content) can hold entries longer, so we
//! lengthen it. The optimizer keeps a hit/miss tally per channel and, once it has
//! enough samples to trust, nudges that channel's TTL up or down within sane
//! bounds.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use serde::Serialize;
use tracing::info;

use crate::manager::CacheTtl;

/// Below this hit rate, the TTL is shortened.
pub const LOW_HIT_RATE: f64 = 0.20;

/// Above this hit rate, the TTL is lengthened.
pub const HIGH_HIT_RATE: f64 = 0.80;

/// Don't adjust a channel until it has at least this many lookups.
pub const MIN_SAMPLES: u64 = 20;

/// TTL never drops below 30 seconds or rises above 24 hours.
const FLOOR: Duration = Duration::from_secs(30);
const CEILING: Duration = Duration::from_secs(86_400);

#[derive(Debug, Default)]
struct HitCounters {
    hits: AtomicU64,
    misses: AtomicU64,
}

/// A serializable record of a TTL change the optimizer wants to make (or made).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TtlAdjustment {
    pub channel: String,
    pub from_secs: u64,
    pub to_secs: u64,
    pub hit_rate: f64,
    pub preset: String,
    pub reason: String,
}

/// Learns a good TTL per channel from its cache hit rate.
#[derive(Debug, Clone)]
pub struct CacheOptimizer {
    stats: Arc<DashMap<String, HitCounters>>,
    ttls: Arc<DashMap<String, Duration>>,
    default_ttl: Duration,
    min_samples: u64,
}

impl Default for CacheOptimizer {
    fn default() -> Self {
        Self::new(CacheTtl::Hot.as_duration())
    }
}

impl CacheOptimizer {
    /// Create an optimizer whose channels start at `default_ttl`.
    pub fn new(default_ttl: Duration) -> Self {
        Self {
            stats: Arc::new(DashMap::new()),
            ttls: Arc::new(DashMap::new()),
            default_ttl,
            min_samples: MIN_SAMPLES,
        }
    }

    /// Record a cache hit for `channel`.
    pub fn record_hit(&self, channel: &str) {
        self.stats
            .entry(channel.to_string())
            .or_default()
            .hits
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Record a cache miss for `channel`.
    pub fn record_miss(&self, channel: &str) {
        self.stats
            .entry(channel.to_string())
            .or_default()
            .misses
            .fetch_add(1, Ordering::Relaxed);
    }

    /// The current hit rate for `channel`, or `None` if it has no lookups yet.
    pub fn hit_rate(&self, channel: &str) -> Option<f64> {
        self.stats.get(channel).and_then(|c| {
            let hits = c.hits.load(Ordering::Relaxed);
            let lookups = hits + c.misses.load(Ordering::Relaxed);
            (lookups > 0).then(|| hits as f64 / lookups as f64)
        })
    }

    /// The TTL currently assigned to `channel` (its tuned value or the default).
    pub fn ttl_for(&self, channel: &str) -> Duration {
        self.ttls
            .get(channel)
            .map(|t| *t)
            .unwrap_or(self.default_ttl)
    }

    /// Compute (but do not apply) the adjustment a channel warrants right now.
    pub fn suggest(&self, channel: &str) -> Option<TtlAdjustment> {
        let counters = self.stats.get(channel)?;
        let hits = counters.hits.load(Ordering::Relaxed);
        let misses = counters.misses.load(Ordering::Relaxed);
        let lookups = hits + misses;
        if lookups < self.min_samples {
            return None;
        }
        let hit_rate = hits as f64 / lookups as f64;
        let current = self.ttl_for(channel);

        let target = if hit_rate < LOW_HIT_RATE {
            clamp(current / 2)
        } else if hit_rate > HIGH_HIT_RATE {
            clamp(current * 2)
        } else {
            return None;
        };
        if target == current {
            return None;
        }

        let from_secs = current.as_secs();
        let to_secs = target.as_secs();
        let direction = if to_secs > from_secs {
            "stable content"
        } else {
            "content changes fast"
        };
        Some(TtlAdjustment {
            channel: channel.to_string(),
            from_secs,
            to_secs,
            hit_rate,
            preset: nearest_preset(target),
            reason: format!("hit rate {:.0}% ({direction})", hit_rate * 100.0),
        })
    }

    /// Apply a previously-suggested adjustment, logging the change.
    pub fn apply(&self, adj: &TtlAdjustment) {
        self.ttls
            .insert(adj.channel.clone(), Duration::from_secs(adj.to_secs.max(1)));
        info!(
            channel = %adj.channel,
            "Auto-adjusted {} TTL from {}s to {}s (hit rate: {:.0}%)",
            adj.channel,
            adj.from_secs,
            adj.to_secs,
            adj.hit_rate * 100.0
        );
    }

    /// Suggest and apply adjustments for every channel that warrants one,
    /// returning the changes made.
    pub fn optimize(&self) -> Vec<TtlAdjustment> {
        let channels: Vec<String> = self.stats.iter().map(|e| e.key().clone()).collect();
        let mut applied = Vec::new();
        for channel in channels {
            if let Some(adj) = self.suggest(&channel) {
                self.apply(&adj);
                applied.push(adj);
            }
        }
        applied.sort_by(|a, b| a.channel.cmp(&b.channel));
        applied
    }

    /// All adjustments currently warranted, without applying them — used by the
    /// suggestions API.
    pub fn suggestions(&self) -> Vec<TtlAdjustment> {
        let mut v: Vec<TtlAdjustment> = self
            .stats
            .iter()
            .filter_map(|e| self.suggest(e.key()))
            .collect();
        v.sort_by(|a, b| a.channel.cmp(&b.channel));
        v
    }
}

fn clamp(d: Duration) -> Duration {
    d.clamp(FLOOR, CEILING)
}

/// Map a duration to the nearest named preset, for human-friendly labels.
fn nearest_preset(d: Duration) -> String {
    let secs = d.as_secs();
    let presets = [
        ("hot", CacheTtl::Hot.as_duration().as_secs()),
        ("warm", CacheTtl::Warm.as_duration().as_secs()),
        ("cold", CacheTtl::Cold.as_duration().as_secs()),
    ];
    presets
        .iter()
        .min_by_key(|(_, s)| s.abs_diff(secs))
        .map(|(name, _)| name.to_string())
        .unwrap_or_else(|| "custom".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(opt: &CacheOptimizer, channel: &str, hits: u64, misses: u64) {
        for _ in 0..hits {
            opt.record_hit(channel);
        }
        for _ in 0..misses {
            opt.record_miss(channel);
        }
    }

    #[test]
    fn hit_rate_computed() {
        let opt = CacheOptimizer::default();
        feed(&opt, "youtube", 8, 2);
        assert_eq!(opt.hit_rate("youtube"), Some(0.8));
        assert_eq!(opt.hit_rate("missing"), None);
    }

    #[test]
    fn high_hit_rate_lengthens_ttl() {
        let opt = CacheOptimizer::new(Duration::from_secs(60));
        feed(&opt, "youtube", 90, 10); // 90% hit rate
        let adj = opt.suggest("youtube").expect("should suggest");
        assert_eq!(adj.from_secs, 60);
        assert_eq!(adj.to_secs, 120);
        assert!(adj.reason.contains("stable"));
    }

    #[test]
    fn low_hit_rate_shortens_ttl() {
        let opt = CacheOptimizer::new(Duration::from_secs(3600));
        feed(&opt, "twitter", 1, 99); // 1% hit rate
        let adj = opt.suggest("twitter").expect("should suggest");
        assert_eq!(adj.from_secs, 3600);
        assert_eq!(adj.to_secs, 1800);
    }

    #[test]
    fn mid_hit_rate_is_left_alone() {
        let opt = CacheOptimizer::new(Duration::from_secs(60));
        feed(&opt, "reddit", 5, 5); // 50%
        assert!(opt.suggest("reddit").is_none());
    }

    #[test]
    fn not_enough_samples_no_change() {
        let opt = CacheOptimizer::default();
        feed(&opt, "x", 9, 0); // only 9 lookups < MIN_SAMPLES
        assert!(opt.suggest("x").is_none());
    }

    #[test]
    fn optimize_applies_and_persists() {
        let opt = CacheOptimizer::new(Duration::from_secs(60));
        feed(&opt, "youtube", 90, 10);
        let applied = opt.optimize();
        assert_eq!(applied.len(), 1);
        assert_eq!(opt.ttl_for("youtube"), Duration::from_secs(120));
    }

    #[test]
    fn ttl_clamped_to_floor() {
        let opt = CacheOptimizer::new(Duration::from_secs(40));
        feed(&opt, "fast", 0, 100); // 0% hit rate -> halve to 20 -> clamp to 30
        let adj = opt.suggest("fast").unwrap();
        assert_eq!(adj.to_secs, 30);
    }
}
