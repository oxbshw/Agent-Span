//! Flight channel — flight status lookup via the Aviationstack API.
//!
//! Tier 1: needs an `AVIATIONSTACK_KEY`. `search`/`read` take a flight IATA code
//! (e.g. `BA286`) and return current/!recent flight legs.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{
    Content, ProbeResult, ProbeStatus, ReadOptions, SearchOptions, SearchResult, Tier,
};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

const DEFAULT_BASE: &str = "http://api.aviationstack.com";

fn leg_to_result(f: &serde_json::Value) -> SearchResult {
    let iata = f["flight"]["iata"].as_str().unwrap_or("?");
    let airline = f["airline"]["name"].as_str().unwrap_or("");
    let dep = f["departure"]["iata"].as_str().unwrap_or("???");
    let arr = f["arrival"]["iata"].as_str().unwrap_or("???");
    let status = f["flight_status"].as_str().unwrap_or("");
    SearchResult {
        url: format!("https://www.flightradar24.com/{iata}"),
        snippet: format!("{dep} → {arr} ({status})"),
        author: Some(airline.to_string()),
        timestamp: f["flight_date"].as_str().map(|s| s.to_string()),
        title: format!("{airline} {iata}").trim().to_string(),
        metadata: f.clone(),
    }
}

/// Aviationstack backend.
#[derive(Debug, Clone)]
pub struct FlightBackend {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

impl Default for FlightBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
            api_key: std::env::var("AVIATIONSTACK_KEY")
                .ok()
                .filter(|k| !k.is_empty()),
        }
    }
}

impl FlightBackend {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base: impl Into<String>) -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: base.into(),
            api_key: Some("test-key".to_string()),
        }
    }

    fn key(&self) -> Result<&str, BackendError> {
        self.api_key
            .as_deref()
            .ok_or_else(|| BackendError::AuthRequired(self.name().to_string()))
    }

    async fn flights(&self, flight_iata: &str) -> Result<Vec<SearchResult>, BackendError> {
        let url = format!(
            "{}/v1/flights?access_key={}&flight_iata={}",
            self.base_url,
            crate::percent_encode(self.key()?),
            crate::percent_encode(flight_iata.trim())
        );
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
        let payload: serde_json::Value = response
            .json()
            .await
            .map_err(|e| BackendError::Parse(self.name().to_string(), e.to_string()))?;
        let data = payload["data"].as_array().cloned().unwrap_or_default();
        Ok(data.iter().map(leg_to_result).collect())
    }
}

#[async_trait]
impl Backend for FlightBackend {
    fn name(&self) -> &str {
        "aviationstack"
    }

    async fn probe(&self) -> ProbeResult {
        if self.api_key.is_some() {
            ProbeResult::ok("aviationstack", "v1")
        } else {
            ProbeResult {
                status: ProbeStatus::Warn,
                message: "AVIATIONSTACK_KEY not set".to_string(),
                version: None,
                hint: Some("free key at https://aviationstack.com/".to_string()),
            }
        }
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let top = self
            .flights(url)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| BackendError::NotFound(format!("no flight for: {}", url.trim())))?;
        Ok(Content {
            url: top.url.clone(),
            title: Some(top.title.clone()),
            body: format!("{}\n{}", top.title, top.snippet),
            metadata: top.metadata,
            cached: false,
        })
    }

    async fn search(
        &self,
        query: &str,
        _opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        self.flights(query).await
    }
}

/// Flight status channel.
#[derive(Debug, Clone)]
pub struct FlightChannel {
    router: BackendRouter,
    backend: FlightBackend,
}

impl FlightChannel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self::from_backend(FlightBackend::with_base_url(base_url))
    }

    fn from_backend(backend: FlightBackend) -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for FlightChannel {
    fn default() -> Self {
        Self::from_backend(FlightBackend::new())
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for FlightChannel {
    fn name(&self) -> &str {
        "flight"
    }

    fn description(&self) -> &str {
        "Flight status by IATA code via Aviationstack (needs AVIATIONSTACK_KEY)"
    }

    fn format_for_llm(&self, raw: &str) -> String {
        crate::format::extract_text_fields(raw, &["flight", "airline", "flight_status"], 4000)
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("aviationstack.com")
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
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn metadata() {
        let ch = FlightChannel::new();
        assert_eq!(ch.name(), "flight");
        assert_eq!(ch.tier(), Tier::One);
        assert!(ch.can_handle("https://api.aviationstack.com/x"));
        assert!(!ch.can_handle("https://example.com"));
    }

    #[tokio::test]
    async fn probe_warns_without_key() {
        let backend = FlightBackend {
            api_key: None,
            ..FlightBackend::new()
        };
        assert_eq!(backend.probe().await.status, ProbeStatus::Warn);
    }

    #[tokio::test]
    async fn search_maps_flight_legs() {
        let server = MockServer::start().await;
        let body = r#"{"data":[{"flight":{"iata":"BA286"},"airline":{"name":"British Airways"},"departure":{"iata":"SFO"},"arrival":{"iata":"LHR"},"flight_status":"active"}]}"#;
        Mock::given(method("GET"))
            .and(path("/v1/flights"))
            .and(query_param("flight_iata", "BA286"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = FlightChannel::with_base_url(server.uri());
        let results = ch.search("BA286", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].title.contains("BA286"));
        assert!(results[0].snippet.contains("SFO → LHR"));
    }
}
