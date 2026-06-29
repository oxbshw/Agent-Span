//! Adaptive backend scoring.
//!
//! Health probes tell us whether a backend is *usable*; they don't tell us which
//! usable backend is *performing best right now*. This module tracks a rolling
//! latency (EWMA) and success rate per backend so the router can prefer the one
//! that's actually fast and reliable, while still trying unknown backends
//! optimistically (otherwise a new backend would never get a chance).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::Serialize;

/// Smoothing factor for the latency EWMA. Higher reacts faster to recent
/// samples; 0.3 is a reasonable middle ground (≈ last 3-4 calls dominate).
const EWMA_ALPHA: f64 = 0.3;

/// Score given to a backend we have no data for yet. Set high so unknown
/// backends are explored before we trust the numbers.
const OPTIMISTIC_SCORE: f64 = 1000.0;

/// Rolling performance stats for a single backend.
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct BackendStat {
    /// EWMA of observed latency in milliseconds; `None` until the first success.
    pub ewma_latency_ms: Option<f64>,
    /// Count of successful calls.
    pub successes: u64,
    /// Count of failed calls.
    pub failures: u64,
}

impl BackendStat {
    fn record_success(&mut self, latency_ms: u64) {
        let sample = latency_ms as f64;
        self.ewma_latency_ms = Some(match self.ewma_latency_ms {
            Some(prev) => EWMA_ALPHA * sample + (1.0 - EWMA_ALPHA) * prev,
            None => sample,
        });
        self.successes += 1;
    }

    fn record_failure(&mut self) {
        self.failures += 1;
    }

    /// Fraction of calls that succeeded. Defaults to `1.0` with no data so an
    /// unproven backend isn't penalised before it's been tried.
    pub fn success_rate(&self) -> f64 {
        let total = self.successes + self.failures;
        if total == 0 {
            1.0
        } else {
            self.successes as f64 / total as f64
        }
    }

    /// Higher is better. Success rate dominates (×1000) so a reliable-but-slower
    /// backend beats a fast-but-flaky one; latency only breaks ties. Latency is
    /// clamped so a single pathological sample can't swamp the ranking.
    pub fn score(&self) -> f64 {
        let latency_penalty = self.ewma_latency_ms.unwrap_or(0.0).min(OPTIMISTIC_SCORE);
        self.success_rate() * OPTIMISTIC_SCORE - latency_penalty
    }
}

/// Tracks [`BackendStat`]s by backend name. Cheap to clone (shared state).
#[derive(Debug, Clone, Default)]
pub struct BackendScorer {
    stats: Arc<Mutex<HashMap<String, BackendStat>>>,
}

impl BackendScorer {
    /// Create an empty scorer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a successful call and its latency.
    pub fn record_success(&self, backend: &str, latency_ms: u64) {
        let mut stats = self.stats.lock().expect("scorer lock poisoned");
        stats
            .entry(backend.to_string())
            .or_default()
            .record_success(latency_ms);
    }

    /// Record a failed call.
    pub fn record_failure(&self, backend: &str) {
        let mut stats = self.stats.lock().expect("scorer lock poisoned");
        stats
            .entry(backend.to_string())
            .or_default()
            .record_failure();
    }

    /// Current score for a backend; unknown backends get the optimistic score.
    pub fn score(&self, backend: &str) -> f64 {
        self.stats
            .lock()
            .expect("scorer lock poisoned")
            .get(backend)
            .map(BackendStat::score)
            .unwrap_or(OPTIMISTIC_SCORE)
    }

    /// Snapshot a backend's stats, if any have been recorded.
    pub fn snapshot(&self, backend: &str) -> Option<BackendStat> {
        self.stats
            .lock()
            .expect("scorer lock poisoned")
            .get(backend)
            .copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_backend_is_optimistic() {
        let scorer = BackendScorer::new();
        assert_eq!(scorer.score("never-seen"), OPTIMISTIC_SCORE);
        assert!(scorer.snapshot("never-seen").is_none());
    }

    #[test]
    fn success_rate_reflects_outcomes() {
        let mut stat = BackendStat::default();
        assert_eq!(stat.success_rate(), 1.0); // no data -> optimistic
        stat.record_success(10);
        stat.record_failure();
        stat.record_failure();
        // 1 of 3 succeeded.
        assert!((stat.success_rate() - 1.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn ewma_smooths_latency() {
        let mut stat = BackendStat::default();
        stat.record_success(100);
        assert_eq!(stat.ewma_latency_ms, Some(100.0));
        stat.record_success(0);
        // 0.3*0 + 0.7*100 = 70
        assert!((stat.ewma_latency_ms.unwrap() - 70.0).abs() < 1e-9);
    }

    #[test]
    fn higher_success_rate_scores_higher() {
        let mut reliable = BackendStat::default();
        reliable.record_success(50);
        reliable.record_success(50);

        let mut flaky = BackendStat::default();
        flaky.record_success(50);
        flaky.record_failure();

        assert!(reliable.score() > flaky.score());
    }

    #[test]
    fn lower_latency_breaks_ties() {
        let mut fast = BackendStat::default();
        fast.record_success(10);

        let mut slow = BackendStat::default();
        slow.record_success(200);

        // Same (perfect) success rate, so the faster one wins.
        assert!(fast.score() > slow.score());
    }

    #[test]
    fn scorer_records_through_shared_handle() {
        let scorer = BackendScorer::new();
        let clone = scorer.clone();
        clone.record_success("a", 5);
        clone.record_failure("a");
        let stat = scorer.snapshot("a").unwrap();
        assert_eq!(stat.successes, 1);
        assert_eq!(stat.failures, 1);
    }
}
