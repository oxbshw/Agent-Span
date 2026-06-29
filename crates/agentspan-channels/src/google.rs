//! Google channel — web search via the Google Custom Search JSON API.
//!
//! Tier 1: needs both a `GOOGLE_API_KEY` and a `GOOGLE_CSE_ID` (the programmable
//! search engine id). `read` returns the top hit; `search` returns the list.

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

const DEFAULT_BASE: &str = "https://www.googleapis.com";

/// Google Custom Search backend.
#[derive(Debug, Clone)]
pub struct GoogleBackend {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
    cse_id: Option<String>,
}

impl Default for GoogleBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
            api_key: std::env::var("GOOGLE_API_KEY")
                .ok()
                .filter(|k| !k.is_empty()),
            cse_id: std::env::var("GOOGLE_CSE_ID")
                .ok()
                .filter(|k| !k.is_empty()),
        }
    }
}

impl GoogleBackend {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base: impl Into<String>) -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: base.into(),
            api_key: Some("test-key".to_string()),
            cse_id: Some("test-cx".to_string()),
        }
    }

    fn credentials(&self) -> Result<(&str, &str), BackendError> {
        match (self.api_key.as_deref(), self.cse_id.as_deref()) {
            (Some(k), Some(cx)) => Ok((k, cx)),
            _ => Err(BackendError::AuthRequired(self.name().to_string())),
        }
    }

    async fn cse(&self, query: &str, num: usize) -> Result<Vec<SearchResult>, BackendError> {
        let (key, cx) = self.credentials()?;
        let url = format!(
            "{}/customsearch/v1?key={}&cx={}&q={}&num={}",
            self.base_url,
            crate::percent_encode(key),
            crate::percent_encode(cx),
            crate::percent_encode(query),
            num.min(10)
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
        let items = payload["items"].as_array().cloned().unwrap_or_default();
        Ok(items
            .into_iter()
            .map(|r| SearchResult {
                url: r["link"].as_str().unwrap_or("").to_string(),
                snippet: r["snippet"].as_str().unwrap_or("").to_string(),
                author: r["displayLink"].as_str().map(|s| s.to_string()),
                timestamp: None,
                title: r["title"].as_str().unwrap_or("").to_string(),
                metadata: r,
            })
            .collect())
    }
}

#[async_trait]
impl Backend for GoogleBackend {
    fn name(&self) -> &str {
        "google-cse"
    }

    async fn probe(&self) -> ProbeResult {
        if self.api_key.is_some() && self.cse_id.is_some() {
            ProbeResult::ok("google-cse", "v1")
        } else {
            ProbeResult {
                status: ProbeStatus::Warn,
                message: "GOOGLE_API_KEY / GOOGLE_CSE_ID not set".to_string(),
                version: None,
                hint: Some("set both GOOGLE_API_KEY and GOOGLE_CSE_ID".to_string()),
            }
        }
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let top = self
            .cse(url.trim(), 1)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| {
                BackendError::NotFound(format!("no Google result for: {}", url.trim()))
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
        let num = if opts.limit == 0 {
            10
        } else {
            opts.limit.min(10)
        };
        self.cse(query, num).await
    }
}

/// Google channel.
#[derive(Debug, Clone)]
pub struct GoogleChannel {
    router: BackendRouter,
    backend: GoogleBackend,
}

impl GoogleChannel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self::from_backend(GoogleBackend::with_base_url(base_url))
    }

    fn from_backend(backend: GoogleBackend) -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for GoogleChannel {
    fn default() -> Self {
        Self::from_backend(GoogleBackend::new())
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for GoogleChannel {
    fn name(&self) -> &str {
        "google"
    }

    fn description(&self) -> &str {
        "Web search via Google Custom Search (needs GOOGLE_API_KEY + GOOGLE_CSE_ID)"
    }

    fn format_for_llm(&self, raw: &str) -> String {
        crate::format::extract_text_fields(raw, &["title", "link", "snippet"], 6000)
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("google.com/search")
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
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn can_handle_and_tier() {
        let ch = GoogleChannel::new();
        assert!(ch.can_handle("https://www.google.com/search?q=rust"));
        assert!(!ch.can_handle("https://example.com"));
        assert_eq!(ch.name(), "google");
        assert_eq!(ch.tier(), Tier::One);
    }

    #[tokio::test]
    async fn probe_warns_without_credentials() {
        let backend = GoogleBackend {
            api_key: None,
            cse_id: None,
            ..GoogleBackend::new()
        };
        assert_eq!(backend.probe().await.status, ProbeStatus::Warn);
    }

    #[tokio::test]
    async fn search_maps_items() {
        let server = MockServer::start().await;
        let body = r#"{"items":[{"title":"Rust","link":"https://rust-lang.org","snippet":"a language","displayLink":"rust-lang.org"}]}"#;
        Mock::given(method("GET"))
            .and(path("/customsearch/v1"))
            .and(query_param("q", "rust"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = GoogleChannel::with_base_url(server.uri());
        let results = ch.search("rust", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust");
        assert_eq!(results[0].url, "https://rust-lang.org");
    }
}
