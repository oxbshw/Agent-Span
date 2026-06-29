//! Google Scholar channel — academic search via SerpAPI's `google_scholar` engine.
//!
//! Search-only. A `SERPAPI_KEY` is optional but recommended (the backend warns
//! when absent). Tier 0 with a graceful no-key fallback.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

const DEFAULT_BASE: &str = "https://serpapi.com";

/// SerpAPI-backed Google Scholar search backend.
#[derive(Debug, Clone)]
pub struct SerpApiScholarBackend {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

impl Default for SerpApiScholarBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
            api_key: std::env::var("SERPAPI_KEY").ok(),
        }
    }
}

impl SerpApiScholarBackend {
    /// Create a backend reading `SERPAPI_KEY` from the environment.
    pub fn new() -> Self {
        Self::default()
    }

    /// Point the API at `base` with an explicit key (tests).
    pub fn with_api(base: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: base.into(),
            api_key: Some(key.into()),
        }
    }
}

#[async_trait]
impl Backend for SerpApiScholarBackend {
    fn name(&self) -> &str {
        "scholar-serpapi"
    }

    async fn probe(&self) -> ProbeResult {
        if self.api_key.is_some() {
            ProbeResult::ok("scholar-serpapi", "google_scholar")
        } else {
            ProbeResult::warn(
                "scholar-serpapi",
                "no SERPAPI_KEY configured",
                "Set SERPAPI_KEY to query Google Scholar",
            )
        }
    }

    async fn read(&self, _url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        Err(BackendError::Other(
            self.name().to_string(),
            "scholar is search-only".to_string(),
        ))
    }

    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let key = self
            .api_key
            .as_deref()
            .ok_or_else(|| BackendError::AuthRequired(self.name().to_string()))?;
        let limit = if opts.limit == 0 { 10 } else { opts.limit };
        let url = format!(
            "{}/search.json?engine=google_scholar&q={}&api_key={}&num={}",
            self.base_url,
            crate::percent_encode(query),
            crate::percent_encode(key),
            limit
        );
        let response = self
            .client
            .get(&url)
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
        Ok(payload["organic_results"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|r| SearchResult {
                title: r["title"].as_str().unwrap_or("").to_string(),
                url: r["link"].as_str().unwrap_or("").to_string(),
                snippet: r["snippet"].as_str().unwrap_or("").to_string(),
                author: r["publication_info"]["summary"]
                    .as_str()
                    .map(|s| s.to_string()),
                timestamp: None,
                metadata: r,
            })
            .collect())
    }
}

/// Google Scholar channel.
#[derive(Debug, Clone)]
pub struct ScholarChannel {
    router: BackendRouter,
    backend: SerpApiScholarBackend,
}

impl ScholarChannel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a channel pointed at `base` with a key (tests).
    pub fn with_api(base: impl Into<String>, key: impl Into<String>) -> Self {
        let backend = SerpApiScholarBackend::with_api(base, key);
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for ScholarChannel {
    fn default() -> Self {
        let backend = SerpApiScholarBackend::new();
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
impl agentspan_core::channel::Channel for ScholarChannel {
    fn name(&self) -> &str {
        "scholar"
    }

    fn description(&self) -> &str {
        "Search academic papers on Google Scholar via SerpAPI"
    }

    fn can_handle(&self, _url: &str) -> bool {
        false
    }

    fn tier(&self) -> Tier {
        Tier::Zero
    }

    fn backends(&self) -> Vec<Box<dyn Backend>> {
        vec![Box::new(self.backend.clone())]
    }

    async fn read(&self, _url: &str, _opts: ReadOptions) -> Result<Content, ChannelError> {
        Err(ChannelError::Other(
            "scholar is a search-only channel; use search instead".to_string(),
        ))
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
    fn channel_is_search_only_tier_zero() {
        let ch = ScholarChannel::new();
        assert_eq!(ch.name(), "scholar");
        assert_eq!(ch.tier(), Tier::Zero);
        assert!(!ch.can_handle("https://scholar.google.com"));
    }

    #[tokio::test]
    async fn read_is_unsupported() {
        let ch = ScholarChannel::new();
        assert!(ch.read("https://x", ReadOptions::default()).await.is_err());
    }

    #[tokio::test]
    async fn backend_warns_without_key() {
        let probe = SerpApiScholarBackend::new().probe().await;
        assert!(!probe.message.is_empty());
    }

    #[tokio::test]
    async fn search_maps_organic_results() {
        let server = MockServer::start().await;
        let body = r#"{"organic_results":[{"title":"Attention Is All You Need","link":"https://arxiv.org/abs/1706.03762","snippet":"transformers","publication_info":{"summary":"Vaswani et al., 2017"}}]}"#;
        Mock::given(method("GET"))
            .and(path("/search.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = ScholarChannel::with_api(server.uri(), "test-key");
        let results = ch
            .search("transformers", SearchOptions::default())
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Attention Is All You Need");
        assert_eq!(results[0].author.as_deref(), Some("Vaswani et al., 2017"));
    }

    #[tokio::test]
    async fn search_requires_key() {
        // Default backend in CI has no key → AuthRequired.
        let ch = ScholarChannel::new();
        assert!(ch.search("x", SearchOptions::default()).await.is_err());
    }
}
