//! Notion channel — search a workspace via the Notion API.
//!
//! Tier 1: needs a `NOTION_API_KEY` (integration token) shared with the pages
//! you want reachable. `search` POSTs the search endpoint; `read` returns the
//! top match for a query.

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

const DEFAULT_BASE: &str = "https://api.notion.com";
const NOTION_VERSION: &str = "2022-06-28";

/// Best-effort plain-text title from a Notion object's `properties`.
fn title_of(obj: &serde_json::Value) -> String {
    if let Some(props) = obj["properties"].as_object() {
        for v in props.values() {
            if v["type"] == "title" {
                if let Some(arr) = v["title"].as_array() {
                    let t: String = arr
                        .iter()
                        .filter_map(|r| r["plain_text"].as_str())
                        .collect();
                    if !t.is_empty() {
                        return t;
                    }
                }
            }
        }
    }
    obj["url"].as_str().unwrap_or("(untitled)").to_string()
}

/// Notion API backend.
#[derive(Debug, Clone)]
pub struct NotionBackend {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

impl Default for NotionBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
            api_key: std::env::var("NOTION_API_KEY")
                .ok()
                .filter(|k| !k.is_empty()),
        }
    }
}

impl NotionBackend {
    pub fn new() -> Self {
        Self::default()
    }

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

    async fn do_search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let body = serde_json::json!({ "query": query, "page_size": limit });
        let response = self
            .client
            .post(format!("{}/v1/search", self.base_url))
            .bearer_auth(self.key()?)
            .header("Notion-Version", NOTION_VERSION)
            .json(&body)
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
        let results = payload["results"].as_array().cloned().unwrap_or_default();
        Ok(results
            .into_iter()
            .map(|o| SearchResult {
                url: o["url"].as_str().unwrap_or("").to_string(),
                snippet: o["object"].as_str().unwrap_or("").to_string(),
                author: None,
                timestamp: o["last_edited_time"].as_str().map(|s| s.to_string()),
                title: title_of(&o),
                metadata: o,
            })
            .collect())
    }
}

#[async_trait]
impl Backend for NotionBackend {
    fn name(&self) -> &str {
        "notion-api"
    }

    async fn probe(&self) -> ProbeResult {
        if self.api_key.is_some() {
            ProbeResult::ok("notion-api", NOTION_VERSION)
        } else {
            ProbeResult {
                status: ProbeStatus::Warn,
                message: "NOTION_API_KEY not set".to_string(),
                version: None,
                hint: Some("create a Notion integration and share pages with it".to_string()),
            }
        }
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let top = self
            .do_search(url.trim(), 1)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| {
                BackendError::NotFound(format!("no Notion match for: {}", url.trim()))
            })?;
        Ok(Content {
            url: top.url.clone(),
            title: Some(top.title.clone()),
            body: format!("{}\n{}", top.title, top.url),
            metadata: top.metadata,
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
            opts.limit.min(100)
        };
        self.do_search(query, limit).await
    }
}

/// Notion channel.
#[derive(Debug, Clone)]
pub struct NotionChannel {
    router: BackendRouter,
    backend: NotionBackend,
}

impl NotionChannel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self::from_backend(NotionBackend::with_base_url(base_url))
    }

    fn from_backend(backend: NotionBackend) -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for NotionChannel {
    fn default() -> Self {
        Self::from_backend(NotionBackend::new())
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for NotionChannel {
    fn name(&self) -> &str {
        "notion"
    }

    fn description(&self) -> &str {
        "Search a Notion workspace (needs NOTION_API_KEY)"
    }

    fn format_for_llm(&self, raw: &str) -> String {
        crate::format::extract_text_fields(raw, &["plain_text", "url", "object"], 6000)
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("notion.so/") || url.contains("notion.site/")
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
        let ch = NotionChannel::new();
        assert!(ch.can_handle("https://www.notion.so/Some-Page-abc123"));
        assert!(!ch.can_handle("https://example.com"));
        assert_eq!(ch.name(), "notion");
        assert_eq!(ch.tier(), Tier::One);
    }

    #[tokio::test]
    async fn probe_warns_without_key() {
        let backend = NotionBackend {
            api_key: None,
            ..NotionBackend::new()
        };
        assert_eq!(backend.probe().await.status, ProbeStatus::Warn);
    }

    #[tokio::test]
    async fn search_posts_with_version_and_extracts_title() {
        let server = MockServer::start().await;
        let body = r#"{"results":[{"object":"page","url":"https://notion.so/p1","properties":{"Name":{"type":"title","title":[{"plain_text":"My Page"}]}}}]}"#;
        Mock::given(method("POST"))
            .and(path("/v1/search"))
            .and(header("notion-version", NOTION_VERSION))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = NotionChannel::with_base_url(server.uri());
        let results = ch.search("page", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "My Page");
        assert_eq!(results[0].url, "https://notion.so/p1");
    }
}
