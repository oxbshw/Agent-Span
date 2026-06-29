//! Twitter / X channel — wraps a `twitter-cli` style backend.
//!
//! Twitter requires authentication, so this is a Tier-1 channel. The CLI backend
//! shells out to a configured binary; when it is absent the probe reports it as
//! missing and the channel degrades gracefully.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::{ProbeEngine, ProbeTarget};
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

/// Extract a numeric tweet id from a status URL.
fn parse_tweet_id(url: &str) -> Option<String> {
    let rest = url.split("/status/").nth(1)?;
    let id: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if id.is_empty() {
        None
    } else {
        Some(id)
    }
}

fn read_args(id: &str) -> Vec<String> {
    vec!["show".to_string(), id.to_string(), "--json".to_string()]
}

fn search_args(query: &str, limit: usize) -> Vec<String> {
    vec![
        "search".to_string(),
        query.to_string(),
        "--limit".to_string(),
        limit.clamp(1, 100).to_string(),
        "--json".to_string(),
    ]
}

/// Parse `auth_token` + `ct0` from a stored Twitter cookie string.
fn parse_twitter_creds(cookie: &str) -> Option<(String, String)> {
    let mut auth_token = None;
    let mut ct0 = None;
    for part in cookie.split(';') {
        let part = part.trim();
        if let Some(v) = part.strip_prefix("auth_token=") {
            auth_token = Some(v.trim().to_string());
        } else if let Some(v) = part.strip_prefix("ct0=") {
            ct0 = Some(v.trim().to_string());
        }
    }
    match (auth_token, ct0) {
        (Some(a), Some(c)) if !a.is_empty() && !c.is_empty() => Some((a, c)),
        _ => None,
    }
}

/// CLI backend for Twitter/X.
#[derive(Debug, Clone)]
pub struct TwitterCliBackend {
    bin: String,
    /// `(auth_token, ct0)` imported via `agentspan config cookies`.
    creds: Option<(String, String)>,
}

impl Default for TwitterCliBackend {
    fn default() -> Self {
        Self {
            bin: "twitter-cli".to_string(),
            creds: crate::http::cookie_for("twitter")
                .as_deref()
                .and_then(parse_twitter_creds),
        }
    }
}

impl TwitterCliBackend {
    /// Create a backend using the default `twitter-cli` binary.
    pub fn new() -> Self {
        Self::default()
    }

    /// Use a specific binary name/path.
    pub fn with_bin(bin: impl Into<String>) -> Self {
        Self {
            bin: bin.into(),
            creds: None,
        }
    }

    /// Set credentials from a stored cookie string (tests / explicit config).
    pub fn with_cookie(mut self, cookie: &str) -> Self {
        self.creds = parse_twitter_creds(cookie);
        self
    }

    /// Environment variables `twitter-cli` reads for cookie auth.
    fn env_pairs(&self) -> Vec<(&'static str, String)> {
        match &self.creds {
            Some((token, ct0)) => vec![
                ("TWITTER_AUTH_TOKEN", token.clone()),
                ("TWITTER_CT0", ct0.clone()),
                ("AUTH_TOKEN", token.clone()),
                ("CT0", ct0.clone()),
            ],
            None => vec![],
        }
    }

    async fn run(&self, args: &[String]) -> Result<String, BackendError> {
        let output = tokio::process::Command::new(&self.bin)
            .args(args)
            .envs(self.env_pairs())
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
            return Err(BackendError::CommandFailed(
                self.name().to_string(),
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[async_trait]
impl Backend for TwitterCliBackend {
    fn name(&self) -> &str {
        "twitter-cli"
    }

    async fn probe(&self) -> ProbeResult {
        let engine = ProbeEngine::new(Duration::from_secs(5));
        let target = ProbeTarget::version(&self.bin, "Install a twitter-cli compatible tool");
        engine.probe(&target).await
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let id = parse_tweet_id(url).ok_or_else(|| {
            BackendError::Parse(self.name().to_string(), format!("not a tweet URL: {url}"))
        })?;
        let body = self.run(&read_args(&id)).await?;
        Ok(Content {
            url: url.to_string(),
            title: Some(format!("tweet {id}")),
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
        let limit = if opts.limit == 0 { 20 } else { opts.limit };
        let out = self.run(&search_args(query, limit)).await?;
        // TODO: verify twitter-cli's JSON shape against a pinned version.
        let parsed: serde_json::Value = match serde_json::from_str(&out) {
            Ok(v) => v,
            Err(_) => return Ok(crate::format::raw_search_fallback(&out)),
        };
        Ok(parsed
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|t| SearchResult {
                title: t["text"].as_str().unwrap_or("").chars().take(80).collect(),
                url: t["url"].as_str().unwrap_or("").to_string(),
                snippet: t["text"].as_str().unwrap_or("").to_string(),
                author: t["author"].as_str().map(|s| s.to_string()),
                timestamp: t["created_at"].as_str().map(|s| s.to_string()),
                metadata: t.clone(),
            })
            .collect())
    }
}

/// Twitter/X channel.
#[derive(Debug, Clone)]
pub struct TwitterChannel {
    router: BackendRouter,
}

impl TwitterChannel {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for TwitterChannel {
    fn default() -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(TwitterCliBackend::new())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router }
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for TwitterChannel {
    fn name(&self) -> &str {
        "twitter"
    }

    fn description(&self) -> &str {
        "Read tweets and search via a twitter-cli compatible backend (requires auth)"
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("twitter.com") || url.contains("://x.com")
    }

    fn tier(&self) -> Tier {
        Tier::One
    }

    fn backends(&self) -> Vec<Box<dyn Backend>> {
        vec![Box::new(TwitterCliBackend::new())]
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
    fn can_handle_twitter_and_x_urls() {
        let ch = TwitterChannel::new();
        assert!(ch.can_handle("https://twitter.com/user/status/123"));
        assert!(ch.can_handle("https://x.com/user/status/123"));
        assert!(!ch.can_handle("https://example.com"));
    }

    #[test]
    fn channel_is_tier_one() {
        assert_eq!(TwitterChannel::new().tier(), Tier::One);
    }

    #[test]
    fn parse_tweet_id_from_status_url() {
        assert_eq!(
            parse_tweet_id("https://twitter.com/user/status/1700000000000000000"),
            Some("1700000000000000000".to_string())
        );
        assert_eq!(parse_tweet_id("https://twitter.com/user"), None);
    }

    #[test]
    fn arg_builders_are_well_formed() {
        assert_eq!(read_args("42"), vec!["show", "42", "--json"]);
        assert_eq!(
            search_args("rust", 5),
            vec!["search", "rust", "--limit", "5", "--json"]
        );
    }

    #[tokio::test]
    async fn probe_returns_a_result() {
        let probe = TwitterCliBackend::new().probe().await;
        assert!(!probe.message.is_empty());
    }

    #[test]
    fn parse_creds_from_cookie_string() {
        assert_eq!(
            parse_twitter_creds("auth_token=abc; ct0=xyz; other=1"),
            Some(("abc".to_string(), "xyz".to_string()))
        );
        assert_eq!(parse_twitter_creds("auth_token=abc"), None);
        assert_eq!(parse_twitter_creds("nope=1"), None);
    }

    #[test]
    fn configured_creds_become_subprocess_env() {
        // Regression: imported cookies must be forwarded to twitter-cli.
        let backend = TwitterCliBackend::new().with_cookie("auth_token=abc; ct0=xyz");
        let env = backend.env_pairs();
        assert!(env.contains(&("TWITTER_AUTH_TOKEN", "abc".to_string())));
        assert!(env.contains(&("TWITTER_CT0", "xyz".to_string())));
        // No creds → no env injected.
        assert!(TwitterCliBackend::with_bin("twitter-cli")
            .env_pairs()
            .is_empty());
    }
}
