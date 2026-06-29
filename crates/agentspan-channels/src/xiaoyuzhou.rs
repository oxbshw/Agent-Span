//! Xiaoyuzhou (小宇宙) podcast channel — reads a podcast by transcribing its audio.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

use crate::transcribe::{providers_from_config, transcribe_url};

/// Whisper-transcription backend for podcasts.
#[derive(Debug, Clone, Default)]
pub struct WhisperBackend;

impl WhisperBackend {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Backend for WhisperBackend {
    fn name(&self) -> &str {
        "whisper"
    }

    async fn probe(&self) -> ProbeResult {
        let config = agentspan_core::Config::load().unwrap_or_default();
        if providers_from_config(&config).is_empty() {
            ProbeResult::warn(
                "whisper",
                "no transcription API key configured",
                "Get a free Groq key and run: agentspan config set api_keys.groq gsk_xxx",
            )
        } else {
            ProbeResult::ok("whisper", "transcription provider configured")
        }
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let config = agentspan_core::Config::load().unwrap_or_default();
        let text = transcribe_url(url, &config)
            .await
            .map_err(|e| BackendError::Other(self.name().to_string(), e.to_string()))?;
        Ok(Content {
            url: url.to_string(),
            title: Some("podcast transcript".to_string()),
            body: text,
            metadata: serde_json::Value::Null,
            cached: false,
        })
    }

    async fn search(
        &self,
        _query: &str,
        _opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        Err(BackendError::Other(
            self.name().to_string(),
            "xiaoyuzhou is read (transcribe) only".to_string(),
        ))
    }
}

/// Xiaoyuzhou podcast channel.
#[derive(Debug, Clone, Default)]
pub struct XiaoyuzhouChannel {
    router: Option<BackendRouter>,
}

impl XiaoyuzhouChannel {
    pub fn new() -> Self {
        let router = BackendRouter::new(
            vec![Arc::new(WhisperBackend::new()) as Arc<dyn Backend>],
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self {
            router: Some(router),
        }
    }

    fn router(&self) -> BackendRouter {
        self.router.clone().unwrap_or_else(|| {
            BackendRouter::new(
                vec![Arc::new(WhisperBackend::new()) as Arc<dyn Backend>],
                ProbeEngine::new(Duration::from_secs(5)),
                RetryConfig::default(),
            )
        })
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for XiaoyuzhouChannel {
    fn name(&self) -> &str {
        "xiaoyuzhou"
    }

    fn description(&self) -> &str {
        "Transcribe Xiaoyuzhou (and other) podcast audio to text via Whisper"
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("xiaoyuzhoufm.com") || url.contains("xyzcdn.net")
    }

    fn tier(&self) -> Tier {
        Tier::One
    }

    fn backends(&self) -> Vec<Box<dyn Backend>> {
        vec![Box::new(WhisperBackend::new())]
    }

    async fn read(&self, url: &str, opts: ReadOptions) -> Result<Content, ChannelError> {
        self.router().read(url, opts).await
    }

    async fn search(
        &self,
        _query: &str,
        _opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, ChannelError> {
        Err(ChannelError::Other(
            "xiaoyuzhou is read (transcribe) only".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentspan_core::channel::Channel;

    #[test]
    fn can_handle_xiaoyuzhou_urls() {
        let ch = XiaoyuzhouChannel::new();
        assert!(ch.can_handle("https://www.xiaoyuzhoufm.com/episode/abc"));
        assert!(!ch.can_handle("https://example.com"));
    }

    #[test]
    fn channel_is_tier_one() {
        assert_eq!(XiaoyuzhouChannel::new().tier(), Tier::One);
    }

    #[test]
    fn backend_is_whisper() {
        let names: Vec<_> = XiaoyuzhouChannel::new()
            .backends()
            .iter()
            .map(|b| b.name().to_string())
            .collect();
        assert_eq!(names, vec!["whisper"]);
    }

    #[tokio::test]
    async fn search_is_unsupported() {
        let ch = XiaoyuzhouChannel::new();
        assert!(ch.search("x", SearchOptions::default()).await.is_err());
    }
}
