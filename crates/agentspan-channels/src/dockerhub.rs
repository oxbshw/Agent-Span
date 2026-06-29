//! Docker Hub channel — image repository metadata and search.
//!
//! Public `hub.docker.com` v2 API, no auth. Official images live under the
//! `library` namespace, which `parse_repo` fills in for bare/`_` names.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

const DEFAULT_BASE: &str = "https://hub.docker.com";

/// Normalise an image reference to `namespace/repo`.
///
/// Handles `hub.docker.com/r/ns/repo`, official `hub.docker.com/_/repo`, and
/// bare `ns/repo` or `repo` (the latter resolving to `library/repo`).
fn parse_repo(input: &str) -> Option<String> {
    let raw = if let Some(after) = input.split("hub.docker.com/").nth(1) {
        after
            .split(['?', '#'])
            .next()
            .unwrap_or(after)
            .trim_matches('/')
            .to_string()
    } else if input.contains("://") {
        return None;
    } else {
        input.trim().to_string()
    };
    if raw.is_empty() {
        return None;
    }
    let repo = if let Some(rest) = raw.strip_prefix("r/") {
        rest.to_string()
    } else if let Some(rest) = raw.strip_prefix("_/") {
        format!("library/{rest}")
    } else if raw.contains('/') {
        raw
    } else {
        format!("library/{raw}")
    };
    repo.contains('/').then_some(repo)
}

/// Docker Hub HTTP backend.
#[derive(Debug, Clone)]
pub struct DockerHubBackend {
    client: reqwest::Client,
    base_url: String,
}

impl Default for DockerHubBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
        }
    }
}

impl DockerHubBackend {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    async fn get_json(&self, url: &str) -> Result<serde_json::Value, BackendError> {
        let response = self
            .client
            .get(url)
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
impl Backend for DockerHubBackend {
    fn name(&self) -> &str {
        "dockerhub"
    }

    async fn probe(&self) -> ProbeResult {
        ProbeResult::ok("dockerhub", "v2")
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let repo = parse_repo(url).ok_or_else(|| {
            BackendError::Parse(
                self.name().to_string(),
                format!("not a Docker image: {url}"),
            )
        })?;
        let payload = self
            .get_json(&format!("{}/v2/repositories/{}/", self.base_url, repo))
            .await?;
        let description = payload["description"].as_str().unwrap_or("");
        let stars = payload["star_count"].as_u64().unwrap_or(0);
        let pulls = payload["pull_count"].as_u64().unwrap_or(0);
        Ok(Content {
            url: url.to_string(),
            title: Some(repo.clone()),
            body: format!("{repo}\n{description}\n⭐ {stars}  ⬇ {pulls}")
                .trim()
                .to_string(),
            metadata: payload,
            cached: false,
        })
    }

    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let page_size = if opts.limit == 0 {
            10
        } else {
            opts.limit.min(50)
        };
        let url = format!(
            "{}/v2/search/repositories/?query={}&page_size={}",
            self.base_url,
            crate::percent_encode(query),
            page_size
        );
        let payload = self.get_json(&url).await?;
        let results = payload["results"].as_array().cloned().unwrap_or_default();
        Ok(results
            .into_iter()
            .map(|r| {
                let name = r["repo_name"].as_str().unwrap_or("").to_string();
                SearchResult {
                    url: format!("https://hub.docker.com/r/{name}"),
                    snippet: r["short_description"].as_str().unwrap_or("").to_string(),
                    author: None,
                    timestamp: None,
                    title: name,
                    metadata: r,
                }
            })
            .collect())
    }
}

/// Docker Hub channel.
#[derive(Debug, Clone)]
pub struct DockerHubChannel {
    router: BackendRouter,
    backend: DockerHubBackend,
}

impl DockerHubChannel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self::from_backend(DockerHubBackend::new().with_base_url(base_url))
    }

    fn from_backend(backend: DockerHubBackend) -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for DockerHubChannel {
    fn default() -> Self {
        Self::from_backend(DockerHubBackend::new())
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for DockerHubChannel {
    fn name(&self) -> &str {
        "dockerhub"
    }

    fn description(&self) -> &str {
        "Look up Docker image metadata and search Docker Hub"
    }

    fn format_for_llm(&self, raw: &str) -> String {
        crate::format::extract_text_fields(raw, &["name", "description", "short_description"], 6000)
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("hub.docker.com/")
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
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn parse_repo_normalises_references() {
        assert_eq!(
            parse_repo("https://hub.docker.com/_/nginx"),
            Some("library/nginx".to_string())
        );
        assert_eq!(
            parse_repo("https://hub.docker.com/r/bitnami/redis"),
            Some("bitnami/redis".to_string())
        );
        assert_eq!(parse_repo("postgres"), Some("library/postgres".to_string()));
        assert_eq!(
            parse_repo("grafana/grafana"),
            Some("grafana/grafana".to_string())
        );
        assert_eq!(parse_repo("https://example.com"), None);
    }

    #[test]
    fn can_handle_and_metadata() {
        let ch = DockerHubChannel::new();
        assert!(ch.can_handle("https://hub.docker.com/_/nginx"));
        assert!(!ch.can_handle("https://crates.io/crates/x"));
        assert_eq!(ch.name(), "dockerhub");
        assert_eq!(ch.tier(), Tier::Zero);
    }

    #[tokio::test]
    async fn read_returns_repo_metadata() {
        let server = MockServer::start().await;
        let body = r#"{"name":"nginx","namespace":"library","description":"official nginx","star_count":9000,"pull_count":1000000000}"#;
        Mock::given(method("GET"))
            .and(path("/v2/repositories/library/nginx/"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = DockerHubChannel::with_base_url(server.uri());
        let content = ch
            .read("https://hub.docker.com/_/nginx", ReadOptions::default())
            .await
            .unwrap();
        assert_eq!(content.title.as_deref(), Some("library/nginx"));
        assert!(content.body.contains("official nginx"));
    }

    #[tokio::test]
    async fn search_maps_results() {
        let server = MockServer::start().await;
        let body =
            r#"{"results":[{"repo_name":"grafana/grafana","short_description":"dashboards"}]}"#;
        Mock::given(method("GET"))
            .and(path("/v2/search/repositories/"))
            .and(query_param("query", "grafana"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = DockerHubChannel::with_base_url(server.uri());
        let results = ch
            .search("grafana", SearchOptions::default())
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "grafana/grafana");
    }
}
