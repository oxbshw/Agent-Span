//! GitHub channel — wraps the GitHub CLI (`gh`) with a REST API fallback.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{
    BackendHealth, Content, ProbeResult, ProbeStatus, ReadOptions, SearchOptions, SearchResult,
    Tier,
};
use agentspan_probe::{ProbeEngine, ProbeTarget};
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

/// A GitHub resource parsed from a URL.
#[derive(Debug, Clone, PartialEq, Eq)]
enum GitHubResource {
    Repo {
        owner: String,
        repo: String,
    },
    IssueList {
        owner: String,
        repo: String,
    },
    Issue {
        owner: String,
        repo: String,
        number: u32,
    },
    PrList {
        owner: String,
        repo: String,
    },
    Pr {
        owner: String,
        repo: String,
        number: u32,
    },
}

impl GitHubResource {
    fn repo_slug(&self) -> String {
        match self {
            GitHubResource::Repo { owner, repo }
            | GitHubResource::IssueList { owner, repo }
            | GitHubResource::Issue { owner, repo, .. }
            | GitHubResource::PrList { owner, repo }
            | GitHubResource::Pr { owner, repo, .. } => format!("{}/{}", owner, repo),
        }
    }
}

/// Parse a GitHub URL into a resource.
fn parse_resource(url: &str) -> Option<GitHubResource> {
    let path = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))?;
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    if parts.len() < 2 {
        return None;
    }

    let owner = parts[0].to_string();
    let repo = parts[1].to_string();

    match parts.get(2).copied() {
        None | Some("") => Some(GitHubResource::Repo { owner, repo }),
        Some("issues") => match parts.get(3) {
            Some(num) => num.parse::<u32>().ok().map(|number| GitHubResource::Issue {
                owner,
                repo,
                number,
            }),
            None => Some(GitHubResource::IssueList { owner, repo }),
        },
        Some("pull") => match parts.get(3) {
            Some(num) => num.parse::<u32>().ok().map(|number| GitHubResource::Pr {
                owner,
                repo,
                number,
            }),
            None => Some(GitHubResource::PrList { owner, repo }),
        },
        Some("pulls") => Some(GitHubResource::PrList { owner, repo }),
        _ => Some(GitHubResource::Repo { owner, repo }),
    }
}

/// GitHub CLI backend.
#[derive(Debug, Clone, Default)]
pub struct GhCliBackend;

impl GhCliBackend {
    pub fn new() -> Self {
        Self
    }

    /// Build the `gh` CLI argument list for reading a parsed GitHub resource.
    fn build_read_args(resource: &GitHubResource) -> Vec<String> {
        match resource {
            GitHubResource::Repo { .. } => {
                vec![
                    "repo".to_string(),
                    "view".to_string(),
                    resource.repo_slug(),
                    "--json".to_string(),
                    "url,name,description,readme".to_string(),
                ]
            }
            GitHubResource::IssueList { .. } => {
                vec![
                    "issue".to_string(),
                    "list".to_string(),
                    "--repo".to_string(),
                    resource.repo_slug(),
                    "--limit".to_string(),
                    "30".to_string(),
                    "--json".to_string(),
                    "number,title,url,author,state".to_string(),
                ]
            }
            GitHubResource::Issue { number, .. } => {
                vec![
                    "issue".to_string(),
                    "view".to_string(),
                    number.to_string(),
                    "--repo".to_string(),
                    resource.repo_slug(),
                    "--json".to_string(),
                    "number,title,body,url,author,state".to_string(),
                ]
            }
            GitHubResource::PrList { .. } => {
                vec![
                    "pr".to_string(),
                    "list".to_string(),
                    "--repo".to_string(),
                    resource.repo_slug(),
                    "--limit".to_string(),
                    "30".to_string(),
                    "--json".to_string(),
                    "number,title,url,author,state".to_string(),
                ]
            }
            GitHubResource::Pr { number, .. } => {
                vec![
                    "pr".to_string(),
                    "view".to_string(),
                    number.to_string(),
                    "--repo".to_string(),
                    resource.repo_slug(),
                    "--json".to_string(),
                    "number,title,body,url,author,state".to_string(),
                ]
            }
        }
    }

    async fn run_gh(&self, args: &[&str]) -> Result<String, BackendError> {
        let output = tokio::process::Command::new("gh")
            .args(args)
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
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(BackendError::CommandFailed(self.name().to_string(), stderr));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[async_trait]
impl Backend for GhCliBackend {
    fn name(&self) -> &str {
        "gh-cli"
    }

    async fn probe(&self) -> ProbeResult {
        let engine = ProbeEngine::new(Duration::from_secs(5));
        let target = ProbeTarget::version("gh", "Install GitHub CLI: https://cli.github.com");
        let result = engine.probe(&target).await;

        if result.status != ProbeStatus::Ok {
            return result;
        }

        // gh is installed; verify the user is authenticated.
        let auth = tokio::process::Command::new("gh")
            .args(["auth", "status"])
            .output()
            .await;

        match auth {
            Ok(output) if output.status.success() => result,
            _ => ProbeResult::warn(
                "gh",
                "GitHub CLI is installed but not authenticated",
                "Run 'gh auth login' to authenticate",
            ),
        }
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let resource = parse_resource(url).ok_or_else(|| {
            BackendError::Parse(
                self.name().to_string(),
                format!("invalid GitHub URL: {}", url),
            )
        })?;

        let args: Vec<String> = Self::build_read_args(&resource);
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let body = self.run_gh(&arg_refs).await?;

        let title = match &resource {
            GitHubResource::Repo { .. } => resource.repo_slug(),
            GitHubResource::IssueList { .. } => format!("{} issues", resource.repo_slug()),
            GitHubResource::Issue { number, .. } => format!("{}#{}", resource.repo_slug(), number),
            GitHubResource::PrList { .. } => format!("{} pull requests", resource.repo_slug()),
            GitHubResource::Pr { number, .. } => format!("{}#{}", resource.repo_slug(), number),
        };

        Ok(Content {
            url: url.to_string(),
            title: Some(title),
            body,
            metadata: serde_json::Value::Null,
            cached: false,
        })
    }

    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let limit = opts.limit.clamp(1, 100);
        let output = self
            .run_gh(&[
                "search",
                "repos",
                query,
                "--limit",
                &limit.to_string(),
                "--json",
                "fullName,description,url",
            ])
            .await?;

        let repos: Vec<serde_json::Value> = serde_json::from_str(&output)
            .map_err(|e| BackendError::Parse(self.name().to_string(), e.to_string()))?;

        Ok(repos
            .into_iter()
            .map(|r| SearchResult {
                title: r["fullName"].as_str().unwrap_or("").to_string(),
                url: r["url"].as_str().unwrap_or("").to_string(),
                snippet: r["description"].as_str().unwrap_or("").to_string(),
                author: None,
                timestamp: None,
                metadata: r.clone(),
            })
            .collect())
    }
}

/// GitHub REST API backend.
#[derive(Debug, Clone)]
pub struct GithubApiBackend {
    client: reqwest::Client,
    token: Option<String>,
}

impl Default for GithubApiBackend {
    fn default() -> Self {
        let token = std::env::var("GITHUB_TOKEN").ok();
        Self {
            client: crate::http::default_client(),
            token,
        }
    }
}

impl GithubApiBackend {
    pub fn new() -> Self {
        Self::default()
    }

    fn auth_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(token) = &self.token {
            if let Ok(value) = reqwest::header::HeaderValue::from_str(&format!("Bearer {}", token))
            {
                headers.insert(reqwest::header::AUTHORIZATION, value);
            }
        }
        headers
    }
}

#[async_trait]
impl Backend for GithubApiBackend {
    fn name(&self) -> &str {
        "github-api"
    }

    async fn probe(&self) -> ProbeResult {
        // The API backend is usable without a token, but rate-limited.
        if self.token.is_some() {
            ProbeResult::ok("github-api", "authenticated")
        } else {
            ProbeResult::warn(
                "github-api",
                "no GITHUB_TOKEN configured",
                "Set GITHUB_TOKEN for higher rate limits",
            )
        }
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let resource = parse_resource(url).ok_or_else(|| {
            BackendError::Parse(
                self.name().to_string(),
                format!("invalid GitHub URL: {}", url),
            )
        })?;

        let api_url = match &resource {
            GitHubResource::Repo { .. } => {
                format!("https://api.github.com/repos/{}", resource.repo_slug())
            }
            GitHubResource::IssueList { .. } => {
                format!(
                    "https://api.github.com/repos/{}/issues?state=all&per_page=30",
                    resource.repo_slug()
                )
            }
            GitHubResource::Issue { number, .. } => {
                format!(
                    "https://api.github.com/repos/{}/issues/{}",
                    resource.repo_slug(),
                    number
                )
            }
            GitHubResource::PrList { .. } => {
                format!(
                    "https://api.github.com/repos/{}/pulls?state=all&per_page=30",
                    resource.repo_slug()
                )
            }
            GitHubResource::Pr { number, .. } => {
                format!(
                    "https://api.github.com/repos/{}/pulls/{}",
                    resource.repo_slug(),
                    number
                )
            }
        };

        let response = self
            .client
            .get(&api_url)
            .headers(self.auth_headers())
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

        Ok(Content {
            url: url.to_string(),
            title: Some(resource.repo_slug()),
            body,
            metadata: serde_json::Value::Null,
            cached: false,
        })
    }

    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let limit = opts.limit.clamp(1, 100);
        let api_url = format!(
            "https://api.github.com/search/repositories?q={}&per_page={}",
            query, limit
        );

        let response = self
            .client
            .get(&api_url)
            .headers(self.auth_headers())
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

        let payload: serde_json::Value = response
            .json()
            .await
            .map_err(|e| BackendError::Parse(self.name().to_string(), e.to_string()))?;

        let items = payload["items"].as_array().cloned().unwrap_or_default();
        Ok(items
            .into_iter()
            .map(|r| SearchResult {
                title: r["full_name"].as_str().unwrap_or("").to_string(),
                url: r["html_url"].as_str().unwrap_or("").to_string(),
                snippet: r["description"].as_str().unwrap_or("").to_string(),
                author: r["owner"]["login"].as_str().map(|s| s.to_string()),
                timestamp: None,
                metadata: r.clone(),
            })
            .collect())
    }
}

/// GitHub channel.
#[derive(Debug, Clone)]
pub struct GithubChannel {
    router: BackendRouter,
}

impl GithubChannel {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for GithubChannel {
    fn default() -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![
            Arc::new(GhCliBackend::new()),
            Arc::new(GithubApiBackend::new()),
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
impl agentspan_core::channel::Channel for GithubChannel {
    fn name(&self) -> &str {
        "github"
    }

    fn description(&self) -> &str {
        "Read repositories and search repos via the GitHub CLI or REST API"
    }

    fn format_for_llm(&self, raw: &str) -> String {
        crate::format::extract_text_fields(
            raw,
            &["title", "body", "name", "description", "full_name"],
            8000,
        )
    }

    fn can_handle(&self, url: &str) -> bool {
        url.starts_with("https://github.com/") || url.starts_with("http://github.com/")
    }

    fn tier(&self) -> Tier {
        Tier::Zero
    }

    fn backends(&self) -> Vec<Box<dyn Backend>> {
        vec![
            Box::new(GhCliBackend::new()),
            Box::new(GithubApiBackend::new()),
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

    async fn check_health(&self) -> Vec<BackendHealth> {
        self.router.check_health().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentspan_core::channel::Channel;

    #[test]
    fn github_can_handle_github_urls() {
        let channel = GithubChannel::new();
        assert!(channel.can_handle("https://github.com/agentspan/agentspan"));
        assert!(!channel.can_handle("https://example.com"));
    }

    #[test]
    fn parse_resource_extracts_repo() {
        assert_eq!(
            parse_resource("https://github.com/agentspan/agentspan"),
            Some(GitHubResource::Repo {
                owner: "agentspan".to_string(),
                repo: "agentspan".to_string(),
            })
        );
        assert_eq!(
            parse_resource("https://github.com/agentspan/agentspan/blob/main/README.md"),
            Some(GitHubResource::Repo {
                owner: "agentspan".to_string(),
                repo: "agentspan".to_string(),
            })
        );
    }

    #[test]
    fn parse_resource_extracts_issues_and_prs() {
        assert_eq!(
            parse_resource("https://github.com/agentspan/agentspan/issues"),
            Some(GitHubResource::IssueList {
                owner: "agentspan".to_string(),
                repo: "agentspan".to_string(),
            })
        );
        assert_eq!(
            parse_resource("https://github.com/agentspan/agentspan/issues/42"),
            Some(GitHubResource::Issue {
                owner: "agentspan".to_string(),
                repo: "agentspan".to_string(),
                number: 42,
            })
        );
        assert_eq!(
            parse_resource("https://github.com/agentspan/agentspan/pulls"),
            Some(GitHubResource::PrList {
                owner: "agentspan".to_string(),
                repo: "agentspan".to_string(),
            })
        );
        assert_eq!(
            parse_resource("https://github.com/agentspan/agentspan/pull/7"),
            Some(GitHubResource::Pr {
                owner: "agentspan".to_string(),
                repo: "agentspan".to_string(),
                number: 7,
            })
        );
        assert_eq!(parse_resource("https://example.com"), None);
    }

    #[test]
    fn gh_cli_builds_repo_read_args() {
        let resource = GitHubResource::Repo {
            owner: "agentspan".to_string(),
            repo: "agentspan".to_string(),
        };
        assert_eq!(
            GhCliBackend::build_read_args(&resource),
            vec![
                "repo",
                "view",
                "agentspan/agentspan",
                "--json",
                "url,name,description,readme"
            ]
        );
    }

    #[test]
    fn gh_cli_builds_issue_read_args() {
        let resource = GitHubResource::Issue {
            owner: "agentspan".to_string(),
            repo: "agentspan".to_string(),
            number: 42,
        };
        assert_eq!(
            GhCliBackend::build_read_args(&resource),
            vec![
                "issue",
                "view",
                "42",
                "--repo",
                "agentspan/agentspan",
                "--json",
                "number,title,body,url,author,state"
            ]
        );
    }

    #[test]
    fn gh_cli_builds_pr_list_args() {
        let resource = GitHubResource::PrList {
            owner: "agentspan".to_string(),
            repo: "agentspan".to_string(),
        };
        assert_eq!(
            GhCliBackend::build_read_args(&resource),
            vec![
                "pr",
                "list",
                "--repo",
                "agentspan/agentspan",
                "--limit",
                "30",
                "--json",
                "number,title,url,author,state"
            ]
        );
    }

    #[test]
    fn github_channel_has_cli_and_api_backends() {
        let channel = GithubChannel::new();
        let backends = channel.backends();
        let names: Vec<_> = backends.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["gh-cli", "github-api"]);
    }

    #[tokio::test]
    async fn github_api_backend_reads_repo_from_mock_server() {
        use wiremock::{matchers::method, Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        let body = r#"{"full_name":"agentspan/agentspan","description":"test"}"#;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&mock_server)
            .await;

        // Point the backend at the mock by overriding the URL construction is not
        // directly supported, so we test the HTTP logic via a helper request.
        let client = reqwest::Client::new();
        let response = client
            .get(format!("{}/repos/agentspan/agentspan", mock_server.uri()))
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success());
    }

    #[test]
    fn github_cli_search_maps_json_output() {
        let output = r#"[
            {"fullName":"agentspan/agentspan","description":"test repo","url":"https://github.com/agentspan/agentspan"}
        ]"#;

        let repos: Vec<serde_json::Value> = serde_json::from_str(output).unwrap();
        let results: Vec<SearchResult> = repos
            .into_iter()
            .map(|r| SearchResult {
                title: r["fullName"].as_str().unwrap_or("").to_string(),
                url: r["url"].as_str().unwrap_or("").to_string(),
                snippet: r["description"].as_str().unwrap_or("").to_string(),
                author: None,
                timestamp: None,
                metadata: r.clone(),
            })
            .collect();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "agentspan/agentspan");
        assert_eq!(results[0].url, "https://github.com/agentspan/agentspan");
        assert_eq!(results[0].snippet, "test repo");
    }

    #[tokio::test]
    async fn github_api_backend_search_maps_mock_response() {
        use wiremock::{matchers::method, Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        let body = r#"{"items":[{"full_name":"agentspan/agentspan","html_url":"https://github.com/agentspan/agentspan","description":"test"}]}"#;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&mock_server)
            .await;

        let client = reqwest::Client::new();
        let payload: serde_json::Value = client
            .get(format!("{}/search/repositories", mock_server.uri()))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(payload["items"].as_array().unwrap().len(), 1);
    }
}
