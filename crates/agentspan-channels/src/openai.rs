//! OpenAI channel — list and look up available models via the OpenAI API.
//!
//! Tier 1: needs an `OPENAI_API_KEY`. `search` lists models and filters by the
//! query; `read` resolves a single model id. (Model *listing*, not chat.)

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

const DEFAULT_BASE: &str = "https://api.openai.com";

/// Pull a model id from an `openai.com/.../models/<id>` URL, or accept a bare id.
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

/// OpenAI API backend.
#[derive(Debug, Clone)]
pub struct OpenAiBackend {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

impl Default for OpenAiBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
            api_key: std::env::var("OPENAI_API_KEY")
                .ok()
                .filter(|k| !k.is_empty()),
        }
    }
}

impl OpenAiBackend {
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
            .bearer_auth(self.key()?)
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
impl Backend for OpenAiBackend {
    fn name(&self) -> &str {
        "openai-api"
    }

    async fn probe(&self) -> ProbeResult {
        if self.api_key.is_some() {
            ProbeResult::ok("openai-api", "v1")
        } else {
            ProbeResult {
                status: ProbeStatus::Warn,
                message: "OPENAI_API_KEY not set".to_string(),
                version: None,
                hint: Some("export OPENAI_API_KEY=sk-...".to_string()),
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
        let owned_by = payload["owned_by"].as_str().unwrap_or("");
        Ok(Content {
            url: url.to_string(),
            title: Some(id.clone()),
            body: format!("{id}\nowned by: {owned_by}").trim().to_string(),
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
                q.is_empty()
                    || m["id"]
                        .as_str()
                        .map(|id| id.to_lowercase().contains(&q))
                        .unwrap_or(false)
            })
            .map(|m| {
                let id = m["id"].as_str().unwrap_or("").to_string();
                SearchResult {
                    url: format!("https://platform.openai.com/docs/models/{id}"),
                    snippet: m["owned_by"].as_str().unwrap_or("").to_string(),
                    author: m["owned_by"].as_str().map(|s| s.to_string()),
                    timestamp: None,
                    title: id,
                    metadata: m,
                }
            })
            .collect())
    }
}

/// OpenAI channel.
#[derive(Debug, Clone)]
pub struct OpenAiChannel {
    router: BackendRouter,
    backend: OpenAiBackend,
}

impl OpenAiChannel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self::from_backend(OpenAiBackend::with_base_url(base_url))
    }

    fn from_backend(backend: OpenAiBackend) -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for OpenAiChannel {
    fn default() -> Self {
        Self::from_backend(OpenAiBackend::new())
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for OpenAiChannel {
    fn name(&self) -> &str {
        "openai"
    }

    fn description(&self) -> &str {
        "List and look up OpenAI models (needs OPENAI_API_KEY)"
    }

    fn format_for_llm(&self, raw: &str) -> String {
        crate::format::extract_text_fields(raw, &["id", "owned_by"], 4000)
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("openai.com/")
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
        let ch = OpenAiChannel::new();
        assert!(ch.can_handle("https://platform.openai.com/docs/models/gpt-4o"));
        assert!(!ch.can_handle("https://example.com"));
        assert_eq!(ch.name(), "openai");
        assert_eq!(ch.tier(), Tier::One);
    }

    #[tokio::test]
    async fn probe_warns_without_key() {
        // Default backend with no env key -> warn (not ok).
        let backend = OpenAiBackend {
            api_key: None,
            ..OpenAiBackend::new()
        };
        assert_eq!(backend.probe().await.status, ProbeStatus::Warn);
    }

    #[tokio::test]
    async fn search_filters_models_by_query() {
        let server = MockServer::start().await;
        let body = r#"{"data":[{"id":"gpt-4o","owned_by":"openai"},{"id":"dall-e-3","owned_by":"openai"}]}"#;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = OpenAiChannel::with_base_url(server.uri());
        let results = ch.search("gpt", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "gpt-4o");
    }

    #[tokio::test]
    async fn read_resolves_model() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models/gpt-4o"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"id":"gpt-4o","owned_by":"openai"}"#),
            )
            .mount(&server)
            .await;

        let ch = OpenAiChannel::with_base_url(server.uri());
        let content = ch.read("gpt-4o", ReadOptions::default()).await.unwrap();
        assert_eq!(content.title.as_deref(), Some("gpt-4o"));
        assert!(content.body.contains("openai"));
    }
}
