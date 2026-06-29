//! Coinbase channel — spot cryptocurrency prices.
//!
//! Uses Coinbase's public `v2/prices/<pair>/spot` endpoint (no key). A bare
//! currency like `BTC` is normalised to the `BTC-USD` pair. Both `read` and
//! `search` resolve a price — there's no listing endpoint to "search" over.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

const DEFAULT_BASE: &str = "https://api.coinbase.com";

/// Normalise an input to a `BASE-QUOTE` trading pair (defaulting quote to USD).
///
/// For a `coinbase.com/...` URL the last path segment is used as the asset.
/// Note: marketing slugs like `/price/solana` aren't real tickers — this is
/// best-effort; the reliable input is a ticker/pair such as `BTC` or `ETH-EUR`.
fn parse_pair(input: &str) -> String {
    let raw = if let Some(after) = input.split("coinbase.com/").nth(1) {
        let path = after
            .split(['?', '#'])
            .next()
            .unwrap_or(after)
            .trim_matches('/');
        path.rsplit('/').next().unwrap_or(path).to_string()
    } else {
        input.trim().to_string()
    };
    let raw = raw.to_uppercase();
    if raw.is_empty() {
        "BTC-USD".to_string()
    } else if raw.contains('-') {
        raw
    } else {
        format!("{raw}-USD")
    }
}

/// Coinbase prices backend.
#[derive(Debug, Clone)]
pub struct CoinbaseBackend {
    client: reqwest::Client,
    base_url: String,
}

impl Default for CoinbaseBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
        }
    }
}

impl CoinbaseBackend {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    async fn spot(&self, pair: &str) -> Result<serde_json::Value, BackendError> {
        let url = format!("{}/v2/prices/{}/spot", self.base_url, pair);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| BackendError::RequestFailed(self.name().to_string(), e.to_string()))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(BackendError::NotFound(format!("unknown pair: {pair}")));
        }
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
impl Backend for CoinbaseBackend {
    fn name(&self) -> &str {
        "coinbase"
    }

    async fn probe(&self) -> ProbeResult {
        ProbeResult::ok("coinbase", "v2")
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let pair = parse_pair(url);
        let payload = self.spot(&pair).await?;
        let amount = payload["data"]["amount"].as_str().unwrap_or("?");
        let currency = payload["data"]["currency"].as_str().unwrap_or("");
        Ok(Content {
            url: url.to_string(),
            title: Some(format!("{pair} spot")),
            body: format!("{pair}: {amount} {currency}"),
            metadata: payload,
            cached: false,
        })
    }

    async fn search(
        &self,
        query: &str,
        _opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let pair = parse_pair(query);
        let payload = self.spot(&pair).await?;
        let amount = payload["data"]["amount"].as_str().unwrap_or("?");
        let currency = payload["data"]["currency"].as_str().unwrap_or("");
        Ok(vec![SearchResult {
            url: format!("https://www.coinbase.com/price/{}", pair.to_lowercase()),
            snippet: format!("{amount} {currency}"),
            author: None,
            timestamp: None,
            title: format!("{pair} spot price"),
            metadata: payload,
        }])
    }
}

/// Coinbase channel.
#[derive(Debug, Clone)]
pub struct CoinbaseChannel {
    router: BackendRouter,
    backend: CoinbaseBackend,
}

impl CoinbaseChannel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self::from_backend(CoinbaseBackend::new().with_base_url(base_url))
    }

    fn from_backend(backend: CoinbaseBackend) -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for CoinbaseChannel {
    fn default() -> Self {
        Self::from_backend(CoinbaseBackend::new())
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for CoinbaseChannel {
    fn name(&self) -> &str {
        "coinbase"
    }

    fn description(&self) -> &str {
        "Look up spot cryptocurrency prices via Coinbase"
    }

    fn format_for_llm(&self, raw: &str) -> String {
        crate::format::extract_text_fields(raw, &["amount", "base", "currency"], 2000)
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("coinbase.com/")
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
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn parse_pair_normalises() {
        assert_eq!(parse_pair("BTC"), "BTC-USD");
        assert_eq!(parse_pair("eth-eur"), "ETH-EUR");
        // The last path segment is taken as the asset.
        assert_eq!(parse_pair("https://www.coinbase.com/price/ada"), "ADA-USD");
        assert_eq!(
            parse_pair("https://www.coinbase.com/advanced-trade/BTC-USD"),
            "BTC-USD"
        );
        assert_eq!(parse_pair(""), "BTC-USD");
    }

    #[test]
    fn can_handle_and_metadata() {
        let ch = CoinbaseChannel::new();
        assert!(ch.can_handle("https://www.coinbase.com/price/bitcoin"));
        assert!(!ch.can_handle("https://example.com"));
        assert_eq!(ch.tier(), Tier::Zero);
    }

    #[tokio::test]
    async fn read_returns_spot_price() {
        let server = MockServer::start().await;
        let body = r#"{"data":{"amount":"43000.00","base":"BTC","currency":"USD"}}"#;
        Mock::given(method("GET"))
            .and(path("/v2/prices/BTC-USD/spot"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = CoinbaseChannel::with_base_url(server.uri());
        let content = ch.read("BTC", ReadOptions::default()).await.unwrap();
        assert!(content.body.contains("43000.00"));
        assert_eq!(content.title.as_deref(), Some("BTC-USD spot"));
    }

    #[tokio::test]
    async fn search_returns_one_price_result() {
        let server = MockServer::start().await;
        let body = r#"{"data":{"amount":"2500.00","base":"ETH","currency":"USD"}}"#;
        Mock::given(method("GET"))
            .and(path("/v2/prices/ETH-USD/spot"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = CoinbaseChannel::with_base_url(server.uri());
        let results = ch.search("ETH", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].snippet.contains("2500.00"));
    }
}
