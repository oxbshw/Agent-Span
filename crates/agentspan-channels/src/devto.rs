//! Dev.to channel — articles from the DEV Community public API.
//!
//! Tier 0: no key. `search` lists recent articles for a tag (the query is the
//! tag); `read` fetches a specific article by its dev.to URL.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

const DEFAULT_BASE: &str = "https://dev.to";

fn article_to_result(a: &serde_json::Value) -> SearchResult {
    SearchResult {
        url: a["url"].as_str().unwrap_or("").to_string(),
        snippet: a["description"].as_str().unwrap_or("").to_string(),
        author: a["user"]["name"].as_str().map(|s| s.to_string()),
        timestamp: a["published_at"].as_str().map(|s| s.to_string()),
        title: a["title"].as_str().unwrap_or("").to_string(),
        metadata: a.clone(),
    }
}

/// DEV Community backend.
#[derive(Debug, Clone)]
pub struct DevToBackend {
    client: reqwest::Client,
    base_url: String,
}

impl Default for DevToBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
        }
    }
}

impl DevToBackend {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base: impl Into<String>) -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: base.into(),
        }
    }

    async fn get_json(&self, url: &str) -> Result<serde_json::Value, BackendError> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| BackendError::RequestFailed(self.name().to_string(), e.to_string()))?;
        if !response.status().is_success() {
            return Err(BackendError::RequestFailed(
                self.name().to_string(),
                format!("HTTP {}", response.status()),
            ));
        }
        response
            .json()
            .await
            .map_err(|e| BackendError::Parse(self.name().to_string(), e.to_string()))
    }

    /// Parse the `<user>/<slug>` portion from a dev.to article URL.
    fn parse_path(url: &str) -> Option<String> {
        let rest = url
            .split("dev.to/")
            .nth(1)?
            .trim_end_matches('/')
            .split(['?', '#'])
            .next()?;
        if rest.matches('/').count() >= 1 && !rest.is_empty() {
            Some(rest.to_string())
        } else {
            None
        }
    }
}

#[async_trait]
impl Backend for DevToBackend {
    fn name(&self) -> &str {
        "devto-api"
    }

    async fn probe(&self) -> ProbeResult {
        ProbeResult::ok("devto-api", "v1")
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let path = Self::parse_path(url)
            .ok_or_else(|| BackendError::NotFound(format!("not a dev.to article: {url}")))?;
        let article = self
            .get_json(&format!("{}/api/articles/{}", self.base_url, path))
            .await?;
        Ok(Content {
            url: article["url"].as_str().unwrap_or(url).to_string(),
            title: article["title"].as_str().map(|s| s.to_string()),
            body: article["body_markdown"]
                .as_str()
                .or_else(|| article["description"].as_str())
                .unwrap_or("")
                .to_string(),
            metadata: article,
            cached: false,
        })
    }

    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let per_page = if opts.limit == 0 {
            10
        } else {
            opts.limit.min(30)
        };
        let url = format!(
            "{}/api/articles?tag={}&per_page={}",
            self.base_url,
            crate::percent_encode(query.trim()),
            per_page
        );
        let payload = self.get_json(&url).await?;
        Ok(payload
            .as_array()
            .map(|arr| arr.iter().map(article_to_result).collect())
            .unwrap_or_default())
    }
}

/// Dev.to channel.
#[derive(Debug, Clone)]
pub struct DevToChannel {
    router: BackendRouter,
    backend: DevToBackend,
}

impl DevToChannel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self::from_backend(DevToBackend::with_base_url(base_url))
    }

    fn from_backend(backend: DevToBackend) -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for DevToChannel {
    fn default() -> Self {
        Self::from_backend(DevToBackend::new())
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for DevToChannel {
    fn name(&self) -> &str {
        "devto"
    }

    fn description(&self) -> &str {
        "DEV Community articles (read by URL, search by tag)"
    }

    fn format_for_llm(&self, raw: &str) -> String {
        crate::format::extract_text_fields(raw, &["title", "description", "body_markdown"], 8000)
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("dev.to/")
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
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn metadata_and_path_parsing() {
        let ch = DevToChannel::new();
        assert_eq!(ch.name(), "devto");
        assert_eq!(ch.tier(), Tier::Zero);
        assert!(ch.can_handle("https://dev.to/ben/some-post-123"));
        assert_eq!(
            DevToBackend::parse_path("https://dev.to/ben/some-post-123"),
            Some("ben/some-post-123".to_string())
        );
        assert_eq!(DevToBackend::parse_path("https://dev.to/"), None);
    }

    #[tokio::test]
    async fn search_lists_articles_by_tag() {
        let server = MockServer::start().await;
        let body = r#"[{"title":"Async Rust","url":"https://dev.to/a/async-rust","description":"intro","user":{"name":"Alice"}}]"#;
        Mock::given(method("GET"))
            .and(path("/api/articles"))
            .and(query_param("tag", "rust"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = DevToChannel::with_base_url(server.uri());
        let results = ch.search("rust", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Async Rust");
        assert_eq!(results[0].author.as_deref(), Some("Alice"));
    }

    #[tokio::test]
    async fn read_fetches_article_by_path() {
        let server = MockServer::start().await;
        let body = r#"{"title":"Async Rust","url":"https://dev.to/a/async-rust","body_markdown":"Hello world"}"#;
        Mock::given(method("GET"))
            .and(path("/api/articles/a/async-rust"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = DevToChannel::with_base_url(server.uri());
        let content = ch
            .read("https://dev.to/a/async-rust", ReadOptions::default())
            .await
            .unwrap();
        assert_eq!(content.title.as_deref(), Some("Async Rust"));
        assert!(content.body.contains("Hello"));
    }
}
