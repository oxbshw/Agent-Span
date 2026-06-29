//! Bilibili channel — `bili-cli` preferred, with a public web-API fallback.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::{ProbeEngine, ProbeTarget};
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

use crate::opencli::OpenCliBackend;

const DEFAULT_BASE: &str = "https://api.bilibili.com";

/// Extract a `BV...` id from a Bilibili video URL.
fn parse_bvid(url: &str) -> Option<String> {
    url.split('/')
        .flat_map(|seg| seg.split('?'))
        .find(|seg| seg.starts_with("BV") && seg.len() > 2)
        .map(|s| s.to_string())
}

/// `bili-cli` backend (preferred).
#[derive(Debug, Clone)]
pub struct BiliCliBackend {
    bin: String,
}

impl Default for BiliCliBackend {
    fn default() -> Self {
        Self {
            bin: "bili-cli".to_string(),
        }
    }
}

impl BiliCliBackend {
    /// Create a backend using the default `bili-cli` binary.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl Backend for BiliCliBackend {
    fn name(&self) -> &str {
        "bili-cli"
    }

    async fn probe(&self) -> ProbeResult {
        let engine = ProbeEngine::new(Duration::from_secs(5));
        let target = ProbeTarget::version(&self.bin, "Install bili-cli for Bilibili access");
        engine.probe(&target).await
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let bvid = parse_bvid(url).ok_or_else(|| {
            BackendError::Parse(self.name().to_string(), format!("no BV id in URL: {url}"))
        })?;
        let output = tokio::process::Command::new(&self.bin)
            .args(["info", bvid.as_str(), "--json"])
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
        Ok(Content {
            url: url.to_string(),
            title: Some(bvid),
            body: String::from_utf8_lossy(&output.stdout).to_string(),
            metadata: serde_json::Value::Null,
            cached: false,
        })
    }

    async fn search(
        &self,
        _query: &str,
        _opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        Err(BackendError::Other(
            self.name().to_string(),
            "search handled by the API backend".to_string(),
        ))
    }
}

/// Bilibili public web-API backend (fallback).
#[derive(Debug, Clone)]
pub struct BiliApiBackend {
    client: reqwest::Client,
    base_url: String,
    cookie: Option<String>,
}

impl Default for BiliApiBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
            // SESSDATA/bili_jct session imported via `agentspan config cookies`.
            cookie: crate::http::cookie_for("bilibili"),
        }
    }
}

impl BiliApiBackend {
    /// Create a backend pointed at the public Bilibili API.
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the base URL (tests).
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Set the session cookie header (tests / explicit config).
    pub fn with_cookie(mut self, cookie: impl Into<String>) -> Self {
        self.cookie = Some(cookie.into());
        self
    }

    /// Apply the configured cookie to a request builder.
    fn with_auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.cookie {
            Some(c) => req.header("Cookie", c),
            None => req,
        }
    }
}

#[async_trait]
impl Backend for BiliApiBackend {
    fn name(&self) -> &str {
        "bili-api"
    }

    async fn probe(&self) -> ProbeResult {
        ProbeResult::warn(
            "bili-api",
            "unauthenticated public API",
            "Some content requires login cookies",
        )
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let bvid = parse_bvid(url).ok_or_else(|| {
            BackendError::Parse(self.name().to_string(), format!("no BV id in URL: {url}"))
        })?;
        let api = format!("{}/x/web-interface/view?bvid={}", self.base_url, bvid);
        let response = self
            .with_auth(self.client.get(&api))
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
        let data = &payload["data"];
        Ok(Content {
            url: url.to_string(),
            title: data["title"].as_str().map(|s| s.to_string()),
            body: data["desc"]
                .as_str()
                .map(|s| s.to_string())
                .unwrap_or_default(),
            metadata: payload.clone(),
            cached: false,
        })
    }

    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let limit = if opts.limit == 0 { 20 } else { opts.limit };
        let api = format!(
            "{}/x/web-interface/search/type?search_type=video&keyword={}",
            self.base_url,
            crate::percent_encode(query)
        );
        let response = self
            .with_auth(self.client.get(&api))
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
        Ok(payload["data"]["result"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .take(limit)
            .map(|v| SearchResult {
                title: v["title"].as_str().unwrap_or("").to_string(),
                url: v["arcurl"].as_str().unwrap_or("").to_string(),
                snippet: v["description"]
                    .as_str()
                    .unwrap_or("")
                    .chars()
                    .take(280)
                    .collect(),
                author: v["author"].as_str().map(|s| s.to_string()),
                timestamp: v["pubdate"].as_i64().map(|n| n.to_string()),
                metadata: v.clone(),
            })
            .collect())
    }
}

/// Bilibili channel.
#[derive(Debug, Clone)]
pub struct BilibiliChannel {
    router: BackendRouter,
    api: BiliApiBackend,
}

impl BilibiliChannel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an API-only channel pointed at `base_url` (tests).
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        let api = BiliApiBackend::new().with_base_url(base_url);
        let router = BackendRouter::new(
            vec![Arc::new(api.clone()) as Arc<dyn Backend>],
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, api }
    }
}

impl Default for BilibiliChannel {
    fn default() -> Self {
        // bili-cli ▸ OpenCLI (browser session) ▸ public API, mirroring the
        // backend order Agent Reach settled on after yt-dlp was risk-controlled.
        let api = BiliApiBackend::new();
        let backends: Vec<Arc<dyn Backend>> = vec![
            Arc::new(BiliCliBackend::new()),
            Arc::new(OpenCliBackend::bilibili()),
            Arc::new(api.clone()),
        ];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        )
        .with_preferred_backend(agentspan_router::env_backend_override("bilibili"));
        Self { router, api }
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for BilibiliChannel {
    fn name(&self) -> &str {
        "bilibili"
    }

    fn description(&self) -> &str {
        "Read Bilibili video info and search videos via bili-cli or the public API"
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("bilibili.com") || url.contains("b23.tv")
    }

    fn tier(&self) -> Tier {
        Tier::One
    }

    fn backends(&self) -> Vec<Box<dyn Backend>> {
        vec![
            Box::new(BiliCliBackend::new()),
            Box::new(OpenCliBackend::bilibili()),
            Box::new(self.api.clone()),
        ]
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
    fn can_handle_bilibili_urls() {
        let ch = BilibiliChannel::new();
        assert!(ch.can_handle("https://www.bilibili.com/video/BV1xx411c7mD"));
        assert!(!ch.can_handle("https://example.com"));
    }

    #[test]
    fn channel_is_tier_one() {
        assert_eq!(BilibiliChannel::new().tier(), Tier::One);
    }

    #[test]
    fn parse_bvid_extracts_id() {
        assert_eq!(
            parse_bvid("https://www.bilibili.com/video/BV1xx411c7mD?p=1"),
            Some("BV1xx411c7mD".to_string())
        );
        assert_eq!(parse_bvid("https://www.bilibili.com/anime"), None);
    }

    #[tokio::test]
    async fn api_read_sends_cookie_header() {
        use wiremock::matchers::{header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        // Matches only when the SESSDATA cookie is sent; otherwise 404 → error.
        Mock::given(method("GET"))
            .and(path("/x/web-interface/view"))
            .and(header("Cookie", "SESSDATA=xyz"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(r#"{"data":{"title":"T","desc":"d"}}"#),
            )
            .mount(&server)
            .await;

        let backend = BiliApiBackend::new()
            .with_base_url(server.uri())
            .with_cookie("SESSDATA=xyz");
        let content = backend
            .read(
                "https://www.bilibili.com/video/BV1xx411",
                ReadOptions::default(),
            )
            .await;
        assert!(content.is_ok(), "cookie header was not sent: {content:?}");
    }

    #[test]
    fn channel_has_cli_opencli_and_api_backends() {
        let names: Vec<_> = BilibiliChannel::new()
            .backends()
            .iter()
            .map(|b| b.name().to_string())
            .collect();
        assert_eq!(names, vec!["bili-cli", "opencli-bilibili", "bili-api"]);
    }

    #[tokio::test]
    async fn api_read_fetches_view() {
        let server = MockServer::start().await;
        let body = r#"{"code":0,"data":{"title":"Test Video","desc":"a description"}}"#;
        Mock::given(method("GET"))
            .and(path("/x/web-interface/view"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = BilibiliChannel::with_base_url(server.uri());
        let content = ch
            .read(
                "https://www.bilibili.com/video/BV1xx411c7mD",
                ReadOptions::default(),
            )
            .await
            .unwrap();
        assert_eq!(content.title.as_deref(), Some("Test Video"));
        assert_eq!(content.body, "a description");
    }

    #[tokio::test]
    async fn api_search_maps_results() {
        let server = MockServer::start().await;
        let body = r#"{"code":0,"data":{"result":[{"title":"Rust Tutorial","arcurl":"https://b.tv/1","author":"teacher"}]}}"#;
        Mock::given(method("GET"))
            .and(path("/x/web-interface/search/type"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = BilibiliChannel::with_base_url(server.uri());
        let results = ch.search("rust", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust Tutorial");
        assert_eq!(results[0].author.as_deref(), Some("teacher"));
    }
}
