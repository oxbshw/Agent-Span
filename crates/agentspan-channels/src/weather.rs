//! Weather channel — current conditions via the free Open-Meteo API.
//!
//! No key required. `search` geocodes a place name to candidate locations;
//! `read` geocodes the place and then fetches its current weather. Geocoding and
//! forecast live on different Open-Meteo hosts, so the backend keeps two bases
//! (a single `with_base_url` points both at one mock server for tests).

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

const GEO_BASE: &str = "https://geocoding-api.open-meteo.com";
const FORECAST_BASE: &str = "https://api.open-meteo.com";

/// Open-Meteo backend (geocoding + forecast).
#[derive(Debug, Clone)]
pub struct OpenMeteoBackend {
    client: reqwest::Client,
    geo_base: String,
    forecast_base: String,
}

impl Default for OpenMeteoBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            geo_base: GEO_BASE.to_string(),
            forecast_base: FORECAST_BASE.to_string(),
        }
    }
}

impl OpenMeteoBackend {
    pub fn new() -> Self {
        Self::default()
    }

    /// Point both the geocoding and forecast bases at one URL (tests).
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        let base = base_url.into();
        self.geo_base = base.clone();
        self.forecast_base = base;
        self
    }

    async fn get_json(&self, url: &str) -> Result<serde_json::Value, BackendError> {
        let response = self
            .client
            .get(url)
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

    async fn geocode(
        &self,
        place: &str,
        count: usize,
    ) -> Result<Vec<serde_json::Value>, BackendError> {
        let url = format!(
            "{}/v1/search?name={}&count={}",
            self.geo_base,
            crate::percent_encode(place),
            count
        );
        let payload = self.get_json(&url).await?;
        Ok(payload["results"].as_array().cloned().unwrap_or_default())
    }
}

#[async_trait]
impl Backend for OpenMeteoBackend {
    fn name(&self) -> &str {
        "open-meteo"
    }

    async fn probe(&self) -> ProbeResult {
        ProbeResult::ok("open-meteo", "v1")
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let place = url.trim();
        let location = self
            .geocode(place, 1)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| BackendError::NotFound(format!("no location for: {place}")))?;
        let name = location["name"].as_str().unwrap_or(place);
        let country = location["country"].as_str().unwrap_or("");
        let lat = location["latitude"].as_f64().unwrap_or(0.0);
        let lon = location["longitude"].as_f64().unwrap_or(0.0);

        let forecast = self
            .get_json(&format!(
                "{}/v1/forecast?latitude={lat}&longitude={lon}&current_weather=true",
                self.forecast_base
            ))
            .await?;
        let cw = &forecast["current_weather"];
        let temp = cw["temperature"].as_f64().unwrap_or(f64::NAN);
        let wind = cw["windspeed"].as_f64().unwrap_or(f64::NAN);
        Ok(Content {
            url: url.to_string(),
            title: Some(format!("Weather: {name}, {country}")),
            body: format!("{name}, {country}\ntemperature {temp} °C, wind {wind} km/h"),
            metadata: forecast,
            cached: false,
        })
    }

    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let count = if opts.limit == 0 {
            5
        } else {
            opts.limit.min(50)
        };
        let places = self.geocode(query, count).await?;
        Ok(places
            .into_iter()
            .map(|p| {
                let name = p["name"].as_str().unwrap_or("").to_string();
                let country = p["country"].as_str().unwrap_or("");
                let lat = p["latitude"].as_f64().unwrap_or(0.0);
                let lon = p["longitude"].as_f64().unwrap_or(0.0);
                SearchResult {
                    url: format!("https://open-meteo.com/en/docs#latitude={lat}&longitude={lon}"),
                    snippet: format!("{lat},{lon}"),
                    author: None,
                    timestamp: None,
                    title: format!("{name}, {country}"),
                    metadata: p,
                }
            })
            .collect())
    }
}

/// Weather channel.
#[derive(Debug, Clone)]
pub struct WeatherChannel {
    router: BackendRouter,
    backend: OpenMeteoBackend,
}

impl WeatherChannel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self::from_backend(OpenMeteoBackend::new().with_base_url(base_url))
    }

    fn from_backend(backend: OpenMeteoBackend) -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for WeatherChannel {
    fn default() -> Self {
        Self::from_backend(OpenMeteoBackend::new())
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for WeatherChannel {
    fn name(&self) -> &str {
        "weather"
    }

    fn description(&self) -> &str {
        "Current weather for a place via the free Open-Meteo API"
    }

    fn format_for_llm(&self, raw: &str) -> String {
        crate::format::extract_text_fields(raw, &["temperature", "windspeed", "name"], 2000)
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("open-meteo.com")
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
    fn can_handle_and_metadata() {
        let ch = WeatherChannel::new();
        assert!(ch.can_handle("https://open-meteo.com/en/docs"));
        assert!(!ch.can_handle("https://example.com"));
        assert_eq!(ch.name(), "weather");
        assert_eq!(ch.tier(), Tier::Zero);
    }

    #[tokio::test]
    async fn search_geocodes() {
        let server = MockServer::start().await;
        let body = r#"{"results":[{"name":"Berlin","country":"Germany","latitude":52.52,"longitude":13.40}]}"#;
        Mock::given(method("GET"))
            .and(path("/v1/search"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = WeatherChannel::with_base_url(server.uri());
        let results = ch.search("berlin", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Berlin, Germany");
    }

    #[tokio::test]
    async fn read_geocodes_then_forecasts() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/search"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"results":[{"name":"Berlin","country":"Germany","latitude":52.52,"longitude":13.40}]}"#,
            ))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/forecast"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"current_weather":{"temperature":15.0,"windspeed":10.0,"weathercode":1}}"#,
            ))
            .mount(&server)
            .await;

        let ch = WeatherChannel::with_base_url(server.uri());
        let content = ch.read("Berlin", ReadOptions::default()).await.unwrap();
        assert!(content.title.as_deref().unwrap().contains("Berlin"));
        assert!(content.body.contains("15"));
    }

    #[tokio::test]
    async fn read_unknown_place_errors() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/search"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"results":[]}"#))
            .mount(&server)
            .await;

        let ch = WeatherChannel::with_base_url(server.uri());
        assert!(ch
            .read("nowhere-xyz", ReadOptions::default())
            .await
            .is_err());
    }
}
