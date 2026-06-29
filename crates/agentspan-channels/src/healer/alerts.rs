//! Alert system.
//!
//! When a channel has been broken for longer than [`ALERT_AFTER`], the healer
//! builds an [`Alert`] and hands it to [`AlertManager::send`], which logs at
//! ERROR level and posts a webhook notification (Discord or Slack, auto-detected
//! from the URL). Alerts are rate-limited to one per channel per
//! [`ALERT_COOLDOWN`] so a persistently-down channel can't spam the webhook.

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::Serialize;
use serde_json::{json, Value};
use tracing::{error, warn};

/// How long a channel must stay continuously broken before it triggers an alert.
pub const ALERT_AFTER: Duration = Duration::from_secs(5 * 60);

/// Minimum spacing between alerts for the same channel.
pub const ALERT_COOLDOWN: Duration = Duration::from_secs(3600);

/// Alert severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AlertSeverity {
    /// Degraded but possibly self-recovering.
    Warning,
    /// Hard-down; needs attention.
    Critical,
}

/// An alert about a broken channel.
#[derive(Debug, Clone)]
pub struct Alert {
    /// Channel the alert is about.
    pub channel: String,
    /// How serious it is.
    pub severity: AlertSeverity,
    /// What went wrong.
    pub message: String,
    /// Suggested remediation.
    pub suggested_fix: String,
    /// Link to the channel's docs.
    pub docs_link: String,
    /// When the alert was created.
    pub sent_at: Instant,
}

impl Alert {
    /// Build an alert for `channel`, deriving the docs link from its name.
    pub fn new(
        channel: impl Into<String>,
        severity: AlertSeverity,
        message: impl Into<String>,
        suggested_fix: impl Into<String>,
    ) -> Self {
        let channel = channel.into();
        let docs_link = format!("https://agentspan.dev/docs/channels/{channel}");
        Self {
            channel,
            severity,
            message: message.into(),
            suggested_fix: suggested_fix.into(),
            docs_link,
            sent_at: Instant::now(),
        }
    }
}

/// A serializable record of an alert that was sent (for the API / dashboard).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AlertRecord {
    /// Channel the alert was about.
    pub channel: String,
    /// Severity at the time.
    pub severity: AlertSeverity,
    /// Message that was sent.
    pub message: String,
    /// Suggested fix that was included.
    pub suggested_fix: String,
    /// Docs link that was included.
    pub docs_link: String,
    /// Wall-clock time the alert was sent.
    pub sent_at: DateTime<Utc>,
}

/// Sends webhook alerts for broken channels, rate-limited per channel. Cheap to
/// clone (shared state behind `Arc`).
#[derive(Clone)]
pub struct AlertManager {
    webhook_url: Option<String>,
    last_sent: Arc<DashMap<String, Instant>>,
    history: Arc<RwLock<Vec<AlertRecord>>>,
    cooldown: Duration,
}

impl std::fmt::Debug for AlertManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AlertManager")
            .field("webhook_configured", &self.webhook_url.is_some())
            .field("alerts_sent", &self.alerts_sent())
            .finish_non_exhaustive()
    }
}

impl Default for AlertManager {
    fn default() -> Self {
        Self::new(None)
    }
}

impl AlertManager {
    /// Create a manager that posts to `webhook_url` (if any).
    pub fn new(webhook_url: Option<String>) -> Self {
        Self {
            webhook_url,
            last_sent: Arc::new(DashMap::new()),
            history: Arc::new(RwLock::new(Vec::new())),
            cooldown: ALERT_COOLDOWN,
        }
    }

    /// Read the webhook URL from `AGENTSPAN_ALERT_WEBHOOK`, if set and non-empty.
    pub fn from_env() -> Self {
        Self::new(
            std::env::var("AGENTSPAN_ALERT_WEBHOOK")
                .ok()
                .filter(|s| !s.is_empty()),
        )
    }

    /// Override the per-channel cooldown (mainly for tests).
    pub fn with_cooldown(mut self, cooldown: Duration) -> Self {
        self.cooldown = cooldown;
        self
    }

    /// Whether a webhook destination is configured.
    pub fn webhook_configured(&self) -> bool {
        self.webhook_url.is_some()
    }

    /// Whether an alert for `channel` is allowed now (cooldown elapsed).
    pub fn should_send(&self, channel: &str) -> bool {
        match self.last_sent.get(channel) {
            Some(t) => t.elapsed() >= self.cooldown,
            None => true,
        }
    }

    /// All alerts sent so far, oldest first.
    pub fn history(&self) -> Vec<AlertRecord> {
        self.history.read().expect("alerts lock poisoned").clone()
    }

    /// Total number of alerts sent.
    pub fn alerts_sent(&self) -> usize {
        self.history.read().expect("alerts lock poisoned").len()
    }

    /// Build the webhook JSON payload, matched to the provider in `url`.
    ///
    /// Discord expects `{"content": ...}`; Slack and most generic webhooks accept
    /// `{"text": ...}`.
    pub fn webhook_payload(url: &str, alert: &Alert) -> Value {
        let text = format!(
            "🩹 AgentSpan alert [{:?}] — channel `{}` is broken\n{}\nSuggested fix: {}\nDocs: {}",
            alert.severity, alert.channel, alert.message, alert.suggested_fix, alert.docs_link
        );
        if url.contains("discord") {
            json!({ "content": text })
        } else {
            json!({ "text": text })
        }
    }

    /// Send an alert if the per-channel cooldown allows.
    ///
    /// Logs at ERROR, posts the webhook (best-effort — a failed POST does not
    /// abort the alert), records it in history, and returns whether it was sent.
    pub async fn send(&self, alert: Alert) -> bool {
        if !self.should_send(&alert.channel) {
            return false;
        }

        error!(
            channel = %alert.channel,
            severity = ?alert.severity,
            suggested_fix = %alert.suggested_fix,
            docs = %alert.docs_link,
            "channel alert: {}",
            alert.message
        );

        if let Some(url) = &self.webhook_url {
            let payload = Self::webhook_payload(url, &alert);
            let client = crate::http::default_client();
            if let Err(e) = client.post(url).json(&payload).send().await {
                warn!(error = %e, channel = %alert.channel, "failed to post alert webhook");
            }
        }

        self.last_sent.insert(alert.channel.clone(), Instant::now());
        self.history
            .write()
            .expect("alerts lock poisoned")
            .push(AlertRecord {
                channel: alert.channel,
                severity: alert.severity,
                message: alert.message,
                suggested_fix: alert.suggested_fix,
                docs_link: alert.docs_link,
                sent_at: Utc::now(),
            });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn alert(channel: &str) -> Alert {
        Alert::new(
            channel,
            AlertSeverity::Critical,
            "all backends down",
            "run agentspan repair",
        )
    }

    #[test]
    fn webhook_payload_matches_provider() {
        let a = alert("twitter");
        let discord = AlertManager::webhook_payload("https://discord.com/api/webhooks/xyz", &a);
        assert!(discord.get("content").is_some());
        assert!(discord.get("text").is_none());

        let slack = AlertManager::webhook_payload("https://hooks.slack.com/services/xyz", &a);
        assert!(slack.get("text").is_some());
        assert!(slack.get("content").is_none());
    }

    #[test]
    fn alert_derives_docs_link() {
        let a = alert("reddit");
        assert_eq!(a.docs_link, "https://agentspan.dev/docs/channels/reddit");
    }

    #[tokio::test]
    async fn send_without_webhook_records_history() {
        let mgr = AlertManager::new(None);
        assert!(!mgr.webhook_configured());
        assert!(mgr.send(alert("github")).await);
        assert_eq!(mgr.alerts_sent(), 1);
        assert_eq!(mgr.history()[0].channel, "github");
        assert_eq!(mgr.history()[0].severity, AlertSeverity::Critical);
    }

    #[tokio::test]
    async fn cooldown_suppresses_repeat_alerts() {
        let mgr = AlertManager::new(None);
        assert!(mgr.send(alert("spotify")).await);
        assert!(!mgr.should_send("spotify"));
        // Second alert within the cooldown is suppressed.
        assert!(!mgr.send(alert("spotify")).await);
        assert_eq!(mgr.alerts_sent(), 1);
        // A different channel is unaffected.
        assert!(mgr.send(alert("discord")).await);
        assert_eq!(mgr.alerts_sent(), 2);
    }

    #[tokio::test]
    async fn expired_cooldown_allows_resend() {
        let mgr = AlertManager::new(None).with_cooldown(Duration::from_millis(0));
        assert!(mgr.send(alert("maps")).await);
        assert!(mgr.should_send("maps"));
        assert!(mgr.send(alert("maps")).await);
        assert_eq!(mgr.alerts_sent(), 2);
    }

    #[tokio::test]
    async fn send_posts_to_configured_webhook() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let mgr = AlertManager::new(Some(server.uri()));
        assert!(mgr.send(alert("weather")).await);

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        let body: Value = serde_json::from_slice(&requests[0].body).unwrap();
        // server.uri() isn't a discord URL, so the Slack-style "text" field is used.
        assert!(body.get("text").is_some());
    }
}
