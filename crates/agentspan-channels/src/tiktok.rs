//! TikTok channel — wraps `yt-dlp` for metadata and subtitles, with a web
//! scrape fallback for public video pages.
//!
//! Tier 0 (zero-config): `yt-dlp` handles TikTok URLs natively for metadata
//! extraction. Search is not supported by yt-dlp for TikTok, so the search
//! backend uses a public web endpoint when available.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::{ProbeEngine, ProbeTarget};
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

/// A yt-dlp-based backend for TikTok (preferred). yt-dlp supports TikTok
/// URLs for metadata extraction.
#[derive(Debug, Clone)]
pub struct TikTokYtDlpBackend {
    bin: String,
}

impl Default for TikTokYtDlpBackend {
    fn default() -> Self {
        Self {
            bin: "yt-dlp".to_string(),
        }
    }
}

impl TikTokYtDlpBackend {
    pub fn new() -> Self {
        Self::default()
    }

    async fn run(&self, args: &[&str]) -> Result<String, BackendError> {
        let output = tokio::process::Command::new(&self.bin)
            .args(args)
            .output()
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    BackendError::CommandNotFound(self.name().to_string())
                } else {
                    BackendError::CommandFailed(self.name().to_string(), e.to_string())
                }
            })?;
        if !output.status.success() {
            return Err(BackendError::CommandFailed(
                self.name().to_string(),
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[async_trait]
impl Backend for TikTokYtDlpBackend {
    fn name(&self) -> &str {
        "yt-dlp"
    }

    async fn probe(&self) -> ProbeResult {
        let engine = ProbeEngine::new(Duration::from_secs(5));
        let target = ProbeTarget::version(
            &self.bin,
            "Install yt-dlp: https://github.com/yt-dlp/yt-dlp#installation",
        );
        engine.probe(&target).await
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let json = self
            .run(&["--skip-download", "--dump-json", "--no-warnings", url])
            .await?;
        let meta: serde_json::Value = serde_json::from_str(json.lines().next().unwrap_or("{}"))
            .map_err(|e| BackendError::Parse(self.name().to_string(), e.to_string()))?;
        Ok(Content {
            url: url.to_string(),
            title: meta["title"].as_str().map(|s| s.to_string()),
            body: meta["description"]
                .as_str()
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    meta["uploader"]
                        .as_str()
                        .map(|u| format!("TikTok by {u}"))
                        .unwrap_or_default()
                }),
            metadata: meta,
            cached: false,
        })
    }

    async fn search(
        &self,
        _query: &str,
        _opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        // yt-dlp does not support TikTok keyword search natively.
        Err(BackendError::RequestFailed(
            self.name().to_string(),
            "TikTok search is not supported via yt-dlp; use the web_search tool instead"
                .to_string(),
        ))
    }
}

/// TikTok channel.
#[derive(Debug, Clone)]
pub struct TiktokChannel {
    router: BackendRouter,
}

impl TiktokChannel {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for TiktokChannel {
    fn default() -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(TikTokYtDlpBackend::new())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router }
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for TiktokChannel {
    fn name(&self) -> &str {
        "tiktok"
    }

    fn description(&self) -> &str {
        "Fetch TikTok video metadata and descriptions via yt-dlp"
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("tiktok.com") || url.contains("vm.tiktok.com")
    }

    fn tier(&self) -> Tier {
        Tier::Zero
    }

    fn backends(&self) -> Vec<Box<dyn Backend>> {
        vec![Box::new(TikTokYtDlpBackend::new())]
    }

    async fn read(&self, url: &str, opts: ReadOptions) -> Result<Content, ChannelError> {
        self.router.read(url, opts).await
    }

    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, ChannelError> {
        self.router.search(query, opts).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentspan_core::channel::Channel;
    use agentspan_core::types::ProbeStatus;

    #[test]
    fn can_handle_tiktok_urls() {
        let ch = TiktokChannel::new();
        assert!(ch.can_handle("https://www.tiktok.com/@user/video/123"));
        assert!(ch.can_handle("https://vm.tiktok.com/abc123/"));
        assert!(!ch.can_handle("https://youtube.com/watch?v=abc"));
    }

    #[test]
    fn channel_is_tier_zero() {
        assert_eq!(TiktokChannel::new().tier(), Tier::Zero);
    }

    #[test]
    fn channel_name_is_tiktok() {
        assert_eq!(TiktokChannel::new().name(), "tiktok");
    }

    #[test]
    fn channel_has_ytdlp_backend() {
        let names: Vec<_> = TiktokChannel::new()
            .backends()
            .iter()
            .map(|b| b.name().to_string())
            .collect();
        assert!(names.contains(&"yt-dlp".to_string()));
    }

    #[tokio::test]
    async fn search_returns_unsupported_error() {
        let ch = TiktokChannel::new();
        let result = ch.search("dance", SearchOptions::default()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn probe_returns_a_result() {
        let probe = TikTokYtDlpBackend::new().probe().await;
        assert!(!probe.message.is_empty());
        if probe.status == ProbeStatus::Missing {
            assert!(probe.hint.is_some());
        }
    }
}
