//! Bing channel — web search via the Bing Web Search API (Azure).
//!
//! Tier 1: needs a `BING_API_KEY` (the `Ocp-Apim-Subscription-Key` header). A
//! search engine, so `read` returns the top hit and `search` the list.

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

const DEFAULT_BASE: &str = "https://api.bing.microsoft.com";

/// Bing Web Search backend.
#[derive(Debug, Clone)]
pub struct BingBackend {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

impl Default for BingBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
            api_key: std::env::var("BING_API_KEY").ok().filter(|k| !k.is_empty()),
        }
    }
}

impl BingBackend {
    pub fn new() -> Self {
        Self::default()
    }

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
            "{}/v7.0/search?q={}&count={}",
            self.base_url,
            crate::percent_encode(query),
            count
        );
        let response = self
            .client
            .get(&url)
            .header("Ocp-Apim-Subscription-Key", self.key()?)
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
        let results = payload["webPages"]["value"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        Ok(results
            .into_iter()
            .map(|r| SearchResult {
                url: r["url"].as_str().unwrap_or("").to_string(),
                snippet: r["snippet"].as_str().unwrap_or("").to_string(),
                author: None,
                timestamp: r["dateLastCrawled"].as_str().map(|s| s.to_string()),
                title: r["name"].as_str().unwrap_or("").to_string(),
                metadata: r,
            })
            .collect())
    }
}

#[async_trait]
impl Backend for BingBackend {
    fn name(&self) -> &str {
        "bing-search"
    }

    async fn probe(&self) -> ProbeResult {
        if self.api_key.is_some() {
            ProbeResult::ok("bing-search", "v7")
        } else {
            ProbeResult {
                status: ProbeStatus::Warn,
                message: "BING_API_KEY not set".to_string(),
                version: None,
                hint: Some("create a Bing Search resource in Azure for a key".to_string()),
            }
        }
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let top = self
            .web_search(url.trim(), 1)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| BackendError::NotFound(format!("no Bing result for: {}", url.trim())))?;
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
            opts.limit.min(50)
        };
        self.web_search(query, count).await
    }
}

/// Bing channel.
#[derive(Debug, Clone)]
pub struct BingChannel {
    router: BackendRouter,
    backend: BingBackend,
}

impl BingChannel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self::from_backend(BingBackend::with_base_url(base_url))
    }

    fn from_backend(backend: BingBackend) -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for BingChannel {
    fn default() -> Self {
        Self::from_backend(BingBackend::new())
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for BingChannel {
    fn name(&self) -> &str {
        "bing"
    }

    fn description(&self) -> &str {
        "Web search via the Bing Web Search API (needs BING_API_KEY)"
    }

    fn format_for_llm(&self, raw: &str) -> String {
        crate::format::extract_text_fields(raw, &["name", "url", "snippet"], 6000)
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("bing.com/search")
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
        let ch = BingChannel::new();
        assert!(ch.can_handle("https://www.bing.com/search?q=rust"));
        assert!(!ch.can_handle("https://example.com"));
        assert_eq!(ch.name(), "bing");
        assert_eq!(ch.tier(), Tier::One);
    }

    #[tokio::test]
    async fn probe_warns_without_key() {
        let backend = BingBackend {
            api_key: None,
            ..BingBackend::new()
        };
        assert_eq!(backend.probe().await.status, ProbeStatus::Warn);
    }

    #[tokio::test]
    async fn search_maps_webpages_with_key_header() {
        let server = MockServer::start().await;
        let body = r#"{"webPages":{"value":[{"name":"Rust","url":"https://rust-lang.org","snippet":"a language"}]}}"#;
        Mock::given(method("GET"))
            .and(path("/v7.0/search"))
            .and(header("ocp-apim-subscription-key", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = BingChannel::with_base_url(server.uri());
        let results = ch.search("rust", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust");
    }
}
