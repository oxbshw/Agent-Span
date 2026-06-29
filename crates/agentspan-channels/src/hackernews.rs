//! Hacker News channel — backed by the free Algolia HN Search API (no key).

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

const DEFAULT_BASE: &str = "https://hn.algolia.com/api/v1";

/// Extract the numeric item id from an HN item URL.
fn parse_item_id(url: &str) -> Option<String> {
    let q = url.split("item?id=").nth(1)?;
    let id: String = q.chars().take_while(|c| c.is_ascii_digit()).collect();
    if id.is_empty() {
        None
    } else {
        Some(id)
    }
}

/// Algolia HN Search API backend.
#[derive(Debug, Clone)]
pub struct AlgoliaHnBackend {
    client: reqwest::Client,
    base_url: String,
}

impl Default for AlgoliaHnBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
        }
    }
}

impl AlgoliaHnBackend {
    /// Create a backend pointed at the public Algolia API.
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the base URL (used in tests against a mock server).
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Map an Algolia search payload to [`SearchResult`]s.
    fn map_hits(payload: &serde_json::Value) -> Vec<SearchResult> {
        payload["hits"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|h| {
                let id = h["objectID"].as_str().unwrap_or("");
                SearchResult {
                    title: h["title"]
                        .as_str()
                        .or_else(|| h["story_title"].as_str())
                        .unwrap_or("")
                        .to_string(),
                    url: h["url"]
                        .as_str()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| format!("https://news.ycombinator.com/item?id={id}")),
                    snippet: h["story_text"]
                        .as_str()
                        .or_else(|| h["comment_text"].as_str())
                        .unwrap_or("")
                        .chars()
                        .take(280)
                        .collect(),
                    author: h["author"].as_str().map(|s| s.to_string()),
                    timestamp: h["created_at"].as_str().map(|s| s.to_string()),
                    metadata: h.clone(),
                }
            })
            .collect()
    }
}

#[async_trait]
impl Backend for AlgoliaHnBackend {
    fn name(&self) -> &str {
        "hn-algolia"
    }

    async fn probe(&self) -> ProbeResult {
        ProbeResult::ok("hn-algolia", "v1")
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let id = parse_item_id(url).ok_or_else(|| {
            BackendError::Parse(
                self.name().to_string(),
                format!("not an HN item URL: {url}"),
            )
        })?;
        let api = format!("{}/items/{}", self.base_url, id);
        let response = self
            .client
            .get(&api)
            .send()
            .await
            .map_err(|e| BackendError::RequestFailed(self.name().to_string(), e.to_string()))?;
        if !response.status().is_success() {
            return Err(BackendError::RequestFailed(
                self.name().to_string(),
                format!("HTTP {}", response.status()),
            ));
        }
        let payload: serde_json::Value = response
            .json()
            .await
            .map_err(|e| BackendError::Parse(self.name().to_string(), e.to_string()))?;
        let title = payload["title"].as_str().map(|s| s.to_string());
        let body = payload["text"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| payload.to_string());
        Ok(Content {
            url: url.to_string(),
            title,
            body,
            metadata: payload,
            cached: false,
        })
    }

    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let limit = if opts.limit == 0 {
            20
        } else {
            opts.limit.min(100)
        };
        let api = format!(
            "{}/search?query={}&hitsPerPage={}",
            self.base_url,
            crate::percent_encode(query),
            limit
        );
        let response = self
            .client
            .get(&api)
            .send()
            .await
            .map_err(|e| BackendError::RequestFailed(self.name().to_string(), e.to_string()))?;
        if !response.status().is_success() {
            return Err(BackendError::RequestFailed(
                self.name().to_string(),
                format!("HTTP {}", response.status()),
            ));
        }
        let payload: serde_json::Value = response
            .json()
            .await
            .map_err(|e| BackendError::Parse(self.name().to_string(), e.to_string()))?;
        Ok(Self::map_hits(&payload))
    }
}

/// Hacker News channel.
#[derive(Debug, Clone)]
pub struct HackerNewsChannel {
    router: BackendRouter,
    backend: AlgoliaHnBackend,
}

impl HackerNewsChannel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a channel whose backend targets `base_url` (tests).
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        let backend = AlgoliaHnBackend::new().with_base_url(base_url);
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for HackerNewsChannel {
    fn default() -> Self {
        let backend = AlgoliaHnBackend::new();
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for HackerNewsChannel {
    fn name(&self) -> &str {
        "hackernews"
    }

    fn description(&self) -> &str {
        "Search and read Hacker News stories and comments via the Algolia HN API"
    }

    fn format_for_llm(&self, raw: &str) -> String {
        crate::format::extract_text_fields(
            raw,
            &["title", "text", "story_text", "comment_text"],
            8000,
        )
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("news.ycombinator.com")
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
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn can_handle_hn_urls() {
        let ch = HackerNewsChannel::new();
        assert!(ch.can_handle("https://news.ycombinator.com/item?id=42"));
        assert!(!ch.can_handle("https://example.com"));
    }

    #[test]
    fn channel_is_tier_zero() {
        assert_eq!(HackerNewsChannel::new().tier(), Tier::Zero);
    }

    #[test]
    fn parse_item_id_extracts_digits() {
        assert_eq!(
            parse_item_id("https://news.ycombinator.com/item?id=12345"),
            Some("12345".to_string())
        );
        assert_eq!(parse_item_id("https://news.ycombinator.com/news"), None);
    }

    #[test]
    fn backend_name_is_stable() {
        assert_eq!(AlgoliaHnBackend::new().name(), "hn-algolia");
    }

    #[tokio::test]
    async fn search_maps_algolia_hits() {
        let server = MockServer::start().await;
        let body = r#"{"hits":[{"objectID":"1","title":"Rust","url":"https://rust-lang.org","author":"steve","created_at":"2024-01-01"}]}"#;
        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = HackerNewsChannel::with_base_url(server.uri());
        let results = ch.search("rust", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust");
        assert_eq!(results[0].author.as_deref(), Some("steve"));
    }

    #[tokio::test]
    async fn read_fetches_item() {
        let server = MockServer::start().await;
        let body = r#"{"title":"Ask HN","text":"hello world"}"#;
        Mock::given(method("GET"))
            .and(path("/items/99"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = HackerNewsChannel::with_base_url(server.uri());
        let content = ch
            .read(
                "https://news.ycombinator.com/item?id=99",
                ReadOptions::default(),
            )
            .await
            .unwrap();
        assert_eq!(content.title.as_deref(), Some("Ask HN"));
        assert_eq!(content.body, "hello world");
    }

    #[tokio::test]
    async fn check_health_reports_ok() {
        let ch = HackerNewsChannel::new();
        let health = ch.check_health().await;
        assert_eq!(health.len(), 1);
        assert_eq!(health[0].backend_name, "hn-algolia");
    }
}
