//! Pinterest channel — reads public pins/boards via Jina Reader.
//!
//! Pinterest has no open public API, so reads and searches go through Jina
//! Reader, which renders pages to clean Markdown. Tier 0.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

use crate::web::JinaReaderBackend;

/// Pinterest channel (Jina Reader backed).
#[derive(Debug, Clone)]
pub struct PinterestChannel {
    router: BackendRouter,
    backend: JinaReaderBackend,
}

impl PinterestChannel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a channel whose Jina backend targets `base` (tests).
    pub fn with_base_url(base: impl Into<String>) -> Self {
        Self::from_backend(JinaReaderBackend::new().with_base_url(base))
    }

    fn from_backend(backend: JinaReaderBackend) -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for PinterestChannel {
    fn default() -> Self {
        Self::from_backend(JinaReaderBackend::new())
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for PinterestChannel {
    fn name(&self) -> &str {
        "pinterest"
    }

    fn description(&self) -> &str {
        "Read Pinterest pins and boards via Jina Reader (no API)"
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("pinterest.com") || url.contains("pin.it")
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
        _opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, ChannelError> {
        let search_url = format!(
            "https://www.pinterest.com/search/pins/?q={}",
            crate::percent_encode(query)
        );
        let content = self
            .backend
            .read(&search_url, ReadOptions::default())
            .await
            .map_err(|e: BackendError| ChannelError::BackendUnavailable(e.to_string()))?;
        Ok(vec![SearchResult {
            title: format!("Pinterest results for \"{query}\""),
            url: search_url,
            snippet: content.body.trim().chars().take(800).collect(),
            author: None,
            timestamp: None,
            metadata: serde_json::Value::Null,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentspan_core::channel::Channel;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn can_handle_pinterest_urls() {
        let ch = PinterestChannel::new();
        assert!(ch.can_handle("https://www.pinterest.com/pin/12345"));
        assert!(ch.can_handle("https://pin.it/abc"));
        assert!(!ch.can_handle("https://example.com"));
    }

    #[test]
    fn channel_is_tier_zero() {
        assert_eq!(PinterestChannel::new().tier(), Tier::Zero);
    }

    #[test]
    fn backend_is_jina() {
        let names: Vec<_> = PinterestChannel::new()
            .backends()
            .iter()
            .map(|b| b.name().to_string())
            .collect();
        assert_eq!(names, vec!["jina-reader"]);
    }

    #[tokio::test]
    async fn read_returns_page_text() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("# Pin\nA nice recipe."))
            .mount(&server)
            .await;

        let ch = PinterestChannel::with_base_url(format!("{}/", server.uri()));
        let content = ch
            .read("https://www.pinterest.com/pin/1", ReadOptions::default())
            .await
            .unwrap();
        assert!(content.body.contains("A nice recipe."));
    }

    #[tokio::test]
    async fn search_wraps_results_page() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("pin one\npin two"))
            .mount(&server)
            .await;

        let ch = PinterestChannel::with_base_url(format!("{}/", server.uri()));
        let results = ch
            .search("recipes", SearchOptions::default())
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].snippet.contains("pin one"));
    }
}
