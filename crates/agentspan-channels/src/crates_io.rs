//! crates.io channel — Rust package metadata and search.
//!
//! Uses the public crates.io JSON API (no key). crates.io enforces a
//! `User-Agent`, which `http::default_client` already sets.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

const DEFAULT_BASE: &str = "https://crates.io";

fn parse_crate(input: &str) -> Option<String> {
    if let Some(after) = input.split("/crates/").nth(1) {
        let name = after.split(['?', '#', '/']).next().unwrap_or(after);
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }
    if !input.contains("://") && !input.trim().is_empty() {
        return Some(input.trim().to_string());
    }
    None
}

/// crates.io JSON API backend.
#[derive(Debug, Clone)]
pub struct CratesIoBackend {
    client: reqwest::Client,
    base_url: String,
}

impl Default for CratesIoBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
        }
    }
}

impl CratesIoBackend {
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
impl Backend for CratesIoBackend {
    fn name(&self) -> &str {
        "crates-io"
    }

    async fn probe(&self) -> ProbeResult {
        ProbeResult::ok("crates-io", "v1")
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let name = parse_crate(url).ok_or_else(|| {
            BackendError::Parse(self.name().to_string(), format!("not a crate: {url}"))
        })?;
        let payload = self
            .get_json(&format!("{}/api/v1/crates/{}", self.base_url, name))
            .await?;
        let krate = &payload["crate"];
        let title = krate["name"].as_str().unwrap_or(&name).to_string();
        let version = krate["max_stable_version"]
            .as_str()
            .or_else(|| krate["max_version"].as_str())
            .unwrap_or("?");
        let description = krate["description"].as_str().unwrap_or("");
        let body = format!(
            "{title} {version}\n{description}\nrepository: {}",
            krate["repository"].as_str().unwrap_or("")
        )
        .trim()
        .to_string();
        Ok(Content {
            url: url.to_string(),
            title: Some(format!("{title} {version}")),
            body,
            metadata: payload,
            cached: false,
        })
    }

    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let per_page = if opts.limit == 0 {
            10
        } else {
            opts.limit.min(50)
        };
        let url = format!(
            "{}/api/v1/crates?q={}&per_page={}",
            self.base_url,
            crate::percent_encode(query),
            per_page
        );
        let payload = self.get_json(&url).await?;
        let crates = payload["crates"].as_array().cloned().unwrap_or_default();
        Ok(crates
            .into_iter()
            .map(|c| {
                let name = c["name"].as_str().unwrap_or("").to_string();
                SearchResult {
                    url: format!("https://crates.io/crates/{name}"),
                    snippet: c["description"].as_str().unwrap_or("").to_string(),
                    author: None,
                    timestamp: c["updated_at"].as_str().map(|s| s.to_string()),
                    title: name,
                    metadata: c,
                }
            })
            .collect())
    }
}

/// crates.io channel.
#[derive(Debug, Clone)]
pub struct CratesChannel {
    router: BackendRouter,
    backend: CratesIoBackend,
}

impl CratesChannel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self::from_backend(CratesIoBackend::new().with_base_url(base_url))
    }

    fn from_backend(backend: CratesIoBackend) -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for CratesChannel {
    fn default() -> Self {
        Self::from_backend(CratesIoBackend::new())
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for CratesChannel {
    fn name(&self) -> &str {
        "crates"
    }

    fn description(&self) -> &str {
        "Look up Rust crate metadata and search crates.io"
    }

    fn format_for_llm(&self, raw: &str) -> String {
        crate::format::extract_text_fields(raw, &["name", "description", "repository"], 6000)
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("crates.io/crates/")
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
    fn parse_crate_handles_urls_and_names() {
        assert_eq!(
            parse_crate("https://crates.io/crates/serde"),
            Some("serde".to_string())
        );
        assert_eq!(
            parse_crate("https://crates.io/crates/tokio/1.0.0"),
            Some("tokio".to_string())
        );
        assert_eq!(parse_crate("anyhow"), Some("anyhow".to_string()));
        assert_eq!(parse_crate("https://example.com"), None);
    }

    #[test]
    fn can_handle_and_metadata() {
        let ch = CratesChannel::new();
        assert!(ch.can_handle("https://crates.io/crates/serde"));
        assert!(!ch.can_handle("https://www.npmjs.com/package/x"));
        assert_eq!(ch.name(), "crates");
        assert_eq!(ch.tier(), Tier::Zero);
    }

    #[tokio::test]
    async fn read_returns_crate_metadata() {
        let server = MockServer::start().await;
        let body = r#"{"crate":{"name":"serde","max_stable_version":"1.0.0","description":"serde framework","repository":"https://github.com/serde-rs/serde"}}"#;
        Mock::given(method("GET"))
            .and(path("/api/v1/crates/serde"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = CratesChannel::with_base_url(server.uri());
        let content = ch
            .read("https://crates.io/crates/serde", ReadOptions::default())
            .await
            .unwrap();
        assert_eq!(content.title.as_deref(), Some("serde 1.0.0"));
        assert!(content.body.contains("serde framework"));
    }

    #[tokio::test]
    async fn search_maps_crates() {
        let server = MockServer::start().await;
        let body = r#"{"crates":[{"name":"tokio","description":"async runtime"}]}"#;
        Mock::given(method("GET"))
            .and(path("/api/v1/crates"))
            .and(query_param("q", "async"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = CratesChannel::with_base_url(server.uri());
        let results = ch.search("async", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "tokio");
        assert!(results[0].url.ends_with("/crates/tokio"));
    }

    #[tokio::test]
    async fn check_health_reports_backend() {
        let health = CratesChannel::new().check_health().await;
        assert_eq!(health.len(), 1);
        assert_eq!(health[0].backend_name, "crates-io");
    }
}
