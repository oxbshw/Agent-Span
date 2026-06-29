//! Shared HTTP client construction with AgentSpan proxy support.
//!
//! Backends call [`default_client`] so that a proxy configured in
//! `~/.agentspan/config.yaml` (or `AGENTSPAN_PROXY__URL`) is applied uniformly —
//! previously each backend built its own client and ignored the proxy setting.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use reqwest::header::{ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED};

use agentspan_core::config::ProxyConfig;

/// Build a reqwest client, applying the proxy URL when present.
pub fn client_with_proxy(proxy: Option<&ProxyConfig>) -> reqwest::Client {
    let mut builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("AgentSpan/0.1 (+https://github.com/agentspan/agentspan)");

    if let Some(proxy) = proxy {
        if let Some(url) = proxy.url.as_ref().filter(|u| !u.is_empty()) {
            if let Ok(p) = reqwest::Proxy::all(url) {
                builder = builder.proxy(p);
            }
        }
    }

    builder.build().unwrap_or_default()
}

/// Load the proxy setting from the AgentSpan config (best effort).
fn load_proxy() -> Option<ProxyConfig> {
    agentspan_core::Config::load()
        .ok()
        .map(|c| c.proxy)
        .filter(|p| p.url.is_some())
}

/// Build the default HTTP client, honoring any configured proxy.
pub fn default_client() -> reqwest::Client {
    client_with_proxy(load_proxy().as_ref())
}

/// Load a stored per-platform cookie string from the config `cookies` map.
///
/// Backends call this so credentials imported via `agentspan config cookies`
/// (e.g. `cookies.bilibili`, `cookies.reddit`) are actually sent on requests.
pub fn cookie_for(platform: &str) -> Option<String> {
    agentspan_core::Config::load()
        .ok()
        .and_then(|c| c.cookies.get(platform).cloned())
        .filter(|s| !s.is_empty())
}

/// HTTP cache validators captured from a response (`ETag` / `Last-Modified`).
#[derive(Debug, Clone, Default)]
pub struct Validators {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

impl Validators {
    /// Read `ETag` and `Last-Modified` from a response's headers.
    pub fn from_headers(headers: &reqwest::header::HeaderMap) -> Self {
        let read = |name: reqwest::header::HeaderName| {
            headers
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string)
        };
        Self {
            etag: read(ETAG),
            last_modified: read(LAST_MODIFIED),
        }
    }

    /// True when there's nothing to revalidate against.
    pub fn is_empty(&self) -> bool {
        self.etag.is_none() && self.last_modified.is_none()
    }
}

/// In-memory map of URL → (validators, last body). Cheap to clone; the inner
/// state is shared, so clones observe each other's writes.
#[derive(Debug, Clone, Default)]
pub struct ValidatorStore {
    inner: Arc<Mutex<HashMap<String, (Validators, String)>>>,
}

impl ValidatorStore {
    /// Create an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Remember the validators and body for a URL.
    pub fn insert(&self, url: &str, validators: Validators, body: String) {
        self.inner
            .lock()
            .expect("validator store poisoned")
            .insert(url.to_string(), (validators, body));
    }

    /// Look up the stored `(validators, body)` for a URL.
    pub fn get(&self, url: &str) -> Option<(Validators, String)> {
        self.inner
            .lock()
            .expect("validator store poisoned")
            .get(url)
            .cloned()
    }
}

/// Outcome of a [`conditional_get`].
#[derive(Debug)]
pub enum ConditionalFetch {
    /// Server replied `304 Not Modified`; the stored body was reused (no
    /// re-download).
    NotModified(String),
    /// A full response of any non-304 status, with its body.
    Fetched {
        status: reqwest::StatusCode,
        body: String,
    },
}

/// GET `url`, revalidating against any stored validators.
///
/// If the store holds an `ETag`/`Last-Modified` for `url` and `force` is false,
/// the request carries `If-None-Match`/`If-Modified-Since`; a `304` reply reuses
/// the cached body. A successful full response refreshes the stored validators
/// and body. `force` skips the conditional headers entirely — an explicit
/// "fetch it fresh" — though a fresh response still updates the store.
pub async fn conditional_get(
    client: &reqwest::Client,
    url: &str,
    store: &ValidatorStore,
    force: bool,
) -> Result<ConditionalFetch, reqwest::Error> {
    let prior = store.get(url);

    let mut request = client.get(url);
    if !force {
        if let Some((validators, _)) = &prior {
            if let Some(etag) = &validators.etag {
                request = request.header(IF_NONE_MATCH, etag);
            }
            if let Some(last_modified) = &validators.last_modified {
                request = request.header(IF_MODIFIED_SINCE, last_modified);
            }
        }
    }

    let response = request.send().await?;

    if response.status() == reqwest::StatusCode::NOT_MODIFIED {
        if let Some((_, body)) = prior {
            return Ok(ConditionalFetch::NotModified(body));
        }
        // A 304 with nothing cached shouldn't happen (we only send validators we
        // have), but fall through and surface whatever the server returned.
    }

    let status = response.status();
    let validators = Validators::from_headers(response.headers());
    let body = response.text().await?;
    if status.is_success() && !validators.is_empty() {
        store.insert(url, validators, body.clone());
    }
    Ok(ConditionalFetch::Fetched { status, body })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_without_proxy() {
        let _client = client_with_proxy(None);
    }

    #[test]
    fn builds_with_proxy_url() {
        let proxy = ProxyConfig {
            url: Some("http://127.0.0.1:8080".to_string()),
            no_proxy: vec![],
        };
        let _client = client_with_proxy(Some(&proxy));
    }

    #[test]
    fn ignores_empty_proxy_url() {
        let proxy = ProxyConfig {
            url: Some(String::new()),
            no_proxy: vec![],
        };
        // Empty URL must not panic or fail the build.
        let _client = client_with_proxy(Some(&proxy));
    }

    #[test]
    fn validators_from_headers_reads_both() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::ETAG, "abc".parse().unwrap());
        headers.insert(
            reqwest::header::LAST_MODIFIED,
            "Wed, 21 Oct 2015 07:28:00 GMT".parse().unwrap(),
        );
        let v = Validators::from_headers(&headers);
        assert_eq!(v.etag.as_deref(), Some("abc"));
        assert!(v.last_modified.is_some());
        assert!(!v.is_empty());
        assert!(Validators::default().is_empty());
    }

    #[test]
    fn validator_store_roundtrip() {
        let store = ValidatorStore::new();
        assert!(store.get("u").is_none());
        store.insert(
            "u",
            Validators {
                etag: Some("e".to_string()),
                last_modified: None,
            },
            "body".to_string(),
        );
        let (validators, body) = store.get("u").unwrap();
        assert_eq!(validators.etag.as_deref(), Some("e"));
        assert_eq!(body, "body");
    }

    #[tokio::test]
    async fn conditional_get_revalidates_with_304() {
        use wiremock::matchers::{header, method};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        // First (unconditional) request: 200 + ETag, served exactly once.
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("etag", "v1")
                    .set_body_string("hello"),
            )
            .up_to_n_times(1)
            .mount(&server)
            .await;
        // Once we've cached the validator, the revalidating request gets a 304.
        Mock::given(method("GET"))
            .and(header("if-none-match", "v1"))
            .respond_with(ResponseTemplate::new(304))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let store = ValidatorStore::new();
        let url = server.uri();

        match conditional_get(&client, &url, &store, false).await.unwrap() {
            ConditionalFetch::Fetched { status, body } => {
                assert_eq!(status, reqwest::StatusCode::OK);
                assert_eq!(body, "hello");
            }
            other => panic!("expected a fresh fetch, got {other:?}"),
        }

        match conditional_get(&client, &url, &store, false).await.unwrap() {
            ConditionalFetch::NotModified(body) => assert_eq!(body, "hello"),
            other => panic!("expected a 304 reuse, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn conditional_get_force_skips_revalidation() {
        use wiremock::matchers::{header_exists, method};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        // Any conditional request would 304...
        Mock::given(method("GET"))
            .and(header_exists("if-none-match"))
            .respond_with(ResponseTemplate::new(304))
            .mount(&server)
            .await;
        // ...but an unconditional GET gets a fresh 200.
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("fresh"))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let store = ValidatorStore::new();
        let url = server.uri();
        // Pretend we already have a validator cached.
        store.insert(
            &url,
            Validators {
                etag: Some("v1".to_string()),
                last_modified: None,
            },
            "old".to_string(),
        );

        // force = true must NOT send If-None-Match, so we get the fresh body.
        match conditional_get(&client, &url, &store, true).await.unwrap() {
            ConditionalFetch::Fetched { status, body } => {
                assert_eq!(status, reqwest::StatusCode::OK);
                assert_eq!(body, "fresh");
            }
            other => panic!("force should fetch fresh, got {other:?}"),
        }
    }
}
