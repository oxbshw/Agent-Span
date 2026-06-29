//! Anthropic channel — list and look up Claude models via the Anthropic API.
//!
//! Tier 1: needs an `ANTHROPIC_API_KEY`. Auth is the `x-api-key` header plus the
//! required `anthropic-version`. `search` lists models and filters by the query;
//! `read` resolves a single model id.

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

const DEFAULT_BASE: &str = "https://api.anthropic.com";
const API_VERSION: &str = "2023-06-01";

fn parse_model(input: &str) -> Option<String> {
    if let Some(after) = input.split("/models/").nth(1) {
        let id = after.split(['?', '#', '/']).next().unwrap_or(after);
        if !id.is_empty() {
            return Some(id.to_string());
        }
    }
    if !input.contains("://") && !input.trim().is_empty() {
        return Some(input.trim().to_string());
    }
    None
}

/// Anthropic API backend.
#[derive(Debug, Clone)]
pub struct AnthropicBackend {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

impl Default for AnthropicBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
            api_key: std::env::var("ANTHROPIC_API_KEY")
                .ok()
                .filter(|k| !k.is_empty()),
        }
    }
}

impl AnthropicBackend {
    pub fn new() -> Self {
        Self::default()
    }

    /// Point at `base` with a test key (tests).
    pub fn with_base_url(base: impl Into<String>) -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: base.into(),
            api_key: Some("test-key".to_string()),
        }
    }

    fn key(&self) -> Result<&str, BackendError> {
        self.api_key
            .as_deref()
            .ok_or_else(|| BackendError::AuthRequired(self.name().to_string()))
    }

    async fn get_json(&self, url: &str) -> Result<serde_json::Value, BackendError> {
        let response = self
            .client
            .get(url)
            .header("x-api-key", self.key()?)
            .header("anthropic-version", API_VERSION)
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
impl Backend for AnthropicBackend {
    fn name(&self) -> &str {
        "anthropic-api"
    }

    async fn probe(&self) -> ProbeResult {
        if self.api_key.is_some() {
            ProbeResult::ok("anthropic-api", API_VERSION)
        } else {
            ProbeResult {
                status: ProbeStatus::Warn,
                message: "ANTHROPIC_API_KEY not set".to_string(),
                version: None,
                hint: Some("export ANTHROPIC_API_KEY=sk-ant-...".to_string()),
            }
        }
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let id = parse_model(url).ok_or_else(|| {
            BackendError::Parse(self.name().to_string(), format!("not a model id: {url}"))
        })?;
        let payload = self
            .get_json(&format!("{}/v1/models/{}", self.base_url, id))
            .await?;
        let display = payload["display_name"].as_str().unwrap_or(&id);
        Ok(Content {
            url: url.to_string(),
            title: Some(display.to_string()),
            body: format!("{display} ({id})").trim().to_string(),
            metadata: payload,
            cached: false,
        })
    }

    async fn search(
        &self,
        query: &str,
        _opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let payload = self
            .get_json(&format!("{}/v1/models", self.base_url))
            .await?;
        let q = query.to_lowercase();
        let models = payload["data"].as_array().cloned().unwrap_or_default();
        Ok(models
            .into_iter()
            .filter(|m| {
                let hay = format!(
                    "{} {}",
                    m["id"].as_str().unwrap_or(""),
                    m["display_name"].as_str().unwrap_or("")
                )
                .to_lowercase();
                q.is_empty() || hay.contains(&q)
            })
            .map(|m| {
                let id = m["id"].as_str().unwrap_or("").to_string();
                SearchResult {
                    url: "https://docs.anthropic.com/en/docs/about-claude/models".to_string(),
                    snippet: m["display_name"].as_str().unwrap_or("").to_string(),
                    author: Some("anthropic".to_string()),
                    timestamp: m["created_at"].as_str().map(|s| s.to_string()),
                    title: id,
                    metadata: m,
                }
            })
            .collect())
    }
}

/// Anthropic channel.
#[derive(Debug, Clone)]
pub struct AnthropicChannel {
    router: BackendRouter,
    backend: AnthropicBackend,
}

impl AnthropicChannel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self::from_backend(AnthropicBackend::with_base_url(base_url))
    }

    fn from_backend(backend: AnthropicBackend) -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for AnthropicChannel {
    fn default() -> Self {
        Self::from_backend(AnthropicBackend::new())
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for AnthropicChannel {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn description(&self) -> &str {
        "List and look up Anthropic (Claude) models (needs ANTHROPIC_API_KEY)"
    }

    fn format_for_llm(&self, raw: &str) -> String {
        crate::format::extract_text_fields(raw, &["id", "display_name"], 4000)
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("anthropic.com/")
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
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn can_handle_and_tier() {
        let ch = AnthropicChannel::new();
        assert!(ch.can_handle("https://docs.anthropic.com/models"));
        assert!(!ch.can_handle("https://example.com"));
        assert_eq!(ch.name(), "anthropic");
        assert_eq!(ch.tier(), Tier::One);
    }

    #[tokio::test]
    async fn probe_warns_without_key() {
        let backend = AnthropicBackend {
            api_key: None,
            ..AnthropicBackend::new()
        };
        assert_eq!(backend.probe().await.status, ProbeStatus::Warn);
    }

    #[tokio::test]
    async fn search_sends_version_header_and_filters() {
        let server = MockServer::start().await;
        let body = r#"{"data":[{"id":"claude-3-5-sonnet-20241022","display_name":"Claude 3.5 Sonnet"},{"id":"claude-3-opus-20240229","display_name":"Claude 3 Opus"}]}"#;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .and(header("anthropic-version", API_VERSION))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = AnthropicChannel::with_base_url(server.uri());
        let results = ch.search("opus", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "claude-3-opus-20240229");
    }
}
