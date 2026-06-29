//! Open Library channel — book search via the Open Library public API.
//!
//! Tier 0: no key. `search` queries the catalogue; `read` fetches a work by its
//! `openlibrary.org/works/<id>` URL.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

const DEFAULT_BASE: &str = "https://openlibrary.org";

/// Open Library backend.
#[derive(Debug, Clone)]
pub struct OpenLibraryBackend {
    client: reqwest::Client,
    base_url: String,
}

impl Default for OpenLibraryBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
        }
    }
}

impl OpenLibraryBackend {
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

    /// Extract the `works/<id>` segment from an Open Library work URL.
    fn parse_work(url: &str) -> Option<String> {
        let rest = url.split("openlibrary.org/").nth(1)?;
        if rest.starts_with("works/") {
            Some(rest.trim_end_matches('/').to_string())
        } else {
            None
        }
    }
}

#[async_trait]
impl Backend for OpenLibraryBackend {
    fn name(&self) -> &str {
        "openlibrary-api"
    }

    async fn probe(&self) -> ProbeResult {
        ProbeResult::ok("openlibrary-api", "v1")
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let work = Self::parse_work(url)
            .ok_or_else(|| BackendError::NotFound(format!("not an Open Library work: {url}")))?;
        let obj = self
            .get_json(&format!("{}/{}.json", self.base_url, work))
            .await?;
        // `description` can be a plain string or an object with a `value`.
        let body = obj["description"]
            .as_str()
            .or_else(|| obj["description"]["value"].as_str())
            .unwrap_or("")
            .to_string();
        Ok(Content {
            url: format!("{}/{}", self.base_url, work),
            title: obj["title"].as_str().map(|s| s.to_string()),
            body,
            metadata: obj,
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
        let url = format!(
            "{}/search.json?q={}&limit={}",
            self.base_url,
            crate::percent_encode(query.trim()),
            limit
        );
        let payload = self.get_json(&url).await?;
        let docs = payload["docs"].as_array().cloned().unwrap_or_default();
        Ok(docs
            .into_iter()
            .map(|d| {
                let key = d["key"].as_str().unwrap_or("");
                let author = d["author_name"]
                    .as_array()
                    .and_then(|a| a.first())
                    .and_then(|a| a.as_str())
                    .map(|s| s.to_string());
                SearchResult {
                    url: format!("{}{}", self.base_url, key),
                    snippet: d["first_publish_year"]
                        .as_i64()
                        .map(|y| format!("First published {y}"))
                        .unwrap_or_default(),
                    author,
                    timestamp: None,
                    title: d["title"].as_str().unwrap_or("").to_string(),
                    metadata: d,
                }
            })
            .collect())
    }
}

/// Open Library channel.
#[derive(Debug, Clone)]
pub struct OpenLibraryChannel {
    router: BackendRouter,
    backend: OpenLibraryBackend,
}

impl OpenLibraryChannel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self::from_backend(OpenLibraryBackend::with_base_url(base_url))
    }

    fn from_backend(backend: OpenLibraryBackend) -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for OpenLibraryChannel {
    fn default() -> Self {
        Self::from_backend(OpenLibraryBackend::new())
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for OpenLibraryChannel {
    fn name(&self) -> &str {
        "openlibrary"
    }

    fn description(&self) -> &str {
        "Book search and works via the Open Library API"
    }

    fn format_for_llm(&self, raw: &str) -> String {
        crate::format::extract_text_fields(raw, &["title", "author_name", "description"], 6000)
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("openlibrary.org/")
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
    fn metadata_and_work_parsing() {
        let ch = OpenLibraryChannel::new();
        assert_eq!(ch.name(), "openlibrary");
        assert_eq!(ch.tier(), Tier::Zero);
        assert!(ch.can_handle("https://openlibrary.org/works/OL45883W"));
        assert_eq!(
            OpenLibraryBackend::parse_work("https://openlibrary.org/works/OL45883W"),
            Some("works/OL45883W".to_string())
        );
        assert_eq!(
            OpenLibraryBackend::parse_work("https://openlibrary.org/authors/OL1A"),
            None
        );
    }

    #[tokio::test]
    async fn search_maps_docs() {
        let server = MockServer::start().await;
        let body = r#"{"docs":[{"title":"The Rust Book","author_name":["Steve"],"key":"/works/OL1W","first_publish_year":2019}]}"#;
        Mock::given(method("GET"))
            .and(path("/search.json"))
            .and(query_param("q", "rust"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = OpenLibraryChannel::with_base_url(server.uri());
        let results = ch.search("rust", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "The Rust Book");
        assert!(results[0].url.ends_with("/works/OL1W"));
        assert!(results[0].snippet.contains("2019"));
    }

    #[tokio::test]
    async fn read_handles_object_description() {
        let server = MockServer::start().await;
        let body = r#"{"title":"Dune","description":{"value":"A desert planet."}}"#;
        Mock::given(method("GET"))
            .and(path("/works/OL1W.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = OpenLibraryChannel::with_base_url(server.uri());
        let content = ch
            .read("https://openlibrary.org/works/OL1W", ReadOptions::default())
            .await
            .unwrap();
        assert_eq!(content.title.as_deref(), Some("Dune"));
        assert!(content.body.contains("desert planet"));
    }
}
