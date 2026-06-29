//! Twitch channel — backed by the official Twitch Helix API.
//!
//! Auth uses the app client-credentials flow: a `TWITCH_CLIENT_ID` /
//! `TWITCH_CLIENT_SECRET` pair is exchanged for an app access token. Tier 1.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

const DEFAULT_API_BASE: &str = "https://api.twitch.tv/helix";
const DEFAULT_TOKEN_URL: &str = "https://id.twitch.tv/oauth2/token";

/// Extract the channel login from a `twitch.tv/{login}` URL.
fn parse_login(url: &str) -> Option<String> {
    let after = url.split("twitch.tv/").nth(1)?;
    let login = after.split(['?', '#', '/']).next().unwrap_or(after);
    if login.is_empty() {
        None
    } else {
        Some(login.to_string())
    }
}

/// Twitch Helix API backend.
#[derive(Debug, Clone)]
pub struct TwitchApiBackend {
    client: reqwest::Client,
    api_base: String,
    token_url: String,
    client_id: Option<String>,
    client_secret: Option<String>,
}

impl Default for TwitchApiBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            api_base: DEFAULT_API_BASE.to_string(),
            token_url: DEFAULT_TOKEN_URL.to_string(),
            client_id: std::env::var("TWITCH_CLIENT_ID").ok(),
            client_secret: std::env::var("TWITCH_CLIENT_SECRET").ok(),
        }
    }
}

impl TwitchApiBackend {
    /// Create a backend reading credentials from the environment.
    pub fn new() -> Self {
        Self::default()
    }

    /// Point the API and token endpoints at `base` with test credentials.
    pub fn with_base_url(base: impl Into<String>) -> Self {
        let base = base.into();
        Self {
            client: crate::http::default_client(),
            token_url: format!("{base}/token"),
            api_base: base,
            client_id: Some("test-id".to_string()),
            client_secret: Some("test-secret".to_string()),
        }
    }

    fn credentials(&self) -> Result<(&str, &str), BackendError> {
        match (&self.client_id, &self.client_secret) {
            (Some(id), Some(secret)) => Ok((id, secret)),
            _ => Err(BackendError::AuthRequired(self.name().to_string())),
        }
    }

    async fn token(&self) -> Result<String, BackendError> {
        let (id, secret) = self.credentials()?;
        let response = self
            .client
            .post(&self.token_url)
            .form(&[
                ("client_id", id),
                ("client_secret", secret),
                ("grant_type", "client_credentials"),
            ])
            .send()
            .await
            .map_err(|e| BackendError::RequestFailed(self.name().to_string(), e.to_string()))?;
        if !response.status().is_success() {
            return Err(BackendError::AuthRequired(self.name().to_string()));
        }
        let payload: serde_json::Value = response
            .json()
            .await
            .map_err(|e| BackendError::Parse(self.name().to_string(), e.to_string()))?;
        payload["access_token"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| BackendError::AuthRequired(self.name().to_string()))
    }

    async fn get_json(&self, url: &str, token: &str) -> Result<serde_json::Value, BackendError> {
        let id = self.credentials()?.0;
        let response = self
            .client
            .get(url)
            .header("Client-Id", id)
            .bearer_auth(token)
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
impl Backend for TwitchApiBackend {
    fn name(&self) -> &str {
        "twitch-api"
    }

    async fn probe(&self) -> ProbeResult {
        if self.client_id.is_some() && self.client_secret.is_some() {
            ProbeResult::ok("twitch-api", "helix")
        } else {
            ProbeResult::warn(
                "twitch-api",
                "no Twitch credentials configured",
                "Set TWITCH_CLIENT_ID and TWITCH_CLIENT_SECRET",
            )
        }
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let login = parse_login(url).ok_or_else(|| {
            BackendError::Parse(
                self.name().to_string(),
                format!("not a Twitch channel URL: {url}"),
            )
        })?;
        let token = self.token().await?;
        let payload = self
            .get_json(
                &format!("{}/streams?user_login={}", self.api_base, login),
                &token,
            )
            .await?;
        let stream = payload["data"]
            .as_array()
            .and_then(|a| a.first())
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let title = stream["title"].as_str().unwrap_or("(offline)");
        let game = stream["game_name"].as_str().unwrap_or("");
        let viewers = stream["viewer_count"].as_u64().unwrap_or(0);
        Ok(Content {
            url: url.to_string(),
            title: Some(format!("{login} on Twitch")),
            body: format!("{title}\nGame: {game}\nViewers: {viewers}"),
            metadata: stream,
            cached: false,
        })
    }

    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let limit = if opts.limit == 0 {
            10
        } else {
            opts.limit.min(100)
        };
        let token = self.token().await?;
        let url = format!(
            "{}/search/channels?query={}&first={}",
            self.api_base,
            crate::percent_encode(query),
            limit
        );
        let payload = self.get_json(&url, &token).await?;
        Ok(payload["data"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|c| {
                let login = c["broadcaster_login"].as_str().unwrap_or("");
                SearchResult {
                    title: c["display_name"].as_str().unwrap_or("").to_string(),
                    url: format!("https://twitch.tv/{login}"),
                    snippet: c["title"].as_str().unwrap_or("").to_string(),
                    author: Some(login.to_string()),
                    timestamp: c["started_at"].as_str().map(|s| s.to_string()),
                    metadata: c,
                }
            })
            .collect())
    }
}

/// Twitch channel.
#[derive(Debug, Clone)]
pub struct TwitchChannel {
    router: BackendRouter,
    backend: TwitchApiBackend,
}

impl TwitchChannel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a channel whose backend targets `base` (tests).
    pub fn with_base_url(base: impl Into<String>) -> Self {
        let backend = TwitchApiBackend::with_base_url(base);
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for TwitchChannel {
    fn default() -> Self {
        let backend = TwitchApiBackend::new();
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for TwitchChannel {
    fn name(&self) -> &str {
        "twitch"
    }

    fn description(&self) -> &str {
        "Search Twitch channels and read live stream info via the official Helix API"
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("twitch.tv/")
    }

    fn tier(&self) -> Tier {
        Tier::One
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
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn can_handle_twitch_urls() {
        let ch = TwitchChannel::new();
        assert!(ch.can_handle("https://twitch.tv/ninja"));
        assert!(!ch.can_handle("https://example.com"));
    }

    #[test]
    fn channel_is_tier_one() {
        assert_eq!(TwitchChannel::new().tier(), Tier::One);
    }

    #[test]
    fn parse_login_extracts_login() {
        assert_eq!(
            parse_login("https://twitch.tv/ninja?x=1"),
            Some("ninja".to_string())
        );
        assert_eq!(parse_login("https://example.com"), None);
    }

    async fn mock_token(server: &MockServer) {
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"access_token":"tok"}"#))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn search_maps_channels() {
        let server = MockServer::start().await;
        mock_token(&server).await;
        let body =
            r#"{"data":[{"display_name":"Ninja","broadcaster_login":"ninja","title":"Playing"}]}"#;
        Mock::given(method("GET"))
            .and(path("/search/channels"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = TwitchChannel::with_base_url(server.uri());
        let results = ch.search("ninja", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Ninja");
        assert_eq!(results[0].url, "https://twitch.tv/ninja");
    }

    #[tokio::test]
    async fn read_returns_stream() {
        let server = MockServer::start().await;
        mock_token(&server).await;
        let body = r#"{"data":[{"title":"Live now","game_name":"Fortnite","viewer_count":1000}]}"#;
        Mock::given(method("GET"))
            .and(path("/streams"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = TwitchChannel::with_base_url(server.uri());
        let content = ch
            .read("https://twitch.tv/ninja", ReadOptions::default())
            .await
            .unwrap();
        assert!(content.body.contains("Live now"));
        assert!(content.body.contains("Fortnite"));
    }
}
