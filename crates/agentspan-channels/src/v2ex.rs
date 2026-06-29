//! V2EX channel — backed by the public V2EX REST API (no auth).

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

const DEFAULT_BASE: &str = "https://www.v2ex.com/api";

/// Extract a topic id from a `/t/{id}` URL.
fn parse_topic_id(url: &str) -> Option<String> {
    let rest = url.split("/t/").nth(1)?;
    let id: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if id.is_empty() {
        None
    } else {
        Some(id)
    }
}

fn topic_to_result(t: &serde_json::Value) -> SearchResult {
    SearchResult {
        title: t["title"].as_str().unwrap_or("").to_string(),
        url: t["url"].as_str().unwrap_or("").to_string(),
        snippet: t["content"]
            .as_str()
            .unwrap_or("")
            .chars()
            .take(280)
            .collect(),
        author: t["member"]["username"].as_str().map(|s| s.to_string()),
        timestamp: t["created"].as_i64().map(|n| n.to_string()),
        metadata: t.clone(),
    }
}

/// V2EX REST API backend.
#[derive(Debug, Clone)]
pub struct V2exApiBackend {
    client: reqwest::Client,
    base_url: String,
}

impl Default for V2exApiBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
        }
    }
}

impl V2exApiBackend {
    /// Create a backend pointed at the public V2EX API.
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the base URL (tests).
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }
}

#[async_trait]
impl Backend for V2exApiBackend {
    fn name(&self) -> &str {
        "v2ex-api"
    }

    async fn probe(&self) -> ProbeResult {
        ProbeResult::ok("v2ex-api", "v1")
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let id = parse_topic_id(url).ok_or_else(|| {
            BackendError::Parse(
                self.name().to_string(),
                format!("not a V2EX topic URL: {url}"),
            )
        })?;
        let api = format!("{}/topics/show.json?id={}", self.base_url, id);
        let response = self
            .client
            .get(&api)
            .send()
            .await
            .map_err(|e| BackendError::RequestFailed(self.name().to_string(), e.to_string()))?;
        if !response.status().is_success() {
            return Err(BackendError::RequestFailed(
                self.name().to_string(),
                format!("HTTP {}", response.status()),
            ));
        }
        let topics: serde_json::Value = response
            .json()
            .await
            .map_err(|e| BackendError::Parse(self.name().to_string(), e.to_string()))?;
        let topic = topics
            .as_array()
            .and_then(|a| a.first())
            .cloned()
            .ok_or_else(|| BackendError::NotFound(self.name().to_string()))?;
        Ok(Content {
            url: url.to_string(),
            title: topic["title"].as_str().map(|s| s.to_string()),
            body: topic["content"]
                .as_str()
                .map(|s| s.to_string())
                .unwrap_or_default(),
            metadata: topic,
            cached: false,
        })
    }

    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        // V2EX has no public search endpoint; filter hot topics by keyword.
        let api = format!("{}/topics/hot.json", self.base_url);
        let response = self
            .client
            .get(&api)
            .send()
            .await
            .map_err(|e| BackendError::RequestFailed(self.name().to_string(), e.to_string()))?;
        if !response.status().is_success() {
            return Err(BackendError::RequestFailed(
                self.name().to_string(),
                format!("HTTP {}", response.status()),
            ));
        }
        let topics: serde_json::Value = response
            .json()
            .await
            .map_err(|e| BackendError::Parse(self.name().to_string(), e.to_string()))?;
        let needle = query.to_lowercase();
        let limit = if opts.limit == 0 { 20 } else { opts.limit };
        Ok(topics
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter(|t| {
                needle.is_empty()
                    || t["title"]
                        .as_str()
                        .map(|s| s.to_lowercase().contains(&needle))
                        .unwrap_or(false)
            })
            .take(limit)
            .map(topic_to_result)
            .collect())
    }
}

/// V2EX channel.
#[derive(Debug, Clone)]
pub struct V2exChannel {
    router: BackendRouter,
    backend: V2exApiBackend,
}

impl V2exChannel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a channel whose backend targets `base_url` (tests).
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        let backend = V2exApiBackend::new().with_base_url(base_url);
        let router = BackendRouter::new(
            vec![Arc::new(backend.clone()) as Arc<dyn Backend>],
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for V2exChannel {
    fn default() -> Self {
        let backend = V2exApiBackend::new();
        let router = BackendRouter::new(
            vec![Arc::new(backend.clone()) as Arc<dyn Backend>],
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for V2exChannel {
    fn name(&self) -> &str {
        "v2ex"
    }

    fn description(&self) -> &str {
        "Read V2EX topics and search hot topics via the public V2EX API"
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("v2ex.com")
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
    fn can_handle_v2ex_urls() {
        let ch = V2exChannel::new();
        assert!(ch.can_handle("https://www.v2ex.com/t/123"));
        assert!(!ch.can_handle("https://example.com"));
    }

    #[test]
    fn channel_is_tier_zero() {
        assert_eq!(V2exChannel::new().tier(), Tier::Zero);
    }

    #[test]
    fn parse_topic_id_works() {
        assert_eq!(
            parse_topic_id("https://www.v2ex.com/t/987654#reply3"),
            Some("987654".to_string())
        );
        assert_eq!(parse_topic_id("https://www.v2ex.com/go/rust"), None);
    }

    #[test]
    fn backend_name_is_stable() {
        assert_eq!(V2exApiBackend::new().name(), "v2ex-api");
    }

    #[tokio::test]
    async fn read_fetches_topic() {
        let server = MockServer::start().await;
        let body =
            r#"[{"title":"Hello V2EX","content":"body text","member":{"username":"alice"}}]"#;
        Mock::given(method("GET"))
            .and(path("/topics/show.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = V2exChannel::with_base_url(server.uri());
        let content = ch
            .read("https://www.v2ex.com/t/123", ReadOptions::default())
            .await
            .unwrap();
        assert_eq!(content.title.as_deref(), Some("Hello V2EX"));
        assert_eq!(content.body, "body text");
    }

    #[tokio::test]
    async fn search_filters_hot_topics() {
        let server = MockServer::start().await;
        let body = r#"[
            {"title":"Rust is great","url":"https://www.v2ex.com/t/1","member":{"username":"a"}},
            {"title":"Python tips","url":"https://www.v2ex.com/t/2","member":{"username":"b"}}
        ]"#;
        Mock::given(method("GET"))
            .and(path("/topics/hot.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = V2exChannel::with_base_url(server.uri());
        let results = ch.search("rust", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust is great");
    }

    #[tokio::test]
    async fn check_health_reports_backend() {
        let health = V2exChannel::new().check_health().await;
        assert_eq!(health.len(), 1);
        assert_eq!(health[0].backend_name, "v2ex-api");
    }
}
