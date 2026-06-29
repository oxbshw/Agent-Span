//! YouTube channel — wraps the `yt-dlp` CLI for metadata, subtitles, and search.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::{ProbeEngine, ProbeTarget};
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

/// Build the `ytsearchN:query` term yt-dlp uses for keyword search.
fn search_term(query: &str, limit: usize) -> String {
    let n = limit.clamp(1, 50);
    format!("ytsearch{n}:{query}")
}

/// A yt-dlp-based backend (preferred); falls back to legacy youtube-dl by name.
#[derive(Debug, Clone)]
pub struct YtDlpBackend {
    bin: String,
}

impl Default for YtDlpBackend {
    fn default() -> Self {
        Self {
            bin: "yt-dlp".to_string(),
        }
    }
}

impl YtDlpBackend {
    /// Create a backend using the `yt-dlp` binary.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a backend using a specific binary (e.g. `youtube-dl`).
    pub fn with_bin(bin: impl Into<String>) -> Self {
        Self { bin: bin.into() }
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
impl Backend for YtDlpBackend {
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
                .unwrap_or_default(),
            metadata: meta,
            cached: false,
        })
    }

    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let limit = if opts.limit == 0 { 10 } else { opts.limit };
        let term = search_term(query, limit);
        let out = self
            .run(&[
                term.as_str(),
                "--dump-json",
                "--flat-playlist",
                "--no-warnings",
            ])
            .await?;
        Ok(out
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
            .map(|v| SearchResult {
                title: v["title"].as_str().unwrap_or("").to_string(),
                url: v["url"].as_str().map(|s| s.to_string()).unwrap_or_else(|| {
                    format!(
                        "https://www.youtube.com/watch?v={}",
                        v["id"].as_str().unwrap_or("")
                    )
                }),
                snippet: v["description"]
                    .as_str()
                    .unwrap_or("")
                    .chars()
                    .take(280)
                    .collect(),
                author: v["uploader"].as_str().map(|s| s.to_string()),
                timestamp: None,
                metadata: v.clone(),
            })
            .collect())
    }
}

/// YouTube channel.
#[derive(Debug, Clone)]
pub struct YoutubeChannel {
    router: BackendRouter,
}

impl YoutubeChannel {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for YoutubeChannel {
    fn default() -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![
            Arc::new(YtDlpBackend::new()),
            Arc::new(YtDlpBackend::with_bin("youtube-dl")),
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
impl agentspan_core::channel::Channel for YoutubeChannel {
    fn name(&self) -> &str {
        "youtube"
    }

    fn description(&self) -> &str {
        "Fetch YouTube video metadata/subtitles and search videos via yt-dlp"
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("youtube.com") || url.contains("youtu.be")
    }

    fn tier(&self) -> Tier {
        Tier::Zero
    }

    fn backends(&self) -> Vec<Box<dyn Backend>> {
        vec![
            Box::new(YtDlpBackend::new()),
            Box::new(YtDlpBackend::with_bin("youtube-dl")),
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
    fn can_handle_youtube_urls() {
        let ch = YoutubeChannel::new();
        assert!(ch.can_handle("https://www.youtube.com/watch?v=abc"));
        assert!(ch.can_handle("https://youtu.be/abc"));
        assert!(!ch.can_handle("https://example.com"));
    }

    #[test]
    fn channel_is_tier_zero() {
        assert_eq!(YoutubeChannel::new().tier(), Tier::Zero);
    }

    #[test]
    fn search_term_is_well_formed() {
        assert_eq!(search_term("rust tutorial", 5), "ytsearch5:rust tutorial");
        // Limit is clamped to a sane range.
        assert_eq!(search_term("x", 999), "ytsearch50:x");
    }

    #[test]
    fn channel_has_ytdlp_and_legacy_backends() {
        let names: Vec<_> = YoutubeChannel::new()
            .backends()
            .iter()
            .map(|b| b.name().to_string())
            .collect();
        assert_eq!(names, vec!["yt-dlp", "yt-dlp"]);
    }

    #[tokio::test]
    async fn probe_returns_a_result() {
        // Env-independent: yt-dlp may or may not be installed here.
        let probe = YtDlpBackend::new().probe().await;
        assert!(!probe.message.is_empty());
        // When missing, a helpful install hint is provided.
        if probe.status == ProbeStatus::Missing {
            assert!(probe.hint.is_some());
        }
    }
}
