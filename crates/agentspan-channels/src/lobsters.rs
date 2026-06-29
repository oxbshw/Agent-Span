//! Lobsters channel — the lobste.rs link aggregator (computing-focused).
//!
//! Tier 0: no key. `search` queries stories; `read` fetches a story (and its
//! description) by its `lobste.rs/s/<id>` URL.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

const DEFAULT_BASE: &str = "https://lobste.rs";

/// `submitter_user` is a bare string in current Lobsters JSON, an object in old.
fn submitter(story: &serde_json::Value) -> Option<String> {
    story["submitter_user"]
        .as_str()
        .or_else(|| story["submitter_user"]["username"].as_str())
        .map(|s| s.to_string())
}

fn story_to_result(s: &serde_json::Value) -> SearchResult {
    let external = s["url"].as_str().filter(|u| !u.is_empty());
    let comments = s["short_id_url"]
        .as_str()
        .or_else(|| s["comments_url"].as_str());
    let tags = s["tags"]
        .as_array()
        .map(|t| {
            t.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let score = s["score"].as_i64().unwrap_or(0);
    SearchResult {
        url: external.or(comments).unwrap_or("").to_string(),
        snippet: format!("[{tags}] score {score}"),
        author: submitter(s),
        timestamp: s["created_at"].as_str().map(|x| x.to_string()),
        title: s["title"].as_str().unwrap_or("").to_string(),
        metadata: s.clone(),
    }
}

/// Lobsters backend.
#[derive(Debug, Clone)]
pub struct LobstersBackend {
    client: reqwest::Client,
    base_url: String,
}

impl Default for LobstersBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
        }
    }
}

impl LobstersBackend {
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

    /// Extract the short id from a `lobste.rs/s/<id>[/slug]` URL.
    fn parse_short_id(url: &str) -> Option<String> {
        let rest = url.split("/s/").nth(1)?;
        let id: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        if id.is_empty() {
            None
        } else {
            Some(id)
        }
    }
}

#[async_trait]
impl Backend for LobstersBackend {
    fn name(&self) -> &str {
        "lobsters-api"
    }

    async fn probe(&self) -> ProbeResult {
        ProbeResult::ok("lobsters-api", "v1")
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let id = Self::parse_short_id(url)
            .ok_or_else(|| BackendError::NotFound(format!("no Lobsters story id in: {url}")))?;
        let story = self
            .get_json(&format!("{}/s/{}.json", self.base_url, id))
            .await?;
        let result = story_to_result(&story);
        let desc = story["description"].as_str().unwrap_or("");
        Ok(Content {
            url: result.url.clone(),
            title: Some(result.title.clone()),
            body: format!("{}\n{}\n{}", result.title, result.snippet, desc),
            metadata: story,
            cached: false,
        })
    }

    async fn search(
        &self,
        query: &str,
        _opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let url = format!(
            "{}/search.json?q={}&what=stories&order=relevance",
            self.base_url,
            crate::percent_encode(query.trim())
        );
        let payload = self.get_json(&url).await?;
        // Lobsters search returns a bare array; tolerate a {stories:[]} wrapper too.
        let stories = payload
            .as_array()
            .cloned()
            .or_else(|| payload["stories"].as_array().cloned())
            .unwrap_or_default();
        Ok(stories.iter().map(story_to_result).collect())
    }
}

/// Lobsters channel.
#[derive(Debug, Clone)]
pub struct LobstersChannel {
    router: BackendRouter,
    backend: LobstersBackend,
}

impl LobstersChannel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self::from_backend(LobstersBackend::with_base_url(base_url))
    }

    fn from_backend(backend: LobstersBackend) -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for LobstersChannel {
    fn default() -> Self {
        Self::from_backend(LobstersBackend::new())
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for LobstersChannel {
    fn name(&self) -> &str {
        "lobsters"
    }

    fn description(&self) -> &str {
        "Lobsters (lobste.rs) computing link aggregator"
    }

    fn format_for_llm(&self, raw: &str) -> String {
        crate::format::extract_text_fields(raw, &["title", "url", "description", "tags"], 6000)
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("lobste.rs")
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
    fn metadata_and_short_id_parsing() {
        let ch = LobstersChannel::new();
        assert_eq!(ch.name(), "lobsters");
        assert_eq!(ch.tier(), Tier::Zero);
        assert!(ch.can_handle("https://lobste.rs/s/abc123/some-title"));
        assert_eq!(
            LobstersBackend::parse_short_id("https://lobste.rs/s/abc123/some-title"),
            Some("abc123".to_string())
        );
        assert_eq!(LobstersBackend::parse_short_id("https://lobste.rs/"), None);
    }

    #[tokio::test]
    async fn search_maps_story_array() {
        let server = MockServer::start().await;
        let body = r#"[{"title":"Rust 2.0","url":"https://blog/rust2","short_id_url":"https://lobste.rs/s/x","score":42,"tags":["rust","programming"],"submitter_user":"alice"}]"#;
        Mock::given(method("GET"))
            .and(path("/search.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = LobstersChannel::with_base_url(server.uri());
        let results = ch.search("rust", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust 2.0");
        assert_eq!(results[0].url, "https://blog/rust2");
        assert_eq!(results[0].author.as_deref(), Some("alice"));
        assert!(results[0].snippet.contains("rust, programming"));
    }

    #[tokio::test]
    async fn read_fetches_story_by_short_id() {
        let server = MockServer::start().await;
        let body = r#"{"title":"A Story","url":"","short_id_url":"https://lobste.rs/s/x","description":"the body text","tags":["ask"],"score":3}"#;
        Mock::given(method("GET"))
            .and(path("/s/x.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = LobstersChannel::with_base_url(server.uri());
        let content = ch
            .read("https://lobste.rs/s/x/a-story", ReadOptions::default())
            .await
            .unwrap();
        assert_eq!(content.title.as_deref(), Some("A Story"));
        assert!(content.body.contains("the body text"));
    }
}
