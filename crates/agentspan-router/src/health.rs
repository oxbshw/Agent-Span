//! Health check aggregator.

use agentspan_core::types::{BackendHealth, ProbeResult, ProbeStatus};
use chrono::Utc;
use serde::Serialize;

/// Aggregates health checks across backends.
#[derive(Debug, Default, Clone)]
pub struct HealthCheck;

/// A per-backend entry inside a channel report.
#[derive(Debug, Clone, Serialize)]
pub struct BackendHealthEntry {
    pub backend: String,
    pub status: String,
    pub message: String,
    pub latency_ms: u64,
}

/// A per-channel health summary.
#[derive(Debug, Clone, Serialize)]
pub struct ChannelHealthEntry {
    pub channel: String,
    pub status: String,
    pub backends: Vec<BackendHealthEntry>,
}

/// Aggregated health report across all channels.
#[derive(Debug, Clone, Serialize)]
pub struct HealthReport {
    pub status: String,
    pub total_channels: usize,
    pub healthy_channels: usize,
    pub channels: Vec<ChannelHealthEntry>,
}

impl HealthCheck {
    pub fn new() -> Self {
        Self
    }

    /// Build a BackendHealth from a probe result.
    pub fn build(
        &self,
        backend_name: impl Into<String>,
        probe: ProbeResult,
        latency_ms: u64,
    ) -> BackendHealth {
        BackendHealth {
            backend_name: backend_name.into(),
            probe,
            latency_ms,
            last_checked: Utc::now(),
        }
    }

    /// Aggregate a set of backend health checks into a single status probe.
    pub async fn check_all(&self, healths: Vec<BackendHealth>) -> ProbeResult {
        let ok_count = healths
            .iter()
            .filter(|h| h.probe.status == ProbeStatus::Ok)
            .count();
        let total = healths.len();

        if ok_count == total && total > 0 {
            ProbeResult {
                status: ProbeStatus::Ok,
                message: format!("All {} backends healthy", total),
                version: None,
                hint: None,
            }
        } else if ok_count > 0 {
            ProbeResult {
                status: ProbeStatus::Warn,
                message: format!("{}/{} backends healthy", ok_count, total),
                version: None,
                hint: Some("Run 'agentspan doctor' for details".to_string()),
            }
        } else {
            ProbeResult {
                status: ProbeStatus::Broken,
                message: format!("0/{} backends healthy", total),
                version: None,
                hint: Some("No healthy backends; check installation and configuration".to_string()),
            }
        }
    }

    /// Build a full health report from per-channel backend health checks.
    pub fn report(&self, channels: Vec<(String, Vec<BackendHealth>)>) -> HealthReport {
        let total_channels = channels.len();
        let mut healthy_channels = 0;
        let mut channel_entries = Vec::with_capacity(total_channels);

        for (channel_name, backends) in channels {
            let backend_entries: Vec<BackendHealthEntry> = backends
                .iter()
                .map(|h| BackendHealthEntry {
                    backend: h.backend_name.clone(),
                    status: format!("{:?}", h.probe.status),
                    message: h.probe.message.clone(),
                    latency_ms: h.latency_ms,
                })
                .collect();

            let channel_status = channel_status(&backends);
            if channel_status == ProbeStatus::Ok {
                healthy_channels += 1;
            }

            channel_entries.push(ChannelHealthEntry {
                channel: channel_name,
                status: format!("{:?}", channel_status),
                backends: backend_entries,
            });
        }

        let overall = if healthy_channels == total_channels {
            ProbeStatus::Ok
        } else if healthy_channels > 0 {
            ProbeStatus::Warn
        } else {
            ProbeStatus::Broken
        };

        HealthReport {
            status: format!("{:?}", overall),
            total_channels,
            healthy_channels,
            channels: channel_entries,
        }
    }
}

/// Determine a channel's overall status from its backend health checks.
fn channel_status(backends: &[BackendHealth]) -> ProbeStatus {
    if backends.is_empty() {
        return ProbeStatus::Error;
    }
    if backends.iter().any(|h| h.probe.status == ProbeStatus::Ok) {
        ProbeStatus::Ok
    } else if backends.iter().any(|h| h.probe.status == ProbeStatus::Warn) {
        ProbeStatus::Warn
    } else if backends
        .iter()
        .all(|h| h.probe.status == ProbeStatus::Missing)
    {
        ProbeStatus::Missing
    } else {
        ProbeStatus::Broken
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn all_healthy_returns_ok() {
        let checker = HealthCheck::new();
        let healths = vec![
            checker.build("a", ProbeResult::ok("a", "1"), 10),
            checker.build("b", ProbeResult::ok("b", "2"), 20),
        ];
        let result = checker.check_all(healths).await;
        assert_eq!(result.status, ProbeStatus::Ok);
    }

    #[tokio::test]
    async fn partial_healthy_returns_warn() {
        let checker = HealthCheck::new();
        let healths = vec![
            checker.build("a", ProbeResult::ok("a", "1"), 10),
            checker.build("b", ProbeResult::missing("b", "install"), 0),
        ];
        let result = checker.check_all(healths).await;
        assert_eq!(result.status, ProbeStatus::Warn);
    }

    #[tokio::test]
    async fn no_healthy_returns_broken() {
        let checker = HealthCheck::new();
        let healths = vec![
            checker.build("a", ProbeResult::missing("a", "install"), 0),
            checker.build("b", ProbeResult::broken("b", "exec", "fix"), 0),
        ];
        let result = checker.check_all(healths).await;
        assert_eq!(result.status, ProbeStatus::Broken);
    }

    #[test]
    fn report_summarizes_channels() {
        let checker = HealthCheck::new();
        let channels = vec![
            (
                "web".to_string(),
                vec![checker.build("jina", ProbeResult::ok("jina", "1"), 5)],
            ),
            (
                "github".to_string(),
                vec![checker.build("gh", ProbeResult::missing("gh", "Install GitHub CLI"), 0)],
            ),
        ];

        let report = checker.report(channels);
        assert_eq!(report.status, "Warn");
        assert_eq!(report.total_channels, 2);
        assert_eq!(report.healthy_channels, 1);
        assert_eq!(report.channels[0].status, "Ok");
        assert_eq!(report.channels[1].status, "Missing");
        assert_eq!(report.channels[0].backends[0].backend, "jina");
    }

    #[test]
    fn report_all_healthy_is_ok() {
        let checker = HealthCheck::new();
        let channels = vec![(
            "web".to_string(),
            vec![checker.build("jina", ProbeResult::ok("jina", "1"), 5)],
        )];

        let report = checker.report(channels);
        assert_eq!(report.status, "Ok");
        assert_eq!(report.healthy_channels, 1);
    }

    #[test]
    fn report_no_channels_is_ok() {
        let checker = HealthCheck::new();
        let report = checker.report(vec![]);
        assert_eq!(report.status, "Ok");
        assert_eq!(report.total_channels, 0);
    }
}
