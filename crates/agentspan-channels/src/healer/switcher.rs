//! Auto-switch backends.
//!
//! When a channel's *active* backend fails [`FAILURE_THRESHOLD`] probe cycles in
//! a row, [`BackendSwitcher::observe`] promotes the next healthy backend in the
//! preference order, records the switch, and logs
//! `Auto-switched twitter from twitter-cli to opencli`. The chosen backend is
//! exposed via [`BackendSwitcher::active_backend`] (operators / the router can
//! consult it) and the switch log is surfaced at
//! `GET /api/v1/admin/auto-switches`.
//!
//! The switcher tracks the active backend's consecutive failures *itself* (keyed
//! by channel) rather than reusing the channel-level counter, because a channel
//! can still aggregate as healthy via a fallback while its preferred backend is
//! down — and that is precisely the case we want to switch on.

use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::Serialize;
use tracing::info;

use agentspan_core::types::ProbeStatus;

use crate::healer::monitor::{is_failure, FAILURE_THRESHOLD};

/// A record of one automatic backend promotion.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AutoSwitch {
    /// Channel whose backend was switched.
    pub channel: String,
    /// Backend that was failing.
    pub from: String,
    /// Backend that was promoted.
    pub to: String,
    /// Human-readable reason for the switch.
    pub reason: String,
    /// When the switch happened.
    pub at: DateTime<Utc>,
}

/// Decides and records automatic backend promotions. Cheap to clone (shared
/// state behind `Arc`).
#[derive(Debug, Clone, Default)]
pub struct BackendSwitcher {
    /// channel -> currently-active backend name (overrides the default order).
    active: Arc<DashMap<String, String>>,
    /// channel -> consecutive failures of the *active* backend.
    fails: Arc<DashMap<String, u32>>,
    /// Append-only log of switches performed.
    switches: Arc<RwLock<Vec<AutoSwitch>>>,
}

impl BackendSwitcher {
    /// Create an empty switcher.
    pub fn new() -> Self {
        Self::default()
    }

    /// The backend currently chosen for `channel`, falling back to `default`
    /// (the channel's preferred backend) when no switch has occurred.
    pub fn active_backend(&self, channel: &str, default: &str) -> String {
        self.active
            .get(channel)
            .map(|e| e.value().clone())
            .unwrap_or_else(|| default.to_string())
    }

    /// Feed a channel's latest per-backend statuses (preference order) to the
    /// switcher.
    ///
    /// Returns `Some(AutoSwitch)` when the active backend has now failed
    /// [`FAILURE_THRESHOLD`] times in a row and a later healthy backend was
    /// promoted in its place.
    pub fn observe(&self, channel: &str, backends: &[(String, ProbeStatus)]) -> Option<AutoSwitch> {
        if backends.is_empty() {
            return None;
        }
        let default_backend = backends[0].0.clone();
        let current = self.active_backend(channel, &default_backend);

        // Treat an active backend that no longer appears in the list as failing.
        let current_failing = backends
            .iter()
            .find(|(n, _)| *n == current)
            .map(|(_, s)| is_failure(*s))
            .unwrap_or(true);

        if !current_failing {
            self.fails.insert(channel.to_string(), 0);
            return None;
        }

        let failures = {
            let mut entry = self.fails.entry(channel.to_string()).or_insert(0);
            *entry = entry.saturating_add(1);
            *entry
        };
        if failures < FAILURE_THRESHOLD {
            return None;
        }

        // Promote the first healthy backend that isn't the current one.
        let next = backends
            .iter()
            .find(|(n, s)| *n != current && !is_failure(*s))
            .map(|(n, _)| n.clone())?;

        let switch = AutoSwitch {
            channel: channel.to_string(),
            from: current,
            to: next.clone(),
            reason: format!("active backend failed {failures} consecutive probes"),
            at: Utc::now(),
        };

        // Reset the counter against the newly-active backend and persist.
        self.fails.insert(channel.to_string(), 0);
        self.active.insert(channel.to_string(), next);
        info!(
            channel = %switch.channel,
            from = %switch.from,
            to = %switch.to,
            "Auto-switched {} from {} to {}",
            switch.channel, switch.from, switch.to
        );
        self.switches
            .write()
            .expect("switcher lock poisoned")
            .push(switch.clone());
        Some(switch)
    }

    /// Every switch recorded so far, oldest first.
    pub fn switches(&self) -> Vec<AutoSwitch> {
        self.switches
            .read()
            .expect("switcher lock poisoned")
            .clone()
    }

    /// Count switches performed in the last 24 hours.
    pub fn switches_today(&self) -> usize {
        let cutoff = Utc::now() - chrono::Duration::hours(24);
        self.switches
            .read()
            .expect("switcher lock poisoned")
            .iter()
            .filter(|s| s.at >= cutoff)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(name: &str, status: ProbeStatus) -> (String, ProbeStatus) {
        (name.to_string(), status)
    }

    #[test]
    fn no_switch_below_threshold() {
        let sw = BackendSwitcher::new();
        let backends = [
            b("primary", ProbeStatus::Broken),
            b("backup", ProbeStatus::Ok),
        ];
        assert!(sw.observe("twitter", &backends).is_none());
        assert!(sw.observe("twitter", &backends).is_none());
        assert!(sw.switches().is_empty());
    }

    #[test]
    fn switch_promotes_next_healthy_after_threshold() {
        let sw = BackendSwitcher::new();
        let backends = [
            b("twitter-cli", ProbeStatus::Broken),
            b("opencli", ProbeStatus::Ok),
        ];
        assert!(sw.observe("twitter", &backends).is_none());
        assert!(sw.observe("twitter", &backends).is_none());
        let switch = sw
            .observe("twitter", &backends)
            .expect("should switch on 3rd failure");
        assert_eq!(switch.from, "twitter-cli");
        assert_eq!(switch.to, "opencli");
        assert_eq!(sw.active_backend("twitter", "twitter-cli"), "opencli");
        assert_eq!(sw.switches().len(), 1);
        assert_eq!(sw.switches_today(), 1);
    }

    #[test]
    fn no_switch_without_a_healthy_alternative() {
        let sw = BackendSwitcher::new();
        let backends = [
            b("primary", ProbeStatus::Broken),
            b("backup", ProbeStatus::Missing),
        ];
        for _ in 0..5 {
            assert!(sw.observe("reddit", &backends).is_none());
        }
        assert!(sw.switches().is_empty());
    }

    #[test]
    fn healthy_active_backend_resets_counter() {
        let sw = BackendSwitcher::new();
        let failing = [
            b("primary", ProbeStatus::Broken),
            b("backup", ProbeStatus::Ok),
        ];
        let healthy = [b("primary", ProbeStatus::Ok), b("backup", ProbeStatus::Ok)];
        sw.observe("maps", &failing);
        sw.observe("maps", &failing);
        // A healthy probe in between resets the streak, so two more failures
        // still don't trigger a switch.
        sw.observe("maps", &healthy);
        sw.observe("maps", &failing);
        sw.observe("maps", &failing);
        assert!(sw.switches().is_empty());
    }

    #[test]
    fn active_backend_defaults_until_switched() {
        let sw = BackendSwitcher::new();
        assert_eq!(sw.active_backend("github", "gh-cli"), "gh-cli");
    }

    #[test]
    fn switch_chains_to_third_backend_when_promoted_one_also_fails() {
        let sw = BackendSwitcher::new();
        // First switch: a -> b.
        let stage1 = [
            b("a", ProbeStatus::Broken),
            b("b", ProbeStatus::Ok),
            b("c", ProbeStatus::Ok),
        ];
        for _ in 0..3 {
            sw.observe("ch", &stage1);
        }
        assert_eq!(sw.active_backend("ch", "a"), "b");

        // Now b fails too; should promote c.
        let stage2 = [
            b("a", ProbeStatus::Broken),
            b("b", ProbeStatus::Broken),
            b("c", ProbeStatus::Ok),
        ];
        let mut switched_to = None;
        for _ in 0..3 {
            if let Some(s) = sw.observe("ch", &stage2) {
                switched_to = Some(s.to);
            }
        }
        assert_eq!(switched_to.as_deref(), Some("c"));
        assert_eq!(sw.switches().len(), 2);
    }
}
