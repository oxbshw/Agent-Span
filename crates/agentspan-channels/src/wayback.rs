//! Wayback Machine channel — historical snapshots from the Internet Archive.
//!
//! Backed by the CDX server (`web.archive.org/cdx`), which needs no key. `read`
//! returns the most recent capture of a URL; `search` lists captures. Responses
//! are JSON arrays-of-arrays whose first row is a header.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

const DEFAULT_BASE: &str = "https://web.archive.org";

/// If `input` is itself a `…/web/<timestamp>/<original>` URL, return the
/// original; otherwise return the input unchanged (it's the target to look up).
fn target_url(input: &str) -> String {
    if let Some(idx) = input.find("/web/") {
        let after = &input[idx + "/web/".len()..];
        if let Some(slash) = after.find('/') {
            return after[slash + 1..].to_string();
        }
    }
    input.to_string()
}

fn snapshot_url(timestamp: &str, original: &str) -> String {
    format!("https://web.archive.org/web/{timestamp}/{original}")
}

/// Internet Archive CDX backend.
#[derive(Debug, Clone)]
pub struct WaybackBackend {
    client: reqwest::Client,
    base_url: String,
}

impl Default for WaybackBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
        }
    }
}

impl WaybackBackend {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Query the CDX API. `limit` of `-1` asks for the most recent capture.
    async fn cdx(&self, target: &str, limit: i64) -> Result<Vec<Vec<String>>, BackendError> {
        let url = format!(
            "{}/cdx/search/cdx?url={}&output=json&limit={}",
            self.base_url,
            crate::percent_encode(target),
            limit
        );
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
impl Backend for WaybackBackend {
    fn name(&self) -> &str {
        "wayback-cdx"
    }

    async fn probe(&self) -> ProbeResult {
        ProbeResult::ok("wayback-cdx", "cdx")
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let target = target_url(url);
        let rows = self.cdx(&target, -1).await?;
        // rows[0] is the header; the single data row (if any) is the latest.
        let row = rows
            .into_iter()
            .nth(1)
            .ok_or_else(|| BackendError::NotFound(format!("no Wayback snapshot for {target}")))?;
        let timestamp = row.get(1).cloned().unwrap_or_default();
        let original = row.get(2).cloned().unwrap_or_else(|| target.clone());
        let snap = snapshot_url(&timestamp, &original);
        Ok(Content {
            url: url.to_string(),
            title: Some(format!("Wayback snapshot {timestamp}")),
            body: format!("Most recent capture of {original}\n{timestamp}\n{snap}"),
            metadata: serde_json::json!({ "timestamp": timestamp, "original": original, "snapshot": snap }),
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
        } as i64;
        let rows = self.cdx(query.trim(), limit).await?;
        Ok(rows
            .into_iter()
            .skip(1) // header
            .filter_map(|row| {
                let timestamp = row.get(1)?.clone();
                let original = row.get(2)?.clone();
                Some(SearchResult {
                    url: snapshot_url(&timestamp, &original),
                    snippet: original.clone(),
                    author: None,
                    timestamp: Some(timestamp),
                    title: original,
                    metadata: serde_json::Value::Array(
                        row.into_iter().map(serde_json::Value::String).collect(),
                    ),
                })
            })
            .collect())
    }
}

/// Wayback Machine channel.
#[derive(Debug, Clone)]
pub struct WaybackChannel {
    router: BackendRouter,
    backend: WaybackBackend,
}

impl WaybackChannel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self::from_backend(WaybackBackend::new().with_base_url(base_url))
    }

    fn from_backend(backend: WaybackBackend) -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for WaybackChannel {
    fn default() -> Self {
        Self::from_backend(WaybackBackend::new())
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for WaybackChannel {
    fn name(&self) -> &str {
        "wayback"
    }

    fn description(&self) -> &str {
        "Find historical snapshots of a URL via the Internet Archive Wayback Machine"
    }

    fn format_for_llm(&self, raw: &str) -> String {
        crate::format::extract_text_fields(raw, &["original", "timestamp", "snapshot"], 4000)
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("web.archive.org/")
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
    fn target_url_unwraps_wayback_links() {
        assert_eq!(
            target_url("https://web.archive.org/web/20230101000000/http://example.com/"),
            "http://example.com/"
        );
        assert_eq!(target_url("http://example.com"), "http://example.com");
    }

    #[test]
    fn can_handle_and_metadata() {
        let ch = WaybackChannel::new();
        assert!(ch.can_handle("https://web.archive.org/web/2023/http://x.com"));
        assert!(!ch.can_handle("https://example.com"));
        assert_eq!(ch.name(), "wayback");
        assert_eq!(ch.tier(), Tier::Zero);
    }

    #[tokio::test]
    async fn read_returns_latest_snapshot() {
        let server = MockServer::start().await;
        let body = r#"[["urlkey","timestamp","original","mimetype","statuscode","digest","length"],["com,example)/","20230101000000","http://example.com/","text/html","200","ABC","123"]]"#;
        Mock::given(method("GET"))
            .and(path("/cdx/search/cdx"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = WaybackChannel::with_base_url(server.uri());
        let content = ch
            .read("http://example.com/", ReadOptions::default())
            .await
            .unwrap();
        assert!(content.body.contains("20230101000000"));
        assert!(content
            .body
            .contains("web/20230101000000/http://example.com/"));
    }

    #[tokio::test]
    async fn search_lists_snapshots() {
        let server = MockServer::start().await;
        let body = r#"[["urlkey","timestamp","original"],["k","20200101000000","http://a.com/"],["k","20210101000000","http://a.com/"]]"#;
        Mock::given(method("GET"))
            .and(path("/cdx/search/cdx"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = WaybackChannel::with_base_url(server.uri());
        let results = ch
            .search("http://a.com/", SearchOptions::default())
            .await
            .unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].timestamp.as_deref(), Some("20200101000000"));
    }

    #[tokio::test]
    async fn read_no_snapshot_is_error() {
        let server = MockServer::start().await;
        // Only the header row -> no captures.
        Mock::given(method("GET"))
            .and(path("/cdx/search/cdx"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"[["urlkey","timestamp","original"]]"#),
            )
            .mount(&server)
            .await;

        let ch = WaybackChannel::with_base_url(server.uri());
        let result = ch
            .read("http://nope.example/", ReadOptions::default())
            .await;
        assert!(result.is_err());
    }
}
