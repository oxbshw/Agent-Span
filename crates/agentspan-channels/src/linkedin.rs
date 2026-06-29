//! LinkedIn channel — OpenCLI (logged-in session) for full access, Jina Reader
//! for public pages as a zero-config fallback.

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
use crate::web::JinaReaderBackend;

/// LinkedIn channel.
#[derive(Debug, Clone)]
pub struct LinkedInChannel {
    router: BackendRouter,
}

impl LinkedInChannel {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for LinkedInChannel {
    fn default() -> Self {
        // OpenCLI (full, logged-in) preferred; Jina Reader reads public pages.
        let backends: Vec<Arc<dyn Backend>> = vec![
            Arc::new(OpenCliBackend::linkedin()),
            Arc::new(JinaReaderBackend::new()),
        ];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router }
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for LinkedInChannel {
    fn name(&self) -> &str {
        "linkedin"
    }

    fn description(&self) -> &str {
        "Read LinkedIn profiles/company pages via OpenCLI, with Jina Reader for public pages"
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("linkedin.com")
    }

    fn tier(&self) -> Tier {
        Tier::One
    }

    fn backends(&self) -> Vec<Box<dyn Backend>> {
        vec![
            Box::new(OpenCliBackend::linkedin()),
            Box::new(JinaReaderBackend::new()),
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

    #[test]
    fn can_handle_linkedin_urls() {
        let ch = LinkedInChannel::new();
        assert!(ch.can_handle("https://www.linkedin.com/in/someone"));
        assert!(!ch.can_handle("https://example.com"));
    }

    #[test]
    fn channel_is_tier_one() {
        assert_eq!(LinkedInChannel::new().tier(), Tier::One);
    }

    #[test]
    fn backends_are_opencli_then_jina() {
        let names: Vec<_> = LinkedInChannel::new()
            .backends()
            .iter()
            .map(|b| b.name().to_string())
            .collect();
        assert_eq!(names, vec!["opencli-linkedin", "jina-reader"]);
    }
}
