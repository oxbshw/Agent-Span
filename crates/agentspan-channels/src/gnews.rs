//! Google News channel — headlines and topic search via Google News RSS.
//!
//! Zero-config: Google News exposes RSS feeds we parse with the `rss` crate.
//! `search` queries the news search feed; `read` returns the headlines for a
//! query (or a given Google News feed URL).

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

const DEFAULT_BASE: &str = "https://news.google.com";

/// Google News RSS backend.
#[derive(Debug, Clone)]
pub struct GoogleNewsBackend {
    client: reqwest::Client,
    base_url: String,
}

impl Default for GoogleNewsBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
        }
    }
}

impl GoogleNewsBackend {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    fn search_url(&self, query: &str) -> String {
        format!(
            "{}/rss/search?q={}&hl=en-US&gl=US&ceid=US:en",
            self.base_url,
            crate::percent_encode(query)
        )
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
impl Backend for GoogleNewsBackend {
    fn name(&self) -> &str {
        "gnews-rss"
    }

    async fn probe(&self) -> ProbeResult {
        ProbeResult::ok("gnews-rss", "rss")
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        // A full URL is fetched directly; anything else is treated as a query.
        let feed_url = if url.starts_with("http") {
            url.to_string()
        } else {
            self.search_url(url.trim())
        };
        let channel = self.fetch_feed(&feed_url).await?;
        let mut body = format!("# {}\n", channel.title());
        for item in channel.items().iter().take(20) {
            body.push_str(&format!(
                "\n- {} ({})",
                item.title().unwrap_or("Untitled"),
                item.link().unwrap_or("")
            ));
        }
        Ok(Content {
            url: url.to_string(),
            title: Some(channel.title().to_string()),
            body,
            metadata: serde_json::json!({ "items": channel.items().len() }),
            cached: false,
        })
    }

    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let limit = if opts.limit == 0 { 10 } else { opts.limit };
        let channel = self.fetch_feed(&self.search_url(query)).await?;
        Ok(channel
            .items()
            .iter()
            .take(limit)
            .map(|item| SearchResult {
                url: item.link().unwrap_or("").to_string(),
                snippet: item.description().unwrap_or("").to_string(),
                author: item.source().and_then(|s| s.title()).map(|s| s.to_string()),
                timestamp: item.pub_date().map(|s| s.to_string()),
                title: item.title().unwrap_or("Untitled").to_string(),
                metadata: serde_json::Value::Null,
            })
            .collect())
    }
}

/// Google News channel.
#[derive(Debug, Clone)]
pub struct GoogleNewsChannel {
    router: BackendRouter,
    backend: GoogleNewsBackend,
}

impl GoogleNewsChannel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self::from_backend(GoogleNewsBackend::new().with_base_url(base_url))
    }

    fn from_backend(backend: GoogleNewsBackend) -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for GoogleNewsChannel {
    fn default() -> Self {
        Self::from_backend(GoogleNewsBackend::new())
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for GoogleNewsChannel {
    fn name(&self) -> &str {
        "gnews"
    }

    fn description(&self) -> &str {
        "News headlines and topic search via Google News RSS"
    }

    fn format_for_llm(&self, raw: &str) -> String {
        crate::format::extract_text_fields(raw, &["title", "link", "description"], 8000)
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("news.google.com")
    }

    fn tier(&self) -> Tier {
        Tier::Zero
    }

    fn backends(&self) -> Vec<Box<dyn Backend>> {
        vec![Box::new(self.backend.clone())]
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
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const FEED: &str = r#"<?xml version="1.0"?><rss version="2.0"><channel>
        <title>Google News</title>
        <item><title>Headline One</title><link>https://example.com/1</link>
        <description>desc one</description><pubDate>Mon, 01 Jan 2024 00:00:00 GMT</pubDate></item>
        <item><title>Headline Two</title><link>https://example.com/2</link>
        <description>desc two</description></item>
        </channel></rss>"#;

    #[test]
    fn can_handle_and_metadata() {
        let ch = GoogleNewsChannel::new();
        assert!(ch.can_handle("https://news.google.com/rss/search?q=x"));
        assert!(!ch.can_handle("https://example.com"));
        assert_eq!(ch.name(), "gnews");
        assert_eq!(ch.tier(), Tier::Zero);
    }

    #[tokio::test]
    async fn search_maps_feed_items() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string(FEED))
            .mount(&server)
            .await;

        let ch = GoogleNewsChannel::with_base_url(server.uri());
        let results = ch.search("rust", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Headline One");
        assert_eq!(results[0].url, "https://example.com/1");
        assert_eq!(
            results[0].timestamp.as_deref(),
            Some("Mon, 01 Jan 2024 00:00:00 GMT")
        );
    }

    #[tokio::test]
    async fn read_returns_headlines() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string(FEED))
            .mount(&server)
            .await;

        let ch = GoogleNewsChannel::with_base_url(server.uri());
        let content = ch.read("rust", ReadOptions::default()).await.unwrap();
        assert_eq!(content.title.as_deref(), Some("Google News"));
        assert!(content.body.contains("Headline One"));
        assert!(content.body.contains("Headline Two"));
    }
}
