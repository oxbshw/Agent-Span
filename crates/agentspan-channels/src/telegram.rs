//! Telegram channel — backed by the official Telegram Bot API.
//!
//! Auth uses a bot token (`TELEGRAM_BOT_TOKEN`). Reads resolve a chat/channel
//! via `getChat`; search filters recent updates (`getUpdates`) by keyword. Tier 1.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

const DEFAULT_BASE: &str = "https://api.telegram.org";

/// Extract a `@username` chat id from a `t.me/{name}` URL.
fn parse_chat(url: &str) -> Option<String> {
    let after = url
        .split("t.me/")
        .nth(1)
        .or_else(|| url.split("telegram.me/").nth(1))?;
    let name = after.split(['?', '#', '/']).next().unwrap_or(after);
    let name = name.strip_prefix('@').unwrap_or(name);
    if name.is_empty() {
        None
    } else {
        Some(format!("@{name}"))
    }
}

/// Telegram Bot API backend.
#[derive(Debug, Clone)]
pub struct TelegramApiBackend {
    client: reqwest::Client,
    base_url: String,
    token: Option<String>,
}

impl Default for TelegramApiBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
            token: std::env::var("TELEGRAM_BOT_TOKEN").ok(),
        }
    }
}

impl TelegramApiBackend {
    /// Create a backend reading the token from the environment.
    pub fn new() -> Self {
        Self::default()
    }

    /// Point the API at `base` with a test token (tests).
    pub fn with_base_url(base: impl Into<String>) -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: base.into(),
            token: Some("test".to_string()),
        }
    }

    fn token(&self) -> Result<&str, BackendError> {
        self.token
            .as_deref()
            .ok_or_else(|| BackendError::AuthRequired(self.name().to_string()))
    }

    async fn call(&self, method: &str, query: &str) -> Result<serde_json::Value, BackendError> {
        let token = self.token()?;
        let url = format!("{}/bot{}/{}{}", self.base_url, token, method, query);
        let response = self
            .client
            .get(&url)
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
impl Backend for TelegramApiBackend {
    fn name(&self) -> &str {
        "telegram-api"
    }

    async fn probe(&self) -> ProbeResult {
        if self.token.is_some() {
            ProbeResult::ok("telegram-api", "bot")
        } else {
            ProbeResult::warn(
                "telegram-api",
                "no Telegram bot token configured",
                "Set TELEGRAM_BOT_TOKEN",
            )
        }
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let chat = parse_chat(url).ok_or_else(|| {
            BackendError::Parse(
                self.name().to_string(),
                format!("not a Telegram chat URL: {url}"),
            )
        })?;
        let payload = self
            .call(
                "getChat",
                &format!("?chat_id={}", crate::percent_encode(&chat)),
            )
            .await?;
        let result = &payload["result"];
        let title = result["title"]
            .as_str()
            .or_else(|| result["username"].as_str())
            .unwrap_or("");
        let description = result["description"].as_str().unwrap_or("");
        Ok(Content {
            url: url.to_string(),
            title: Some(title.to_string()),
            body: description.to_string(),
            metadata: result.clone(),
            cached: false,
        })
    }

    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let limit = if opts.limit == 0 { 25 } else { opts.limit };
        let payload = self.call("getUpdates", "").await?;
        let needle = query.to_lowercase();
        Ok(payload["result"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|u| {
                let msg = u.get("message").cloned()?;
                let text = msg["text"].as_str()?.to_string();
                if !text.to_lowercase().contains(&needle) {
                    return None;
                }
                let author = msg["from"]["username"].as_str().map(|s| s.to_string());
                Some(SearchResult {
                    title: format!("Message from {}", author.as_deref().unwrap_or("unknown")),
                    url: String::new(),
                    snippet: text.chars().take(280).collect(),
                    author,
                    timestamp: msg["date"].as_i64().map(|d| d.to_string()),
                    metadata: msg,
                })
            })
            .take(limit)
            .collect())
    }
}

/// Telegram channel.
#[derive(Debug, Clone)]
pub struct TelegramChannel {
    router: BackendRouter,
    backend: TelegramApiBackend,
}

impl TelegramChannel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a channel whose backend targets `base` (tests).
    pub fn with_base_url(base: impl Into<String>) -> Self {
        let backend = TelegramApiBackend::with_base_url(base);
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for TelegramChannel {
    fn default() -> Self {
        let backend = TelegramApiBackend::new();
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
impl agentspan_core::channel::Channel for TelegramChannel {
    fn name(&self) -> &str {
        "telegram"
    }

    fn description(&self) -> &str {
        "Read Telegram channel/chat info and search recent updates via the Bot API"
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("t.me/") || url.contains("telegram.me/")
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
    fn can_handle_telegram_urls() {
        let ch = TelegramChannel::new();
        assert!(ch.can_handle("https://t.me/durov"));
        assert!(!ch.can_handle("https://example.com"));
    }

    #[test]
    fn channel_is_tier_one() {
        assert_eq!(TelegramChannel::new().tier(), Tier::One);
    }

    #[test]
    fn parse_chat_builds_username() {
        assert_eq!(parse_chat("https://t.me/durov"), Some("@durov".to_string()));
        assert_eq!(
            parse_chat("https://t.me/@durov"),
            Some("@durov".to_string())
        );
        assert_eq!(parse_chat("https://t.me/"), None);
    }

    #[tokio::test]
    async fn read_returns_chat_info() {
        let server = MockServer::start().await;
        let body = r#"{"ok":true,"result":{"title":"Durov's Channel","description":"news here","username":"durov"}}"#;
        Mock::given(method("GET"))
            .and(path("/bottest/getChat"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = TelegramChannel::with_base_url(server.uri());
        let content = ch
            .read("https://t.me/durov", ReadOptions::default())
            .await
            .unwrap();
        assert_eq!(content.title.as_deref(), Some("Durov's Channel"));
        assert_eq!(content.body, "news here");
    }

    #[tokio::test]
    async fn search_filters_updates_by_keyword() {
        let server = MockServer::start().await;
        let body = r#"{"ok":true,"result":[
            {"update_id":1,"message":{"text":"hello rust","from":{"username":"a"},"date":100}},
            {"update_id":2,"message":{"text":"unrelated","from":{"username":"b"},"date":200}}
        ]}"#;
        Mock::given(method("GET"))
            .and(path("/bottest/getUpdates"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = TelegramChannel::with_base_url(server.uri());
        let results = ch.search("rust", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].snippet, "hello rust");
        assert_eq!(results[0].author.as_deref(), Some("a"));
    }
}
