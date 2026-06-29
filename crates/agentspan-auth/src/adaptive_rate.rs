//! Per-platform rate limits that learn from `429 Too Many Requests`.
//!
//! Every platform starts from a baseline requests-per-minute (Twitter is stingy,
//! Reddit a bit less so, most public APIs more generous). When a channel gets a
//! 429 we back its current limit off by 20%; after a quiet spell with no 429s we
//! ease it back up toward the baseline. The result is a limit that converges on
//! whatever the platform actually tolerates without a human guessing the number.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use serde::Serialize;
use tracing::{info, warn};

/// Multiply the current limit by this on a 429.
const BACKOFF: f64 = 0.80;

/// Multiply the current limit by this when recovering.
const RECOVERY: f64 = 1.20;

/// How long without a 429 before we start easing the limit back up.
pub const RECOVERY_AFTER: Duration = Duration::from_secs(600);

/// A limit never drops below this, regardless of how many 429s we see.
const MIN_PER_MINUTE: u32 = 5;

/// Baseline per-minute limit for platforms we have a prior for.
fn baseline_profiles() -> HashMap<&'static str, u32> {
    HashMap::from([
        ("twitter", 100),
        ("reddit", 60),
        ("github", 80),
        ("youtube", 100),
        ("bilibili", 60),
        ("linkedin", 30),
    ])
}

/// The adaptive limit state for one channel.
#[derive(Debug, Clone)]
struct PlatformLimit {
    baseline: u32,
    current: u32,
    last_429: Option<Instant>,
    last_change: Option<Instant>,
}

/// A serializable snapshot of a channel's adaptive limit.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RateProfile {
    pub channel: String,
    pub baseline_per_minute: u32,
    pub current_per_minute: u32,
    /// True when the current limit has been backed off below the baseline.
    pub throttled: bool,
}

/// Learns a per-platform rate limit from observed 429 responses.
#[derive(Debug, Clone)]
pub struct AdaptiveRateLimiter {
    limits: Arc<DashMap<String, PlatformLimit>>,
    profiles: Arc<HashMap<&'static str, u32>>,
    default_per_minute: u32,
}

impl Default for AdaptiveRateLimiter {
    fn default() -> Self {
        Self::new(120)
    }
}

impl AdaptiveRateLimiter {
    /// Create a limiter; channels without a known profile start at
    /// `default_per_minute`.
    pub fn new(default_per_minute: u32) -> Self {
        Self {
            limits: Arc::new(DashMap::new()),
            profiles: Arc::new(baseline_profiles()),
            default_per_minute,
        }
    }

    /// The baseline (un-throttled) per-minute limit for a channel.
    pub fn baseline_for(&self, channel: &str) -> u32 {
        self.profiles
            .get(channel)
            .copied()
            .unwrap_or(self.default_per_minute)
    }

    fn entry(&self, channel: &str) -> dashmap::mapref::one::RefMut<'_, String, PlatformLimit> {
        let baseline = self.baseline_for(channel);
        self.limits
            .entry(channel.to_string())
            .or_insert_with(|| PlatformLimit {
                baseline,
                current: baseline,
                last_429: None,
                last_change: None,
            })
    }

    /// The current per-minute limit for `channel`.
    pub fn current_limit(&self, channel: &str) -> u32 {
        self.limits
            .get(channel)
            .map(|l| l.current)
            .unwrap_or_else(|| self.baseline_for(channel))
    }

    /// Record a 429 for `channel`, backing its limit off by 20%. Returns the new
    /// limit.
    pub fn record_429(&self, channel: &str) -> u32 {
        self.record_429_at(channel, Instant::now())
    }

    /// [`record_429`](Self::record_429) with an explicit clock, for tests.
    pub fn record_429_at(&self, channel: &str, now: Instant) -> u32 {
        let mut limit = self.entry(channel);
        let reduced = ((limit.current as f64) * BACKOFF).floor() as u32;
        limit.current = reduced.max(MIN_PER_MINUTE);
        limit.last_429 = Some(now);
        limit.last_change = Some(now);
        warn!(
            channel,
            limit = limit.current,
            "rate limit backed off after 429"
        );
        limit.current
    }

    /// Ease `channel`'s limit back toward its baseline if it has been quiet for
    /// [`RECOVERY_AFTER`]. Returns the new limit if it changed.
    pub fn recover(&self, channel: &str) -> Option<u32> {
        self.recover_at(channel, Instant::now())
    }

    /// [`recover`](Self::recover) with an explicit clock, for tests.
    pub fn recover_at(&self, channel: &str, now: Instant) -> Option<u32> {
        let mut limit = self.limits.get_mut(channel)?;
        if limit.current >= limit.baseline {
            return None;
        }
        let quiet_enough = limit
            .last_429
            .map(|t| now.duration_since(t) >= RECOVERY_AFTER)
            .unwrap_or(true);
        if !quiet_enough {
            return None;
        }
        let raised = ((limit.current as f64) * RECOVERY).ceil() as u32;
        limit.current = raised.min(limit.baseline);
        limit.last_change = Some(now);
        info!(
            channel,
            limit = limit.current,
            "rate limit eased back toward baseline"
        );
        Some(limit.current)
    }

    /// Snapshot of every channel's adaptive profile, sorted by channel.
    pub fn profiles(&self) -> Vec<RateProfile> {
        let mut v: Vec<RateProfile> = self
            .limits
            .iter()
            .map(|e| {
                let l = e.value();
                RateProfile {
                    channel: e.key().clone(),
                    baseline_per_minute: l.baseline,
                    current_per_minute: l.current,
                    throttled: l.current < l.baseline,
                }
            })
            .collect();
        v.sort_by(|a, b| a.channel.cmp(&b.channel));
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_platforms_have_profiles() {
        let rl = AdaptiveRateLimiter::default();
        assert_eq!(rl.baseline_for("twitter"), 100);
        assert_eq!(rl.baseline_for("reddit"), 60);
        // Unknown channel falls back to the default.
        assert_eq!(rl.baseline_for("unknownsite"), 120);
    }

    #[test]
    fn a_429_reduces_the_limit_by_twenty_percent() {
        let rl = AdaptiveRateLimiter::default();
        assert_eq!(rl.current_limit("twitter"), 100);
        assert_eq!(rl.record_429("twitter"), 80);
        assert_eq!(rl.record_429("twitter"), 64);
    }

    #[test]
    fn limit_never_drops_below_floor() {
        let rl = AdaptiveRateLimiter::new(6);
        for _ in 0..20 {
            rl.record_429("x");
        }
        assert_eq!(rl.current_limit("x"), MIN_PER_MINUTE);
    }

    #[test]
    fn recovery_waits_for_the_quiet_window() {
        let rl = AdaptiveRateLimiter::default();
        let now = Instant::now();
        rl.record_429_at("reddit", now); // 60 -> 48
        assert_eq!(rl.current_limit("reddit"), 48);
        // Too soon: no recovery.
        assert!(rl
            .recover_at("reddit", now + Duration::from_secs(60))
            .is_none());
        // After the window: eases up.
        let raised = rl
            .recover_at("reddit", now + RECOVERY_AFTER + Duration::from_secs(1))
            .unwrap();
        assert!(raised > 48 && raised <= 60);
    }

    #[test]
    fn recovery_never_exceeds_baseline() {
        let rl = AdaptiveRateLimiter::default();
        let mut now = Instant::now();
        rl.record_429_at("github", now); // 80 -> 64
        for _ in 0..20 {
            now += RECOVERY_AFTER + Duration::from_secs(1);
            rl.recover_at("github", now);
        }
        assert_eq!(rl.current_limit("github"), rl.baseline_for("github"));
    }

    #[test]
    fn profiles_report_throttled_state() {
        let rl = AdaptiveRateLimiter::default();
        rl.record_429("twitter");
        let profiles = rl.profiles();
        let twitter = profiles.iter().find(|p| p.channel == "twitter").unwrap();
        assert!(twitter.throttled);
        assert_eq!(twitter.baseline_per_minute, 100);
        assert_eq!(twitter.current_per_minute, 80);
    }

    #[test]
    fn recovery_noop_when_not_throttled() {
        let rl = AdaptiveRateLimiter::default();
        // Never throttled -> nothing to recover.
        assert!(rl.recover("twitter").is_none());
    }
}
