//! Self-healing subsystem.
//!
//! Fifty channels means fifty points of failure, so the gateway is built to fix
//! itself instead of paging a human. [`Healer`] ties four pieces together:
//!
//! - [`HealthMonitor`] probes every channel on an interval and keeps the latest
//!   [`HealthSnapshot`] per channel.
//! - [`BackendSwitcher`] promotes a healthy backend when the preferred one keeps
//!   failing.
//! - [`RepairManager`] reinstalls broken CLI tools.
//! - [`AlertManager`] fires a webhook when a channel stays down.
//!
//! [`Healer::report`] aggregates everything into a [`HealingReport`] served at
//! `GET /api/v1/admin/healing-report`.

pub mod alerts;
pub mod discoverer;
pub mod monitor;
pub mod repair;
pub mod switcher;

use std::time::Duration;

use serde::Serialize;
use tokio::task::JoinHandle;

use agentspan_core::types::{BackendHealth, ProbeStatus};

use crate::registry::ChannelRegistry;

pub use alerts::{Alert, AlertManager, AlertRecord, AlertSeverity};
pub use discoverer::{MissingChannelDetector, UnsupportedPlatform};
pub use monitor::{HealthMonitor, HealthSnapshot, FAILURE_THRESHOLD};
pub use repair::{infer_kind, RepairAttempt, RepairKind, RepairManager};
pub use switcher::{AutoSwitch, BackendSwitcher};

/// Default interval between background probe cycles.
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(30);

/// Top-level self-healing coordinator. Cheap to clone (all state shared).
#[derive(Debug, Clone)]
pub struct Healer {
    /// Per-channel health snapshots.
    pub monitor: HealthMonitor,
    /// Automatic backend promotion.
    pub switcher: BackendSwitcher,
    /// CLI tool repair.
    pub repair: RepairManager,
    /// Webhook alerting.
    pub alerts: AlertManager,
    /// Demand tracking for platforms we don't yet support.
    pub discoverer: MissingChannelDetector,
}

impl Default for Healer {
    fn default() -> Self {
        Self::new()
    }
}

impl Healer {
    /// Create a healer with the default 30s probe interval, reading the alert
    /// webhook from `AGENTSPAN_ALERT_WEBHOOK`.
    pub fn new() -> Self {
        Self::with_interval(DEFAULT_INTERVAL)
    }

    /// Create a healer with a custom probe interval.
    pub fn with_interval(interval: Duration) -> Self {
        Self {
            monitor: HealthMonitor::new(interval),
            switcher: BackendSwitcher::new(),
            repair: RepairManager::new(),
            alerts: AlertManager::from_env(),
            discoverer: MissingChannelDetector::new(),
        }
    }

    /// Probe every channel once (in parallel), update snapshots, auto-switch
    /// failing backends, and alert on channels that have stayed broken.
    pub async fn probe_once(&self, registry: &ChannelRegistry) {
        use futures::future::join_all;

        let channels = registry.list().to_vec();
        let results = join_all(channels.into_iter().map(|ch| async move {
            let healths = ch.check_health().await;
            (ch.name().to_string(), healths)
        }))
        .await;

        for (name, healths) in results {
            let status = aggregate_status(&healths);
            let latency = representative_latency(&healths);
            let error_message = healths
                .iter()
                .find(|h| monitor::is_failure(h.probe.status))
                .map(|h| h.probe.message.clone());

            self.monitor.record(&name, status, latency, error_message);

            // Auto-switch when the active backend keeps failing.
            let backends: Vec<(String, ProbeStatus)> = healths
                .iter()
                .map(|h| (h.backend_name.clone(), h.probe.status))
                .collect();
            if self.switcher.observe(&name, &backends).is_some() {
                self.monitor.mark_healed(&name);
            }

            // Alert when broken beyond the grace window.
            if let Some(snapshot) = self.monitor.snapshot(&name) {
                if snapshot
                    .down_for()
                    .is_some_and(|d| d >= alerts::ALERT_AFTER)
                {
                    let severity = match status {
                        ProbeStatus::Missing | ProbeStatus::Broken => AlertSeverity::Critical,
                        _ => AlertSeverity::Warning,
                    };
                    let msg = snapshot
                        .error_message
                        .clone()
                        .unwrap_or_else(|| format!("{name} is unhealthy ({status:?})"));
                    let fix = suggested_fix(&name, status);
                    self.alerts
                        .send(Alert::new(&name, severity, msg, fix))
                        .await;
                }
            }
        }
    }

    /// Spawn the background monitoring loop. Probes immediately, then
    /// every [`HealthMonitor::interval`]. Returns the task handle; abort it to
    /// stop monitoring.
    pub fn spawn(&self, registry: ChannelRegistry) -> JoinHandle<()> {
        let this = self.clone();
        let interval = self.monitor.interval().max(Duration::from_millis(1));
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                this.probe_once(&registry).await;
            }
        })
    }

    /// All automatic backend switches (exposed at `GET /admin/auto-switches`).
    pub fn auto_switches(&self) -> Vec<AutoSwitch> {
        self.switcher.switches()
    }

    /// Aggregate the current self-healing status.
    pub fn report(&self) -> HealingReport {
        let snapshots = self.monitor.snapshots();
        let channels: Vec<SnapshotView> = snapshots.iter().map(SnapshotView::from).collect();
        HealingReport {
            channels_monitored: snapshots.len(),
            healthy: self.monitor.healthy_count(),
            healed_automatically: self.monitor.healed_count(),
            needs_attention: self.monitor.needs_attention_count(),
            auto_switches_today: self.switcher.switches_today(),
            repairs_attempted: self.repair.total_attempted(),
            repairs_succeeded: self.repair.total_succeeded(),
            alerts_sent: self.alerts.alerts_sent(),
            channels,
            recent_alerts: self.alerts.history(),
        }
    }
}

/// Aggregate a channel's per-backend health into a single status.
///
/// A channel is healthy if *any* backend is `Ok` (fallbacks exist for exactly
/// this reason); otherwise it reflects the best available signal.
fn aggregate_status(healths: &[BackendHealth]) -> ProbeStatus {
    if healths.is_empty() {
        return ProbeStatus::Error;
    }
    if healths.iter().any(|h| h.probe.status == ProbeStatus::Ok) {
        ProbeStatus::Ok
    } else if healths.iter().any(|h| h.probe.status == ProbeStatus::Warn) {
        ProbeStatus::Warn
    } else if healths
        .iter()
        .all(|h| h.probe.status == ProbeStatus::Missing)
    {
        ProbeStatus::Missing
    } else if healths
        .iter()
        .any(|h| h.probe.status == ProbeStatus::Timeout)
    {
        ProbeStatus::Timeout
    } else {
        ProbeStatus::Broken
    }
}

/// Pick a representative latency: the first healthy backend's, else the first.
fn representative_latency(healths: &[BackendHealth]) -> u64 {
    healths
        .iter()
        .find(|h| h.probe.status == ProbeStatus::Ok)
        .or_else(|| healths.first())
        .map(|h| h.latency_ms)
        .unwrap_or(0)
}

/// A short remediation hint for an unhealthy channel.
fn suggested_fix(channel: &str, status: ProbeStatus) -> String {
    match status {
        ProbeStatus::Missing => {
            format!("Install the backend for `{channel}` (try `agentspan repair {channel}`)")
        }
        ProbeStatus::Broken => {
            format!("`{channel}` backend is broken; try `agentspan repair {channel}` or check credentials")
        }
        ProbeStatus::Timeout => {
            format!("`{channel}` is timing out; check network or proxy settings")
        }
        _ => format!("Run `agentspan doctor {channel}` for details"),
    }
}

/// Self-healing summary. The top-level counters match the documented
/// `GET /api/v1/admin/healing-report` contract; `channels` and `recent_alerts`
/// are additive detail for the dashboard.
#[derive(Debug, Clone, Serialize)]
pub struct HealingReport {
    /// Number of channels with a recorded snapshot.
    pub channels_monitored: usize,
    /// Channels whose last probe was fully healthy.
    pub healthy: usize,
    /// Channels auto-healed at least once.
    pub healed_automatically: usize,
    /// Channels currently in a hard-down state.
    pub needs_attention: usize,
    /// Auto-switches performed in the last 24h.
    pub auto_switches_today: usize,
    /// Total repair attempts.
    pub repairs_attempted: usize,
    /// Total successful repairs.
    pub repairs_succeeded: usize,
    /// Total alerts sent.
    pub alerts_sent: usize,
    /// Per-channel detail (sorted by name).
    pub channels: Vec<SnapshotView>,
    /// Alert history, oldest first.
    pub recent_alerts: Vec<AlertRecord>,
}

/// A serializable view of a [`HealthSnapshot`] (monotonic `Instant`s become
/// relative "seconds ago").
#[derive(Debug, Clone, Serialize)]
pub struct SnapshotView {
    /// Channel name.
    pub channel: String,
    /// Aggregated status, e.g. `"Ok"` / `"Broken"`.
    pub status: String,
    /// Representative latency in ms.
    pub latency_ms: u64,
    /// Consecutive failed probe cycles.
    pub consecutive_failures: u32,
    /// Auto-heal attempts so far.
    pub auto_heal_attempts: u32,
    /// Whether this channel has been auto-healed.
    pub healed: bool,
    /// Error from the last failing backend, if any.
    pub error_message: Option<String>,
    /// Seconds since the last healthy probe.
    pub last_success_secs_ago: Option<u64>,
    /// Seconds since the last failing probe.
    pub last_failure_secs_ago: Option<u64>,
}

impl From<&HealthSnapshot> for SnapshotView {
    fn from(s: &HealthSnapshot) -> Self {
        Self {
            channel: s.channel.clone(),
            status: format!("{:?}", s.status),
            latency_ms: s.latency_ms,
            consecutive_failures: s.consecutive_failures,
            auto_heal_attempts: s.auto_heal_attempts,
            healed: s.auto_healed_at.is_some(),
            error_message: s.error_message.clone(),
            last_success_secs_ago: s.last_success.map(|t| t.elapsed().as_secs()),
            last_failure_secs_ago: s.last_failure.map(|t| t.elapsed().as_secs()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use async_trait::async_trait;
    use chrono::Utc;

    use agentspan_core::backend::Backend;
    use agentspan_core::channel::Channel;
    use agentspan_core::error::ChannelError;
    use agentspan_core::types::{
        BackendHealth, Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier,
    };

    /// A channel whose `check_health` returns a fixed, controllable result.
    #[derive(Debug)]
    struct FakeChannel {
        name: String,
        backends: Vec<(String, ProbeStatus)>,
    }

    impl FakeChannel {
        fn new(name: &str, backends: &[(&str, ProbeStatus)]) -> Self {
            Self {
                name: name.to_string(),
                backends: backends.iter().map(|(n, s)| (n.to_string(), *s)).collect(),
            }
        }
    }

    #[async_trait]
    impl Channel for FakeChannel {
        fn name(&self) -> &str {
            &self.name
        }
        fn description(&self) -> &str {
            "fake"
        }
        fn can_handle(&self, _url: &str) -> bool {
            false
        }
        fn tier(&self) -> Tier {
            Tier::Zero
        }
        fn backends(&self) -> Vec<Box<dyn Backend>> {
            vec![]
        }
        async fn read(&self, _url: &str, _opts: ReadOptions) -> Result<Content, ChannelError> {
            unimplemented!()
        }
        async fn search(
            &self,
            _q: &str,
            _opts: SearchOptions,
        ) -> Result<Vec<SearchResult>, ChannelError> {
            unimplemented!()
        }
        async fn check_health(&self) -> Vec<BackendHealth> {
            self.backends
                .iter()
                .map(|(n, s)| BackendHealth {
                    backend_name: n.clone(),
                    probe: ProbeResult {
                        status: *s,
                        message: format!("{n} is {s:?}"),
                        version: None,
                        hint: None,
                    },
                    latency_ms: 10,
                    last_checked: Utc::now(),
                })
                .collect()
        }
    }

    fn registry(channels: Vec<FakeChannel>) -> ChannelRegistry {
        let chans: Vec<Arc<dyn Channel>> = channels
            .into_iter()
            .map(|c| Arc::new(c) as Arc<dyn Channel>)
            .collect();
        ChannelRegistry::new(chans)
    }

    #[test]
    fn aggregate_status_prefers_any_ok() {
        let hc = |s: ProbeStatus| BackendHealth {
            backend_name: "b".into(),
            probe: ProbeResult {
                status: s,
                message: String::new(),
                version: None,
                hint: None,
            },
            latency_ms: 1,
            last_checked: Utc::now(),
        };
        assert_eq!(
            aggregate_status(&[hc(ProbeStatus::Broken), hc(ProbeStatus::Ok)]),
            ProbeStatus::Ok
        );
        assert_eq!(
            aggregate_status(&[hc(ProbeStatus::Missing), hc(ProbeStatus::Missing)]),
            ProbeStatus::Missing
        );
        assert_eq!(aggregate_status(&[]), ProbeStatus::Error);
    }

    #[tokio::test]
    async fn probe_once_populates_snapshots() {
        let reg = registry(vec![
            FakeChannel::new("alpha", &[("primary", ProbeStatus::Ok)]),
            FakeChannel::new("beta", &[("primary", ProbeStatus::Broken)]),
        ]);
        let healer = Healer::with_interval(Duration::from_secs(30));
        healer.probe_once(&reg).await;

        assert_eq!(healer.monitor.len(), 2);
        assert_eq!(healer.monitor.healthy_count(), 1);
        assert_eq!(healer.monitor.needs_attention_count(), 1);
        let beta = healer.monitor.snapshot("beta").unwrap();
        assert!(beta.error_message.is_some());
    }

    #[tokio::test]
    async fn probe_once_switches_after_threshold() {
        // Primary broken, backup healthy: the channel aggregates as Ok, but the
        // active backend keeps failing, so a switch must eventually fire.
        let reg = registry(vec![FakeChannel::new(
            "twitter",
            &[
                ("twitter-cli", ProbeStatus::Broken),
                ("opencli", ProbeStatus::Ok),
            ],
        )]);
        let healer = Healer::new();
        for _ in 0..FAILURE_THRESHOLD {
            healer.probe_once(&reg).await;
        }
        let switches = healer.auto_switches();
        assert_eq!(switches.len(), 1);
        assert_eq!(switches[0].from, "twitter-cli");
        assert_eq!(switches[0].to, "opencli");
        // The switch counts as an auto-heal.
        assert_eq!(
            healer
                .monitor
                .snapshot("twitter")
                .unwrap()
                .auto_heal_attempts,
            1
        );
    }

    #[tokio::test]
    async fn report_aggregates_counts() {
        let reg = registry(vec![
            FakeChannel::new("a", &[("x", ProbeStatus::Ok)]),
            FakeChannel::new("b", &[("x", ProbeStatus::Ok)]),
            FakeChannel::new("c", &[("x", ProbeStatus::Broken)]),
        ]);
        let healer = Healer::new();
        healer.probe_once(&reg).await;

        let report = healer.report();
        assert_eq!(report.channels_monitored, 3);
        assert_eq!(report.healthy, 2);
        assert_eq!(report.needs_attention, 1);
        assert_eq!(report.channels.len(), 3);
    }

    #[tokio::test]
    async fn report_serializes_with_documented_fields() {
        let healer = Healer::new();
        let json = serde_json::to_value(healer.report()).unwrap();
        for key in [
            "channels_monitored",
            "healthy",
            "healed_automatically",
            "needs_attention",
            "auto_switches_today",
            "repairs_attempted",
            "repairs_succeeded",
            "alerts_sent",
        ] {
            assert!(json.get(key).is_some(), "missing report field: {key}");
        }
    }

    #[tokio::test]
    async fn spawn_runs_at_least_once() {
        let reg = registry(vec![FakeChannel::new("a", &[("x", ProbeStatus::Ok)])]);
        let healer = Healer::with_interval(Duration::from_millis(5));
        let handle = healer.spawn(reg);
        // Poll rather than assume a fixed wall-clock delay is enough: under
        // coverage instrumentation (ptrace) the spawned task gets far less CPU,
        // so wait until it has run a cycle, up to a generous cap.
        let mut ran = false;
        for _ in 0..200 {
            if !healer.monitor.is_empty() {
                ran = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        handle.abort();
        assert!(ran, "healer did not run a probe cycle within the timeout");
    }
}
