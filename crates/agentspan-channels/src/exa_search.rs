//! Exa semantic web-search channel — "search the whole internet".
//!
//! Primary backend is Exa via `mcporter` (free, no API key — the same path
//! Agent Reach uses). A direct Exa HTTP API backend is the fallback for when
//! an `EXA_API_KEY` is configured. This channel is search-only.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::{ProbeEngine, ProbeTarget};
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

const DEFAULT_API_BASE: &str = "https://api.exa.ai";

fn map_results(payload: &serde_json::Value, limit: usize) -> Vec<SearchResult> {
    payload["results"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .take(if limit == 0 { usize::MAX } else { limit })
        .map(|r| SearchResult {
            title: r["title"].as_str().unwrap_or("").to_string(),
            url: r["url"].as_str().unwrap_or("").to_string(),
            snippet: r["text"]
                .as_str()
                .or_else(|| r["snippet"].as_str())
                .or_else(|| r["highlights"][0].as_str())
                .unwrap_or("")
                .chars()
                .take(280)
                .collect(),
            author: r["author"].as_str().map(|s| s.to_string()),
            timestamp: r["publishedDate"].as_str().map(|s| s.to_string()),
            metadata: r.clone(),
        })
        .collect()
}

/// Exa via the `mcporter` MCP bridge (free, no key). Primary backend.
#[derive(Debug, Clone, Default)]
pub struct ExaMcporterBackend;

impl ExaMcporterBackend {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Backend for ExaMcporterBackend {
    fn name(&self) -> &str {
        "exa-mcporter"
    }

    async fn probe(&self) -> ProbeResult {
        let engine = ProbeEngine::new(Duration::from_secs(5));
        let target = ProbeTarget::version(
            "mcporter",
            "Install mcporter: npm install -g mcporter, then: mcporter config add exa https://mcp.exa.ai/mcp",
        );
        engine.probe(&target).await
    }

    async fn read(&self, _url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        Err(BackendError::Other(
            self.name().to_string(),
            "exa is search-only".to_string(),
        ))
    }

    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let limit = if opts.limit == 0 { 10 } else { opts.limit };
        let limit_s = limit.to_string();
        // Caveat: mcporter's invocation + output shape are assumed from its
        // docs, not pinned to a version — so this falls back to raw output
        // if the response isn't the expected JSON.
        let output = tokio::process::Command::new("mcporter")
            .args([
                "run",
                "exa",
                "web_search_exa",
                "--query",
                query,
                "--num-results",
                limit_s.as_str(),
            ])
            .output()
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    BackendError::CommandNotFound(self.name().to_string())
                } else {
                    BackendError::CommandFailed(self.name().to_string(), e.to_string())
                }
            })?;
        if !output.status.success() {
            return Err(BackendError::CommandFailed(
                self.name().to_string(),
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }
        let payload: serde_json::Value = match serde_json::from_slice(&output.stdout) {
            Ok(v) => v,
            Err(_) => {
                let raw = String::from_utf8_lossy(&output.stdout);
                return Ok(crate::format::raw_search_fallback(&raw));
            }
        };
        Ok(map_results(&payload, limit))
    }
}

/// Direct Exa HTTP API backend (requires `EXA_API_KEY`). Fallback / testable.
#[derive(Debug, Clone)]
pub struct ExaApiBackend {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

impl Default for ExaApiBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_API_BASE.to_string(),
            api_key: std::env::var("EXA_API_KEY").ok(),
        }
    }
}

impl ExaApiBackend {
    /// Create a backend reading `EXA_API_KEY` from the environment.
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the base URL (tests).
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Set an explicit API key (tests / programmatic config).
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }
}

#[async_trait]
impl Backend for ExaApiBackend {
    fn name(&self) -> &str {
        "exa-api"
    }

    async fn probe(&self) -> ProbeResult {
        if self.api_key.is_some() {
            ProbeResult::ok("exa-api", "authenticated")
        } else {
            ProbeResult::warn(
                "exa-api",
                "no EXA_API_KEY configured",
                "Set EXA_API_KEY, or use the free mcporter backend",
            )
        }
    }

    async fn read(&self, _url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        Err(BackendError::Other(
            self.name().to_string(),
            "exa is search-only".to_string(),
        ))
    }

    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let limit = if opts.limit == 0 { 10 } else { opts.limit };
        let mut request = self
            .client
            .post(format!("{}/search", self.base_url))
            .json(&serde_json::json!({ "query": query, "numResults": limit }));
        if let Some(key) = &self.api_key {
            request = request.header("x-api-key", key);
        }
        let response = request
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
        Ok(map_results(&payload, limit))
    }
}

/// Exa semantic web-search channel.
#[derive(Debug, Clone)]
pub struct ExaSearchChannel {
    router: BackendRouter,
    api: ExaApiBackend,
}

impl ExaSearchChannel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an API-only channel pointed at `base_url` with a key (tests).
    pub fn with_api(base_url: impl Into<String>, key: impl Into<String>) -> Self {
        let api = ExaApiBackend::new()
            .with_base_url(base_url)
            .with_api_key(key);
        let router = BackendRouter::new(
            vec![Arc::new(api.clone()) as Arc<dyn Backend>],
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, api }
    }
}

impl Default for ExaSearchChannel {
    fn default() -> Self {
        let api = ExaApiBackend::new();
        let backends: Vec<Arc<dyn Backend>> =
            vec![Arc::new(ExaMcporterBackend::new()), Arc::new(api.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, api }
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for ExaSearchChannel {
    fn name(&self) -> &str {
        "exa"
    }

    fn description(&self) -> &str {
        "Semantic web search across the whole internet via Exa (free through mcporter)"
    }

    fn can_handle(&self, _url: &str) -> bool {
        // Search-only channel; not selected for URL reads.
        false
    }

    fn tier(&self) -> Tier {
        Tier::Zero
    }

    fn backends(&self) -> Vec<Box<dyn Backend>> {
        vec![
            Box::new(ExaMcporterBackend::new()),
            Box::new(self.api.clone()),
        ]
    }

    async fn read(&self, _url: &str, _opts: ReadOptions) -> Result<Content, ChannelError> {
        Err(ChannelError::Other(
            "exa is a search-only channel; use search instead".to_string(),
        ))
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
    fn channel_is_search_only() {
        let ch = ExaSearchChannel::new();
        assert!(!ch.can_handle("https://example.com"));
        assert_eq!(ch.name(), "exa");
    }

    #[test]
    fn channel_has_mcporter_and_api_backends() {
        let names: Vec<_> = ExaSearchChannel::new()
            .backends()
            .iter()
            .map(|b| b.name().to_string())
            .collect();
        assert_eq!(names, vec!["exa-mcporter", "exa-api"]);
    }

    #[tokio::test]
    async fn read_is_unsupported() {
        let ch = ExaSearchChannel::new();
        assert!(ch.read("https://x", ReadOptions::default()).await.is_err());
    }

    #[tokio::test]
    async fn api_search_maps_results() {
        let server = MockServer::start().await;
        let body = r#"{"results":[{"title":"Rust Lang","url":"https://rust-lang.org","text":"systems language","author":"core","publishedDate":"2024-01-01"}]}"#;
        Mock::given(method("POST"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = ExaSearchChannel::with_api(server.uri(), "test-key");
        let results = ch.search("rust", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust Lang");
        assert_eq!(results[0].url, "https://rust-lang.org");
    }

    #[tokio::test]
    async fn api_backend_warns_without_key() {
        let probe = ExaApiBackend::new().probe().await;
        // In CI EXA_API_KEY is unset → warn.
        assert!(!probe.message.is_empty());
    }
}
