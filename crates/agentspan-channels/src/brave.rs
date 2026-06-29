//! Brave Search channel — web search via the Brave Search API.
//!
//! Tier 1: needs a `BRAVE_API_KEY` (the `X-Subscription-Token` header). A search
//! engine, so `read` returns the top hit for a query and `search` returns the
//! list.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{
    Content, ProbeResult, ProbeStatus, ReadOptions, SearchOptions, SearchResult, Tier,
};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

const DEFAULT_BASE: &str = "https://api.search.brave.com";

/// Brave Search API backend.
#[derive(Debug, Clone)]
pub struct BraveBackend {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

impl Default for BraveBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
            api_key: std::env::var("BRAVE_API_KEY")
                .ok()
                .filter(|k| !k.is_empty()),
        }
    }
}

impl BraveBackend {
    pub fn new() -> Self {
        Self::default()
    }

    /// Point at `base` with a test key (tests).
    pub fn with_base_url(base: impl Into<String>) -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: base.into(),
            api_key: Some("test-key".to_string()),
        }
    }

    fn key(&self) -> Result<&str, BackendError> {
        self.api_key
            .as_deref()
            .ok_or_else(|| BackendError::AuthRequired(self.name().to_string()))
    }

    async fn web_search(
        &self,
        query: &str,
        count: usize,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let url = format!(
            "{}/res/v1/web/search?q={}&count={}",
            self.base_url,
            crate::percent_encode(query),
            count
        );
        let response = self
            .client
            .get(&url)
            .header("X-Subscription-Token", self.key()?)
            .header("Accept", "application/json")
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
        let results = payload["web"]["results"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        Ok(results
            .into_iter()
            .map(|r| SearchResult {
                url: r["url"].as_str().unwrap_or("").to_string(),
                snippet: r["description"].as_str().unwrap_or("").to_string(),
                author: None,
                timestamp: r["age"].as_str().map(|s| s.to_string()),
                title: r["title"].as_str().unwrap_or("").to_string(),
                metadata: r,
            })
            .collect())
    }
}

#[async_trait]
impl Backend for BraveBackend {
    fn name(&self) -> &str {
        "brave-search"
    }

    async fn probe(&self) -> ProbeResult {
        if self.api_key.is_some() {
            ProbeResult::ok("brave-search", "v1")
        } else {
            ProbeResult {
                status: ProbeStatus::Warn,
                message: "BRAVE_API_KEY not set".to_string(),
                version: None,
                hint: Some("get a key at https://brave.com/search/api/".to_string()),
            }
        }
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        // Brave is a search engine: treat the input as a query, return the top hit.
        let top = self
            .web_search(url.trim(), 1)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| {
                BackendError::NotFound(format!("no Brave result for: {}", url.trim()))
            })?;
        Ok(Content {
            url: top.url.clone(),
            title: Some(top.title.clone()),
            body: format!("{}\n{}\n{}", top.title, top.url, top.snippet),
            metadata: top.metadata,
            cached: false,
        })
    }

    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let count = if opts.limit == 0 {
            10
        } else {
            opts.limit.min(20)
        };
        self.web_search(query, count).await
    }
}

/// Brave Search channel.
#[derive(Debug, Clone)]
pub struct BraveChannel {
    router: BackendRouter,
    backend: BraveBackend,
}

impl BraveChannel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self::from_backend(BraveBackend::with_base_url(base_url))
    }

    fn from_backend(backend: BraveBackend) -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for BraveChannel {
    fn default() -> Self {
        Self::from_backend(BraveBackend::new())
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for BraveChannel {
    fn name(&self) -> &str {
        "brave"
    }

    fn description(&self) -> &str {
        "Web search via the Brave Search API (needs BRAVE_API_KEY)"
    }

    fn format_for_llm(&self, raw: &str) -> String {
        crate::format::extract_text_fields(raw, &["title", "url", "description"], 6000)
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("search.brave.com")
    }

    fn tier(&self) -> Tier {
        Tier::One
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
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn can_handle_and_tier() {
        let ch = BraveChannel::new();
        assert!(ch.can_handle("https://search.brave.com/search?q=rust"));
        assert!(!ch.can_handle("https://example.com"));
        assert_eq!(ch.name(), "brave");
        assert_eq!(ch.tier(), Tier::One);
    }

    #[tokio::test]
    async fn probe_warns_without_key() {
        let backend = BraveBackend {
            api_key: None,
            ..BraveBackend::new()
        };
        assert_eq!(backend.probe().await.status, ProbeStatus::Warn);
    }

    #[tokio::test]
    async fn search_maps_web_results_with_token_header() {
        let server = MockServer::start().await;
        let body = r#"{"web":{"results":[{"title":"Rust","url":"https://rust-lang.org","description":"systems language"}]}}"#;
        Mock::given(method("GET"))
            .and(path("/res/v1/web/search"))
            .and(header("x-subscription-token", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = BraveChannel::with_base_url(server.uri());
        let results = ch.search("rust", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust");
        assert_eq!(results[0].url, "https://rust-lang.org");
    }

    #[tokio::test]
    async fn read_returns_top_hit() {
        let server = MockServer::start().await;
        let body =
            r#"{"web":{"results":[{"title":"Top","url":"https://t.io","description":"d"}]}}"#;
        Mock::given(method("GET"))
            .and(path("/res/v1/web/search"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = BraveChannel::with_base_url(server.uri());
        let content = ch.read("anything", ReadOptions::default()).await.unwrap();
        assert_eq!(content.title.as_deref(), Some("Top"));
        assert_eq!(content.url, "https://t.io");
    }
}
