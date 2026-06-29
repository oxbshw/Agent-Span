//! Discord channel — backed by the official Discord Bot REST API.
//!
//! Auth uses a bot token (`DISCORD_BOT_TOKEN`). Reads pull recent messages from
//! a channel URL; search uses the guild message-search endpoint and requires a
//! `DISCORD_GUILD_ID`. Tier 1.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

const DEFAULT_BASE: &str = "https://discord.com/api/v10";

/// Extract the channel id from a `discord.com/channels/{guild}/{channel}` URL.
fn parse_channel_id(url: &str) -> Option<String> {
    let after = url.split("/channels/").nth(1)?;
    let parts: Vec<&str> = after.split('/').collect();
    // .../channels/{guild}/{channel}
    let id = parts.get(1).copied().or_else(|| parts.first().copied())?;
    let id = id.split(['?', '#']).next().unwrap_or(id);
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
        None
    } else {
        Some(id.to_string())
    }
}

fn message_line(m: &serde_json::Value) -> String {
    let author = m["author"]["username"].as_str().unwrap_or("unknown");
    let content = m["content"].as_str().unwrap_or("");
    format!("{author}: {content}")
}

/// Discord Bot REST API backend.
#[derive(Debug, Clone)]
pub struct DiscordApiBackend {
    client: reqwest::Client,
    base_url: String,
    token: Option<String>,
    guild_id: Option<String>,
}

impl Default for DiscordApiBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
            token: std::env::var("DISCORD_BOT_TOKEN").ok(),
            guild_id: std::env::var("DISCORD_GUILD_ID").ok(),
        }
    }
}

impl DiscordApiBackend {
    /// Create a backend reading credentials from the environment.
    pub fn new() -> Self {
        Self::default()
    }

    /// Point the API at `base` with a test token and guild (tests).
    pub fn with_base_url(base: impl Into<String>) -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: base.into(),
            token: Some("test-token".to_string()),
            guild_id: Some("999".to_string()),
        }
    }

    fn token(&self) -> Result<&str, BackendError> {
        self.token
            .as_deref()
            .ok_or_else(|| BackendError::AuthRequired(self.name().to_string()))
    }

    async fn get_json(&self, url: &str) -> Result<serde_json::Value, BackendError> {
        let token = self.token()?;
        let response = self
            .client
            .get(url)
            .header("Authorization", format!("Bot {token}"))
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
impl Backend for DiscordApiBackend {
    fn name(&self) -> &str {
        "discord-api"
    }

    async fn probe(&self) -> ProbeResult {
        if self.token.is_some() {
            ProbeResult::ok("discord-api", "v10")
        } else {
            ProbeResult::warn(
                "discord-api",
                "no Discord bot token configured",
                "Set DISCORD_BOT_TOKEN (and DISCORD_GUILD_ID for search)",
            )
        }
    }

    async fn read(&self, url: &str, opts: ReadOptions) -> Result<Content, BackendError> {
        let channel = parse_channel_id(url).ok_or_else(|| {
            BackendError::Parse(
                self.name().to_string(),
                format!("not a Discord channel URL: {url}"),
            )
        })?;
        let _ = opts;
        let api = format!("{}/channels/{}/messages?limit=50", self.base_url, channel);
        let payload = self.get_json(&api).await?;
        let messages = payload.as_array().cloned().unwrap_or_default();
        let body = messages
            .iter()
            .rev()
            .map(message_line)
            .collect::<Vec<_>>()
            .join("\n");
        Ok(Content {
            url: url.to_string(),
            title: Some(format!("Discord channel {channel}")),
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
        let guild = self
            .guild_id
            .as_deref()
            .ok_or_else(|| BackendError::AuthRequired(self.name().to_string()))?;
        let api = format!(
            "{}/guilds/{}/messages/search?content={}",
            self.base_url,
            guild,
            crate::percent_encode(query)
        );
        let payload = self.get_json(&api).await?;
        let limit = if opts.limit == 0 { 25 } else { opts.limit };
        // Discord returns `messages` as an array of arrays (context groups).
        Ok(payload["messages"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|group| group.as_array().and_then(|g| g.first().cloned()))
            .take(limit)
            .map(|m| {
                let channel = m["channel_id"].as_str().unwrap_or("");
                let id = m["id"].as_str().unwrap_or("");
                SearchResult {
                    title: format!(
                        "Message from {}",
                        m["author"]["username"].as_str().unwrap_or("unknown")
                    ),
                    url: format!("https://discord.com/channels/{guild}/{channel}/{id}"),
                    snippet: m["content"]
                        .as_str()
                        .unwrap_or("")
                        .chars()
                        .take(280)
                        .collect(),
                    author: m["author"]["username"].as_str().map(|s| s.to_string()),
                    timestamp: m["timestamp"].as_str().map(|s| s.to_string()),
                    metadata: m,
                }
            })
            .collect())
    }
}

/// Discord channel.
#[derive(Debug, Clone)]
pub struct DiscordChannel {
    router: BackendRouter,
    backend: DiscordApiBackend,
}

impl DiscordChannel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a channel whose backend targets `base` (tests).
    pub fn with_base_url(base: impl Into<String>) -> Self {
        let backend = DiscordApiBackend::with_base_url(base);
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for DiscordChannel {
    fn default() -> Self {
        let backend = DiscordApiBackend::new();
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
impl agentspan_core::channel::Channel for DiscordChannel {
    fn name(&self) -> &str {
        "discord"
    }

    fn description(&self) -> &str {
        "Read messages from a Discord channel and search a guild via the Bot API"
    }

    fn format_for_llm(&self, raw: &str) -> String {
        crate::format::extract_text_fields(raw, &["content"], 8000)
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("discord.com/channels/")
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
    fn can_handle_discord_urls() {
        let ch = DiscordChannel::new();
        assert!(ch.can_handle("https://discord.com/channels/111/222"));
        assert!(!ch.can_handle("https://example.com"));
    }

    #[test]
    fn channel_is_tier_one() {
        assert_eq!(DiscordChannel::new().tier(), Tier::One);
    }

    #[test]
    fn parse_channel_id_extracts_second_segment() {
        assert_eq!(
            parse_channel_id("https://discord.com/channels/111/222"),
            Some("222".to_string())
        );
        assert_eq!(parse_channel_id("https://discord.com/channels/"), None);
    }

    #[tokio::test]
    async fn read_concatenates_messages() {
        let server = MockServer::start().await;
        let body = r#"[{"id":"2","content":"second","author":{"username":"bob"}},{"id":"1","content":"first","author":{"username":"alice"}}]"#;
        Mock::given(method("GET"))
            .and(path("/channels/222/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = DiscordChannel::with_base_url(server.uri());
        let content = ch
            .read(
                "https://discord.com/channels/111/222",
                ReadOptions::default(),
            )
            .await
            .unwrap();
        // Reversed to chronological: alice (first) then bob (second).
        assert!(content.body.starts_with("alice: first"));
        assert!(content.body.contains("bob: second"));
    }

    #[tokio::test]
    async fn search_maps_messages() {
        let server = MockServer::start().await;
        let body = r#"{"messages":[[{"id":"5","channel_id":"222","content":"hello world","author":{"username":"carol"}}]]}"#;
        Mock::given(method("GET"))
            .and(path("/guilds/999/messages/search"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = DiscordChannel::with_base_url(server.uri());
        let results = ch.search("hello", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].snippet, "hello world");
        assert_eq!(results[0].author.as_deref(), Some("carol"));
    }
}
