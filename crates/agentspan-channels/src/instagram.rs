//! Instagram channel — login-gated, served via OpenCLI's browser session.
//!
//! Instagram has no anonymous API path and actively blocks scraping, so
//! OpenCLI (which reuses the user's logged-in Chrome) is the backend. On
//! servers, use the `instaloader` CLI as a fallback for public profiles.
//!
//! Tier 1: requires either OpenCLI (desktop + Chrome) or instaloader.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::{ProbeEngine, ProbeTarget};
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

use crate::opencli::OpenCliBackend;

/// Instaloader backend — a CLI tool for downloading public Instagram posts
/// and profile metadata. Fallback when OpenCLI is not available.
#[derive(Debug, Clone)]
pub struct InstaloaderBackend {
    bin: String,
}

impl Default for InstaloaderBackend {
    fn default() -> Self {
        Self {
            bin: "instaloader".to_string(),
        }
    }
}

impl InstaloaderBackend {
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
impl Backend for InstaloaderBackend {
    fn name(&self) -> &str {
        "instaloader"
    }

    async fn probe(&self) -> ProbeResult {
        let engine = ProbeEngine::new(Duration::from_secs(5));
        let target =
            ProbeTarget::version(&self.bin, "Install instaloader: pip install instaloader");
        engine.probe(&target).await
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        // Extract the post shortcode from the URL.
        // Instagram post URLs: instagram.com/p/{shortcode}/ or instagram.com/reel/{shortcode}/
        let shortcode = extract_shortcode(url).unwrap_or_default();
        if shortcode.is_empty() {
            return Err(BackendError::RequestFailed(
                self.name().to_string(),
                "could not extract post shortcode from URL".to_string(),
            ));
        }

        // Use --no-pictures --no-videos to skip downloads; just fetch metadata.
        let out = self
            .run(&[
                "--no-pictures",
                "--no-videos",
                "--no-captions",
                "--no-metadata-json",
                "--post-metadata",
                &format!("--shortcode={shortcode}"),
                "--",
            ])
            .await;

        // instaloader doesn't have a clean JSON output for single posts,
        // so we return what we have as body.
        let body = match out {
            Ok(text) => text,
            Err(e) => {
                // If instaloader fails (private post, login required), return
                // a structured error so the router can fall back to OpenCLI.
                return Err(e);
            }
        };

        Ok(Content {
            url: url.to_string(),
            title: Some(format!("Instagram post {shortcode}")),
            body,
            metadata: serde_json::json!({ "shortcode": shortcode }),
            cached: false,
        })
    }

    async fn search(
        &self,
        query: &str,
        _opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        // instaloader can fetch profile metadata by username (not keyword search).
        // Treat the query as a username and return the profile.
        let out = self
            .run(&["--no-pictures", "--no-videos", "--profile", query])
            .await;
        match out {
            Ok(text) => Ok(vec![SearchResult {
                title: format!("@{query} on Instagram"),
                url: format!("https://www.instagram.com/{query}/"),
                snippet: text.chars().take(280).collect(),
                author: Some(query.to_string()),
                timestamp: None,
                metadata: serde_json::json!({ "username": query }),
            }]),
            Err(e) => Err(e),
        }
    }
}

/// Extract the post shortcode from an Instagram URL.
/// Handles /p/{shortcode}/, /reel/{shortcode}/, /reels/{shortcode}/
fn extract_shortcode(url: &str) -> Option<String> {
    let segments: Vec<&str> = url.split('/').filter(|s| !s.is_empty()).collect();
    for (i, seg) in segments.iter().enumerate() {
        if (*seg == "p" || *seg == "reel" || *seg == "reels") && i + 1 < segments.len() {
            return Some(segments[i + 1].to_string());
        }
    }
    None
}

/// Instagram channel.
#[derive(Debug, Clone)]
pub struct InstagramChannel {
    router: BackendRouter,
}

impl InstagramChannel {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for InstagramChannel {
    fn default() -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![
            Arc::new(OpenCliBackend::instagram()) as Arc<dyn Backend>,
            Arc::new(InstaloaderBackend::new()) as Arc<dyn Backend>,
        ];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router }
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for InstagramChannel {
    fn name(&self) -> &str {
        "instagram"
    }

    fn description(&self) -> &str {
        "Read and search Instagram posts and profiles via OpenCLI or instaloader"
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("instagram.com") || url.contains("instagr.am")
    }

    fn tier(&self) -> Tier {
        Tier::One
    }

    fn backends(&self) -> Vec<Box<dyn Backend>> {
        vec![
            Box::new(OpenCliBackend::instagram()),
            Box::new(InstaloaderBackend::new()),
        ]
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
    fn can_handle_instagram_urls() {
        let ch = InstagramChannel::new();
        assert!(ch.can_handle("https://www.instagram.com/p/abc123/"));
        assert!(ch.can_handle("https://instagram.com/reel/xyz/"));
        assert!(ch.can_handle("https://instagr.am/p/abc/"));
        assert!(!ch.can_handle("https://example.com"));
    }

    #[test]
    fn channel_is_tier_one() {
        assert_eq!(InstagramChannel::new().tier(), Tier::One);
    }

    #[test]
    fn channel_name_is_instagram() {
        assert_eq!(InstagramChannel::new().name(), "instagram");
    }

    #[test]
    fn channel_has_opencli_and_instaloader_backends() {
        let names: Vec<_> = InstagramChannel::new()
            .backends()
            .iter()
            .map(|b| b.name().to_string())
            .collect();
        assert!(names.contains(&"opencli-instagram".to_string()));
        assert!(names.contains(&"instaloader".to_string()));
    }

    #[test]
    fn extract_shortcode_from_post_url() {
        assert_eq!(
            extract_shortcode("https://www.instagram.com/p/Cabc123/"),
            Some("Cabc123".to_string())
        );
    }

    #[test]
    fn extract_shortcode_from_reel_url() {
        assert_eq!(
            extract_shortcode("https://www.instagram.com/reel/Cxyz789/"),
            Some("Cxyz789".to_string())
        );
    }

    #[test]
    fn extract_shortcode_from_reels_url() {
        assert_eq!(
            extract_shortcode("https://www.instagram.com/reels/Cdef456/"),
            Some("Cdef456".to_string())
        );
    }

    #[test]
    fn extract_shortcode_from_non_post_url_returns_none() {
        assert_eq!(extract_shortcode("https://www.instagram.com/user/"), None);
    }

    #[tokio::test]
    async fn check_health_reports_two_backends() {
        let health = InstagramChannel::new().check_health().await;
        assert_eq!(health.len(), 2);
    }

    #[tokio::test]
    async fn probe_returns_a_result() {
        let probe = InstaloaderBackend::new().probe().await;
        assert!(!probe.message.is_empty());
        if probe.status == ProbeStatus::Missing {
            assert!(probe.hint.is_some());
        }
    }
}
