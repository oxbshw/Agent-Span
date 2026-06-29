//! GitLab channel — project metadata and search via the GitLab v4 API.
//!
//! Public projects need no token. The project read uses GitLab's
//! URL-encoded-path lookup (`group/project` -> `group%2Fproject`).

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

const DEFAULT_BASE: &str = "https://gitlab.com/api/v4";

/// Extract a `group/project` path from a gitlab.com project URL.
///
/// Reserved prefixes (`api/`, `-/`, `dashboard`, `users`, `explore`) are
/// rejected so we don't treat GitLab's own routes as projects.
fn parse_project(url: &str) -> Option<String> {
    let after = url.split("gitlab.com/").nth(1)?;
    let path = after
        .split(['?', '#'])
        .next()
        .unwrap_or(after)
        .trim_matches('/');
    if path.is_empty() || !path.contains('/') {
        return None;
    }
    const RESERVED: [&str; 5] = ["api/", "-/", "dashboard", "users/", "explore"];
    if RESERVED.iter().any(|p| path.starts_with(p)) {
        return None;
    }
    Some(path.to_string())
}

/// GitLab v4 API backend.
#[derive(Debug, Clone)]
pub struct GitLabBackend {
    client: reqwest::Client,
    base_url: String,
}

impl Default for GitLabBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
        }
    }
}

impl GitLabBackend {
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
impl Backend for GitLabBackend {
    fn name(&self) -> &str {
        "gitlab-api"
    }

    async fn probe(&self) -> ProbeResult {
        ProbeResult::ok("gitlab-api", "v4")
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let project = parse_project(url).ok_or_else(|| {
            BackendError::Parse(
                self.name().to_string(),
                format!("not a GitLab project: {url}"),
            )
        })?;
        let api = format!(
            "{}/projects/{}",
            self.base_url,
            crate::percent_encode(&project)
        );
        let payload = self.get_json(&api).await?;
        let name = payload["name"].as_str().unwrap_or(&project);
        let description = payload["description"].as_str().unwrap_or("");
        let stars = payload["star_count"].as_u64().unwrap_or(0);
        Ok(Content {
            url: url.to_string(),
            title: Some(name.to_string()),
            body: format!("{name}\n{description}\n⭐ {stars}")
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
        let per_page = if opts.limit == 0 {
            10
        } else {
            opts.limit.min(50)
        };
        let api = format!(
            "{}/projects?search={}&per_page={}&order_by=star_count",
            self.base_url,
            crate::percent_encode(query),
            per_page
        );
        let payload = self.get_json(&api).await?;
        let projects = payload.as_array().cloned().unwrap_or_default();
        Ok(projects
            .into_iter()
            .map(|p| SearchResult {
                url: p["web_url"].as_str().unwrap_or("").to_string(),
                snippet: p["description"].as_str().unwrap_or("").to_string(),
                author: p["namespace"]["name"].as_str().map(|s| s.to_string()),
                timestamp: p["last_activity_at"].as_str().map(|s| s.to_string()),
                title: p["name_with_namespace"]
                    .as_str()
                    .or_else(|| p["name"].as_str())
                    .unwrap_or("")
                    .to_string(),
                metadata: p,
            })
            .collect())
    }
}

/// GitLab channel.
#[derive(Debug, Clone)]
pub struct GitLabChannel {
    router: BackendRouter,
    backend: GitLabBackend,
}

impl GitLabChannel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self::from_backend(GitLabBackend::new().with_base_url(base_url))
    }

    fn from_backend(backend: GitLabBackend) -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for GitLabChannel {
    fn default() -> Self {
        Self::from_backend(GitLabBackend::new())
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for GitLabChannel {
    fn name(&self) -> &str {
        "gitlab"
    }

    fn description(&self) -> &str {
        "Look up GitLab project metadata and search public projects"
    }

    fn format_for_llm(&self, raw: &str) -> String {
        crate::format::extract_text_fields(raw, &["name", "description", "web_url"], 6000)
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("gitlab.com/") && !url.contains("gitlab.com/api/")
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
    use wiremock::matchers::{method, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn parse_project_extracts_path() {
        assert_eq!(
            parse_project("https://gitlab.com/gitlab-org/gitlab"),
            Some("gitlab-org/gitlab".to_string())
        );
        assert_eq!(parse_project("https://gitlab.com/explore"), None);
        assert_eq!(parse_project("https://gitlab.com/api/v4/projects"), None);
        assert_eq!(parse_project("https://example.com/a/b"), None);
    }

    #[test]
    fn can_handle_gitlab_projects_not_api() {
        let ch = GitLabChannel::new();
        assert!(ch.can_handle("https://gitlab.com/gitlab-org/gitlab"));
        assert!(!ch.can_handle("https://gitlab.com/api/v4/x"));
        assert_eq!(ch.tier(), Tier::Zero);
    }

    #[tokio::test]
    async fn read_returns_project_metadata() {
        let server = MockServer::start().await;
        let body = r#"{"name":"GitLab","description":"DevOps platform","star_count":1234}"#;
        // Match on method only — the encoded %2F path varies by HTTP stack.
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = GitLabChannel::with_base_url(server.uri());
        let content = ch
            .read(
                "https://gitlab.com/gitlab-org/gitlab",
                ReadOptions::default(),
            )
            .await
            .unwrap();
        assert_eq!(content.title.as_deref(), Some("GitLab"));
        assert!(content.body.contains("DevOps platform"));
    }

    #[tokio::test]
    async fn search_maps_projects() {
        let server = MockServer::start().await;
        let body = r#"[{"name":"awesome","name_with_namespace":"grp / awesome","description":"cool","web_url":"https://gitlab.com/grp/awesome"}]"#;
        Mock::given(method("GET"))
            .and(query_param("search", "awesome"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = GitLabChannel::with_base_url(server.uri());
        let results = ch
            .search("awesome", SearchOptions::default())
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "grp / awesome");
        assert!(results[0].url.ends_with("/grp/awesome"));
    }
}
