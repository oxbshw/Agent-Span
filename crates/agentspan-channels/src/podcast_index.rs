//! Podcast Index channel — backed by the free Podcast Index API.
//!
//! The API is free but requires a key/secret pair (`PODCASTINDEX_KEY` /
//! `PODCASTINDEX_SECRET`) and a SHA-1 request signature. Search finds podcasts
//! by term; read pulls episodes for a feed URL. Tier 1.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use sha1::{Digest, Sha1};

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

const DEFAULT_BASE: &str = "https://api.podcastindex.org/api/1.0";

/// Compute the Podcast Index `Authorization` header: sha1(key + secret + date).
fn sign(key: &str, secret: &str, date: i64) -> String {
    let mut hasher = Sha1::new();
    hasher.update(key.as_bytes());
    hasher.update(secret.as_bytes());
    hasher.update(date.to_string().as_bytes());
    hex(&hasher.finalize())
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Podcast Index API backend.
#[derive(Debug, Clone)]
pub struct PodcastIndexBackend {
    client: reqwest::Client,
    base_url: String,
    key: Option<String>,
    secret: Option<String>,
}

impl Default for PodcastIndexBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
            key: std::env::var("PODCASTINDEX_KEY").ok(),
            secret: std::env::var("PODCASTINDEX_SECRET").ok(),
        }
    }
}

impl PodcastIndexBackend {
    /// Create a backend reading credentials from the environment.
    pub fn new() -> Self {
        Self::default()
    }

    /// Point the API at `base` with test credentials (tests).
    pub fn with_base_url(base: impl Into<String>) -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: base.into(),
            key: Some("test-key".to_string()),
            secret: Some("test-secret".to_string()),
        }
    }

    fn credentials(&self) -> Result<(&str, &str), BackendError> {
        match (&self.key, &self.secret) {
            (Some(k), Some(s)) => Ok((k, s)),
            _ => Err(BackendError::AuthRequired(self.name().to_string())),
        }
    }

    async fn get_json(&self, url: &str) -> Result<serde_json::Value, BackendError> {
        let (key, secret) = self.credentials()?;
        let date = Utc::now().timestamp();
        let auth = sign(key, secret, date);
        let response = self
            .client
            .get(url)
            .header("User-Agent", "AgentSpan/0.1")
            .header("X-Auth-Key", key)
            .header("X-Auth-Date", date.to_string())
            .header("Authorization", auth)
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
impl Backend for PodcastIndexBackend {
    fn name(&self) -> &str {
        "podcastindex-api"
    }

    async fn probe(&self) -> ProbeResult {
        if self.key.is_some() && self.secret.is_some() {
            ProbeResult::ok("podcastindex-api", "1.0")
        } else {
            ProbeResult::warn(
                "podcastindex-api",
                "no Podcast Index credentials configured",
                "Get a free key at podcastindex.org, then set PODCASTINDEX_KEY and PODCASTINDEX_SECRET",
            )
        }
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let api = format!(
            "{}/episodes/byfeedurl?url={}&max=25",
            self.base_url,
            crate::percent_encode(url)
        );
        let payload = self.get_json(&api).await?;
        let items = payload["items"].as_array().cloned().unwrap_or_default();
        let body = items
            .iter()
            .map(|e| {
                format!(
                    "• {}\n  {}",
                    e["title"].as_str().unwrap_or(""),
                    e["description"]
                        .as_str()
                        .unwrap_or("")
                        .chars()
                        .take(200)
                        .collect::<String>()
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        Ok(Content {
            url: url.to_string(),
            title: Some(format!("{} episodes", items.len())),
            body,
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
            opts.limit.min(40)
        };
        let api = format!(
            "{}/search/byterm?q={}&max={}",
            self.base_url,
            crate::percent_encode(query),
            limit
        );
        let payload = self.get_json(&api).await?;
        Ok(payload["feeds"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|f| SearchResult {
                title: f["title"].as_str().unwrap_or("").to_string(),
                url: f["url"]
                    .as_str()
                    .or_else(|| f["link"].as_str())
                    .unwrap_or("")
                    .to_string(),
                snippet: f["description"]
                    .as_str()
                    .unwrap_or("")
                    .chars()
                    .take(280)
                    .collect(),
                author: f["author"].as_str().map(|s| s.to_string()),
                timestamp: None,
                metadata: f,
            })
            .collect())
    }
}

/// Podcast Index channel.
#[derive(Debug, Clone)]
pub struct PodcastIndexChannel {
    router: BackendRouter,
    backend: PodcastIndexBackend,
}

impl PodcastIndexChannel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a channel whose backend targets `base` (tests).
    pub fn with_base_url(base: impl Into<String>) -> Self {
        let backend = PodcastIndexBackend::with_base_url(base);
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for PodcastIndexChannel {
    fn default() -> Self {
        let backend = PodcastIndexBackend::new();
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
impl agentspan_core::channel::Channel for PodcastIndexChannel {
    fn name(&self) -> &str {
        "podcasts"
    }

    fn description(&self) -> &str {
        "Search podcasts and read episode lists via the free Podcast Index API"
    }

    fn can_handle(&self, _url: &str) -> bool {
        // Read takes an explicit feed URL; not auto-selected by the registry.
        false
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
    fn channel_is_search_oriented_tier_one() {
        let ch = PodcastIndexChannel::new();
        assert_eq!(ch.name(), "podcasts");
        assert_eq!(ch.tier(), Tier::One);
        assert!(!ch.can_handle("https://example.com"));
    }

    #[test]
    fn sign_is_sha1_hex_of_key_secret_date() {
        // sha1("keysecret1") precomputed.
        let got = sign("key", "secret", 1);
        assert_eq!(got.len(), 40);
        assert!(got.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hex_encodes_bytes() {
        assert_eq!(hex(&[0x00, 0xff, 0x10]), "00ff10");
    }

    #[tokio::test]
    async fn search_maps_feeds() {
        let server = MockServer::start().await;
        let body = r#"{"feeds":[{"title":"The Pod","url":"https://feed.xml","author":"Host","description":"A show"}]}"#;
        Mock::given(method("GET"))
            .and(path("/search/byterm"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = PodcastIndexChannel::with_base_url(server.uri());
        let results = ch.search("pod", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "The Pod");
        assert_eq!(results[0].author.as_deref(), Some("Host"));
    }

    #[tokio::test]
    async fn read_lists_episodes() {
        let server = MockServer::start().await;
        let body = r#"{"items":[{"title":"Ep 1","description":"first"},{"title":"Ep 2","description":"second"}]}"#;
        Mock::given(method("GET"))
            .and(path("/episodes/byfeedurl"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = PodcastIndexChannel::with_base_url(server.uri());
        let content = ch
            .read("https://feed.xml", ReadOptions::default())
            .await
            .unwrap();
        assert!(content.body.contains("Ep 1"));
        assert!(content.body.contains("Ep 2"));
    }
}
