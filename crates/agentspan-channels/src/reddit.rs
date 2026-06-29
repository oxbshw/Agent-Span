//! Reddit channel — backed by Reddit's public JSON endpoints.
//!
//! Works unauthenticated (rate-limited). A CLI backend (OpenCLI / rdt-cli) can be
//! added as a higher-preference backend when available.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

use crate::opencli::OpenCliBackend;

const DEFAULT_BASE: &str = "https://www.reddit.com";

/// Reddit public JSON backend.
#[derive(Debug, Clone)]
pub struct RedditJsonBackend {
    client: reqwest::Client,
    base_url: String,
    cookie: Option<String>,
}

impl Default for RedditJsonBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
            // Logged-in session cookie imported via `agentspan config cookies`.
            cookie: crate::http::cookie_for("reddit"),
        }
    }
}

impl RedditJsonBackend {
    /// Create a backend pointed at public Reddit.
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the base URL used for search (tests).
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Set the session cookie header (tests / explicit config).
    pub fn with_cookie(mut self, cookie: impl Into<String>) -> Self {
        self.cookie = Some(cookie.into());
        self
    }

    /// Apply the configured cookie to a request builder.
    fn with_auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.cookie {
            Some(c) => req.header("Cookie", c),
            None => req,
        }
    }

    /// Map a Reddit listing payload to [`SearchResult`]s.
    fn map_listing(payload: &serde_json::Value) -> Vec<SearchResult> {
        payload["data"]["children"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|c| {
                let d = &c["data"];
                let permalink = d["permalink"].as_str().unwrap_or("");
                SearchResult {
                    title: d["title"].as_str().unwrap_or("").to_string(),
                    url: if permalink.is_empty() {
                        d["url"].as_str().unwrap_or("").to_string()
                    } else {
                        format!("https://www.reddit.com{permalink}")
                    },
                    snippet: d["selftext"]
                        .as_str()
                        .unwrap_or("")
                        .chars()
                        .take(280)
                        .collect(),
                    author: d["author"].as_str().map(|s| s.to_string()),
                    timestamp: d["created_utc"].as_f64().map(|n| (n as i64).to_string()),
                    metadata: d.clone(),
                }
            })
            .collect()
    }
}

#[async_trait]
impl Backend for RedditJsonBackend {
    fn name(&self) -> &str {
        "reddit-json"
    }

    async fn probe(&self) -> ProbeResult {
        ProbeResult::warn(
            "reddit-json",
            "unauthenticated (rate-limited)",
            "Configure Reddit OAuth for higher limits",
        )
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let json_url = if url.ends_with(".json") {
            url.to_string()
        } else {
            format!("{}.json", url.trim_end_matches('/'))
        };
        let response = self
            .with_auth(self.client.get(&json_url))
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
        // A post page is `[post_listing, comments_listing]`; pull the post title.
        let title = payload
            .as_array()
            .and_then(|a| a.first())
            .and_then(|l| l["data"]["children"][0]["data"]["title"].as_str())
            .map(|s| s.to_string());
        Ok(Content {
            url: url.to_string(),
            title,
            body: payload.to_string(),
            metadata: payload,
            cached: false,
        })
    }

    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let limit = if opts.limit == 0 {
            25
        } else {
            opts.limit.min(100)
        };
        let api = format!(
            "{}/search.json?q={}&limit={}",
            self.base_url,
            crate::percent_encode(query),
            limit
        );
        let response = self
            .with_auth(self.client.get(&api))
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
        Ok(Self::map_listing(&payload))
    }
}

/// Reddit channel.
#[derive(Debug, Clone)]
pub struct RedditChannel {
    router: BackendRouter,
    backend: RedditJsonBackend,
}

impl RedditChannel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a channel whose backend targets `base_url` (tests).
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        let backend = RedditJsonBackend::new().with_base_url(base_url);
        let router = BackendRouter::new(
            vec![Arc::new(backend.clone()) as Arc<dyn Backend>],
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for RedditChannel {
    fn default() -> Self {
        // OpenCLI (logged-in browser session) is primary because Reddit's
        // anonymous JSON path is 403-blocked; reddit-json is the fallback.
        let backend = RedditJsonBackend::new();
        let backends: Vec<Arc<dyn Backend>> = vec![
            Arc::new(OpenCliBackend::reddit()),
            Arc::new(backend.clone()),
        ];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        )
        .with_preferred_backend(agentspan_router::env_backend_override("reddit"));
        Self { router, backend }
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for RedditChannel {
    fn name(&self) -> &str {
        "reddit"
    }

    fn description(&self) -> &str {
        "Read Reddit posts/comments and search via Reddit's public JSON API"
    }

    fn format_for_llm(&self, raw: &str) -> String {
        // Keep titles, post bodies, and comment text; drop awards/scores/media.
        crate::format::extract_text_fields(raw, &["title", "selftext", "body"], 8000)
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("reddit.com")
    }

    fn tier(&self) -> Tier {
        Tier::One
    }

    fn backends(&self) -> Vec<Box<dyn Backend>> {
        vec![
            Box::new(OpenCliBackend::reddit()),
            Box::new(self.backend.clone()),
        ]
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
    use agentspan_core::types::ProbeStatus;

    #[tokio::test]
    async fn search_sends_cookie_header_when_configured() {
        use wiremock::matchers::{header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        // Mock only matches when the Cookie header is present → without the fix
        // the request is unauthenticated and gets a 404 (search errors).
        Mock::given(method("GET"))
            .and(path("/search.json"))
            .and(header("Cookie", "reddit_session=abc"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"data":{"children":[]}}"#))
            .mount(&server)
            .await;

        let backend = RedditJsonBackend::new()
            .with_base_url(server.uri())
            .with_cookie("reddit_session=abc");
        let results = backend.search("rust", SearchOptions::default()).await;
        assert!(results.is_ok(), "cookie header was not sent: {results:?}");
    }

    #[test]
    fn format_for_llm_strips_bloat() {
        let raw = r#"{"data":{"children":[{"data":{"title":"Bug?","selftext":"repro steps","ups":42,"all_awardings":[1,2]}}]}}"#;
        let out = RedditChannel::new().format_for_llm(raw);
        assert!(out.contains("Bug?"));
        assert!(out.contains("repro steps"));
        assert!(!out.contains("all_awardings"));
        assert!(!out.contains("42"));
    }
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn can_handle_reddit_urls() {
        let ch = RedditChannel::new();
        assert!(ch.can_handle("https://www.reddit.com/r/rust"));
        assert!(!ch.can_handle("https://example.com"));
    }

    #[test]
    fn channel_is_tier_one() {
        assert_eq!(RedditChannel::new().tier(), Tier::One);
    }

    #[test]
    fn backend_name_is_stable() {
        assert_eq!(RedditJsonBackend::new().name(), "reddit-json");
    }

    #[tokio::test]
    async fn probe_warns_when_unauthenticated() {
        let probe = RedditJsonBackend::new().probe().await;
        assert_eq!(probe.status, ProbeStatus::Warn);
    }

    #[tokio::test]
    async fn read_appends_json_suffix() {
        let server = MockServer::start().await;
        let body = r#"[{"data":{"children":[{"data":{"title":"Cool post"}}]}}]"#;
        Mock::given(method("GET"))
            .and(path("/r/rust/comments/1.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let backend = RedditJsonBackend::new();
        let content = backend
            .read(
                &format!("{}/r/rust/comments/1", server.uri()),
                ReadOptions::default(),
            )
            .await
            .unwrap();
        assert_eq!(content.title.as_deref(), Some("Cool post"));
    }

    #[tokio::test]
    async fn search_maps_listing() {
        let server = MockServer::start().await;
        let body = r#"{"data":{"children":[{"data":{"title":"Rust 2.0","permalink":"/r/rust/x","author":"ferris"}}]}}"#;
        Mock::given(method("GET"))
            .and(path("/search.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = RedditChannel::with_base_url(server.uri());
        let results = ch.search("rust", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust 2.0");
        assert_eq!(results[0].author.as_deref(), Some("ferris"));
    }
}
