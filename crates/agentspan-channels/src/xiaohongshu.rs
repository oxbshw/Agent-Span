//! XiaoHongShu (小红书) channel — login-gated, served via OpenCLI's browser session.
//!
//! XiaoHongShu has no anonymous path, so OpenCLI (which reuses the user's logged-in
//! Chrome) is the backend. On servers, run `xiaohongshu-mcp` and point OpenCLI/mcporter
//! at it (see docs); that path is configured operationally rather than in code here.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::ChannelError;
use agentspan_core::types::{Content, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

use crate::opencli::OpenCliBackend;

/// XiaoHongShu channel.
#[derive(Debug, Clone)]
pub struct XiaohongshuChannel {
    router: BackendRouter,
}

impl XiaohongshuChannel {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for XiaohongshuChannel {
    fn default() -> Self {
        let router = BackendRouter::new(
            vec![Arc::new(OpenCliBackend::xiaohongshu()) as Arc<dyn Backend>],
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router }
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for XiaohongshuChannel {
    fn name(&self) -> &str {
        "xiaohongshu"
    }

    fn description(&self) -> &str {
        "Read and search XiaoHongShu notes via OpenCLI's logged-in browser session"
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("xiaohongshu.com") || url.contains("xhslink.com")
    }

    fn tier(&self) -> Tier {
        Tier::One
    }

    fn backends(&self) -> Vec<Box<dyn Backend>> {
        vec![Box::new(OpenCliBackend::xiaohongshu())]
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

    #[test]
    fn can_handle_xhs_urls() {
        let ch = XiaohongshuChannel::new();
        assert!(ch.can_handle("https://www.xiaohongshu.com/explore/abc"));
        assert!(ch.can_handle("https://xhslink.com/abc"));
        assert!(!ch.can_handle("https://example.com"));
    }

    #[test]
    fn channel_is_tier_one() {
        assert_eq!(XiaohongshuChannel::new().tier(), Tier::One);
    }

    #[test]
    fn backend_is_opencli() {
        let names: Vec<_> = XiaohongshuChannel::new()
            .backends()
            .iter()
            .map(|b| b.name().to_string())
            .collect();
        assert_eq!(names, vec!["opencli-xiaohongshu"]);
    }

    #[tokio::test]
    async fn check_health_reports_backend() {
        let health = XiaohongshuChannel::new().check_health().await;
        assert_eq!(health.len(), 1);
        assert_eq!(health[0].backend_name, "opencli-xiaohongshu");
    }
}
