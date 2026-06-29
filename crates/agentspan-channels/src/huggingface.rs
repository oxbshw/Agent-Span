//! Hugging Face channel — model metadata and search via the public Hub API.
//!
//! Zero-config: `huggingface.co/api/models` is public (an optional token raises
//! rate limits but isn't required). `read` resolves a model id; `search` queries
//! the Hub.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

const DEFAULT_BASE: &str = "https://huggingface.co";

/// Pull a model id from a `huggingface.co/<org>/<model>` URL, or accept a bare
/// id (`org/model` or a single-segment id) directly.
fn parse_model(input: &str) -> Option<String> {
    let raw = if let Some(after) = input.split("huggingface.co/").nth(1) {
        after
            .split(['?', '#'])
            .next()
            .unwrap_or(after)
            .trim_matches('/')
    } else if input.contains("://") {
        return None;
    } else {
        input.trim()
    };
    // Skip Hub sections that aren't models.
    if raw.is_empty() || raw.starts_with("datasets/") || raw.starts_with("spaces/") {
        return None;
    }
    Some(raw.to_string())
}

/// Hugging Face Hub API backend.
#[derive(Debug, Clone)]
pub struct HuggingFaceBackend {
    client: reqwest::Client,
    base_url: String,
}

impl Default for HuggingFaceBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
        }
    }
}

impl HuggingFaceBackend {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
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
}

#[async_trait]
impl Backend for HuggingFaceBackend {
    fn name(&self) -> &str {
        "huggingface"
    }

    async fn probe(&self) -> ProbeResult {
        ProbeResult::ok("huggingface", "hub")
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let model = parse_model(url).ok_or_else(|| {
            BackendError::Parse(self.name().to_string(), format!("not a HF model: {url}"))
        })?;
        let payload = self
            .get_json(&format!("{}/api/models/{}", self.base_url, model))
            .await?;
        let id = payload["id"].as_str().unwrap_or(&model);
        let pipeline = payload["pipeline_tag"].as_str().unwrap_or("");
        let downloads = payload["downloads"].as_u64().unwrap_or(0);
        let likes = payload["likes"].as_u64().unwrap_or(0);
        Ok(Content {
            url: url.to_string(),
            title: Some(id.to_string()),
            body: format!("{id}\ntask: {pipeline}\n⬇ {downloads}  ♥ {likes}")
                .trim()
                .to_string(),
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
            10
        } else {
            opts.limit.min(50)
        };
        let url = format!(
            "{}/api/models?search={}&limit={}",
            self.base_url,
            crate::percent_encode(query),
            limit
        );
        let payload = self.get_json(&url).await?;
        let models = payload.as_array().cloned().unwrap_or_default();
        Ok(models
            .into_iter()
            .map(|m| {
                let id = m["id"].as_str().unwrap_or("").to_string();
                let task = m["pipeline_tag"].as_str().unwrap_or("");
                SearchResult {
                    url: format!("https://huggingface.co/{id}"),
                    snippet: format!("{task}  ⬇ {}", m["downloads"].as_u64().unwrap_or(0)),
                    author: id.split('/').next().map(|s| s.to_string()),
                    timestamp: m["lastModified"].as_str().map(|s| s.to_string()),
                    title: id,
                    metadata: m,
                }
            })
            .collect())
    }
}

/// Hugging Face channel.
#[derive(Debug, Clone)]
pub struct HuggingFaceChannel {
    router: BackendRouter,
    backend: HuggingFaceBackend,
}

impl HuggingFaceChannel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self::from_backend(HuggingFaceBackend::new().with_base_url(base_url))
    }

    fn from_backend(backend: HuggingFaceBackend) -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for HuggingFaceChannel {
    fn default() -> Self {
        Self::from_backend(HuggingFaceBackend::new())
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for HuggingFaceChannel {
    fn name(&self) -> &str {
        "huggingface"
    }

    fn description(&self) -> &str {
        "Look up and search models on the Hugging Face Hub"
    }

    fn format_for_llm(&self, raw: &str) -> String {
        crate::format::extract_text_fields(raw, &["id", "pipeline_tag", "downloads", "likes"], 6000)
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("huggingface.co/")
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
    fn parse_model_handles_urls_and_ids() {
        assert_eq!(
            parse_model("https://huggingface.co/bert-base-uncased"),
            Some("bert-base-uncased".to_string())
        );
        assert_eq!(
            parse_model("https://huggingface.co/openai/whisper-large-v3"),
            Some("openai/whisper-large-v3".to_string())
        );
        assert_eq!(parse_model("gpt2"), Some("gpt2".to_string()));
        assert_eq!(parse_model("https://huggingface.co/datasets/squad"), None);
        assert_eq!(parse_model("https://example.com"), None);
    }

    #[test]
    fn can_handle_and_metadata() {
        let ch = HuggingFaceChannel::new();
        assert!(ch.can_handle("https://huggingface.co/gpt2"));
        assert!(!ch.can_handle("https://example.com"));
        assert_eq!(ch.name(), "huggingface");
        assert_eq!(ch.tier(), Tier::Zero);
    }

    #[tokio::test]
    async fn read_returns_model_metadata() {
        let server = MockServer::start().await;
        let body = r#"{"id":"bert-base-uncased","pipeline_tag":"fill-mask","downloads":1000000,"likes":500}"#;
        Mock::given(method("GET"))
            .and(path("/api/models/bert-base-uncased"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = HuggingFaceChannel::with_base_url(server.uri());
        let content = ch
            .read(
                "https://huggingface.co/bert-base-uncased",
                ReadOptions::default(),
            )
            .await
            .unwrap();
        assert_eq!(content.title.as_deref(), Some("bert-base-uncased"));
        assert!(content.body.contains("fill-mask"));
    }

    #[tokio::test]
    async fn search_maps_models() {
        let server = MockServer::start().await;
        let body = r#"[{"id":"openai/whisper-large-v3","pipeline_tag":"automatic-speech-recognition","downloads":42}]"#;
        Mock::given(method("GET"))
            .and(path("/api/models"))
            .and(query_param("search", "whisper"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = HuggingFaceChannel::with_base_url(server.uri());
        let results = ch
            .search("whisper", SearchOptions::default())
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "openai/whisper-large-v3");
        assert_eq!(results[0].author.as_deref(), Some("openai"));
    }
}
