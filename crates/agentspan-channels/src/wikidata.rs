//! Wikidata channel — structured-knowledge entity search.
//!
//! Tier 0: no key. `search` uses `wbsearchentities`; `read` resolves an entity
//! by its `wikidata.org/wiki/Q<id>` URL via Special:EntityData.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

const DEFAULT_BASE: &str = "https://www.wikidata.org";

/// Wikidata backend.
#[derive(Debug, Clone)]
pub struct WikidataBackend {
    client: reqwest::Client,
    base_url: String,
}

impl Default for WikidataBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
        }
    }
}

impl WikidataBackend {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base: impl Into<String>) -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: base.into(),
        }
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

    /// Extract a `Q<n>` (or `P<n>`) entity id from a Wikidata URL.
    fn parse_entity(url: &str) -> Option<String> {
        let tail = url.rsplit(['/', ':']).next()?;
        let mut chars = tail.chars();
        let first = chars.next()?;
        if matches!(first, 'Q' | 'P')
            && chars.clone().count() > 0
            && chars.all(|c| c.is_ascii_digit())
        {
            Some(tail.to_string())
        } else {
            None
        }
    }
}

#[async_trait]
impl Backend for WikidataBackend {
    fn name(&self) -> &str {
        "wikidata-api"
    }

    async fn probe(&self) -> ProbeResult {
        ProbeResult::ok("wikidata-api", "v1")
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let id = Self::parse_entity(url)
            .ok_or_else(|| BackendError::NotFound(format!("no Wikidata entity id in: {url}")))?;
        let obj = self
            .get_json(&format!(
                "{}/wiki/Special:EntityData/{}.json",
                self.base_url, id
            ))
            .await?;
        let entity = &obj["entities"][&id];
        let title = entity["labels"]["en"]["value"]
            .as_str()
            .unwrap_or(&id)
            .to_string();
        let desc = entity["descriptions"]["en"]["value"]
            .as_str()
            .unwrap_or("")
            .to_string();
        Ok(Content {
            url: format!("{}/wiki/{}", self.base_url, id),
            title: Some(title.clone()),
            body: format!("{title}\n{desc}"),
            metadata: entity.clone(),
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
        let url = format!(
            "{}/w/api.php?action=wbsearchentities&search={}&language=en&format=json&limit={}",
            self.base_url,
            crate::percent_encode(query.trim()),
            limit
        );
        let payload = self.get_json(&url).await?;
        let hits = payload["search"].as_array().cloned().unwrap_or_default();
        Ok(hits
            .into_iter()
            .map(|h| SearchResult {
                url: h["concepturi"]
                    .as_str()
                    .or_else(|| h["url"].as_str())
                    .unwrap_or("")
                    .to_string(),
                snippet: h["description"].as_str().unwrap_or("").to_string(),
                author: None,
                timestamp: None,
                title: h["label"].as_str().unwrap_or("").to_string(),
                metadata: h,
            })
            .collect())
    }
}

/// Wikidata channel.
#[derive(Debug, Clone)]
pub struct WikidataChannel {
    router: BackendRouter,
    backend: WikidataBackend,
}

impl WikidataChannel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self::from_backend(WikidataBackend::with_base_url(base_url))
    }

    fn from_backend(backend: WikidataBackend) -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for WikidataChannel {
    fn default() -> Self {
        Self::from_backend(WikidataBackend::new())
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for WikidataChannel {
    fn name(&self) -> &str {
        "wikidata"
    }

    fn description(&self) -> &str {
        "Structured-knowledge entity search via Wikidata"
    }

    fn format_for_llm(&self, raw: &str) -> String {
        crate::format::extract_text_fields(raw, &["label", "description", "value"], 6000)
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("wikidata.org/")
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
    fn metadata_and_entity_parsing() {
        let ch = WikidataChannel::new();
        assert_eq!(ch.name(), "wikidata");
        assert_eq!(ch.tier(), Tier::Zero);
        assert!(ch.can_handle("https://www.wikidata.org/wiki/Q42"));
        assert_eq!(
            WikidataBackend::parse_entity("https://www.wikidata.org/wiki/Q42"),
            Some("Q42".to_string())
        );
        assert_eq!(
            WikidataBackend::parse_entity("https://www.wikidata.org/wiki/Help"),
            None
        );
    }

    #[tokio::test]
    async fn search_maps_entities() {
        let server = MockServer::start().await;
        let body = r#"{"search":[{"id":"Q42","label":"Douglas Adams","description":"English author","concepturi":"http://www.wikidata.org/entity/Q42"}]}"#;
        Mock::given(method("GET"))
            .and(path("/w/api.php"))
            .and(query_param("action", "wbsearchentities"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = WikidataChannel::with_base_url(server.uri());
        let results = ch.search("adams", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Douglas Adams");
        assert_eq!(results[0].snippet, "English author");
    }

    #[tokio::test]
    async fn read_resolves_entity_label() {
        let server = MockServer::start().await;
        let body = r#"{"entities":{"Q42":{"labels":{"en":{"value":"Douglas Adams"}},"descriptions":{"en":{"value":"author"}}}}}"#;
        Mock::given(method("GET"))
            .and(path("/wiki/Special:EntityData/Q42.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = WikidataChannel::with_base_url(server.uri());
        let content = ch
            .read("https://www.wikidata.org/wiki/Q42", ReadOptions::default())
            .await
            .unwrap();
        assert_eq!(content.title.as_deref(), Some("Douglas Adams"));
        assert!(content.body.contains("author"));
    }
}
