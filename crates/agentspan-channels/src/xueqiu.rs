//! Xueqiu (雪球) channel — stock quotes, search, and hot stocks.
//!
//! Uses Xueqiu's public JSON endpoints. Authenticated endpoints need an
//! `xq_a_token` cookie, read from the config `cookies.xueqiu` entry.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

const STOCK_BASE: &str = "https://stock.xueqiu.com";
const WEB_BASE: &str = "https://xueqiu.com";

/// Extract a stock symbol from a Xueqiu URL (`/S/SH600519`).
fn parse_symbol(url: &str) -> Option<String> {
    url.split("/S/")
        .nth(1)
        .map(|s| s.split(['/', '?', '#']).next().unwrap_or("").to_string())
        .filter(|s| !s.is_empty())
}

/// Xueqiu public-API backend.
#[derive(Debug, Clone)]
pub struct XueqiuApiBackend {
    client: reqwest::Client,
    stock_base: String,
    web_base: String,
    cookie: Option<String>,
}

impl Default for XueqiuApiBackend {
    fn default() -> Self {
        let cookie = agentspan_core::Config::load()
            .ok()
            .and_then(|c| c.cookies.get("xueqiu").cloned());
        Self {
            client: crate::http::default_client(),
            stock_base: STOCK_BASE.to_string(),
            web_base: WEB_BASE.to_string(),
            cookie,
        }
    }
}

impl XueqiuApiBackend {
    /// Create a backend reading the Xueqiu cookie from config.
    pub fn new() -> Self {
        Self::default()
    }

    /// Point both bases at one URL (tests).
    pub fn with_base_url(mut self, base: impl Into<String>) -> Self {
        let base = base.into();
        self.stock_base = base.clone();
        self.web_base = base;
        self
    }

    /// Set the auth cookie (tests).
    pub fn with_cookie(mut self, cookie: impl Into<String>) -> Self {
        self.cookie = Some(cookie.into());
        self
    }

    async fn get_json(&self, url: &str) -> Result<serde_json::Value, BackendError> {
        let mut req = self.client.get(url).header("Referer", WEB_BASE);
        if let Some(cookie) = &self.cookie {
            req = req.header("Cookie", cookie);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| BackendError::RequestFailed(self.name().to_string(), e.to_string()))?;
        if !resp.status().is_success() {
            return Err(BackendError::RequestFailed(
                self.name().to_string(),
                format!("HTTP {}", resp.status()),
            ));
        }
        resp.json()
            .await
            .map_err(|e| BackendError::Parse(self.name().to_string(), e.to_string()))
    }
}

#[async_trait]
impl Backend for XueqiuApiBackend {
    fn name(&self) -> &str {
        "xueqiu-api"
    }

    async fn probe(&self) -> ProbeResult {
        if self.cookie.is_some() {
            ProbeResult::ok("xueqiu-api", "cookie configured")
        } else {
            ProbeResult::warn(
                "xueqiu-api",
                "no xueqiu cookie configured",
                "Run: agentspan config from-browser chrome (after logging into xueqiu.com)",
            )
        }
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let symbol = parse_symbol(url).ok_or_else(|| {
            BackendError::Parse(
                self.name().to_string(),
                format!("no stock symbol in URL: {url}"),
            )
        })?;
        let api = format!(
            "{}/v5/stock/batch/quote.json?symbol={}",
            self.stock_base, symbol
        );
        let payload = self.get_json(&api).await?;
        let quote = payload["data"]["items"][0]["quote"].clone();
        let name = quote["name"].as_str().unwrap_or(&symbol).to_string();
        Ok(Content {
            url: url.to_string(),
            title: Some(format!("{name} ({symbol})")),
            body: serde_json::to_string_pretty(&quote).unwrap_or_default(),
            metadata: payload,
            cached: false,
        })
    }

    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let limit = if opts.limit == 0 { 10 } else { opts.limit };
        let api = format!(
            "{}/stock/search.json?code={}&size={}",
            self.web_base,
            crate::percent_encode(query),
            limit
        );
        let payload = self.get_json(&api).await?;
        Ok(payload["stocks"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .take(limit)
            .map(|s| SearchResult {
                title: format!(
                    "{} ({})",
                    s["name"].as_str().unwrap_or(""),
                    s["code"].as_str().unwrap_or("")
                ),
                url: format!("https://xueqiu.com/S/{}", s["code"].as_str().unwrap_or("")),
                snippet: s["stock_type"].as_str().unwrap_or("").to_string(),
                author: None,
                timestamp: None,
                metadata: s.clone(),
            })
            .collect())
    }
}

/// Xueqiu channel.
#[derive(Debug, Clone)]
pub struct XueqiuChannel {
    router: BackendRouter,
    backend: XueqiuApiBackend,
}

impl XueqiuChannel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Test channel pointed at `base` with a cookie.
    pub fn with_base_url(base: impl Into<String>) -> Self {
        let backend = XueqiuApiBackend::new()
            .with_base_url(base)
            .with_cookie("xq_a_token=t");
        let router = BackendRouter::new(
            vec![Arc::new(backend.clone()) as Arc<dyn Backend>],
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for XueqiuChannel {
    fn default() -> Self {
        let backend = XueqiuApiBackend::new();
        let router = BackendRouter::new(
            vec![Arc::new(backend.clone()) as Arc<dyn Backend>],
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for XueqiuChannel {
    fn name(&self) -> &str {
        "xueqiu"
    }

    fn description(&self) -> &str {
        "Stock quotes, stock search, and hot stocks via the Xueqiu public API"
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("xueqiu.com")
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
    fn can_handle_xueqiu_urls() {
        assert!(XueqiuChannel::new().can_handle("https://xueqiu.com/S/SH600519"));
        assert!(!XueqiuChannel::new().can_handle("https://example.com"));
    }

    #[test]
    fn channel_is_tier_one() {
        assert_eq!(XueqiuChannel::new().tier(), Tier::One);
    }

    #[test]
    fn parse_symbol_from_url() {
        assert_eq!(
            parse_symbol("https://xueqiu.com/S/SH600519"),
            Some("SH600519".to_string())
        );
        assert_eq!(parse_symbol("https://xueqiu.com/today"), None);
    }

    #[tokio::test]
    async fn read_fetches_quote() {
        let server = MockServer::start().await;
        let body = r#"{"data":{"items":[{"quote":{"name":"贵州茅台","current":1680.0}}]}}"#;
        Mock::given(method("GET"))
            .and(path("/v5/stock/batch/quote.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = XueqiuChannel::with_base_url(server.uri());
        let content = ch
            .read("https://xueqiu.com/S/SH600519", ReadOptions::default())
            .await
            .unwrap();
        assert!(content.title.unwrap().contains("贵州茅台"));
    }

    #[tokio::test]
    async fn search_maps_stocks() {
        let server = MockServer::start().await;
        let body = r#"{"stocks":[{"code":"SH600519","name":"贵州茅台","stock_type":"11"}]}"#;
        Mock::given(method("GET"))
            .and(path("/stock/search.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = XueqiuChannel::with_base_url(server.uri());
        let results = ch.search("茅台", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].title.contains("SH600519"));
    }
}
