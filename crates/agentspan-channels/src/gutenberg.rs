//! Project Gutenberg channel — public-domain books via the Gutendex API.
//!
//! Tier 0: no key. `search` queries Gutendex; `read` fetches a book by its
//! `gutenberg.org/ebooks/<id>` URL.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

const DEFAULT_BASE: &str = "https://gutendex.com";

fn book_to_result(b: &serde_json::Value) -> SearchResult {
    let id = b["id"].as_i64().unwrap_or(0);
    let author = b["authors"]
        .as_array()
        .and_then(|a| a.first())
        .and_then(|a| a["name"].as_str())
        .map(|s| s.to_string());
    SearchResult {
        url: format!("https://www.gutenberg.org/ebooks/{id}"),
        snippet: b["subjects"]
            .as_array()
            .map(|s| {
                s.iter()
                    .filter_map(|v| v.as_str())
                    .take(3)
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .unwrap_or_default(),
        author,
        timestamp: None,
        title: b["title"].as_str().unwrap_or("").to_string(),
        metadata: b.clone(),
    }
}

/// Gutendex backend.
#[derive(Debug, Clone)]
pub struct GutenbergBackend {
    client: reqwest::Client,
    base_url: String,
}

impl Default for GutenbergBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
        }
    }
}

impl GutenbergBackend {
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

    /// Extract the numeric ebook id from a Project Gutenberg URL.
    fn parse_id(url: &str) -> Option<String> {
        let rest = url.split("/ebooks/").nth(1)?;
        let id: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if id.is_empty() {
            None
        } else {
            Some(id)
        }
    }
}

#[async_trait]
impl Backend for GutenbergBackend {
    fn name(&self) -> &str {
        "gutendex-api"
    }

    async fn probe(&self) -> ProbeResult {
        ProbeResult::ok("gutendex-api", "v1")
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let id = Self::parse_id(url)
            .ok_or_else(|| BackendError::NotFound(format!("no Gutenberg ebook id in: {url}")))?;
        let book = self
            .get_json(&format!("{}/books/{}", self.base_url, id))
            .await?;
        let result = book_to_result(&book);
        Ok(Content {
            url: result.url.clone(),
            title: Some(result.title.clone()),
            body: format!("{}\n{}", result.title, result.snippet),
            metadata: book,
            cached: false,
        })
    }

    async fn search(
        &self,
        query: &str,
        _opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let url = format!(
            "{}/books?search={}",
            self.base_url,
            crate::percent_encode(query.trim())
        );
        let payload = self.get_json(&url).await?;
        let results = payload["results"].as_array().cloned().unwrap_or_default();
        Ok(results.iter().map(book_to_result).collect())
    }
}

/// Project Gutenberg channel.
#[derive(Debug, Clone)]
pub struct GutenbergChannel {
    router: BackendRouter,
    backend: GutenbergBackend,
}

impl GutenbergChannel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self::from_backend(GutenbergBackend::with_base_url(base_url))
    }

    fn from_backend(backend: GutenbergBackend) -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for GutenbergChannel {
    fn default() -> Self {
        Self::from_backend(GutenbergBackend::new())
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for GutenbergChannel {
    fn name(&self) -> &str {
        "gutenberg"
    }

    fn description(&self) -> &str {
        "Public-domain books via Project Gutenberg / Gutendex"
    }

    fn format_for_llm(&self, raw: &str) -> String {
        crate::format::extract_text_fields(raw, &["title", "authors", "subjects"], 6000)
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("gutenberg.org/") || url.contains("gutendex.com/")
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
    fn metadata_and_id_parsing() {
        let ch = GutenbergChannel::new();
        assert_eq!(ch.name(), "gutenberg");
        assert_eq!(ch.tier(), Tier::Zero);
        assert!(ch.can_handle("https://www.gutenberg.org/ebooks/1342"));
        assert_eq!(
            GutenbergBackend::parse_id("https://www.gutenberg.org/ebooks/1342"),
            Some("1342".to_string())
        );
        assert_eq!(
            GutenbergBackend::parse_id("https://www.gutenberg.org/about"),
            None
        );
    }

    #[tokio::test]
    async fn search_maps_books() {
        let server = MockServer::start().await;
        let body = r#"{"results":[{"id":1342,"title":"Pride and Prejudice","authors":[{"name":"Austen, Jane"}],"subjects":["Love stories","England"]}]}"#;
        Mock::given(method("GET"))
            .and(path("/books"))
            .and(query_param("search", "pride"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = GutenbergChannel::with_base_url(server.uri());
        let results = ch.search("pride", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Pride and Prejudice");
        assert!(results[0].url.ends_with("/ebooks/1342"));
        assert_eq!(results[0].author.as_deref(), Some("Austen, Jane"));
    }
}
