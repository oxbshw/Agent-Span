//! Slack channel — search messages via the Slack Web API.
//!
//! Tier 1: needs a `SLACK_TOKEN` (a user token with `search:read`). Slack
//! returns HTTP 200 with `{"ok": false, "error": ...}` on failure, which we
//! surface as an error. `read` returns the top matching message.

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

const DEFAULT_BASE: &str = "https://slack.com/api";

/// Slack Web API backend.
#[derive(Debug, Clone)]
pub struct SlackBackend {
    client: reqwest::Client,
    base_url: String,
    token: Option<String>,
}

impl Default for SlackBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
            token: std::env::var("SLACK_TOKEN").ok().filter(|k| !k.is_empty()),
        }
    }
}

impl SlackBackend {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base: impl Into<String>) -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: base.into(),
            token: Some("test-token".to_string()),
        }
    }

    fn token(&self) -> Result<&str, BackendError> {
        self.token
            .as_deref()
            .ok_or_else(|| BackendError::AuthRequired(self.name().to_string()))
    }

    async fn search_messages(
        &self,
        query: &str,
        count: usize,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let url = format!(
            "{}/search.messages?query={}&count={}",
            self.base_url,
            crate::percent_encode(query),
            count
        );
        let response = self
            .client
            .get(&url)
            .bearer_auth(self.token()?)
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
        // Slack signals logical errors with ok:false even on HTTP 200.
        if payload["ok"].as_bool() != Some(true) {
            let err = payload["error"].as_str().unwrap_or("unknown");
            return Err(BackendError::RequestFailed(
                self.name().to_string(),
                format!("slack error: {err}"),
            ));
        }
        let matches = payload["messages"]["matches"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        Ok(matches
            .into_iter()
            .map(|m| SearchResult {
                url: m["permalink"].as_str().unwrap_or("").to_string(),
                snippet: m["text"].as_str().unwrap_or("").to_string(),
                author: m["username"].as_str().map(|s| s.to_string()),
                timestamp: m["ts"].as_str().map(|s| s.to_string()),
                title: m["channel"]["name"]
                    .as_str()
                    .map(|c| format!("#{c}"))
                    .unwrap_or_else(|| "message".to_string()),
                metadata: m,
            })
            .collect())
    }
}

#[async_trait]
impl Backend for SlackBackend {
    fn name(&self) -> &str {
        "slack-api"
    }

    async fn probe(&self) -> ProbeResult {
        if self.token.is_some() {
            ProbeResult::ok("slack-api", "web")
        } else {
            ProbeResult {
                status: ProbeStatus::Warn,
                message: "SLACK_TOKEN not set".to_string(),
                version: None,
                hint: Some("a user token with search:read scope".to_string()),
            }
        }
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let top = self
            .search_messages(url.trim(), 1)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| BackendError::NotFound(format!("no Slack match for: {}", url.trim())))?;
        Ok(Content {
            url: top.url.clone(),
            title: Some(top.title.clone()),
            body: format!("{}\n{}", top.title, top.snippet),
            metadata: top.metadata,
            cached: false,
        })
    }

    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let count = if opts.limit == 0 {
            10
        } else {
            opts.limit.min(100)
        };
        self.search_messages(query, count).await
    }
}

/// Slack channel.
#[derive(Debug, Clone)]
pub struct SlackChannel {
    router: BackendRouter,
    backend: SlackBackend,
}

impl SlackChannel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self::from_backend(SlackBackend::with_base_url(base_url))
    }

    fn from_backend(backend: SlackBackend) -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for SlackChannel {
    fn default() -> Self {
        Self::from_backend(SlackBackend::new())
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for SlackChannel {
    fn name(&self) -> &str {
        "slack"
    }

    fn description(&self) -> &str {
        "Search Slack messages (needs SLACK_TOKEN with search:read)"
    }

    fn format_for_llm(&self, raw: &str) -> String {
        crate::format::extract_text_fields(raw, &["text", "permalink", "username"], 6000)
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("slack.com/archives") || url.contains("app.slack.com")
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
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn can_handle_and_tier() {
        let ch = SlackChannel::new();
        assert!(ch.can_handle("https://acme.slack.com/archives/C123/p456"));
        assert!(!ch.can_handle("https://example.com"));
        assert_eq!(ch.name(), "slack");
        assert_eq!(ch.tier(), Tier::One);
    }

    #[tokio::test]
    async fn probe_warns_without_token() {
        let backend = SlackBackend {
            token: None,
            ..SlackBackend::new()
        };
        assert_eq!(backend.probe().await.status, ProbeStatus::Warn);
    }

    #[tokio::test]
    async fn search_maps_matches() {
        let server = MockServer::start().await;
        let body = r#"{"ok":true,"messages":{"matches":[{"text":"hello world","permalink":"https://s/p1","username":"alice","channel":{"name":"general"}}]}}"#;
        Mock::given(method("GET"))
            .and(path("/search.messages"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = SlackChannel::with_base_url(server.uri());
        let results = ch.search("hello", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "#general");
        assert_eq!(results[0].author.as_deref(), Some("alice"));
    }

    #[tokio::test]
    async fn slack_logical_error_is_surfaced() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search.messages"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"ok":false,"error":"not_allowed_token_type"}"#),
            )
            .mount(&server)
            .await;

        let ch = SlackChannel::with_base_url(server.uri());
        assert!(ch.search("x", SearchOptions::default()).await.is_err());
    }
}
