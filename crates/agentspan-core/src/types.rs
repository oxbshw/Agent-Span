//! Shared types for AgentSpan channels, backends, and search results.

use serde::{Deserialize, Serialize};

/// Content returned from read operations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema", derive(utoipa::ToSchema))]
pub struct Content {
    pub url: String,
    pub title: Option<String>,
    pub body: String,
    pub metadata: serde_json::Value,
    pub cached: bool,
}

/// Search result from platform search.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema", derive(utoipa::ToSchema))]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub author: Option<String>,
    pub timestamp: Option<String>,
    pub metadata: serde_json::Value,
}

/// Read options per request.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReadOptions {
    pub force_refresh: bool, // Skip cache
    pub timeout_secs: u64,   // Per-request timeout
    pub format: OutputFormat,
}

/// Search options.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SearchOptions {
    pub limit: usize,        // Max results
    pub force_refresh: bool, // Skip cache
    pub timeout_secs: u64,
}

/// Output format for read operations.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputFormat {
    #[default]
    Markdown,
    Json,
    Html,
}

/// Platform tier classification.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    /// Zero-config, works out of the box.
    #[default]
    Zero,
    /// Needs API key or browser auth.
    One,
    /// Complex enterprise setup.
    Two,
}

/// Probe result — 6-state classification (inspired by Agent Reach).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProbeStatus {
    /// Backend is healthy and responsive.
    #[default]
    Ok,
    /// Works but has issues (unauthenticated, outdated, warning exit code).
    Warn,
    /// Binary/tool not found on PATH.
    Missing,
    /// Binary exists but can't execute or returned an unexpected error.
    Broken,
    /// Command timed out.
    Timeout,
    /// Other internal or configuration error.
    Error,
}

/// Result of probing a backend or command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeResult {
    pub status: ProbeStatus,
    pub message: String,
    pub version: Option<String>,
    pub hint: Option<String>,
}

impl ProbeResult {
    pub fn ok(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            status: ProbeStatus::Ok,
            message: format!("{} is healthy", name.into()),
            version: Some(version.into()),
            hint: None,
        }
    }

    pub fn missing(name: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            status: ProbeStatus::Missing,
            message: format!("{} not found on PATH", name.into()),
            version: None,
            hint: Some(hint.into()),
        }
    }

    pub fn warn(
        name: impl Into<String>,
        message: impl Into<String>,
        hint: impl Into<String>,
    ) -> Self {
        Self {
            status: ProbeStatus::Warn,
            message: format!("{} warning: {}", name.into(), message.into()),
            version: None,
            hint: Some(hint.into()),
        }
    }

    pub fn broken(
        name: impl Into<String>,
        message: impl Into<String>,
        hint: impl Into<String>,
    ) -> Self {
        Self {
            status: ProbeStatus::Broken,
            message: format!("{} broken: {}", name.into(), message.into()),
            version: None,
            hint: Some(hint.into()),
        }
    }

    pub fn timeout(name: impl Into<String>, duration: impl Into<String>) -> Self {
        Self {
            status: ProbeStatus::Timeout,
            message: format!("{} timed out after {}", name.into(), duration.into()),
            version: None,
            hint: Some("Check network or command execution time".to_string()),
        }
    }

    pub fn error(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status: ProbeStatus::Error,
            message: format!("{} error: {}", name.into(), message.into()),
            version: None,
            hint: Some("Check configuration and logs".to_string()),
        }
    }
}

/// Backend health status.
#[derive(Debug, Clone)]
pub struct BackendHealth {
    pub backend_name: String,
    pub probe: ProbeResult,
    pub latency_ms: u64,
    pub last_checked: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_format_default_is_markdown() {
        assert_eq!(OutputFormat::default(), OutputFormat::Markdown);
    }

    #[test]
    fn probe_result_ok_has_ok_status() {
        let p = ProbeResult::ok("yt-dlp", "2024.01.01");
        assert_eq!(p.status, ProbeStatus::Ok);
        assert_eq!(p.version, Some("2024.01.01".to_string()));
        assert!(p.hint.is_none());
    }

    #[test]
    fn probe_result_missing_has_hint() {
        let p = ProbeResult::missing("gh", "Install GitHub CLI");
        assert_eq!(p.status, ProbeStatus::Missing);
        assert!(p.hint.is_some());
    }

    #[test]
    fn probe_result_constructors_cover_all_fields() {
        let warn = ProbeResult::warn("twitter", "unauthenticated", "Add credentials");
        assert_eq!(warn.status, ProbeStatus::Warn);
        assert!(warn.message.contains("twitter"));
        assert_eq!(warn.version, None);
        assert!(warn.hint.as_deref().unwrap().contains("credentials"));

        let broken = ProbeResult::broken("gh", "exit 1", "Reinstall");
        assert_eq!(broken.status, ProbeStatus::Broken);
        assert!(broken.message.contains("exit 1"));
        assert!(broken.hint.as_deref().unwrap().contains("Reinstall"));

        let timeout = ProbeResult::timeout("youtube", "30s");
        assert_eq!(timeout.status, ProbeStatus::Timeout);
        assert!(timeout.message.contains("30s"));
        assert!(timeout.hint.is_some());

        let error = ProbeResult::error("web", "empty command");
        assert_eq!(error.status, ProbeStatus::Error);
        assert!(error.message.contains("empty command"));
        assert!(error.hint.is_some());
    }

    #[test]
    fn probe_status_default_is_ok() {
        assert_eq!(ProbeStatus::default(), ProbeStatus::Ok);
    }

    #[test]
    fn all_probe_status_variants_exist() {
        let variants = [
            ProbeStatus::Ok,
            ProbeStatus::Warn,
            ProbeStatus::Missing,
            ProbeStatus::Broken,
            ProbeStatus::Timeout,
            ProbeStatus::Error,
        ];
        // Ensure each variant is distinct.
        assert_eq!(variants.len(), 6);
        let unique: std::collections::HashSet<_> = variants.iter().copied().collect();
        assert_eq!(unique.len(), 6);
    }

    #[test]
    fn probe_status_serializes_and_deserializes() {
        for status in [
            ProbeStatus::Ok,
            ProbeStatus::Warn,
            ProbeStatus::Missing,
            ProbeStatus::Broken,
            ProbeStatus::Timeout,
            ProbeStatus::Error,
        ] {
            let yaml = serde_yaml::to_string(&status).unwrap();
            let parsed: ProbeStatus = serde_yaml::from_str(&yaml).unwrap();
            assert_eq!(parsed, status);
        }
    }
}
