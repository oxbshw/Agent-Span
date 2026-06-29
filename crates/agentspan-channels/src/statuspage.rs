//! Status page channel — reads any Atlassian-Statuspage-powered status site.
//!
//! Statuspage sites expose a stable `/api/v2/summary.json`. Given a status-page
//! host (e.g. `githubstatus.com`), this derives that endpoint and summarises the
//! overall status, components, and active incidents. No key required.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

/// Derive the `summary.json` API URL from a status-page host or URL.
fn summary_url(input: &str) -> Option<String> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }
    let with_scheme = if s.contains("://") {
        s.to_string()
    } else {
        format!("https://{s}")
    };
    let (scheme, rest) = with_scheme.split_once("://")?;
    let host = rest.split('/').next().filter(|h| !h.is_empty())?;
    Some(format!("{scheme}://{host}/api/v2/summary.json"))
}

/// Statuspage backend. The "base" is the status host given on each call, so
/// there's no fixed base_url to override.
#[derive(Debug, Clone)]
pub struct StatusPageBackend {
    client: reqwest::Client,
}

impl Default for StatusPageBackend {
    fn default() -> Self {
        // default_client() applies the configured proxy; Client::default() would not.
        Self {
            client: crate::http::default_client(),
        }
    }
}

impl StatusPageBackend {
    pub fn new() -> Self {
        Self::default()
    }

    async fn summary(&self, host: &str) -> Result<serde_json::Value, BackendError> {
        let url = summary_url(host).ok_or_else(|| {
            BackendError::Parse(
                self.name().to_string(),
                format!("not a status host: {host}"),
            )
        })?;
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
impl Backend for StatusPageBackend {
    fn name(&self) -> &str {
        "statuspage"
    }

    async fn probe(&self) -> ProbeResult {
        ProbeResult::ok("statuspage", "v2")
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let payload = self.summary(url).await?;
        let page = payload["page"]["name"].as_str().unwrap_or(url);
        let description = payload["status"]["description"]
            .as_str()
            .unwrap_or("unknown");
        let mut body = format!("{page}: {description}");
        for comp in payload["components"].as_array().into_iter().flatten() {
            if let (Some(name), Some(status)) = (comp["name"].as_str(), comp["status"].as_str()) {
                body.push_str(&format!("\n- {name}: {status}"));
            }
        }
        let incidents = payload["incidents"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0);
        if incidents > 0 {
            body.push_str(&format!("\n\n{incidents} active incident(s)"));
        }
        Ok(Content {
            url: url.to_string(),
            title: Some(format!("{page} status")),
            body,
            metadata: payload,
            cached: false,
        })
    }

    async fn search(
        &self,
        query: &str,
        _opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        // "search" resolves a single host to its current status line.
        let payload = self.summary(query).await?;
        let page = payload["page"]["name"].as_str().unwrap_or(query);
        let description = payload["status"]["description"]
            .as_str()
            .unwrap_or("unknown");
        Ok(vec![SearchResult {
            url: query.to_string(),
            snippet: description.to_string(),
            author: None,
            timestamp: payload["page"]["updated_at"]
                .as_str()
                .map(|s| s.to_string()),
            title: format!("{page} status"),
            metadata: payload,
        }])
    }
}

/// Status page channel.
#[derive(Debug, Clone)]
pub struct StatusPageChannel {
    router: BackendRouter,
    backend: StatusPageBackend,
}

impl StatusPageChannel {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for StatusPageChannel {
    fn default() -> Self {
        let backend = StatusPageBackend::new();
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for StatusPageChannel {
    fn name(&self) -> &str {
        "statuspage"
    }

    fn description(&self) -> &str {
        "Read the status of any Atlassian-Statuspage-powered service"
    }

    fn format_for_llm(&self, raw: &str) -> String {
        crate::format::extract_text_fields(raw, &["name", "description", "status"], 4000)
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("statuspage.io")
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
    fn summary_url_derivation() {
        assert_eq!(
            summary_url("https://www.githubstatus.com"),
            Some("https://www.githubstatus.com/api/v2/summary.json".to_string())
        );
        assert_eq!(
            summary_url("status.example.com"),
            Some("https://status.example.com/api/v2/summary.json".to_string())
        );
        assert_eq!(summary_url(""), None);
    }

    #[test]
    fn can_handle_and_metadata() {
        let ch = StatusPageChannel::new();
        assert!(ch.can_handle("https://x.statuspage.io"));
        assert!(!ch.can_handle("https://example.com"));
        assert_eq!(ch.name(), "statuspage");
        assert_eq!(ch.tier(), Tier::Zero);
    }

    #[tokio::test]
    async fn read_summarises_status() {
        let server = MockServer::start().await;
        let body = r#"{"page":{"name":"GitHub"},"status":{"indicator":"none","description":"All Systems Operational"},"components":[{"name":"Git Operations","status":"operational"}],"incidents":[]}"#;
        Mock::given(method("GET"))
            .and(path("/api/v2/summary.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = StatusPageChannel::new();
        let content = ch
            .read(&server.uri(), ReadOptions::default())
            .await
            .unwrap();
        assert!(content.body.contains("All Systems Operational"));
        assert!(content.body.contains("Git Operations: operational"));
    }

    #[tokio::test]
    async fn search_returns_status_line() {
        let server = MockServer::start().await;
        let body = r#"{"page":{"name":"OpenAI"},"status":{"description":"Partial Outage"}}"#;
        Mock::given(method("GET"))
            .and(path("/api/v2/summary.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = StatusPageChannel::new();
        let results = ch
            .search(&server.uri(), SearchOptions::default())
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].snippet, "Partial Outage");
    }
}
