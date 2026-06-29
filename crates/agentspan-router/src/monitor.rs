//! Background health monitoring.
//!
//! [`HealthMonitor`] runs a user-supplied probe on a fixed interval, caches the
//! most recent [`HealthReport`], and broadcasts a [`HealthAlert`] whenever a
//! channel's status degrades (e.g. `Ok` -> `Broken`). It is intentionally
//! decoupled from how reports are produced: callers pass an async closure, so it
//! works equally well over a real [`crate::BackendRouter`] or a test stub.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use serde::Serialize;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tracing::warn;

use crate::health::HealthReport;

/// Capacity of the alert broadcast channel.
const ALERT_BUFFER: usize = 64;

/// An alert emitted when a channel's health changes for the worse.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HealthAlert {
    /// Channel whose status degraded.
    pub channel: String,
    /// Previous status (e.g. `"Ok"`).
    pub from: String,
    /// New, worse status (e.g. `"Broken"`).
    pub to: String,
}

/// Rank statuses by severity so we can tell "worse" from "better".
///
/// Mirrors the `Debug`-formatted [`agentspan_core::types::ProbeStatus`] strings
/// that [`HealthReport`] stores; unknown strings sort as most severe.
fn severity(status: &str) -> u8 {
    match status {
        "Ok" => 0,
        "Warn" => 1,
        "Missing" => 2,
        "Broken" => 3,
        "Error" => 4,
        _ => 5,
    }
}

/// Periodically probes channel health, caches the latest report, and alerts on
/// degradation. Cheap to clone (all state is shared behind `Arc`).
#[derive(Clone)]
pub struct HealthMonitor {
    interval: Duration,
    latest: Arc<RwLock<Option<HealthReport>>>,
    last_status: Arc<RwLock<HashMap<String, String>>>,
    alerts: broadcast::Sender<HealthAlert>,
}

impl std::fmt::Debug for HealthMonitor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HealthMonitor")
            .field("interval", &self.interval)
            .field("subscribers", &self.alerts.receiver_count())
            .finish_non_exhaustive()
    }
}

impl HealthMonitor {
    /// Create a monitor that probes every `interval`.
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            latest: Arc::new(RwLock::new(None)),
            last_status: Arc::new(RwLock::new(HashMap::new())),
            alerts: broadcast::channel(ALERT_BUFFER).0,
        }
    }

    /// The interval between probes.
    pub fn interval(&self) -> Duration {
        self.interval
    }

    /// Snapshot of the most recent health report, if any has been recorded.
    pub fn latest(&self) -> Option<HealthReport> {
        self.latest.read().expect("monitor lock poisoned").clone()
    }

    /// Subscribe to degradation alerts.
    pub fn subscribe(&self) -> broadcast::Receiver<HealthAlert> {
        self.alerts.subscribe()
    }

    /// Record a freshly collected report.
    ///
    /// Stores it as the latest snapshot, diffs each channel's status against the
    /// previous snapshot, broadcasts an alert for every degradation, and returns
    /// the alerts produced. Recoveries (a channel getting *better*) are tracked
    /// but do not alert.
    pub fn record(&self, report: HealthReport) -> Vec<HealthAlert> {
        let mut alerts = Vec::new();
        {
            let mut last = self.last_status.write().expect("monitor lock poisoned");
            for ch in &report.channels {
                if let Some(prev) = last.get(&ch.channel) {
                    if severity(&ch.status) > severity(prev) {
                        alerts.push(HealthAlert {
                            channel: ch.channel.clone(),
                            from: prev.clone(),
                            to: ch.status.clone(),
                        });
                    }
                }
                last.insert(ch.channel.clone(), ch.status.clone());
            }
        }

        *self.latest.write().expect("monitor lock poisoned") = Some(report);

        for alert in &alerts {
            warn!(channel = %alert.channel, from = %alert.from, to = %alert.to,
                "channel health degraded");
            let _ = self.alerts.send(alert.clone());
        }
        alerts
    }

    /// Spawn the background loop. `probe` is invoked every [`Self::interval`] to
    /// produce a fresh report, which is then passed to [`Self::record`]. Returns
    /// the task handle; drop or `abort` it to stop monitoring.
    pub fn spawn<F, Fut>(&self, probe: F) -> JoinHandle<()>
    where
        F: Fn() -> Fut + Send + 'static,
        Fut: Future<Output = HealthReport> + Send,
    {
        let this = self.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(this.interval);
            loop {
                ticker.tick().await;
                let report = probe().await;
                this.record(report);
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::health::{ChannelHealthEntry, HealthReport};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn report(entries: &[(&str, &str)]) -> HealthReport {
        let channels: Vec<ChannelHealthEntry> = entries
            .iter()
            .map(|(name, status)| ChannelHealthEntry {
                channel: name.to_string(),
                status: status.to_string(),
                backends: vec![],
            })
            .collect();
        let healthy = channels.iter().filter(|c| c.status == "Ok").count();
        HealthReport {
            status: "Ok".to_string(),
            total_channels: channels.len(),
            healthy_channels: healthy,
            channels,
        }
    }

    #[test]
    fn first_report_does_not_alert() {
        let monitor = HealthMonitor::new(Duration::from_secs(60));
        let alerts = monitor.record(report(&[("web", "Ok"), ("github", "Ok")]));
        assert!(alerts.is_empty());
        assert!(monitor.latest().is_some());
    }

    #[test]
    fn degradation_emits_alert() {
        let monitor = HealthMonitor::new(Duration::from_secs(60));
        monitor.record(report(&[("web", "Ok")]));
        let alerts = monitor.record(report(&[("web", "Broken")]));
        assert_eq!(
            alerts,
            vec![HealthAlert {
                channel: "web".to_string(),
                from: "Ok".to_string(),
                to: "Broken".to_string(),
            }]
        );
    }

    #[test]
    fn stable_status_does_not_alert() {
        let monitor = HealthMonitor::new(Duration::from_secs(60));
        monitor.record(report(&[("web", "Warn")]));
        let alerts = monitor.record(report(&[("web", "Warn")]));
        assert!(alerts.is_empty());
    }

    #[test]
    fn recovery_does_not_alert() {
        let monitor = HealthMonitor::new(Duration::from_secs(60));
        monitor.record(report(&[("web", "Broken")]));
        let alerts = monitor.record(report(&[("web", "Ok")]));
        assert!(alerts.is_empty(), "recovery should not alert: {alerts:?}");
    }

    #[tokio::test]
    async fn subscribers_receive_alerts() {
        let monitor = HealthMonitor::new(Duration::from_secs(60));
        let mut rx = monitor.subscribe();
        monitor.record(report(&[("web", "Ok")]));
        monitor.record(report(&[("web", "Missing")]));
        let alert = rx.recv().await.unwrap();
        assert_eq!(alert.channel, "web");
        assert_eq!(alert.to, "Missing");
    }

    #[tokio::test]
    async fn spawn_runs_probe_periodically() {
        let monitor = HealthMonitor::new(Duration::from_millis(5));
        let calls = Arc::new(AtomicUsize::new(0));
        let calls2 = calls.clone();
        let handle = monitor.spawn(move || {
            calls2.fetch_add(1, Ordering::SeqCst);
            async { report(&[("web", "Ok")]) }
        });

        tokio::time::sleep(Duration::from_millis(40)).await;
        handle.abort();

        assert!(
            calls.load(Ordering::SeqCst) >= 2,
            "probe should have run multiple times"
        );
        assert!(monitor.latest().is_some());
    }
}
