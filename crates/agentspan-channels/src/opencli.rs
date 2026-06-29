//! OpenCLI cross-channel backend.
//!
//! OpenCLI (github.com/jackwener/opencli) drives the user's real Chrome via a
//! browser-bridge extension + local daemon, reusing existing login sessions —
//! zero per-platform configuration, desktop-only. It is the backend that makes
//! login-gated platforms (Reddit, XiaoHongShu, Twitter, Bilibili, LinkedIn)
//! actually return data without per-site cookie wrangling.
//!
//! Probing notes (mirrors Agent Reach's verified behaviour):
//!   - `opencli doctor` AUTO-STARTS the daemon (side effect) — health checks use
//!     `opencli daemon status` (a pure query) instead.
//!   - Exit codes are always 0; status is parsed from text output.
//!   - A "disconnected" extension may just be a sleeping service worker that
//!     wakes on the first real command, so we treat installed-but-not-connected
//!     as a warning rather than a hard failure.

use std::time::Duration;

use async_trait::async_trait;
use tracing::debug;

use agentspan_core::backend::Backend;
use agentspan_core::error::BackendError;
use agentspan_core::types::{
    Content, ProbeResult, ProbeStatus, ReadOptions, SearchOptions, SearchResult,
};
use agentspan_probe::{ProbeEngine, ProbeTarget};

const OPENCLI_PACKAGE: &str = "@jackwener/opencli";

/// Probe the shared OpenCLI install + daemon/extension state.
///
/// Returns one [`ProbeResult`] reused by every per-platform OpenCLI backend.
pub async fn probe_opencli() -> ProbeResult {
    let engine = ProbeEngine::new(Duration::from_secs(5));
    let target = ProbeTarget::version(
        "opencli",
        format!("Install OpenCLI: npm install -g {OPENCLI_PACKAGE} (desktop + Chrome only)"),
    );
    let version = engine.probe(&target).await;
    if version.status == ProbeStatus::Missing || version.status == ProbeStatus::Broken {
        return version;
    }

    // Installed — inspect the daemon/extension state without side effects.
    let status = tokio::process::Command::new("opencli")
        .args(["daemon", "status"])
        .output()
        .await;

    let output = match status {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_lowercase(),
        Err(_) => String::new(),
    };

    let extension_connected = output.lines().any(|l| {
        l.trim_start().starts_with("extension:")
            && l.contains("connected")
            && !l.contains("disconnected")
    });

    if extension_connected {
        ProbeResult::ok("opencli", "browser session connected")
    } else {
        ProbeResult::warn(
            "opencli",
            "installed but Chrome extension not connected",
            "Install the OpenCLI Chrome extension and keep Chrome open; it wakes on first use",
        )
    }
}

/// A per-platform OpenCLI backend (`opencli <platform> <verb> <arg>`).
#[derive(Debug, Clone)]
pub struct OpenCliBackend {
    name: String,
    platform: String,
    read_verb: String,
    search_verb: String,
}

impl OpenCliBackend {
    /// Build a backend for an arbitrary platform/verb mapping.
    pub fn new(platform: &str, read_verb: &str, search_verb: &str) -> Self {
        Self {
            name: format!("opencli-{platform}"),
            platform: platform.to_string(),
            read_verb: read_verb.to_string(),
            search_verb: search_verb.to_string(),
        }
    }

    /// Reddit via OpenCLI (`opencli reddit post|search`).
    pub fn reddit() -> Self {
        Self::new("reddit", "post", "search")
    }

    /// XiaoHongShu via OpenCLI (`opencli xiaohongshu note|search`).
    pub fn xiaohongshu() -> Self {
        Self::new("xiaohongshu", "note", "search")
    }

    /// Bilibili via OpenCLI (`opencli bilibili video|search`).
    pub fn bilibili() -> Self {
        Self::new("bilibili", "video", "search")
    }

    /// Twitter via OpenCLI (`opencli twitter tweet|search`).
    pub fn twitter() -> Self {
        Self::new("twitter", "tweet", "search")
    }

    /// LinkedIn via OpenCLI (`opencli linkedin profile|search`).
    pub fn linkedin() -> Self {
        Self::new("linkedin", "profile", "search")
    }

    /// Instagram via OpenCLI (`opencli instagram post|search`).
    pub fn instagram() -> Self {
        Self::new("instagram", "post", "search")
    }

    // TODO: verify these subcommands/flags against the real OpenCLI binary;
    // verbs (`reddit post`, `--json`, `--limit`) are based on its docs, not a
    // pinned version. search() degrades gracefully if the output isn't JSON.
    async fn run(&self, args: &[&str]) -> Result<String, BackendError> {
        let output = tokio::process::Command::new("opencli")
            .args(args)
            .output()
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    BackendError::CommandNotFound(self.name.clone())
                } else {
                    BackendError::CommandFailed(self.name.clone(), e.to_string())
                }
            })?;
        if !output.status.success() {
            return Err(BackendError::CommandFailed(
                self.name.clone(),
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[async_trait]
impl Backend for OpenCliBackend {
    fn name(&self) -> &str {
        &self.name
    }

    async fn probe(&self) -> ProbeResult {
        probe_opencli().await
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        debug!(platform = %self.platform, "opencli read");
        let body = self
            .run(&[
                self.platform.as_str(),
                self.read_verb.as_str(),
                url,
                "--json",
            ])
            .await?;
        Ok(Content {
            url: url.to_string(),
            title: None,
            body,
            metadata: serde_json::Value::Null,
            cached: false,
        })
    }

    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let limit = if opts.limit == 0 { 20 } else { opts.limit };
        let limit_s = limit.to_string();
        let out = self
            .run(&[
                self.platform.as_str(),
                self.search_verb.as_str(),
                query,
                "--limit",
                limit_s.as_str(),
                "--json",
            ])
            .await?;
        let parsed: serde_json::Value = match serde_json::from_str(&out) {
            Ok(v) => v,
            // Unknown/changed output shape → surface raw rather than failing.
            Err(_) => return Ok(crate::format::raw_search_fallback(&out)),
        };
        // OpenCLI returns either a bare array or `{ "results": [...] }`.
        let items = parsed
            .as_array()
            .cloned()
            .or_else(|| parsed["results"].as_array().cloned())
            .unwrap_or_default();
        Ok(items
            .into_iter()
            .map(|v| SearchResult {
                title: v["title"].as_str().unwrap_or("").to_string(),
                url: v["url"].as_str().unwrap_or("").to_string(),
                snippet: v["text"]
                    .as_str()
                    .or_else(|| v["snippet"].as_str())
                    .unwrap_or("")
                    .chars()
                    .take(280)
                    .collect(),
                author: v["author"].as_str().map(|s| s.to_string()),
                timestamp: v["timestamp"].as_str().map(|s| s.to_string()),
                metadata: v.clone(),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_names_are_platform_scoped() {
        assert_eq!(OpenCliBackend::reddit().name(), "opencli-reddit");
        assert_eq!(OpenCliBackend::xiaohongshu().name(), "opencli-xiaohongshu");
        assert_eq!(OpenCliBackend::bilibili().name(), "opencli-bilibili");
        assert_eq!(OpenCliBackend::twitter().name(), "opencli-twitter");
        assert_eq!(OpenCliBackend::linkedin().name(), "opencli-linkedin");
    }

    #[test]
    fn verbs_are_set_per_platform() {
        let reddit = OpenCliBackend::reddit();
        assert_eq!(reddit.read_verb, "post");
        assert_eq!(reddit.search_verb, "search");
        let xhs = OpenCliBackend::xiaohongshu();
        assert_eq!(xhs.read_verb, "note");
    }

    #[tokio::test]
    async fn probe_returns_result_with_hint_when_missing() {
        // opencli is not installed in CI → Missing with an install hint.
        let probe = probe_opencli().await;
        assert!(!probe.message.is_empty());
        if probe.status == ProbeStatus::Missing {
            assert!(probe.hint.is_some());
        }
    }

    #[tokio::test]
    async fn backend_probe_delegates_to_shared_probe() {
        let probe = OpenCliBackend::reddit().probe().await;
        assert!(!probe.message.is_empty());
    }

    // Behavioral test against the real binary; run with `--ignored` on a desktop
    // that has OpenCLI installed and a logged-in Chrome session.
    #[ignore = "requires opencli installed + Chrome session"]
    #[tokio::test]
    async fn opencli_reddit_search_real() {
        let results = OpenCliBackend::reddit()
            .search("rust", SearchOptions::default())
            .await;
        assert!(results.is_ok(), "search failed: {results:?}");
    }
}
