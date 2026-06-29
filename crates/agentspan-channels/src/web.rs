//! Web channel — read any URL via Jina Reader, with direct HTTP fallback.

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{
    BackendHealth, Content, ProbeResult, ProbeStatus, ReadOptions, SearchOptions, SearchResult,
    Tier,
};
use agentspan_probe::ProbeEngine;
use agentspan_router::BackendRouter;

const JINA_BASE: &str = "https://r.jina.ai/";

/// Attach content-intelligence (type + key facts) to a fetched page as JSON
/// metadata. Best-effort: falls back to `null` if serialization ever fails.
fn analysis_metadata(body: &str, url: &str) -> serde_json::Value {
    serde_json::to_value(crate::intelligence::analyze(body, Some(url)))
        .unwrap_or(serde_json::Value::Null)
}

/// Jina Reader backend.
#[derive(Debug, Clone)]
pub struct JinaReaderBackend {
    client: reqwest::Client,
    base: String,
}

impl Default for JinaReaderBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base: JINA_BASE.to_string(),
        }
    }
}

impl JinaReaderBackend {
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the Jina Reader base URL (used by tests and proxied deployments).
    pub fn with_base_url(mut self, base: impl Into<String>) -> Self {
        self.base = base.into();
        self
    }

    /// Convert a URL to a Jina Reader URL.
    fn jina_url(&self, url: &str) -> String {
        if url.starts_with("http://") || url.starts_with("https://") {
            format!("{}{}", self.base, url)
        } else {
            format!("{}http://{}", self.base, url)
        }
    }
}

#[async_trait]
impl Backend for JinaReaderBackend {
    fn name(&self) -> &str {
        "jina-reader"
    }

    async fn probe(&self) -> ProbeResult {
        // Jina Reader is a remote HTTP service; assume available (Tier 0).
        ProbeResult {
            status: ProbeStatus::Ok,
            message: "Jina Reader HTTP service is reachable".to_string(),
            version: None,
            hint: None,
        }
    }

    async fn read(&self, url: &str, opts: ReadOptions) -> Result<Content, BackendError> {
        let jina = self.jina_url(url);
        let mut request = self.client.get(&jina);
        if opts.force_refresh {
            request = request.header("Cache-Control", "no-cache");
        }
        let response = request
            .send()
            .await
            .map_err(|e| BackendError::RequestFailed(self.name().to_string(), e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(BackendError::RequestFailed(
                self.name().to_string(),
                format!("HTTP {}: {}", status, body),
            ));
        }

        let body = response
            .text()
            .await
            .map_err(|e| BackendError::Parse(self.name().to_string(), e.to_string()))?;

        let metadata = analysis_metadata(&body, url);
        Ok(Content {
            url: url.to_string(),
            title: None,
            body,
            metadata,
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
            "search not supported".to_string(),
        ))
    }
}

/// Direct HTTP fallback backend — fetches the raw page when Jina Reader fails.
#[derive(Debug, Clone)]
pub struct DirectHttpBackend {
    client: reqwest::Client,
    // Remembers ETag/Last-Modified per URL so repeat reads can revalidate with a
    // cheap 304 instead of re-downloading. Shared across clones (Arc inside).
    store: crate::http::ValidatorStore,
}

impl Default for DirectHttpBackend {
    fn default() -> Self {
        Self {
            // default_client() applies the configured proxy; Client::default()
            // would not, so don't derive Default here.
            client: crate::http::default_client(),
            store: crate::http::ValidatorStore::new(),
        }
    }
}

impl DirectHttpBackend {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl Backend for DirectHttpBackend {
    fn name(&self) -> &str {
        "direct-http"
    }

    async fn probe(&self) -> ProbeResult {
        ProbeResult::ok("direct-http", "1.0")
    }

    async fn read(&self, url: &str, opts: ReadOptions) -> Result<Content, BackendError> {
        use crate::http::ConditionalFetch;

        // Revalidate against any stored ETag/Last-Modified; force_refresh sends a
        // full fetch. A 304 reuses the previously fetched body.
        let fetch =
            crate::http::conditional_get(&self.client, url, &self.store, opts.force_refresh)
                .await
                .map_err(|e| BackendError::RequestFailed(self.name().to_string(), e.to_string()))?;

        let body = match fetch {
            ConditionalFetch::NotModified(body) => body,
            ConditionalFetch::Fetched { status, body } => {
                if !status.is_success() {
                    return Err(BackendError::RequestFailed(
                        self.name().to_string(),
                        format!("HTTP {}: {}", status, body),
                    ));
                }
                body
            }
        };

        let metadata = analysis_metadata(&body, url);
        Ok(Content {
            url: url.to_string(),
            title: None,
            body,
            metadata,
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
            "search not supported".to_string(),
        ))
    }
}

/// Web channel.
#[derive(Debug, Clone)]
pub struct WebChannel {
    router: BackendRouter,
}

impl WebChannel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a web channel with a custom backend router.
    pub fn with_router(router: BackendRouter) -> Self {
        Self { router }
    }
}

impl Default for WebChannel {
    fn default() -> Self {
        let backends: Vec<std::sync::Arc<dyn Backend>> = vec![
            std::sync::Arc::new(JinaReaderBackend::new()),
            std::sync::Arc::new(DirectHttpBackend::new()),
        ];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(std::time::Duration::from_secs(5)),
            agentspan_router::retry::RetryConfig::default(),
        );
        Self { router }
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for WebChannel {
    fn name(&self) -> &str {
        "web"
    }

    fn description(&self) -> &str {
        "Read any webpage as clean Markdown text via Jina Reader, with direct HTTP fallback"
    }

    fn can_handle(&self, url: &str) -> bool {
        url.starts_with("http://") || url.starts_with("https://")
    }

    fn tier(&self) -> Tier {
        Tier::Zero
    }

    fn backends(&self) -> Vec<Box<dyn Backend>> {
        vec![
            Box::new(JinaReaderBackend::new()),
            Box::new(DirectHttpBackend::new()),
        ]
    }

    async fn read(&self, url: &str, opts: ReadOptions) -> Result<Content, ChannelError> {
        self.router.read(url, opts).await
    }

    async fn search(
        &self,
        _query: &str,
        _opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, ChannelError> {
        Err(ChannelError::Other(
            "web search is not supported; use a search-specific channel".to_string(),
        ))
    }

    async fn check_health(&self) -> Vec<BackendHealth> {
        self.router.check_health().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentspan_core::channel::Channel;

    #[test]
    fn web_channel_can_handle_http_urls() {
        let channel = WebChannel::new();
        assert!(channel.can_handle("https://example.com"));
        assert!(channel.can_handle("http://example.com"));
        assert!(!channel.can_handle("ftp://example.com"));
    }

    #[test]
    fn web_channel_is_tier_zero() {
        let channel = WebChannel::new();
        assert_eq!(channel.tier(), Tier::Zero);
    }

    #[test]
    fn web_channel_has_jina_and_direct_backends() {
        let channel = WebChannel::new();
        let backends = channel.backends();
        let names: Vec<_> = backends.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["jina-reader", "direct-http"]);
    }

    #[test]
    fn jina_url_conversion() {
        let backend = JinaReaderBackend::new();
        assert_eq!(
            backend.jina_url("https://example.com"),
            "https://r.jina.ai/https://example.com"
        );
        assert_eq!(
            backend.jina_url("http://example.com"),
            "https://r.jina.ai/http://example.com"
        );
    }

    #[test]
    fn direct_http_backend_probe_is_ok() {
        let backend = DirectHttpBackend::new();
        assert_eq!(backend.name(), "direct-http");
    }

    #[tokio::test]
    async fn direct_http_backend_reads_markdown_from_mock_server() {
        use wiremock::{matchers::method, Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        let body = "# Hello World\n\nThis is markdown.";
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&mock_server)
            .await;

        let backend = DirectHttpBackend::new();
        let content = backend
            .read(&mock_server.uri(), ReadOptions::default())
            .await
            .unwrap();

        assert_eq!(content.body, body);
        assert_eq!(content.url, mock_server.uri());
    }

    #[tokio::test]
    async fn direct_http_backend_returns_error_on_http_failure() {
        use wiremock::{matchers::method, Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(500).set_body_string("server error"))
            .mount(&mock_server)
            .await;

        let backend = DirectHttpBackend::new();
        let result = backend
            .read(&mock_server.uri(), ReadOptions::default())
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("500"));
    }
}
