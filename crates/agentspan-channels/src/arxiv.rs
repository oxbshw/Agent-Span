//! arXiv channel — backed by the free arXiv Atom export API (no key).

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use agentspan_core::backend::Backend;
use agentspan_core::error::{BackendError, ChannelError};
use agentspan_core::types::{Content, ProbeResult, ReadOptions, SearchOptions, SearchResult, Tier};
use agentspan_probe::ProbeEngine;
use agentspan_router::retry::RetryConfig;
use agentspan_router::BackendRouter;

const DEFAULT_BASE: &str = "http://export.arxiv.org/api/query";

/// Extract the arXiv id (e.g. `2401.12345`) from an abstract/pdf URL.
fn parse_arxiv_id(url: &str) -> Option<String> {
    let after = url
        .split("/abs/")
        .nth(1)
        .or_else(|| url.split("/pdf/").nth(1))?;
    let id = after.split(['?', '#']).next().unwrap_or(after);
    let id = id.strip_suffix(".pdf").unwrap_or(id);
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

/// Minimal XML-entity unescape for the fields arXiv returns.
fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Return the trimmed inner text of the first `<tag>…</tag>` in `s`.
fn first_tag(s: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = s.find(&open)? + open.len();
    let end = s[start..].find(&close)? + start;
    Some(unescape(s[start..end].trim()))
}

/// Parse the `<entry>` blocks of an arXiv Atom feed into search results.
fn parse_entries(xml: &str) -> Vec<SearchResult> {
    let mut out = Vec::new();
    // Skip the feed header; entries start after the first "<entry>".
    for chunk in xml.split("<entry>").skip(1) {
        let entry = chunk.split("</entry>").next().unwrap_or(chunk);
        let title = first_tag(entry, "title").unwrap_or_default();
        let summary = first_tag(entry, "summary").unwrap_or_default();
        let id = first_tag(entry, "id").unwrap_or_default();
        let published = first_tag(entry, "published");
        // The first <name> inside an entry is the primary author.
        let author = first_tag(entry, "name");
        out.push(SearchResult {
            title: title.split_whitespace().collect::<Vec<_>>().join(" "),
            url: id,
            snippet: summary
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .take(400)
                .collect(),
            author,
            timestamp: published,
            metadata: serde_json::Value::Null,
        });
    }
    out
}

/// arXiv Atom export API backend.
#[derive(Debug, Clone)]
pub struct ArxivApiBackend {
    client: reqwest::Client,
    base_url: String,
}

impl Default for ArxivApiBackend {
    fn default() -> Self {
        Self {
            client: crate::http::default_client(),
            base_url: DEFAULT_BASE.to_string(),
        }
    }
}

impl ArxivApiBackend {
    /// Create a backend pointed at the public arXiv API.
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the base URL (used in tests against a mock server).
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    async fn get_text(&self, api: &str) -> Result<String, BackendError> {
        let response = self
            .client
            .get(api)
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
            .text()
            .await
            .map_err(|e| BackendError::Parse(self.name().to_string(), e.to_string()))
    }
}

#[async_trait]
impl Backend for ArxivApiBackend {
    fn name(&self) -> &str {
        "arxiv-api"
    }

    async fn probe(&self) -> ProbeResult {
        ProbeResult::ok("arxiv-api", "atom")
    }

    async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
        let id = parse_arxiv_id(url).ok_or_else(|| {
            BackendError::Parse(
                self.name().to_string(),
                format!("not an arXiv paper URL: {url}"),
            )
        })?;
        let api = format!("{}?id_list={}&max_results=1", self.base_url, id);
        let xml = self.get_text(&api).await?;
        let entry = parse_entries(&xml)
            .into_iter()
            .next()
            .ok_or_else(|| BackendError::NotFound(self.name().to_string()))?;
        Ok(Content {
            url: url.to_string(),
            title: Some(entry.title),
            body: entry.snippet,
            metadata: serde_json::json!({
                "author": entry.author,
                "published": entry.timestamp,
                "arxiv_id": id,
            }),
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
        let api = format!(
            "{}?search_query=all:{}&start=0&max_results={}",
            self.base_url,
            crate::percent_encode(query),
            limit
        );
        let xml = self.get_text(&api).await?;
        Ok(parse_entries(&xml))
    }
}

/// arXiv channel.
#[derive(Debug, Clone)]
pub struct ArxivChannel {
    router: BackendRouter,
    backend: ArxivApiBackend,
}

impl ArxivChannel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a channel whose backend targets `base_url` (tests).
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        let backend = ArxivApiBackend::new().with_base_url(base_url);
        let backends: Vec<Arc<dyn Backend>> = vec![Arc::new(backend.clone())];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(5)),
            RetryConfig::default(),
        );
        Self { router, backend }
    }
}

impl Default for ArxivChannel {
    fn default() -> Self {
        let backend = ArxivApiBackend::new();
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
impl agentspan_core::channel::Channel for ArxivChannel {
    fn name(&self) -> &str {
        "arxiv"
    }

    fn description(&self) -> &str {
        "Search and read arXiv papers (abstracts, authors, links) via the free arXiv API"
    }

    fn can_handle(&self, url: &str) -> bool {
        url.contains("arxiv.org/abs/") || url.contains("arxiv.org/pdf/")
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

    const SAMPLE: &str = r#"<?xml version="1.0"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>ArXiv Query</title>
  <entry>
    <id>http://arxiv.org/abs/2401.12345v1</id>
    <published>2024-01-20T00:00:00Z</published>
    <title>Deep Learning for Rust</title>
    <summary>We study &amp; analyze systems programming.</summary>
    <author><name>Ada Lovelace</name></author>
  </entry>
</feed>"#;

    #[test]
    fn can_handle_arxiv_urls() {
        let ch = ArxivChannel::new();
        assert!(ch.can_handle("https://arxiv.org/abs/2401.12345"));
        assert!(ch.can_handle("https://arxiv.org/pdf/2401.12345.pdf"));
        assert!(!ch.can_handle("https://example.com"));
    }

    #[test]
    fn channel_is_tier_zero() {
        assert_eq!(ArxivChannel::new().tier(), Tier::Zero);
    }

    #[test]
    fn parse_arxiv_id_extracts_id() {
        assert_eq!(
            parse_arxiv_id("https://arxiv.org/abs/2401.12345v1"),
            Some("2401.12345v1".to_string())
        );
        assert_eq!(
            parse_arxiv_id("https://arxiv.org/pdf/2401.12345.pdf"),
            Some("2401.12345".to_string())
        );
        assert_eq!(parse_arxiv_id("https://arxiv.org/"), None);
    }

    #[test]
    fn parse_entries_reads_atom_fields() {
        let entries = parse_entries(SAMPLE);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Deep Learning for Rust");
        assert!(entries[0].snippet.contains("study & analyze"));
        assert_eq!(entries[0].author.as_deref(), Some("Ada Lovelace"));
        assert_eq!(entries[0].url, "http://arxiv.org/abs/2401.12345v1");
    }

    #[tokio::test]
    async fn search_parses_feed() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_string(SAMPLE))
            .mount(&server)
            .await;

        let ch = ArxivChannel::with_base_url(format!("{}/", server.uri()));
        let results = ch.search("rust", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Deep Learning for Rust");
    }

    #[tokio::test]
    async fn read_returns_abstract() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_string(SAMPLE))
            .mount(&server)
            .await;

        let ch = ArxivChannel::with_base_url(format!("{}/", server.uri()));
        let content = ch
            .read("https://arxiv.org/abs/2401.12345", ReadOptions::default())
            .await
            .unwrap();
        assert_eq!(content.title.as_deref(), Some("Deep Learning for Rust"));
        assert!(content.body.contains("study & analyze"));
    }
}
