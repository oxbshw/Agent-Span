//! Official Rust SDK for the AgentSpan gateway.
//!
//! A thin async client over the AgentSpan REST API (see `docs/api-reference.md`).
//!
//! ```no_run
//! # async fn demo() -> Result<(), agentspan_sdk::Error> {
//! use agentspan_sdk::AgentSpanClient;
//! let client = AgentSpanClient::new("http://localhost:8080");
//! let content = client.read("https://example.com", false).await?;
//! println!("{}", content.body);
//! # Ok(()) }
//! ```

use serde::Deserialize;
use serde_json::Value;

/// Content returned by read operations.
#[derive(Debug, Clone, Deserialize)]
pub struct Content {
    pub url: String,
    pub title: Option<String>,
    pub body: String,
    #[serde(default)]
    pub metadata: Value,
    #[serde(default)]
    pub cached: bool,
}

/// A search result.
#[derive(Debug, Clone, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub author: Option<String>,
    pub timestamp: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}

/// Channel metadata.
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelInfo {
    pub name: String,
    pub description: String,
    pub tier: String,
}

/// Errors returned by the SDK.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("authentication failed: {0}")]
    Auth(String),
    #[error("rate limited (retry after {retry_after:?}s)")]
    RateLimited { retry_after: Option<u64> },
    #[error("API error {status}: {message}")]
    Api { status: u16, message: String },
    #[error("channel error: {0}")]
    Channel(String),
    #[error("transport error: {0}")]
    Transport(String),
}

/// Async client for the AgentSpan gateway.
#[derive(Debug, Clone)]
pub struct AgentSpanClient {
    base_url: String,
    api_key: Option<String>,
    http: reqwest::Client,
}

impl AgentSpanClient {
    /// Create a client pointed at `base_url` (e.g. `http://localhost:8080`).
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: None,
            http: reqwest::Client::new(),
        }
    }

    /// Set the API key sent as `X-API-Key`.
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let mut req = self
            .http
            .request(method, format!("{}{path}", self.base_url));
        if let Some(key) = &self.api_key {
            req = req.header("X-API-Key", key);
        }
        req
    }

    async fn check(resp: reqwest::Response) -> Result<reqwest::Response, Error> {
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        let retry_after = resp
            .headers()
            .get("Retry-After")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse().ok());
        let message = resp
            .json::<Value>()
            .await
            .ok()
            .and_then(|v| v["error"].as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| status.to_string());
        Err(match status.as_u16() {
            401 => Error::Auth(message),
            429 => Error::RateLimited { retry_after },
            code => Error::Api {
                status: code,
                message,
            },
        })
    }

    async fn json(resp: reqwest::Response) -> Result<Value, Error> {
        resp.json::<Value>()
            .await
            .map_err(|e| Error::Transport(e.to_string()))
    }

    /// Read a URL via the best matching channel.
    pub async fn read(&self, url: &str, force_refresh: bool) -> Result<Content, Error> {
        let resp = self
            .request(reqwest::Method::GET, "/api/v1/read")
            .query(&[("url", url), ("force_refresh", &force_refresh.to_string())])
            .send()
            .await
            .map_err(|e| Error::Transport(e.to_string()))?;
        let data = Self::json(Self::check(resp).await?).await?;
        if let Some(err) = data["error"].as_str() {
            return Err(Error::Channel(err.to_string()));
        }
        serde_json::from_value(data["content"].clone()).map_err(|e| Error::Transport(e.to_string()))
    }

    /// Search a platform via a named channel.
    pub async fn search(
        &self,
        channel: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>, Error> {
        let path = format!("/api/v1/channels/{channel}/search");
        let resp = self
            .request(reqwest::Method::GET, &path)
            .query(&[("q", query), ("limit", &limit.to_string())])
            .send()
            .await
            .map_err(|e| Error::Transport(e.to_string()))?;
        let data = Self::json(Self::check(resp).await?).await?;
        if let Some(err) = data["error"].as_str() {
            return Err(Error::Channel(err.to_string()));
        }
        serde_json::from_value(data["results"].clone()).map_err(|e| Error::Transport(e.to_string()))
    }

    /// List available channels.
    pub async fn list_channels(&self) -> Result<Vec<ChannelInfo>, Error> {
        let resp = self
            .request(reqwest::Method::GET, "/api/v1/channels")
            .send()
            .await
            .map_err(|e| Error::Transport(e.to_string()))?;
        let data = Self::json(Self::check(resp).await?).await?;
        serde_json::from_value(data["channels"].clone())
            .map_err(|e| Error::Transport(e.to_string()))
    }

    /// Run health diagnostics across all channels.
    pub async fn doctor(&self) -> Result<Value, Error> {
        let resp = self
            .request(reqwest::Method::GET, "/api/v1/doctor")
            .send()
            .await
            .map_err(|e| Error::Transport(e.to_string()))?;
        Self::json(Self::check(resp).await?).await
    }

    /// Fetch the non-secret server configuration view.
    pub async fn get_config(&self) -> Result<Value, Error> {
        let resp = self
            .request(reqwest::Method::GET, "/api/v1/config")
            .send()
            .await
            .map_err(|e| Error::Transport(e.to_string()))?;
        Self::json(Self::check(resp).await?).await
    }

    /// Read many URLs in parallel (server-side batch).
    pub async fn batch_read(
        &self,
        urls: &[String],
        force_refresh: bool,
    ) -> Result<Vec<Value>, Error> {
        let resp = self
            .request(reqwest::Method::POST, "/api/v1/batch/read")
            .json(&serde_json::json!({ "urls": urls, "force_refresh": force_refresh }))
            .send()
            .await
            .map_err(|e| Error::Transport(e.to_string()))?;
        let data = Self::json(Self::check(resp).await?).await?;
        Ok(data["results"].as_array().cloned().unwrap_or_default())
    }

    /// Run many queries against one channel in parallel (server-side batch).
    pub async fn batch_search(
        &self,
        channel: &str,
        queries: &[String],
        limit: usize,
    ) -> Result<Vec<Value>, Error> {
        let resp = self
            .request(reqwest::Method::POST, "/api/v1/batch/search")
            .json(&serde_json::json!({ "channel": channel, "queries": queries, "limit": limit }))
            .send()
            .await
            .map_err(|e| Error::Transport(e.to_string()))?;
        let data = Self::json(Self::check(resp).await?).await?;
        Ok(data["results"].as_array().cloned().unwrap_or_default())
    }

    /// Return `true` when the server's `/health` endpoint is OK.
    pub async fn health(&self) -> bool {
        self.request(reqwest::Method::GET, "/health")
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn read_returns_content() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/read"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"channel":"web","content":{"url":"https://x","title":"T","body":"hi","metadata":null,"cached":false}}"#,
            ))
            .mount(&server)
            .await;
        let client = AgentSpanClient::new(server.uri());
        let content = client.read("https://x", false).await.unwrap();
        assert_eq!(content.body, "hi");
        assert_eq!(content.title.as_deref(), Some("T"));
    }

    #[tokio::test]
    async fn read_channel_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/read"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"error":"no channel"}"#))
            .mount(&server)
            .await;
        let client = AgentSpanClient::new(server.uri());
        assert!(matches!(
            client.read("ftp://x", false).await,
            Err(Error::Channel(_))
        ));
    }

    #[tokio::test]
    async fn search_maps_results() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/channels/hackernews/search"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"results":[{"title":"Rust","url":"https://r","snippet":"s","author":null,"timestamp":null,"metadata":null}]}"#,
            ))
            .mount(&server)
            .await;
        let client = AgentSpanClient::new(server.uri());
        let results = client.search("hackernews", "rust", 10).await.unwrap();
        assert_eq!(results[0].title, "Rust");
    }

    #[tokio::test]
    async fn auth_error_on_401() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/channels"))
            .respond_with(ResponseTemplate::new(401).set_body_string(r#"{"error":"bad key"}"#))
            .mount(&server)
            .await;
        let client = AgentSpanClient::new(server.uri());
        assert!(matches!(client.list_channels().await, Err(Error::Auth(_))));
    }

    #[tokio::test]
    async fn rate_limited_carries_retry_after() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/read"))
            .respond_with(
                ResponseTemplate::new(429)
                    .insert_header("Retry-After", "12")
                    .set_body_string(r#"{"error":"slow"}"#),
            )
            .mount(&server)
            .await;
        let client = AgentSpanClient::new(server.uri());
        match client.read("https://x", false).await {
            Err(Error::RateLimited { retry_after }) => assert_eq!(retry_after, Some(12)),
            other => panic!("expected rate limit, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn batch_read_returns_results() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/batch/read"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"count":2,"results":[{"url":"a","ok":true},{"url":"b","ok":false}]}"#,
            ))
            .mount(&server)
            .await;
        let client = AgentSpanClient::new(server.uri());
        let results = client
            .batch_read(&["a".to_string(), "b".to_string()], false)
            .await
            .unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn health_true_on_200() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"status":"ok"}"#))
            .mount(&server)
            .await;
        let client = AgentSpanClient::new(server.uri());
        assert!(client.health().await);
    }
}
