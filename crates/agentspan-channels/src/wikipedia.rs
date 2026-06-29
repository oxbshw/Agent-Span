//! Wikipedia channel — backed by the free MediaWiki Action API (no key).

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

const DEFAULT_BASE: &str = "https://en.wikipedia.org/w/api.php";

/// Extract the article title from a `/wiki/{Title}` URL.
fn parse_title(url: &str) -> Option<String> {
    let after = url.split("/wiki/").nth(1)?;
    let title = after.split(['#', '?']).next().unwrap_or(after);
    if title.is_empty() {
        None
    } else {
        Some(title.replace('_', " "))
    }
}

/// Strip the lightweight HTML tags MediaWiki returns in search snippets.
fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

/// MediaWiki Action API backend.
#[derive(Debug, Clone)]
pub struct WikipediaApiBackend {
    client: reqwest::Client,
    base_url: String,
}

impl Default for WikipediaApiBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
        }
    }
}

impl WikipediaApiBackend {
    /// Create a backend pointed at the public MediaWiki API.
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the base URL (used in tests against a mock server).
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }
}

#[async_trait]
impl Backend for WikipediaApiBackend {
    fn name(&self) -> &str {
        "wikipedia-api"
    }

    async fn probe(&self) -> ProbeResult {
        ProbeResult::ok("wikipedia-api", "mediawiki")
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let title = parse_title(url).ok_or_else(|| {
            BackendError::Parse(
                self.name().to_string(),
                format!("not a Wikipedia article URL: {url}"),
            )
        })?;
        let api = format!(
            "{}?action=query&prop=extracts&explaintext=1&redirects=1&format=json&titles={}",
            self.base_url,
            crate::percent_encode(&title)
        );
        let payload: serde_json::Value = self.get_json(&api).await?;
        let pages = &payload["query"]["pages"];
        let page = pages
            .as_object()
            .and_then(|m| m.values().next())
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let extract = page["extract"].as_str().unwrap_or("").to_string();
        Ok(Content {
            url: url.to_string(),
            title: page["title"].as_str().map(|s| s.to_string()),
            body: extract,
            metadata: page,
            cached: false,
        })
    }

    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let limit = if opts.limit == 0 {
            10
        } else {
            opts.limit.min(50)
        };
        let api = format!(
            "{}?action=query&list=search&format=json&srlimit={}&srsearch={}",
            self.base_url,
            limit,
            crate::percent_encode(query)
        );
        let payload: serde_json::Value = self.get_json(&api).await?;
        let hits = payload["query"]["search"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        Ok(hits
            .into_iter()
            .map(|h| {
                let title = h["title"].as_str().unwrap_or("").to_string();
                SearchResult {
                    url: format!("https://en.wikipedia.org/wiki/{}", title.replace(' ', "_")),
                    snippet: strip_html(h["snippet"].as_str().unwrap_or("")),
                    title,
                    author: None,
                    timestamp: h["timestamp"].as_str().map(|s| s.to_string()),
                    metadata: h,
                }
            })
            .collect())
    }
}

impl WikipediaApiBackend {
    async fn get_json(&self, api: &str) -> Result<serde_json::Value, BackendError> {
        let response = self
            .client
            .get(api)
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
}

/// Wikipedia channel.
#[derive(Debug, Clone)]
pub struct WikipediaChannel {
    router: BackendRouter,
    backend: WikipediaApiBackend,
}

impl WikipediaChannel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a channel whose backend targets `base_url` (tests).
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        let backend = WikipediaApiBackend::new().with_base_url(base_url);
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for WikipediaChannel {
    fn default() -> Self {
        let backend = WikipediaApiBackend::new();
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
impl agentspan_core::channel::Channel for WikipediaChannel {
    fn name(&self) -> &str {
        "wikipedia"
    }

    fn description(&self) -> &str {
        "Search and read Wikipedia articles via the free MediaWiki Action API"
    }

    fn format_for_llm(&self, raw: &str) -> String {
        crate::format::extract_text_fields(raw, &["extract", "snippet", "title"], 8000)
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("wikipedia.org/wiki/")
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
    fn can_handle_wikipedia_urls() {
        let ch = WikipediaChannel::new();
        assert!(ch.can_handle("https://en.wikipedia.org/wiki/Rust_(programming_language)"));
        assert!(!ch.can_handle("https://example.com"));
    }

    #[test]
    fn channel_is_tier_zero() {
        assert_eq!(WikipediaChannel::new().tier(), Tier::Zero);
    }

    #[test]
    fn parse_title_handles_underscores_and_fragments() {
        assert_eq!(
            parse_title("https://en.wikipedia.org/wiki/Rust_(programming_language)#History"),
            Some("Rust (programming language)".to_string())
        );
        assert_eq!(parse_title("https://en.wikipedia.org/"), None);
    }

    #[test]
    fn strip_html_removes_tags() {
        assert_eq!(strip_html("a <span class=\"x\">b</span> c"), "a b c");
    }

    #[tokio::test]
    async fn search_maps_results() {
        let server = MockServer::start().await;
        let body = r#"{"query":{"search":[{"title":"Rust (programming language)","snippet":"A <span>systems</span> language","timestamp":"2024-01-01T00:00:00Z"}]}}"#;
        Mock::given(method("GET"))
            .and(path("/"))
            .and(query_param("list", "search"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = WikipediaChannel::with_base_url(format!("{}/", server.uri()));
        let results = ch.search("rust", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust (programming language)");
        assert_eq!(results[0].snippet, "A systems language");
        assert!(results[0].url.ends_with("Rust_(programming_language)"));
    }

    #[tokio::test]
    async fn read_extracts_article_text() {
        let server = MockServer::start().await;
        let body =
            r#"{"query":{"pages":{"123":{"title":"Rust","extract":"Rust is a language."}}}}"#;
        Mock::given(method("GET"))
            .and(path("/"))
            .and(query_param("prop", "extracts"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = WikipediaChannel::with_base_url(format!("{}/", server.uri()));
        let content = ch
            .read("https://en.wikipedia.org/wiki/Rust", ReadOptions::default())
            .await
            .unwrap();
        assert_eq!(content.title.as_deref(), Some("Rust"));
        assert_eq!(content.body, "Rust is a language.");
    }

    #[tokio::test]
    async fn check_health_reports_ok() {
        let health = WikipediaChannel::new().check_health().await;
        assert_eq!(health.len(), 1);
        assert_eq!(health[0].backend_name, "wikipedia-api");
    }
}
