//! Spotify channel — backed by the official Spotify Web API.
//!
//! Auth uses the client-credentials flow: a `SPOTIFY_CLIENT_ID` /
//! `SPOTIFY_CLIENT_SECRET` pair is exchanged for a bearer token, which is then
//! used for catalog search and reads. Tier 1 (needs credentials).

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

const DEFAULT_API_BASE: &str = "https://api.spotify.com/v1";
const DEFAULT_TOKEN_URL: &str = "https://accounts.spotify.com/api/token";

/// Extract the track id from an `open.spotify.com/track/{id}` URL.
fn parse_track_id(url: &str) -> Option<String> {
    let after = url.split("/track/").nth(1)?;
    let id = after.split(['?', '#', '/']).next().unwrap_or(after);
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

fn artists_of(value: &serde_json::Value) -> String {
    value["artists"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x["name"].as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}

/// Spotify Web API backend.
#[derive(Debug, Clone)]
pub struct SpotifyApiBackend {
    client: reqwest::Client,
    api_base: String,
    token_url: String,
    client_id: Option<String>,
    client_secret: Option<String>,
}

impl Default for SpotifyApiBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            api_base: DEFAULT_API_BASE.to_string(),
            token_url: DEFAULT_TOKEN_URL.to_string(),
            client_id: std::env::var("SPOTIFY_CLIENT_ID").ok(),
            client_secret: std::env::var("SPOTIFY_CLIENT_SECRET").ok(),
        }
    }
}

impl SpotifyApiBackend {
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
        let basic = base64::engine::general_purpose::STANDARD.encode(format!("{id}:{secret}"));
        let response = self
            .client
            .post(&self.token_url)
            .header("Authorization", format!("Basic {basic}"))
            .form(&[("grant_type", "client_credentials")])
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
        let response = self
            .client
            .get(url)
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
impl Backend for SpotifyApiBackend {
    fn name(&self) -> &str {
        "spotify-api"
    }

    async fn probe(&self) -> ProbeResult {
        if self.client_id.is_some() && self.client_secret.is_some() {
            ProbeResult::ok("spotify-api", "v1")
        } else {
            ProbeResult::warn(
                "spotify-api",
                "no Spotify credentials configured",
                "Set SPOTIFY_CLIENT_ID and SPOTIFY_CLIENT_SECRET",
            )
        }
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let id = parse_track_id(url).ok_or_else(|| {
            BackendError::Parse(
                self.name().to_string(),
                format!("not a Spotify track URL: {url}"),
            )
        })?;
        let token = self.token().await?;
        let payload = self
            .get_json(&format!("{}/tracks/{}", self.api_base, id), &token)
            .await?;
        let name = payload["name"].as_str().unwrap_or("");
        let artists = artists_of(&payload);
        let album = payload["album"]["name"].as_str().unwrap_or("");
        Ok(Content {
            url: url.to_string(),
            title: Some(name.to_string()),
            body: format!("{name} — {artists}\nAlbum: {album}"),
            metadata: payload,
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
            opts.limit.min(50)
        };
        let token = self.token().await?;
        let url = format!(
            "{}/search?q={}&type=track&limit={}",
            self.api_base,
            crate::percent_encode(query),
            limit
        );
        let payload = self.get_json(&url, &token).await?;
        Ok(payload["tracks"]["items"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|t| {
                let artists = artists_of(&t);
                SearchResult {
                    title: t["name"].as_str().unwrap_or("").to_string(),
                    url: t["external_urls"]["spotify"]
                        .as_str()
                        .unwrap_or("")
                        .to_string(),
                    snippet: format!("{} — {}", t["name"].as_str().unwrap_or(""), artists),
                    author: Some(artists),
                    timestamp: t["album"]["release_date"].as_str().map(|s| s.to_string()),
                    metadata: t,
                }
            })
            .collect())
    }
}

/// Spotify channel.
#[derive(Debug, Clone)]
pub struct SpotifyChannel {
    router: BackendRouter,
    backend: SpotifyApiBackend,
}

impl SpotifyChannel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a channel whose backend targets `base` (tests).
    pub fn with_base_url(base: impl Into<String>) -> Self {
        let backend = SpotifyApiBackend::with_base_url(base);
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for SpotifyChannel {
    fn default() -> Self {
        let backend = SpotifyApiBackend::new();
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
impl agentspan_core::channel::Channel for SpotifyChannel {
    fn name(&self) -> &str {
        "spotify"
    }

    fn description(&self) -> &str {
        "Search and read Spotify tracks/albums/artists via the official Web API"
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("open.spotify.com")
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
    fn can_handle_spotify_urls() {
        let ch = SpotifyChannel::new();
        assert!(ch.can_handle("https://open.spotify.com/track/abc"));
        assert!(!ch.can_handle("https://example.com"));
    }

    #[test]
    fn channel_is_tier_one() {
        assert_eq!(SpotifyChannel::new().tier(), Tier::One);
    }

    #[test]
    fn parse_track_id_extracts_id() {
        assert_eq!(
            parse_track_id("https://open.spotify.com/track/6rqhFgbbKwnb9MLmUQDhG6?si=x"),
            Some("6rqhFgbbKwnb9MLmUQDhG6".to_string())
        );
        assert_eq!(parse_track_id("https://open.spotify.com/album/x"), None);
    }

    async fn mock_token(server: &MockServer) {
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"access_token":"tok","token_type":"Bearer"}"#),
            )
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn search_maps_tracks() {
        let server = MockServer::start().await;
        mock_token(&server).await;
        let body = r#"{"tracks":{"items":[{"name":"Song","artists":[{"name":"Band"}],"album":{"release_date":"2020"},"external_urls":{"spotify":"https://open.spotify.com/track/1"}}]}}"#;
        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = SpotifyChannel::with_base_url(server.uri());
        let results = ch.search("song", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Song");
        assert_eq!(results[0].author.as_deref(), Some("Band"));
    }

    #[tokio::test]
    async fn read_returns_track() {
        let server = MockServer::start().await;
        mock_token(&server).await;
        let body = r#"{"name":"Song","artists":[{"name":"Band"}],"album":{"name":"LP"}}"#;
        Mock::given(method("GET"))
            .and(path("/tracks/1"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = SpotifyChannel::with_base_url(server.uri());
        let content = ch
            .read("https://open.spotify.com/track/1", ReadOptions::default())
            .await
            .unwrap();
        assert_eq!(content.title.as_deref(), Some("Song"));
        assert!(content.body.contains("Band"));
        assert!(content.body.contains("LP"));
    }
}
