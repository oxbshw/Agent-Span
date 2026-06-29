//! DuckDuckGo channel — web answers via the Instant Answer JSON API.
//!
//! Zero-config and no key. Note this is DDG's *Instant Answer* API, not full web
//! search: it's great for definitions, disambiguations, and official links, but
//! returns nothing for many long-tail queries. `search` flattens the Results and
//! RelatedTopics; `read` returns the abstract for a query.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

const DEFAULT_BASE: &str = "https://api.duckduckgo.com";

/// Turn one `{Text, FirstURL}` entry into a result, if it has both.
fn topic_to_result(entry: &serde_json::Value) -> Option<SearchResult> {
    let url = entry["FirstURL"].as_str()?;
    let text = entry["Text"].as_str().unwrap_or("");
    if url.is_empty() {
        return None;
    }
    Some(SearchResult {
        url: url.to_string(),
        snippet: text.to_string(),
        author: None,
        timestamp: None,
        title: text.split(" - ").next().unwrap_or(text).to_string(),
        metadata: entry.clone(),
    })
}

/// DuckDuckGo Instant Answer backend.
#[derive(Debug, Clone)]
pub struct DuckDuckGoBackend {
    client: reqwest::Client,
    base_url: String,
}

impl Default for DuckDuckGoBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
        }
    }
}

impl DuckDuckGoBackend {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    async fn instant_answer(&self, query: &str) -> Result<serde_json::Value, BackendError> {
        let url = format!(
            "{}/?q={}&format=json&no_html=1&no_redirect=1&skip_disambig=1",
            self.base_url,
            crate::percent_encode(query)
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
        response
            .json()
            .await
            .map_err(|e| BackendError::Parse(self.name().to_string(), e.to_string()))
    }
}

#[async_trait]
impl Backend for DuckDuckGoBackend {
    fn name(&self) -> &str {
        "duckduckgo-ia"
    }

    async fn probe(&self) -> ProbeResult {
        ProbeResult::ok("duckduckgo-ia", "instant-answer")
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        // DDG has no per-URL read; treat the input as a query for an abstract.
        let payload = self.instant_answer(url.trim()).await?;
        let abstract_text = payload["AbstractText"].as_str().unwrap_or("");
        if abstract_text.is_empty() {
            return Err(BackendError::NotFound(format!(
                "no instant answer for: {}",
                url.trim()
            )));
        }
        Ok(Content {
            url: payload["AbstractURL"].as_str().unwrap_or(url).to_string(),
            title: payload["Heading"].as_str().map(|s| s.to_string()),
            body: abstract_text.to_string(),
            metadata: payload,
            cached: false,
        })
    }

    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let payload = self.instant_answer(query).await?;
        let mut out = Vec::new();

        // A direct abstract, when present, is the most useful first hit.
        if let (Some(text), Some(url)) = (
            payload["AbstractText"].as_str(),
            payload["AbstractURL"].as_str(),
        ) {
            if !text.is_empty() && !url.is_empty() {
                out.push(SearchResult {
                    url: url.to_string(),
                    snippet: text.to_string(),
                    author: None,
                    timestamp: None,
                    title: payload["Heading"].as_str().unwrap_or(query).to_string(),
                    metadata: payload["Infobox"].clone(),
                });
            }
        }

        for entry in payload["Results"].as_array().into_iter().flatten() {
            if let Some(r) = topic_to_result(entry) {
                out.push(r);
            }
        }
        // RelatedTopics may nest a level under `{Name, Topics:[...]}`.
        for entry in payload["RelatedTopics"].as_array().into_iter().flatten() {
            if let Some(nested) = entry["Topics"].as_array() {
                out.extend(nested.iter().filter_map(topic_to_result));
            } else if let Some(r) = topic_to_result(entry) {
                out.push(r);
            }
        }

        let limit = if opts.limit == 0 { 10 } else { opts.limit };
        out.truncate(limit);
        Ok(out)
    }
}

/// DuckDuckGo channel.
#[derive(Debug, Clone)]
pub struct DuckDuckGoChannel {
    router: BackendRouter,
    backend: DuckDuckGoBackend,
}

impl DuckDuckGoChannel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self::from_backend(DuckDuckGoBackend::new().with_base_url(base_url))
    }

    fn from_backend(backend: DuckDuckGoBackend) -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for DuckDuckGoChannel {
    fn default() -> Self {
        Self::from_backend(DuckDuckGoBackend::new())
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for DuckDuckGoChannel {
    fn name(&self) -> &str {
        "duckduckgo"
    }

    fn description(&self) -> &str {
        "Web instant answers via DuckDuckGo (definitions, disambiguation, links)"
    }

    fn format_for_llm(&self, raw: &str) -> String {
        crate::format::extract_text_fields(raw, &["AbstractText", "Heading", "Text"], 6000)
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("duckduckgo.com")
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
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn can_handle_and_metadata() {
        let ch = DuckDuckGoChannel::new();
        assert!(ch.can_handle("https://duckduckgo.com/?q=rust"));
        assert!(!ch.can_handle("https://example.com"));
        assert_eq!(ch.name(), "duckduckgo");
        assert_eq!(ch.tier(), Tier::Zero);
    }

    #[tokio::test]
    async fn search_flattens_results_and_related() {
        let server = MockServer::start().await;
        let body = r#"{
            "Heading":"Rust",
            "AbstractText":"A systems language",
            "AbstractURL":"https://example.com/rust",
            "Results":[{"Text":"Official site","FirstURL":"https://rust-lang.org"}],
            "RelatedTopics":[
                {"Text":"Rust (game)","FirstURL":"https://example.com/game"},
                {"Name":"Group","Topics":[{"Text":"Nested topic","FirstURL":"https://example.com/nested"}]}
            ]
        }"#;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = DuckDuckGoChannel::with_base_url(server.uri());
        let results = ch.search("rust", SearchOptions::default()).await.unwrap();
        // abstract + 1 result + 1 related + 1 nested = 4
        assert_eq!(results.len(), 4);
        assert_eq!(results[0].title, "Rust");
        assert_eq!(results[1].url, "https://rust-lang.org");
        assert!(results
            .iter()
            .any(|r| r.url == "https://example.com/nested"));
    }

    #[tokio::test]
    async fn read_returns_abstract() {
        let server = MockServer::start().await;
        let body = r#"{"Heading":"Rust","AbstractText":"A systems language","AbstractURL":"https://example.com/rust"}"#;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = DuckDuckGoChannel::with_base_url(server.uri());
        let content = ch.read("rust", ReadOptions::default()).await.unwrap();
        assert_eq!(content.title.as_deref(), Some("Rust"));
        assert_eq!(content.body, "A systems language");
    }

    #[tokio::test]
    async fn read_empty_abstract_errors() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"AbstractText":""}"#))
            .mount(&server)
            .await;

        let ch = DuckDuckGoChannel::with_base_url(server.uri());
        assert!(ch
            .read("obscure-thing", ReadOptions::default())
            .await
            .is_err());
    }
}
