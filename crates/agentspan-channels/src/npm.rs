//! npm channel — package metadata and search via the public npm registry.
//!
//! Zero-config: the registry at `registry.npmjs.org` needs no auth. `read`
//! resolves a package's latest manifest; `search` hits the registry's v1 search.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

const DEFAULT_BASE: &str = "https://registry.npmjs.org";

/// Pull a package name out of an `npmjs.com/package/<name>` URL, or accept a
/// bare package name passed directly (scoped names like `@scope/pkg` included).
fn parse_package(input: &str) -> Option<String> {
    if let Some(after) = input.split("/package/").nth(1) {
        let name = after
            .split(['?', '#'])
            .next()
            .unwrap_or(after)
            .trim_end_matches('/');
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }
    if !input.contains("://") && !input.trim().is_empty() {
        return Some(input.trim().to_string());
    }
    None
}

/// npm registry HTTP backend.
#[derive(Debug, Clone)]
pub struct NpmRegistryBackend {
    client: reqwest::Client,
    base_url: String,
}

impl Default for NpmRegistryBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
        }
    }
}

impl NpmRegistryBackend {
    pub fn new() -> Self {
        Self::default()
    }

    /// Point the backend at a different base (used by tests).
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
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
}

#[async_trait]
impl Backend for NpmRegistryBackend {
    fn name(&self) -> &str {
        "npm-registry"
    }

    async fn probe(&self) -> ProbeResult {
        ProbeResult::ok("npm-registry", "v1")
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let pkg = parse_package(url).ok_or_else(|| {
            BackendError::Parse(
                self.name().to_string(),
                format!("not an npm package: {url}"),
            )
        })?;
        // `/<pkg>/latest` returns just the latest manifest, not every version.
        let manifest = self
            .get_json(&format!("{}/{}/latest", self.base_url, pkg))
            .await?;
        let name = manifest["name"].as_str().unwrap_or(&pkg);
        let version = manifest["version"].as_str().unwrap_or("?");
        let description = manifest["description"].as_str().unwrap_or("");
        let homepage = manifest["homepage"].as_str().unwrap_or("");
        let body = format!("{name}@{version}\n{description}\n{homepage}")
            .trim()
            .to_string();
        Ok(Content {
            url: url.to_string(),
            title: Some(format!("{name}@{version}")),
            body,
            metadata: manifest,
            cached: false,
        })
    }

    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let size = if opts.limit == 0 {
            10
        } else {
            opts.limit.min(50)
        };
        let url = format!(
            "{}/-/v1/search?text={}&size={}",
            self.base_url,
            crate::percent_encode(query),
            size
        );
        let payload = self.get_json(&url).await?;
        let objects = payload["objects"].as_array().cloned().unwrap_or_default();
        Ok(objects
            .into_iter()
            .map(|o| {
                let pkg = &o["package"];
                let name = pkg["name"].as_str().unwrap_or("").to_string();
                SearchResult {
                    url: pkg["links"]["npm"]
                        .as_str()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| format!("https://www.npmjs.com/package/{name}")),
                    snippet: pkg["description"].as_str().unwrap_or("").to_string(),
                    author: pkg["publisher"]["username"].as_str().map(|s| s.to_string()),
                    timestamp: pkg["date"].as_str().map(|s| s.to_string()),
                    title: name,
                    metadata: o,
                }
            })
            .collect())
    }
}

/// npm channel.
#[derive(Debug, Clone)]
pub struct NpmChannel {
    router: BackendRouter,
    backend: NpmRegistryBackend,
}

impl NpmChannel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a channel whose backend targets `base_url` (tests).
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self::from_backend(NpmRegistryBackend::new().with_base_url(base_url))
    }

    fn from_backend(backend: NpmRegistryBackend) -> Self {
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for NpmChannel {
    fn default() -> Self {
        Self::from_backend(NpmRegistryBackend::new())
    }
}

#[async_trait]
impl agentspan_core::channel::Channel for NpmChannel {
    fn name(&self) -> &str {
        "npm"
    }

    fn description(&self) -> &str {
        "Look up npm package metadata and search the registry"
    }

    fn format_for_llm(&self, raw: &str) -> String {
        crate::format::extract_text_fields(
            raw,
            &["name", "version", "description", "homepage"],
            6000,
        )
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("npmjs.com/package/")
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
    fn parse_package_handles_urls_and_bare_names() {
        assert_eq!(
            parse_package("https://www.npmjs.com/package/left-pad"),
            Some("left-pad".to_string())
        );
        assert_eq!(
            parse_package("https://www.npmjs.com/package/@babel/core?activeTab=versions"),
            Some("@babel/core".to_string())
        );
        assert_eq!(parse_package("lodash"), Some("lodash".to_string()));
        assert_eq!(parse_package("https://example.com"), None);
    }

    #[test]
    fn can_handle_npm_urls() {
        let ch = NpmChannel::new();
        assert!(ch.can_handle("https://www.npmjs.com/package/express"));
        assert!(!ch.can_handle("https://crates.io/crates/serde"));
    }

    #[test]
    fn channel_is_tier_zero() {
        assert_eq!(NpmChannel::new().tier(), Tier::Zero);
        assert_eq!(NpmChannel::new().name(), "npm");
    }

    #[tokio::test]
    async fn read_returns_latest_manifest() {
        let server = MockServer::start().await;
        let body = r#"{"name":"left-pad","version":"1.3.0","description":"pad a string","homepage":"https://example.com/lp"}"#;
        Mock::given(method("GET"))
            .and(path("/left-pad/latest"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = NpmChannel::with_base_url(server.uri());
        let content = ch
            .read(
                "https://www.npmjs.com/package/left-pad",
                ReadOptions::default(),
            )
            .await
            .unwrap();
        assert_eq!(content.title.as_deref(), Some("left-pad@1.3.0"));
        assert!(content.body.contains("pad a string"));
    }

    #[tokio::test]
    async fn search_maps_objects() {
        let server = MockServer::start().await;
        let body = r#"{"objects":[{"package":{"name":"express","description":"web framework","links":{"npm":"https://www.npmjs.com/package/express"}}}]}"#;
        Mock::given(method("GET"))
            .and(path("/-/v1/search"))
            .and(query_param("text", "express"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ch = NpmChannel::with_base_url(server.uri());
        let results = ch
            .search("express", SearchOptions::default())
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "express");
        assert_eq!(results[0].snippet, "web framework");
    }

    #[tokio::test]
    async fn read_rejects_non_package_url() {
        let ch = NpmChannel::with_base_url("http://127.0.0.1:0");
        let err = ch
            .read("https://example.com/not-a-pkg", ReadOptions::default())
            .await;
        assert!(err.is_err());
    }
}
