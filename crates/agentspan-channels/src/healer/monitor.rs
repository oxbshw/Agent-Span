//! Background health monitor.
//!
//! [`HealthMonitor`] keeps the latest [`HealthSnapshot`] for every monitored
//! channel in a shared [`DashMap`]. Each probe cycle the [`Healer`] feeds the
//! outcome of one channel into [`HealthMonitor::record`], which maintains the
//! consecutive-failure counter and the success/failure timestamps that the
//! switcher and alerter reason about.
//!
//! [`Healer`]: crate::healer::Healer

use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;

use agentspan_core::types::ProbeStatus;

/// Consecutive failures of a channel's active backend after which it is
/// considered broken and eligible for auto-switching.
pub const FAILURE_THRESHOLD: u32 = 3;

/// A point-in-time view of one channel's health.
///
/// `Instant` fields are wall-clock-free monotonic stamps; the API layer converts
/// them to relative "seconds ago" for serialization (see
/// [`SnapshotView`](crate::healer::SnapshotView)).
#[derive(Debug, Clone)]
pub struct HealthSnapshot {
    /// Channel this snapshot describes.
    pub channel: String,
    /// Aggregated status from the channel's last probe.
    pub status: ProbeStatus,
    /// Latency of the representative backend, in milliseconds.
    pub latency_ms: u64,
    /// Number of probe cycles the channel has failed in a row.
    pub consecutive_failures: u32,
    /// When the channel last probed healthy.
    pub last_success: Option<Instant>,
    /// When the channel last probed unhealthy.
    pub last_failure: Option<Instant>,
    /// When the current down-streak began (cleared on recovery). Used to decide
    /// how long a channel has *continuously* been broken.
    pub down_since: Option<Instant>,
    /// Message from the first failing backend, if any.
    pub error_message: Option<String>,
    /// How many times the healer has attempted to auto-heal this channel.
    pub auto_heal_attempts: u32,
    /// When the channel was last auto-healed (switch or repair succeeded).
    pub auto_healed_at: Option<Instant>,
}

impl HealthSnapshot {
    fn new(channel: &str) -> Self {
        Self {
            channel: channel.to_string(),
            status: ProbeStatus::Ok,
            latency_ms: 0,
            consecutive_failures: 0,
            last_success: None,
            last_failure: None,
            down_since: None,
            error_message: None,
            auto_heal_attempts: 0,
            auto_healed_at: None,
        }
    }

    /// Hard-down states that warrant human attention.
    pub fn needs_attention(&self) -> bool {
        is_failure(self.status)
    }

    /// Whether the most recent probe considered the channel usable.
    pub fn is_healthy(&self) -> bool {
        matches!(self.status, ProbeStatus::Ok | ProbeStatus::Warn)
    }

    /// How long the channel has been *continuously* failing, if down now.
    pub fn down_for(&self) -> Option<Duration> {
        if self.is_healthy() {
            return None;
        }
        self.down_since.map(|t| t.elapsed())
    }
}

/// Classify a [`ProbeStatus`] as a failed probe.
pub fn is_failure(status: ProbeStatus) -> bool {
    matches!(
        status,
        ProbeStatus::Broken | ProbeStatus::Missing | ProbeStatus::Error | ProbeStatus::Timeout
    )
}

/// Shared, cheaply-cloneable store of the latest [`HealthSnapshot`] per channel.
#[derive(Debug, Clone)]
pub struct HealthMonitor {
    snapshots: Arc<DashMap<String, HealthSnapshot>>,
    interval: Duration,
}

impl Default for HealthMonitor {
    fn default() -> Self {
        Self::new(Duration::from_secs(30))
    }
}

impl HealthMonitor {
    /// Create a monitor that the background loop probes every `interval`.
    pub fn new(interval: Duration) -> Self {
        Self {
            snapshots: Arc::new(DashMap::new()),
            interval,
        }
    }

    /// The configured probe interval.
    pub fn interval(&self) -> Duration {
        self.interval
    }

    /// Record the outcome of one channel probe.
    ///
    /// On failure the consecutive-failure counter is bumped (and the down-streak
    /// start recorded on the first failure); on success it resets. Returns the
    /// channel's updated `consecutive_failures`.
    pub fn record(
        &self,
        channel: &str,
        status: ProbeStatus,
        latency_ms: u64,
        error_message: Option<String>,
    ) -> u32 {
        let now = Instant::now();
        let mut entry = self
            .snapshots
            .entry(channel.to_string())
            .or_insert_with(|| HealthSnapshot::new(channel));

        entry.status = status;
        entry.latency_ms = latency_ms;
        entry.error_message = error_message;

        if is_failure(status) {
            if entry.consecutive_failures == 0 {
                entry.down_since = Some(now);
            }
            entry.consecutive_failures = entry.consecutive_failures.saturating_add(1);
            entry.last_failure = Some(now);
        } else {
            entry.consecutive_failures = 0;
            entry.down_since = None;
            entry.last_success = Some(now);
        }
        entry.consecutive_failures
    }

    /// Mark a channel as auto-healed after a successful switch or repair.
    pub fn mark_healed(&self, channel: &str) {
        if let Some(mut entry) = self.snapshots.get_mut(channel) {
            entry.auto_heal_attempts = entry.auto_heal_attempts.saturating_add(1);
            entry.auto_healed_at = Some(Instant::now());
        }
    }

    /// The latest snapshot for `channel`, if it has been probed.
    pub fn snapshot(&self, channel: &str) -> Option<HealthSnapshot> {
        self.snapshots.get(channel).map(|e| e.value().clone())
    }

    /// All snapshots, sorted by channel name for stable output.
    pub fn snapshots(&self) -> Vec<HealthSnapshot> {
        let mut v: Vec<HealthSnapshot> = self.snapshots.iter().map(|e| e.value().clone()).collect();
        v.sort_by(|a, b| a.channel.cmp(&b.channel));
        v
    }

    /// Number of channels with a recorded snapshot.
    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    /// Whether any channel has been probed yet.
    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    /// Channels whose last probe was fully healthy (`Ok`).
    pub fn healthy_count(&self) -> usize {
        self.snapshots
            .iter()
            .filter(|e| e.value().status == ProbeStatus::Ok)
            .count()
    }

    /// Channels currently in a hard-down state.
    pub fn needs_attention_count(&self) -> usize {
        self.snapshots
            .iter()
            .filter(|e| e.value().needs_attention())
            .count()
    }

    /// Channels that have been auto-healed at least once.
    pub fn healed_count(&self) -> usize {
        self.snapshots
            .iter()
            .filter(|e| e.value().auto_healed_at.is_some())
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_failure_starts_down_streak_and_counts() {
        let mon = HealthMonitor::new(Duration::from_secs(30));
        let n = mon.record("twitter", ProbeStatus::Broken, 12, Some("boom".into()));
        assert_eq!(n, 1);
        let snap = mon.snapshot("twitter").unwrap();
        assert_eq!(snap.consecutive_failures, 1);
        assert!(snap.down_since.is_some());
        assert!(snap.last_failure.is_some());
        assert!(snap.last_success.is_none());
        assert_eq!(snap.error_message.as_deref(), Some("boom"));
        assert!(snap.needs_attention());
    }

    #[test]
    fn consecutive_failures_accumulate_then_reset_on_success() {
        let mon = HealthMonitor::new(Duration::from_secs(30));
        mon.record("reddit", ProbeStatus::Timeout, 0, None);
        mon.record("reddit", ProbeStatus::Timeout, 0, None);
        let n = mon.record("reddit", ProbeStatus::Broken, 0, None);
        assert_eq!(n, FAILURE_THRESHOLD);

        let n = mon.record("reddit", ProbeStatus::Ok, 5, None);
        assert_eq!(n, 0);
        let snap = mon.snapshot("reddit").unwrap();
        assert_eq!(snap.consecutive_failures, 0);
        assert!(snap.down_since.is_none());
        assert!(snap.last_success.is_some());
        assert!(snap.is_healthy());
        assert!(!snap.needs_attention());
    }

    #[test]
    fn warn_is_healthy_not_a_failure() {
        let mon = HealthMonitor::new(Duration::from_secs(30));
        let n = mon.record("openai", ProbeStatus::Warn, 3, None);
        assert_eq!(n, 0, "Warn must not count as a failure");
        let snap = mon.snapshot("openai").unwrap();
        assert!(snap.is_healthy());
        assert!(snap.down_for().is_none());
    }

    #[test]
    fn mark_healed_records_attempt_and_timestamp() {
        let mon = HealthMonitor::new(Duration::from_secs(30));
        mon.record("twitter", ProbeStatus::Broken, 0, None);
        mon.mark_healed("twitter");
        mon.mark_healed("twitter");
        let snap = mon.snapshot("twitter").unwrap();
        assert_eq!(snap.auto_heal_attempts, 2);
        assert!(snap.auto_healed_at.is_some());
        assert_eq!(mon.healed_count(), 1);
    }

    #[test]
    fn counts_and_sorting_aggregate_across_channels() {
        let mon = HealthMonitor::new(Duration::from_secs(30));
        mon.record("web", ProbeStatus::Ok, 5, None);
        mon.record("github", ProbeStatus::Ok, 7, None);
        mon.record("twitter", ProbeStatus::Broken, 0, Some("down".into()));
        mon.record("spotify", ProbeStatus::Missing, 0, None);

        assert_eq!(mon.len(), 4);
        assert_eq!(mon.healthy_count(), 2);
        assert_eq!(mon.needs_attention_count(), 2);

        let names: Vec<String> = mon.snapshots().into_iter().map(|s| s.channel).collect();
        assert_eq!(names, vec!["github", "spotify", "twitter", "web"]);
    }

    #[test]
    fn down_for_tracks_streak_start_not_latest_failure() {
        let mon = HealthMonitor::new(Duration::from_secs(30));
        mon.record("x", ProbeStatus::Broken, 0, None);
        let first = mon.snapshot("x").unwrap().down_since.unwrap();
        // A subsequent failure must NOT reset the streak start, otherwise a
        // continuously-broken channel would never appear "down for >5m".
        mon.record("x", ProbeStatus::Broken, 0, None);
        let second = mon.snapshot("x").unwrap().down_since.unwrap();
        assert_eq!(first, second);
    }
}
