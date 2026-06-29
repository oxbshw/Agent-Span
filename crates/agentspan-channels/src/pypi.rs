//! PyPI channel — Python package metadata via the public JSON API.
//!
//! `read` resolves a package's JSON. PyPI has no public full-text search JSON
//! endpoint (the legacy XML-RPC search is disabled), so `search` is a *name
//! resolve*: it treats the query as a package name and returns that package if
//! it exists, or an empty result set otherwise. Honest about the limitation
//! rather than scraping the HTML search page.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

const DEFAULT_BASE: &str = "https://pypi.org";

fn parse_project(input: &str) -> Option<String> {
    if let Some(after) = input.split("/project/").nth(1) {
        let name = after.split(['?', '#', '/']).next().unwrap_or(after);
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }
    if !input.contains("://") && !input.trim().is_empty() {
        return Some(input.trim().to_string());
    }
    None
}

fn result_from_info(info: &serde_json::Value, name: &str) -> SearchResult {
    SearchResult {
        url: info["package_url"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("https://pypi.org/project/{name}/")),
        snippet: info["summary"].as_str().unwrap_or("").to_string(),
        author: info["author"].as_str().map(|s| s.to_string()),
        timestamp: None,
        title: info["name"].as_str().unwrap_or(name).to_string(),
        metadata: info.clone(),
    }
}

/// PyPI JSON API backend.
#[derive(Debug, Clone)]
pub struct PypiBackend {
    client: reqwest::Client,
    base_url: String,
}

impl Default for PypiBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
        }
    }
}

impl PypiBackend {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Fetch a package's JSON. `Ok(None)` on 404 (unknown package).
    async fn fetch_package(&self, name: &str) -> Result<Option<serde_json::Value>, BackendError> {
        let url = format!("{}/pypi/{}/json", self.base_url, name);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| BackendError::RequestFailed(self.name().to_string(), e.to_string()))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(BackendError::RequestFailed(
                self.name().to_string(),
                format!("HTTP {}", response.status()),
            ));
        }
        response
            .json()
            .await
            .map(Some)
            .map_err(|e| BackendError::Parse(self.name().to_string(), e.to_string()))
    }
}

#[async_trait]
impl Backend for PypiBackend {
    fn name(&self) -> &str {
        "pypi"
    }

    async fn probe(&self) -> ProbeResult {
        ProbeResult::ok("pypi", "json")
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let name = parse_project(url).ok_or_else(|| {
            BackendError::Parse(
                self.name().to_string(),
                format!("not a PyPI project: {url}"),
            )
        })?;
        let payload = self
            .fetch_package(&name)
            .await?
            .ok_or_else(|| BackendError::NotFound(format!("PyPI package not found: {name}")))?;
        let info = &payload["info"];
        let title = info["name"].as_str().unwrap_or(&name);
        let version = info["version"].as_str().unwrap_or("?");
        let summary = info["summary"].as_str().unwrap_or("");
        Ok(Content {
            url: url.to_string(),
            title: Some(format!("{title} {version}")),
            body: format!("{title} {version}\n{summary}").trim().to_string(),
            metadata: payload,
            cached: false,
        })
    }

    async fn search(
        &self,
        query: &str,
        _opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        // Name-resolve: PyPI offers no JSON full-text search.
        match self.fetch_package(query.trim()).await? {
            Some(payload) => Ok(vec![result_from_info(&payload["info"], query.trim())]),
            None => Ok(Vec::new()),
        }
    }
}

/// PyPI channel.
#[derive(Debug, Clone)]
pub struct PypiChannel {
    router: BackendRouter,
    backend: PypiBackend,
}

impl PypiChannel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self::from_backend(PypiBackend::new().with_base_url(base_url))
    }

    fn from_backend(backend: PypiBackend) -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for PypiChannel {
    fn default() -> Self {
        Self::from_backend(PypiBackend::new())
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for PypiChannel {
    fn name(&self) -> &str {
        "pypi"
    }

    fn description(&self) -> &str {
        "Look up Python package metadata on PyPI (search resolves by name)"
    }

    fn format_for_llm(&self, raw: &str) -> String {
        crate::format::extract_text_fields(raw, &["name", "version", "summary"], 6000)
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("pypi.org/project/")
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
    fn parse_project_handles_urls_and_names() {
        assert_eq!(
            parse_project("https://pypi.org/project/requests/"),
            Some("requests".to_string())
        );
        assert_eq!(parse_project("flask"), Some("flask".to_string()));
        assert_eq!(parse_project("https://example.com"), None);
    }

    #[test]
    fn can_handle_pypi_urls() {
        let ch = PypiChannel::new();
        assert!(ch.can_handle("https://pypi.org/project/requests/"));
        assert!(!ch.can_handle("https://crates.io/crates/serde"));
        assert_eq!(ch.tier(), Tier::Zero);
    }

    #[tokio::test]
    async fn read_returns_package_info() {
        let server = MockServer::start().await;
        let body = r#"{"info":{"name":"requests","version":"2.31.0","summary":"HTTP for Humans"}}"#;
        Mock::given(method("GET"))
            .and(path("/pypi/requests/json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = PypiChannel::with_base_url(server.uri());
        let content = ch
            .read("https://pypi.org/project/requests/", ReadOptions::default())
            .await
            .unwrap();
        assert_eq!(content.title.as_deref(), Some("requests 2.31.0"));
        assert!(content.body.contains("HTTP for Humans"));
    }

    #[tokio::test]
    async fn search_resolves_by_name() {
        let server = MockServer::start().await;
        let body = r#"{"info":{"name":"numpy","version":"1.26.0","summary":"array computing"}}"#;
        Mock::given(method("GET"))
            .and(path("/pypi/numpy/json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = PypiChannel::with_base_url(server.uri());
        let results = ch.search("numpy", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "numpy");
    }

    #[tokio::test]
    async fn search_unknown_name_is_empty() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let ch = PypiChannel::with_base_url(server.uri());
        let results = ch
            .search("definitely-not-a-real-pkg", SearchOptions::default())
            .await
            .unwrap();
        assert!(results.is_empty());
    }
}
