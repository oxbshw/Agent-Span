//! RSS/Atom channel — parse RSS 2.0 feeds.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{
    BackendHealth, Content, ProbeResult, ProbeStatus, ReadOptions, SearchOptions, SearchResult,
    Tier,
};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

/// Native RSS parser backend.
#[derive(Debug, Clone, Default)]
pub struct RssBackend {
    client: reqwest::Client,
}

impl RssBackend {
    pub fn new() -> Self {
        Self::default()
    }

    async fn fetch_feed(&self, url: &str) -> Result<rss::Channel, BackendError> {
        let bytes = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| BackendError::RequestFailed(self.name().to_string(), e.to_string()))?
            .bytes()
            .await
            .map_err(|e| BackendError::RequestFailed(self.name().to_string(), e.to_string()))?;

        rss::Channel::read_from(&bytes[..])
            .map_err(|e| BackendError::Parse(self.name().to_string(), e.to_string()))
    }
}

#[async_trait]
impl Backend for RssBackend {
    fn name(&self) -> &str {
        "rss-parser"
    }

    async fn probe(&self) -> ProbeResult {
        // The parser itself has no external binary dependency; network reachability is checked at use time.
        ProbeResult {
            status: ProbeStatus::Ok,
            message: "RSS parser is available".to_string(),
            version: None,
            hint: None,
        }
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let channel = self.fetch_feed(url).await?;
        let mut body = format!("# {}\n\n{}", channel.title(), channel.description());
        let link = channel.link();
        if !link.is_empty() {
            body.push_str(&format!("\n\nLink: {}", link));
        }
        for item in channel.items().iter().take(20) {
            body.push_str(&format!("\n\n## {}\n", item.title().unwrap_or("Untitled")));
            if let Some(link) = item.link() {
                body.push_str(&format!("URL: {}\n", link));
            }
            if let Some(desc) = item.description() {
                body.push_str(desc);
                body.push('\n');
            }
        }

        Ok(Content {
            url: url.to_string(),
            title: Some(channel.title().to_string()),
            body,
            metadata: serde_json::json!({
                "language": channel.language(),
                "last_build_date": channel.last_build_date(),
            }),
            cached: false,
        })
    }

    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let channel = self.fetch_feed(query).await?;
        let limit = opts.limit.clamp(1, 100);
        Ok(channel
            .items()
            .iter()
            .take(limit)
            .map(|item| SearchResult {
                title: item.title().unwrap_or("Untitled").to_string(),
                url: item.link().unwrap_or("").to_string(),
                snippet: item.description().unwrap_or("").to_string(),
                author: item.author().map(|s| s.to_string()),
                timestamp: item.pub_date().map(|s| s.to_string()),
                metadata: serde_json::Value::Null,
            })
            .collect())
    }
}

/// RSS/Atom channel.
#[derive(Debug, Clone)]
pub struct RssChannel {
    router: BackendRouter,
}

impl RssChannel {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for RssChannel {
    fn default() -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(RssBackend::new())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router }
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for RssChannel {
    fn name(&self) -> &str {
        "rss"
    }

    fn description(&self) -> &str {
        "Read RSS/Atom feeds and list recent items"
    }

    fn can_handle(&self, url: &str) -> bool {
        url.ends_with(".rss")
            || url.ends_with(".xml")
            || url.contains("/feed")
            || url.contains("/rss")
            || url.contains("/atom.xml")
    }

    fn tier(&self) -> Tier {
        Tier::Zero
    }

    fn backends(&self) -> Vec<Box<dyn Backend>> {
        vec![Box::new(RssBackend::new())]
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

    async fn check_health(&self) -> Vec<BackendHealth> {
        self.router.check_health().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentspan_core::channel::Channel;

    #[test]
    fn rss_can_handle_feed_urls() {
        let channel = RssChannel::new();
        assert!(channel.can_handle("https://example.com/feed"));
        assert!(channel.can_handle("https://example.com/rss.xml"));
        assert!(channel.can_handle("https://example.com/atom.xml"));
        assert!(!channel.can_handle("https://example.com/page.html"));
    }

    #[test]
    fn rss_is_tier_zero() {
        let channel = RssChannel::new();
        assert_eq!(channel.tier(), Tier::Zero);
    }
}
