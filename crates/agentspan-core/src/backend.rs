//! Backend adapter abstraction.

use async_trait::async_trait;

use crate::error::BackendError;
use crate::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult};

/// A backend adapter — CLI tool, HTTP API, or browser session behind a channel.
#[async_trait]
pub trait Backend: Send + Sync + std::fmt::Debug {
    /// Human-readable backend name.
    fn name(&self) -> &str;

    /// Probe this backend's health.
    async fn probe(&self) -> ProbeResult;

    /// Read content via this backend.
    async fn read(&self, url: &str, opts: ReadOptions) -> Result<Content, BackendError>;

    /// Search via this backend.
    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ProbeResult, ProbeStatus};

    #[derive(Debug)]
    struct AlwaysOkBackend;

    #[async_trait]
    impl Backend for AlwaysOkBackend {
        fn name(&self) -> &str {
            "always-ok"
        }

        async fn probe(&self) -> ProbeResult {
            ProbeResult {
                status: ProbeStatus::Ok,
                message: "ok".to_string(),
                version: Some("1.0".to_string()),
                hint: None,
            }
        }

        async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
            Ok(Content {
                url: url.to_string(),
                title: None,
                body: "ok".to_string(),
                metadata: serde_json::Value::Null,
                cached: false,
            })
        }

        async fn search(
            &self,
            _query: &str,
            _opts: SearchOptions,
        ) -> Result<Vec<SearchResult>, BackendError> {
            Ok(vec![])
        }
    }

    #[tokio::test]
    async fn backend_probe_returns_ok() {
        let backend = AlwaysOkBackend;
        let result = backend.probe().await;
        assert_eq!(result.status, ProbeStatus::Ok);
    }
}
