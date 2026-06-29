//! Channel abstraction.

use std::time::Instant;

use async_trait::async_trait;
use chrono::Utc;

use crate::backend::Backend;
use crate::error::ChannelError;
use crate::types::{BackendHealth, Content, ReadOptions, SearchOptions, SearchResult, Tier};

/// A web platform channel — read URLs, search the platform, report backend health.
#[async_trait]
pub trait Channel: Send + Sync + std::fmt::Debug {
    fn name(&self) -> &str;

    fn description(&self) -> &str;

    fn can_handle(&self, url: &str) -> bool;

    fn tier(&self) -> Tier;

    fn backends(&self) -> Vec<Box<dyn Backend>>;

    async fn read(&self, url: &str, opts: ReadOptions) -> Result<Content, ChannelError>;

    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, ChannelError>;

    /// Probe every backend and collect per-backend health with latency.
    /// Default implementation covers the common pattern; override only if
    /// a channel needs custom probe logic.
    async fn check_health(&self) -> Vec<BackendHealth> {
        let mut out = Vec::new();
        for backend in self.backends() {
            let start = Instant::now();
            let probe = backend.probe().await;
            out.push(BackendHealth {
                backend_name: backend.name().to_string(),
                probe,
                latency_ms: start.elapsed().as_millis() as u64,
                last_checked: Utc::now(),
            });
        }
        out
    }

    /// Reduce tokens in a raw API response before handing it to an LLM.
    fn format_for_llm(&self, raw: &str) -> String {
        raw.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ProbeResult;

    #[derive(Debug)]
    struct DummyBackend {
        name: &'static str,
    }

    #[async_trait]
    impl Backend for DummyBackend {
        fn name(&self) -> &str {
            self.name
        }

        async fn probe(&self) -> ProbeResult {
            ProbeResult::ok(self.name, "1.0")
        }

        async fn read(
            &self,
            _url: &str,
            _opts: ReadOptions,
        ) -> Result<Content, crate::error::BackendError> {
            unimplemented!()
        }

        async fn search(
            &self,
            _query: &str,
            _opts: SearchOptions,
        ) -> Result<Vec<SearchResult>, crate::error::BackendError> {
            unimplemented!()
        }
    }

    #[derive(Debug)]
    struct DummyChannel;

    #[async_trait]
    impl Channel for DummyChannel {
        fn name(&self) -> &str {
            "dummy"
        }

        fn description(&self) -> &str {
            "A dummy channel for tests"
        }

        fn can_handle(&self, _url: &str) -> bool {
            true
        }

        fn tier(&self) -> Tier {
            Tier::Zero
        }

        fn backends(&self) -> Vec<Box<dyn Backend>> {
            vec![Box::new(DummyBackend {
                name: "dummy-backend",
            })]
        }

        async fn read(&self, _url: &str, _opts: ReadOptions) -> Result<Content, ChannelError> {
            Ok(Content {
                url: "http://example.com".to_string(),
                title: None,
                body: "hello".to_string(),
                metadata: serde_json::Value::Null,
                cached: false,
            })
        }

        async fn search(
            &self,
            _query: &str,
            _opts: SearchOptions,
        ) -> Result<Vec<SearchResult>, ChannelError> {
            Ok(vec![])
        }
    }

    #[tokio::test]
    async fn dummy_channel_read_works() {
        let channel = DummyChannel;
        let opts = ReadOptions::default();
        let content = channel.read("http://example.com", opts).await.unwrap();
        assert_eq!(content.body, "hello");
        assert!(!content.cached);
    }

    #[test]
    fn channel_format_for_llm_default_passes_through() {
        let channel = DummyChannel;
        assert_eq!(channel.format_for_llm("raw"), "raw");
    }
}
