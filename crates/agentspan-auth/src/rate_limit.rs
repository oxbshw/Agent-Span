//! Sliding-window rate limiting, keyed by API key (or any opaque id).
//!
//! Each key keeps a log of recent request timestamps. On every check the log is
//! pruned to the last day, then the per-minute and per-day windows are counted.
//! A limit of `0` is treated as "unlimited".

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use dashmap::DashMap;

const MINUTE: Duration = Duration::from_secs(60);
const DAY: Duration = Duration::from_secs(86_400);

/// The outcome of a rate-limit check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitDecision {
    /// Whether the request is permitted.
    pub allowed: bool,
    /// Suggested wait before retrying, when denied.
    pub retry_after: Option<Duration>,
    /// Remaining requests in the current minute window (saturating).
    pub remaining_minute: u32,
}

/// A sliding-window rate limiter shared across requests.
#[derive(Debug, Default)]
pub struct RateLimiter {
    windows: DashMap<String, VecDeque<Instant>>,
}

impl RateLimiter {
    /// Create an empty rate limiter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check (and, when allowed, record) a request for `key`.
    pub fn check(&self, key: &str, per_minute: u32, per_day: u32) -> RateLimitDecision {
        self.check_at(key, per_minute, per_day, Instant::now())
    }

    /// Testable variant of [`check`](Self::check) with an explicit "now".
    pub fn check_at(
        &self,
        key: &str,
        per_minute: u32,
        per_day: u32,
        now: Instant,
    ) -> RateLimitDecision {
        let mut log = self.windows.entry(key.to_string()).or_default();

        // Drop anything older than the longest window we care about.
        while let Some(front) = log.front() {
            if now.duration_since(*front) >= DAY {
                log.pop_front();
            } else {
                break;
            }
        }

        let minute_count = log
            .iter()
            .filter(|t| now.duration_since(**t) < MINUTE)
            .count() as u32;
        let day_count = log.len() as u32;

        // Per-day limit.
        if per_day != 0 && day_count >= per_day {
            let oldest = log.front().copied().unwrap_or(now);
            let retry = DAY.saturating_sub(now.duration_since(oldest));
            return RateLimitDecision {
                allowed: false,
                retry_after: Some(retry),
                remaining_minute: 0,
            };
        }

        // Per-minute limit.
        if per_minute != 0 && minute_count >= per_minute {
            let oldest_in_minute = log
                .iter()
                .find(|t| now.duration_since(**t) < MINUTE)
                .copied()
                .unwrap_or(now);
            let retry = MINUTE.saturating_sub(now.duration_since(oldest_in_minute));
            return RateLimitDecision {
                allowed: false,
                retry_after: Some(retry),
                remaining_minute: 0,
            };
        }

        log.push_back(now);
        let remaining_minute = if per_minute == 0 {
            u32::MAX
        } else {
            per_minute.saturating_sub(minute_count + 1)
        };
        RateLimitDecision {
            allowed: true,
            retry_after: None,
            remaining_minute,
        }
    }

    /// Forget the history for a key (e.g. on revocation).
    pub fn reset(&self, key: &str) {
        self.windows.remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_until_minute_limit() {
        let rl = RateLimiter::new();
        let now = Instant::now();
        for i in 0..3 {
            let d = rl.check_at("k", 3, 0, now);
            assert!(d.allowed, "request {i} should be allowed");
        }
        let denied = rl.check_at("k", 3, 0, now);
        assert!(!denied.allowed);
        assert!(denied.retry_after.is_some());
    }

    #[test]
    fn window_slides_after_a_minute() {
        let rl = RateLimiter::new();
        let now = Instant::now();
        for _ in 0..3 {
            assert!(rl.check_at("k", 3, 0, now).allowed);
        }
        assert!(!rl.check_at("k", 3, 0, now).allowed);
        // 61 seconds later the minute window has slid.
        let later = now + Duration::from_secs(61);
        assert!(rl.check_at("k", 3, 0, later).allowed);
    }

    #[test]
    fn day_limit_is_enforced() {
        let rl = RateLimiter::new();
        let now = Instant::now();
        // Spread requests so the per-minute window never trips.
        for i in 0..5 {
            let t = now + Duration::from_secs(i * 120);
            assert!(rl.check_at("k", 0, 5, t).allowed);
        }
        let t = now + Duration::from_secs(5 * 120);
        let denied = rl.check_at("k", 0, 5, t);
        assert!(!denied.allowed);
    }

    #[test]
    fn zero_limits_are_unlimited() {
        let rl = RateLimiter::new();
        let now = Instant::now();
        for _ in 0..1000 {
            assert!(rl.check_at("k", 0, 0, now).allowed);
        }
    }

    #[test]
    fn keys_are_independent() {
        let rl = RateLimiter::new();
        let now = Instant::now();
        assert!(rl.check_at("a", 1, 0, now).allowed);
        assert!(!rl.check_at("a", 1, 0, now).allowed);
        // Different key has its own budget.
        assert!(rl.check_at("b", 1, 0, now).allowed);
    }

    #[test]
    fn reset_clears_history() {
        let rl = RateLimiter::new();
        let now = Instant::now();
        assert!(rl.check_at("k", 1, 0, now).allowed);
        assert!(!rl.check_at("k", 1, 0, now).allowed);
        rl.reset("k");
        assert!(rl.check_at("k", 1, 0, now).allowed);
    }

    #[test]
    fn remaining_minute_decrements() {
        let rl = RateLimiter::new();
        let now = Instant::now();
        let d1 = rl.check_at("k", 5, 0, now);
        assert_eq!(d1.remaining_minute, 4);
        let d2 = rl.check_at("k", 5, 0, now);
        assert_eq!(d2.remaining_minute, 3);
    }
}
