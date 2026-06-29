//! Maps channel — geocoding via OpenStreetMap's Nominatim service.
//!
//! No key required (Nominatim asks for a `User-Agent`, which our default client
//! sends). `search` geocodes a place query to candidate locations; `read`
//! returns the single best match for a query.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

const DEFAULT_BASE: &str = "https://nominatim.openstreetmap.org";

fn place_to_result(place: &serde_json::Value) -> SearchResult {
    let display = place["display_name"].as_str().unwrap_or("").to_string();
    let lat = place["lat"].as_str().unwrap_or("");
    let lon = place["lon"].as_str().unwrap_or("");
    let kind = place["type"].as_str().unwrap_or("place");
    SearchResult {
        url: format!("https://www.openstreetmap.org/?mlat={lat}&mlon={lon}"),
        snippet: format!("{kind} @ {lat},{lon}"),
        author: None,
        timestamp: None,
        title: display,
        metadata: place.clone(),
    }
}

/// Nominatim geocoding backend.
#[derive(Debug, Clone)]
pub struct NominatimBackend {
    client: reqwest::Client,
    base_url: String,
}

impl Default for NominatimBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
        }
    }
}

impl NominatimBackend {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    async fn geocode(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<serde_json::Value>, BackendError> {
        let url = format!(
            "{}/search?q={}&format=json&limit={}",
            self.base_url,
            crate::percent_encode(query),
            limit
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
        response
            .json::<Vec<serde_json::Value>>()
            .await
            .map_err(|e| BackendError::Parse(self.name().to_string(), e.to_string()))
    }
}

#[async_trait]
impl Backend for NominatimBackend {
    fn name(&self) -> &str {
        "nominatim"
    }

    async fn probe(&self) -> ProbeResult {
        ProbeResult::ok("nominatim", "v1")
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        // `url` is treated as a place query (or an OSM link's query string).
        let query = url.trim();
        let place = self
            .geocode(query, 1)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| BackendError::NotFound(format!("no location for: {query}")))?;
        let display = place["display_name"].as_str().unwrap_or(query);
        let lat = place["lat"].as_str().unwrap_or("");
        let lon = place["lon"].as_str().unwrap_or("");
        Ok(Content {
            url: url.to_string(),
            title: Some(display.to_string()),
            body: format!("{display}\nlat {lat}, lon {lon}"),
            metadata: place,
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
        let places = self.geocode(query, limit).await?;
        Ok(places.iter().map(place_to_result).collect())
    }
}

/// Maps (geocoding) channel.
#[derive(Debug, Clone)]
pub struct MapsChannel {
    router: BackendRouter,
    backend: NominatimBackend,
}

impl MapsChannel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self::from_backend(NominatimBackend::new().with_base_url(base_url))
    }

    fn from_backend(backend: NominatimBackend) -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for MapsChannel {
    fn default() -> Self {
        Self::from_backend(NominatimBackend::new())
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for MapsChannel {
    fn name(&self) -> &str {
        "maps"
    }

    fn description(&self) -> &str {
        "Geocode places via OpenStreetMap Nominatim"
    }

    fn format_for_llm(&self, raw: &str) -> String {
        crate::format::extract_text_fields(raw, &["display_name", "lat", "lon", "type"], 4000)
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("openstreetmap.org/") || url.contains("nominatim.")
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
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn can_handle_and_metadata() {
        let ch = MapsChannel::new();
        assert!(ch.can_handle("https://www.openstreetmap.org/?mlat=1&mlon=2"));
        assert!(!ch.can_handle("https://example.com"));
        assert_eq!(ch.name(), "maps");
        assert_eq!(ch.tier(), Tier::Zero);
    }

    #[tokio::test]
    async fn search_geocodes_places() {
        let server = MockServer::start().await;
        let body =
            r#"[{"display_name":"Berlin, Germany","lat":"52.52","lon":"13.40","type":"city"}]"#;
        Mock::given(method("GET"))
            .and(path("/search"))
            .and(query_param("q", "berlin"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = MapsChannel::with_base_url(server.uri());
        let results = ch.search("berlin", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Berlin, Germany");
        assert!(results[0].snippet.contains("52.52"));
    }

    #[tokio::test]
    async fn read_returns_top_match() {
        let server = MockServer::start().await;
        let body = r#"[{"display_name":"Paris, France","lat":"48.85","lon":"2.35","type":"city"}]"#;
        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = MapsChannel::with_base_url(server.uri());
        let content = ch.read("Paris", ReadOptions::default()).await.unwrap();
        assert_eq!(content.title.as_deref(), Some("Paris, France"));
        assert!(content.body.contains("48.85"));
    }

    #[tokio::test]
    async fn read_no_match_errors() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_string("[]"))
            .mount(&server)
            .await;

        let ch = MapsChannel::with_base_url(server.uri());
        assert!(ch
            .read("nowhere-xyz", ReadOptions::default())
            .await
            .is_err());
    }
}
