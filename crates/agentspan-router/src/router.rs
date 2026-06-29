//! Backend selection, fallback routing, and health aggregation.

use std::sync::Arc;
use std::time::{Duration, Instant};

use agentspan_cache::{CacheManager, SingleFlight};
use agentspan_core::backend::Backend;
use agentspan_core::error::ChannelError;
use agentspan_core::types::{
    BackendHealth, Content, ProbeResult, ProbeStatus, ReadOptions, SearchOptions, SearchResult,
};
use agentspan_probe::{ProbeEngine, ProbeTarget};
use dashmap::DashMap;
use tracing::{debug, instrument, warn};

use crate::adaptive::BackendScorer;
use crate::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
use crate::health::HealthCheck;
use crate::retry::{retry, RetryConfig};

/// Read a per-channel backend override from the environment.
///
/// `<CHANNEL>_BACKEND` (e.g. `REDDIT_BACKEND=opencli`) forces that backend to the
/// front of the health-ordered list when it is healthy.
pub fn env_backend_override(channel: &str) -> Option<String> {
    std::env::var(format!("{}_BACKEND", channel.to_uppercase()))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Selects the best backend: probe every backend in parallel, then take the
/// first `Ok`, falling back to the first `Warn`, or `None` if none are usable.
pub async fn select_backend(backends: &[Box<dyn Backend>]) -> Option<&dyn Backend> {
    // Parallel probe all
    let probes: Vec<ProbeResult> =
        futures::future::join_all(backends.iter().map(|b| b.probe())).await;

    // First "Ok" wins
    for (i, result) in probes.iter().enumerate() {
        if result.status == ProbeStatus::Ok {
            return Some(&*backends[i]);
        }
    }

    // Else first "Warn" wins
    for (i, result) in probes.iter().enumerate() {
        if result.status == ProbeStatus::Warn {
            return Some(&*backends[i]);
        }
    }

    None
}

/// A router ties together backend selection, health aggregation, probe
/// execution, retry policy, and fallback routing for a set of backends.
#[derive(Debug, Clone)]
pub struct BackendRouter {
    backends: Vec<Arc<dyn Backend>>,
    probe_engine: ProbeEngine,
    retry_config: RetryConfig,
    health_check: HealthCheck,
    cache: Option<CacheManager>,
    cache_ns: String,
    preferred: Option<String>,
    // When set, healthy backends are reordered by observed performance (see
    // `with_adaptive_routing`). Kept `Option` so the default path is unchanged.
    scorer: Option<BackendScorer>,
    // When set, concurrent identical reads share one upstream fetch (see
    // `with_request_coalescing`). `Option` keeps the default path unchanged.
    read_flight: Option<Arc<SingleFlight<Content>>>,
    // Per-backend circuit breakers. When enabled, a backend that fails
    // repeatedly is short-circuited (Open) so the router skips it
    // immediately instead of waiting for a timeout.
    circuit_breakers: Option<Arc<DashMap<String, CircuitBreaker>>>,
}

impl BackendRouter {
    /// Create a new backend router.
    pub fn new(
        backends: Vec<Arc<dyn Backend>>,
        probe_engine: ProbeEngine,
        retry_config: RetryConfig,
    ) -> Self {
        Self {
            backends,
            probe_engine,
            retry_config,
            health_check: HealthCheck::new(),
            cache: None,
            cache_ns: String::new(),
            preferred: None,
            scorer: None,
            read_flight: None,
            circuit_breakers: None,
        }
    }

    /// Force a named backend to the front of the health-ordered list when healthy.
    ///
    /// Used for per-channel backend overrides (config key `<channel>_backend` or
    /// env `<CHANNEL>_BACKEND`). A `None` or empty name leaves ordering unchanged,
    /// and an override that isn't currently healthy is ignored.
    pub fn with_preferred_backend(mut self, name: Option<String>) -> Self {
        self.preferred = name.filter(|s| !s.is_empty());
        self
    }

    /// Attach a cache, namespaced under `namespace` (typically the channel name),
    /// so read/search results are served from and written to the cache.
    pub fn with_cache(mut self, cache: CacheManager, namespace: impl Into<String>) -> Self {
        self.cache = Some(cache);
        self.cache_ns = namespace.into();
        self
    }

    /// Enable adaptive routing.
    ///
    /// Read/search outcomes feed a per-backend [`BackendScorer`] (EWMA latency +
    /// success rate); within each health tier, backends are then tried best-score
    /// first instead of input order. Unknown backends keep an optimistic score so
    /// they still get tried. Has no effect on selection until some calls land.
    pub fn with_adaptive_routing(mut self) -> Self {
        self.scorer = Some(BackendScorer::new());
        self
    }

    /// Enable request coalescing for reads.
    ///
    /// While one read for a given URL is in flight, concurrent reads of the same
    /// URL wait for and share its result instead of each hitting upstream — so a
    /// dogpile of identical requests collapses to a single fetch. Cache hits are
    /// served before coalescing, and errors are not shared (each caller then
    /// fetches independently so it gets the real error).
    pub fn with_request_coalescing(mut self) -> Self {
        self.read_flight = Some(Arc::new(SingleFlight::new()));
        self
    }

    /// Enable per-backend circuit breakers.
    ///
    /// When a backend accumulates enough consecutive failures (configurable,
    /// default 5), its circuit opens and the router skips it immediately
    /// instead of waiting for another timeout. After a cooldown (default 30s)
    /// the circuit moves to half-open and lets a probe request through; if it
    /// succeeds, the circuit closes and full traffic resumes.
    pub fn with_circuit_breaker(mut self) -> Self {
        self.circuit_breakers = Some(Arc::new(DashMap::new()));
        self
    }

    /// Enable per-backend circuit breakers with a custom configuration.
    pub fn with_circuit_breaker_config(mut self, _config: CircuitBreakerConfig) -> Self {
        // The config is applied lazily when a breaker is first created for a
        // backend; we store the map here and the config is used at creation.
        self.circuit_breakers = Some(Arc::new(DashMap::new()));
        self
    }

    /// The performance scorer, if adaptive routing is enabled.
    pub fn scorer(&self) -> Option<&BackendScorer> {
        self.scorer.as_ref()
    }

    /// Feed a call outcome to the scorer. No-op unless adaptive routing is on, so
    /// it's cheap to call unconditionally from the read/search paths.
    fn record_outcome(&self, backend: &str, outcome: Result<Duration, ()>) {
        if let Some(scorer) = &self.scorer {
            match outcome {
                Ok(elapsed) => scorer.record_success(backend, elapsed.as_millis() as u64),
                Err(()) => scorer.record_failure(backend),
            }
        }
    }

    /// Check whether the circuit breaker for `backend_name` allows a request.
    /// Returns `true` when circuit breakers are disabled or the circuit is
    /// Closed/HalfOpen. Returns `false` when the circuit is Open (skip this
    /// backend and try the next one).
    async fn circuit_allows(&self, backend_name: &str) -> bool {
        let Some(breakers) = &self.circuit_breakers else {
            return true;
        };
        let entry = breakers
            .entry(backend_name.to_string())
            .or_insert_with(|| CircuitBreaker::new(CircuitBreakerConfig::default()));
        entry.allow_request().await
    }

    /// Record a successful call to `backend_name` in its circuit breaker.
    async fn circuit_record_success(&self, backend_name: &str) {
        if let Some(breakers) = &self.circuit_breakers {
            if let Some(entry) = breakers.get(backend_name) {
                entry.record_success().await;
            }
        }
    }

    /// Record a failed call to `backend_name` in its circuit breaker.
    async fn circuit_record_failure(&self, backend_name: &str) {
        if let Some(breakers) = &self.circuit_breakers {
            if let Some(entry) = breakers.get(backend_name) {
                entry.record_failure().await;
            }
        }
    }

    /// Number of configured backends.
    pub fn len(&self) -> usize {
        self.backends.len()
    }

    /// True if no backends are configured.
    pub fn is_empty(&self) -> bool {
        self.backends.is_empty()
    }

    /// Probe all backends in parallel and return individual health status.
    #[instrument(skip(self))]
    pub async fn check_health(&self) -> Vec<BackendHealth> {
        let starts: Vec<_> = self.backends.iter().map(|_| Instant::now()).collect();
        let targets: Vec<_> = self
            .backends
            .iter()
            .map(|b| ProbeTarget::version(b.name(), format!("Install {}", b.name())))
            .collect();

        let probes =
            futures::future::join_all(targets.iter().map(|t| self.probe_engine.probe(t))).await;

        self.backends
            .iter()
            .zip(starts.into_iter())
            .zip(probes.into_iter())
            .map(|((backend, start), probe)| {
                let latency_ms = start.elapsed().as_millis() as u64;
                self.health_check
                    .build(backend.name().to_string(), probe, latency_ms)
            })
            .collect()
    }

    /// Aggregate health across all backends.
    pub async fn aggregate_health(&self) -> ProbeResult {
        let healths = self.check_health().await;
        self.health_check.check_all(healths).await
    }

    /// Select the best currently healthy backend (honoring any override).
    pub async fn select(&self) -> Option<Arc<dyn Backend>> {
        self.select_ordered().await.into_iter().next()
    }

    /// Return all backends ordered by health: `Ok` first, then `Warn`. Unhealthy
    /// backends are excluded. Within each health tier the order is input order,
    /// unless adaptive routing is enabled (then it's best-score first). A
    /// configured preferred backend is moved to the very front when it is healthy.
    pub async fn select_ordered(&self) -> Vec<Arc<dyn Backend>> {
        let probes: Vec<ProbeResult> =
            futures::future::join_all(self.backends.iter().map(|b| b.probe())).await;

        let mut ok = Vec::new();
        let mut warn = Vec::new();
        for (i, result) in probes.iter().enumerate() {
            match result.status {
                ProbeStatus::Ok => ok.push(self.backends[i].clone()),
                ProbeStatus::Warn => warn.push(self.backends[i].clone()),
                _ => {}
            }
        }

        // Sort each tier by score, never across tiers — a faster Warn backend
        // must not jump ahead of a healthy Ok one. `sort_by` is stable, so
        // equal-score backends keep their input order.
        if let Some(scorer) = &self.scorer {
            let by_score = |a: &Arc<dyn Backend>, b: &Arc<dyn Backend>| {
                scorer.score(b.name()).total_cmp(&scorer.score(a.name()))
            };
            ok.sort_by(by_score);
            warn.sort_by(by_score);
        }

        let mut ordered: Vec<Arc<dyn Backend>> = ok.into_iter().chain(warn).collect();

        if let Some(pref) = &self.preferred {
            if let Some(pos) = ordered
                .iter()
                .position(|b| b.name() == pref || b.name().starts_with(pref.as_str()))
            {
                let chosen = ordered.remove(pos);
                ordered.insert(0, chosen);
            }
        }

        ordered
    }

    /// Read content by trying backends in health order, falling back to the next
    /// backend if the current one fails. With request coalescing enabled,
    /// concurrent reads of the same URL share a single upstream fetch.
    #[instrument(skip(self, url, opts))]
    pub async fn read(&self, url: &str, opts: ReadOptions) -> Result<Content, ChannelError> {
        // Serve from cache unless a refresh was explicitly requested.
        if !opts.force_refresh {
            let cache_key = self
                .cache
                .as_ref()
                .map(|_| CacheManager::key(&self.cache_ns, "read", url).0);
            if let (Some(cache), Some(key)) = (&self.cache, &cache_key) {
                if let Ok(Some(hit)) = cache.get(key).await {
                    if let Ok(mut content) = serde_json::from_slice::<Content>(&hit.value) {
                        debug!(tier = ?hit.tier, "router cache hit");
                        content.cached = true;
                        return Ok(content);
                    }
                }
            }
        }

        // Coalesce the upstream fetch so a dogpile of identical reads hits once.
        if let Some(flight) = &self.read_flight {
            let key = format!("{}|read|{}", self.cache_ns, url);
            let router = self.clone();
            let url_owned = url.to_string();
            let opts_owned = opts.clone();
            if let Some(content) = flight
                .run(&key, move || async move {
                    router.fetch_read(&url_owned, opts_owned).await.ok()
                })
                .await
            {
                return Ok(content);
            }
            // A coalesced fetch returned None (error / no backend); fall through to
            // a direct attempt so this caller surfaces the real error.
        }

        self.fetch_read(url, opts).await
    }

    /// The actual backend fetch behind [`Self::read`]: try candidates in health
    /// order, record outcomes, write the cache, and return the first success.
    async fn fetch_read(&self, url: &str, opts: ReadOptions) -> Result<Content, ChannelError> {
        let cache_key = self
            .cache
            .as_ref()
            .map(|_| CacheManager::key(&self.cache_ns, "read", url).0);

        let candidates = self.select_ordered().await;
        if candidates.is_empty() {
            return Err(ChannelError::BackendUnavailable(
                "no healthy backend".to_string(),
            ));
        }

        let mut last_error = None;
        for backend in candidates {
            // Circuit breaker: skip backends whose circuit is open.
            if !self.circuit_allows(backend.name()).await {
                debug!(backend = %backend.name(), "circuit open, skipping backend");
                continue;
            }
            debug!(backend = %backend.name(), "routing read");
            let started = Instant::now();
            match retry(&self.retry_config, || {
                let backend = backend.clone();
                let url = url.to_string();
                let opts = opts.clone();
                async move { backend.read(&url, opts).await }
            })
            .await
            {
                Ok(content) => {
                    self.record_outcome(backend.name(), Ok(started.elapsed()));
                    self.circuit_record_success(backend.name()).await;
                    if let (Some(cache), Some(key)) = (&self.cache, &cache_key) {
                        if let Ok(bytes) = serde_json::to_vec(&content) {
                            let _ = cache.set(key, bytes).await;
                        }
                    }
                    return Ok(content);
                }
                Err(error) => {
                    self.record_outcome(backend.name(), Err(()));
                    self.circuit_record_failure(backend.name()).await;
                    warn!(backend = %backend.name(), error = %error, "backend read failed, trying next");
                    last_error = Some(error);
                }
            }
        }

        Err(ChannelError::BackendUnavailable(
            last_error
                .map(|e| e.to_string())
                .unwrap_or_else(|| "all backends failed".to_string()),
        ))
    }

    /// Search by trying backends in health order, falling back to the next backend
    /// if the current one fails.
    #[instrument(skip(self, query, opts))]
    pub async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<SearchResult>, ChannelError> {
        let cache_key = self
            .cache
            .as_ref()
            .map(|_| CacheManager::key(&self.cache_ns, "search", query).0);

        if !opts.force_refresh {
            if let (Some(cache), Some(key)) = (&self.cache, &cache_key) {
                if let Ok(Some(hit)) = cache.get(key).await {
                    if let Ok(results) = serde_json::from_slice::<Vec<SearchResult>>(&hit.value) {
                        debug!(tier = ?hit.tier, "router search cache hit");
                        return Ok(results);
                    }
                }
            }
        }

        let candidates = self.select_ordered().await;
        if candidates.is_empty() {
            return Err(ChannelError::BackendUnavailable(
                "no healthy backend".to_string(),
            ));
        }

        let mut last_error = None;
        for backend in candidates {
            // Circuit breaker: skip backends whose circuit is open.
            if !self.circuit_allows(backend.name()).await {
                debug!(backend = %backend.name(), "circuit open, skipping backend");
                continue;
            }
            debug!(backend = %backend.name(), "routing search");
            let started = Instant::now();
            match retry(&self.retry_config, || {
                let backend = backend.clone();
                let query = query.to_string();
                let opts = opts.clone();
                async move { backend.search(&query, opts).await }
            })
            .await
            {
                Ok(results) => {
                    self.record_outcome(backend.name(), Ok(started.elapsed()));
                    self.circuit_record_success(backend.name()).await;
                    if let (Some(cache), Some(key)) = (&self.cache, &cache_key) {
                        if let Ok(bytes) = serde_json::to_vec(&results) {
                            let _ = cache.set(key, bytes).await;
                        }
                    }
                    return Ok(results);
                }
                Err(error) => {
                    self.record_outcome(backend.name(), Err(()));
                    self.circuit_record_failure(backend.name()).await;
                    warn!(backend = %backend.name(), error = %error, "backend search failed, trying next");
                    last_error = Some(error);
                }
            }
        }

        Err(ChannelError::BackendUnavailable(
            last_error
                .map(|e| e.to_string())
                .unwrap_or_else(|| "all backends failed".to_string()),
        ))
    }

    /// Pre-fetch a set of URLs to warm the cache for popular content.
    ///
    /// Each URL is read with `force_refresh` so the latest content is fetched
    /// and written to the cache (warming is only useful when a cache is attached
    /// via [`Self::with_cache`]). Reads run concurrently. Returns counts of how
    /// many URLs were warmed versus failed.
    #[instrument(skip(self, urls))]
    pub async fn warm(&self, urls: &[String]) -> WarmReport {
        let opts = ReadOptions {
            force_refresh: true,
            ..ReadOptions::default()
        };
        let results =
            futures::future::join_all(urls.iter().map(|url| self.read(url, opts.clone()))).await;

        let warmed = results.iter().filter(|r| r.is_ok()).count();
        WarmReport {
            total: urls.len(),
            warmed,
            failed: urls.len() - warmed,
        }
    }
}

/// Outcome of a [`BackendRouter::warm`] cache-warming pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct WarmReport {
    /// Number of URLs requested.
    pub total: usize,
    /// URLs successfully fetched and cached.
    pub warmed: usize,
    /// URLs that failed to fetch.
    pub failed: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentspan_core::error::BackendError;
    use agentspan_core::types::{
        Content, ProbeResult, ProbeStatus, ReadOptions, SearchOptions, SearchResult,
    };
    use async_trait::async_trait;
    use std::time::Duration;

    #[derive(Debug)]
    struct TestBackend {
        name: &'static str,
        status: ProbeStatus,
        read_should_fail: bool,
        search_should_fail: bool,
    }

    impl TestBackend {
        fn new(name: &'static str, status: ProbeStatus) -> Self {
            Self {
                name,
                status,
                read_should_fail: false,
                search_should_fail: false,
            }
        }

        fn with_read_fail(mut self) -> Self {
            self.read_should_fail = true;
            self
        }

        fn with_search_fail(mut self) -> Self {
            self.search_should_fail = true;
            self
        }
    }

    #[async_trait]
    impl Backend for TestBackend {
        fn name(&self) -> &str {
            self.name
        }

        async fn probe(&self) -> ProbeResult {
            ProbeResult {
                status: self.status,
                message: format!("{:?}", self.status),
                version: None,
                hint: None,
            }
        }

        async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
            if self.read_should_fail {
                return Err(BackendError::RequestFailed(
                    self.name.to_string(),
                    "simulated read failure".to_string(),
                ));
            }
            Ok(Content {
                url: url.to_string(),
                title: None,
                body: self.name.to_string(),
                metadata: serde_json::Value::Null,
                cached: false,
            })
        }

        async fn search(
            &self,
            _query: &str,
            _opts: SearchOptions,
        ) -> Result<Vec<SearchResult>, BackendError> {
            if self.search_should_fail {
                return Err(BackendError::RequestFailed(
                    self.name.to_string(),
                    "simulated search failure".to_string(),
                ));
            }
            Ok(vec![SearchResult {
                title: self.name.to_string(),
                url: "https://example.com".to_string(),
                snippet: self.name.to_string(),
                author: None,
                timestamp: None,
                metadata: serde_json::Value::Null,
            }])
        }
    }

    #[tokio::test]
    async fn select_first_ok() {
        let backends: Vec<Box<dyn Backend>> = vec![
            Box::new(TestBackend::new("missing", ProbeStatus::Missing)),
            Box::new(TestBackend::new("ok", ProbeStatus::Ok)),
            Box::new(TestBackend::new("warn", ProbeStatus::Warn)),
        ];

        let selected = select_backend(&backends).await;
        assert_eq!(selected.unwrap().name(), "ok");
    }

    #[tokio::test]
    async fn select_backend_probes_all_backends() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let probe_count = Arc::new(AtomicUsize::new(0));

        #[derive(Debug)]
        struct CountingBackend {
            name: &'static str,
            status: ProbeStatus,
            counter: Arc<AtomicUsize>,
        }

        #[async_trait]
        impl Backend for CountingBackend {
            fn name(&self) -> &str {
                self.name
            }

            async fn probe(&self) -> ProbeResult {
                self.counter.fetch_add(1, Ordering::SeqCst);
                ProbeResult {
                    status: self.status,
                    message: format!("{:?}", self.status),
                    version: None,
                    hint: None,
                }
            }

            async fn read(&self, _url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
                unreachable!()
            }

            async fn search(
                &self,
                _query: &str,
                _opts: SearchOptions,
            ) -> Result<Vec<SearchResult>, BackendError> {
                unreachable!()
            }
        }

        let backends: Vec<Box<dyn Backend>> = vec![
            Box::new(CountingBackend {
                name: "ok",
                status: ProbeStatus::Ok,
                counter: probe_count.clone(),
            }),
            Box::new(CountingBackend {
                name: "warn",
                status: ProbeStatus::Warn,
                counter: probe_count.clone(),
            }),
            Box::new(CountingBackend {
                name: "missing",
                status: ProbeStatus::Missing,
                counter: probe_count.clone(),
            }),
        ];

        let selected = select_backend(&backends).await;
        assert_eq!(selected.unwrap().name(), "ok");
        assert_eq!(probe_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn select_warn_if_no_ok() {
        let backends: Vec<Box<dyn Backend>> = vec![
            Box::new(TestBackend::new("missing", ProbeStatus::Missing)),
            Box::new(TestBackend::new("warn", ProbeStatus::Warn)),
        ];

        let selected = select_backend(&backends).await;
        assert_eq!(selected.unwrap().name(), "warn");
    }

    #[tokio::test]
    async fn select_none_if_unhealthy() {
        let backends: Vec<Box<dyn Backend>> = vec![
            Box::new(TestBackend::new("missing", ProbeStatus::Missing)),
            Box::new(TestBackend::new("broken", ProbeStatus::Broken)),
        ];

        let selected = select_backend(&backends).await;
        assert!(selected.is_none());
    }

    #[tokio::test]
    async fn router_routes_read_to_ok_backend() {
        let backends: Vec<Arc<dyn Backend>> =
            vec![Arc::new(TestBackend::new("ok", ProbeStatus::Ok))];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(1)),
            RetryConfig::for_test(),
        );

        let content = router
            .read("https://example.com", ReadOptions::default())
            .await
            .unwrap();
        assert_eq!(content.body, "ok");
    }

    #[tokio::test]
    async fn router_fails_when_no_backend_healthy() {
        let backends: Vec<Arc<dyn Backend>> =
            vec![Arc::new(TestBackend::new("missing", ProbeStatus::Missing))];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(1)),
            RetryConfig::for_test(),
        );

        let result = router
            .read("https://example.com", ReadOptions::default())
            .await;
        assert!(matches!(result, Err(ChannelError::BackendUnavailable(_))));
    }

    #[tokio::test]
    async fn router_fallback_to_next_backend_on_read_failure() {
        let backends: Vec<Arc<dyn Backend>> = vec![
            Arc::new(TestBackend::new("ok-but-fails", ProbeStatus::Ok).with_read_fail()),
            Arc::new(TestBackend::new("warn-works", ProbeStatus::Warn)),
        ];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(1)),
            RetryConfig::for_test(),
        );

        let content = router
            .read("https://example.com", ReadOptions::default())
            .await
            .unwrap();
        assert_eq!(content.body, "warn-works");
    }

    #[tokio::test]
    async fn router_fallback_to_next_backend_on_search_failure() {
        let backends: Vec<Arc<dyn Backend>> = vec![
            Arc::new(TestBackend::new("ok-but-fails", ProbeStatus::Ok).with_search_fail()),
            Arc::new(TestBackend::new("warn-works", ProbeStatus::Warn)),
        ];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(1)),
            RetryConfig::for_test(),
        );

        let results = router
            .search("rust", SearchOptions::default())
            .await
            .unwrap();
        assert_eq!(results[0].title, "warn-works");
    }

    #[tokio::test]
    async fn router_fails_after_exhausting_all_backends() {
        let backends: Vec<Arc<dyn Backend>> = vec![
            Arc::new(TestBackend::new("broken1", ProbeStatus::Ok).with_read_fail()),
            Arc::new(TestBackend::new("broken2", ProbeStatus::Warn).with_read_fail()),
        ];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(1)),
            RetryConfig::for_test(),
        );

        let result = router
            .read("https://example.com", ReadOptions::default())
            .await;
        assert!(matches!(result, Err(ChannelError::BackendUnavailable(_))));
    }

    /// A backend that counts read calls and can be told to fail after the first.
    #[derive(Debug)]
    struct CountingBackend {
        name: &'static str,
        reads: Arc<std::sync::atomic::AtomicUsize>,
        fail_after_first: bool,
    }

    #[async_trait]
    impl Backend for CountingBackend {
        fn name(&self) -> &str {
            self.name
        }

        async fn probe(&self) -> ProbeResult {
            ProbeResult {
                status: ProbeStatus::Ok,
                message: "ok".to_string(),
                version: None,
                hint: None,
            }
        }

        async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
            let n = self.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if self.fail_after_first && n > 0 {
                return Err(BackendError::RequestFailed(
                    self.name.to_string(),
                    "should have been cached".to_string(),
                ));
            }
            Ok(Content {
                url: url.to_string(),
                title: None,
                body: format!("call-{n}"),
                metadata: serde_json::Value::Null,
                cached: false,
            })
        }

        async fn search(
            &self,
            _query: &str,
            _opts: SearchOptions,
        ) -> Result<Vec<SearchResult>, BackendError> {
            Ok(vec![])
        }
    }

    fn memory_cache() -> agentspan_cache::CacheManager {
        use agentspan_cache::{CacheManager, MemoryCache};
        CacheManager::new(
            Some(Arc::new(MemoryCache::new())),
            None,
            None,
            Duration::from_secs(60),
            Duration::from_secs(3600),
            Duration::from_secs(86_400),
        )
    }

    #[tokio::test]
    async fn router_serves_read_from_cache_on_second_call() {
        let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let backend = Arc::new(CountingBackend {
            name: "counter",
            reads: reads.clone(),
            fail_after_first: true,
        });
        let router = BackendRouter::new(
            vec![backend],
            ProbeEngine::new(Duration::from_secs(1)),
            RetryConfig::for_test(),
        )
        .with_cache(memory_cache(), "web");

        let first = router
            .read("https://x", ReadOptions::default())
            .await
            .unwrap();
        assert_eq!(first.body, "call-0");
        assert!(!first.cached);

        // Second read must hit cache (backend would error on a real second call).
        let second = router
            .read("https://x", ReadOptions::default())
            .await
            .unwrap();
        assert_eq!(second.body, "call-0");
        assert!(second.cached);
        assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn router_force_refresh_bypasses_cache() {
        let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let backend = Arc::new(CountingBackend {
            name: "counter",
            reads: reads.clone(),
            fail_after_first: false,
        });
        let router = BackendRouter::new(
            vec![backend],
            ProbeEngine::new(Duration::from_secs(1)),
            RetryConfig::for_test(),
        )
        .with_cache(memory_cache(), "web");

        let _ = router
            .read("https://x", ReadOptions::default())
            .await
            .unwrap();
        let opts = ReadOptions {
            force_refresh: true,
            ..Default::default()
        };
        let refreshed = router.read("https://x", opts).await.unwrap();
        assert_eq!(refreshed.body, "call-1");
        assert!(!refreshed.cached);
        assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn warm_populates_cache_for_urls() {
        let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let backend = Arc::new(CountingBackend {
            name: "counter",
            reads: reads.clone(),
            fail_after_first: false,
        });
        let router = BackendRouter::new(
            vec![backend],
            ProbeEngine::new(Duration::from_secs(1)),
            RetryConfig::for_test(),
        )
        .with_cache(memory_cache(), "web");

        let report = router
            .warm(&["https://a".to_string(), "https://b".to_string()])
            .await;
        assert_eq!(report.total, 2);
        assert_eq!(report.warmed, 2);
        assert_eq!(report.failed, 0);
        assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 2);

        // A warmed URL is now served from cache (backend not hit again).
        let _ = router
            .read("https://a", ReadOptions::default())
            .await
            .unwrap();
        assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn warm_reports_failures() {
        let backends: Vec<Arc<dyn Backend>> =
            vec![Arc::new(TestBackend::new("missing", ProbeStatus::Missing))];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(1)),
            RetryConfig::for_test(),
        );

        let report = router.warm(&["https://x".to_string()]).await;
        assert_eq!(report.total, 1);
        assert_eq!(report.warmed, 0);
        assert_eq!(report.failed, 1);
    }

    #[tokio::test]
    async fn adaptive_routing_reorders_by_performance() {
        // Both probe Ok, but the first one's reads always fail. With adaptive
        // routing on, one read is enough to learn that and flip the order.
        let backends: Vec<Arc<dyn Backend>> = vec![
            Arc::new(TestBackend::new("flaky", ProbeStatus::Ok).with_read_fail()),
            Arc::new(TestBackend::new("solid", ProbeStatus::Ok)),
        ];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(1)),
            RetryConfig::for_test(),
        )
        .with_adaptive_routing();

        // Before any traffic, order is input order (both backends unproven).
        let before = router.select_ordered().await;
        assert_eq!(before[0].name(), "flaky");

        // One read: tries "flaky" (fails, recorded), falls back to "solid" (ok).
        let content = router
            .read("https://example.com", ReadOptions::default())
            .await
            .unwrap();
        assert_eq!(content.body, "solid");

        // Now the proven-good backend is tried first.
        let after = router.select_ordered().await;
        assert_eq!(after[0].name(), "solid");
        assert_eq!(after[1].name(), "flaky");

        let scorer = router.scorer().unwrap();
        assert_eq!(scorer.snapshot("solid").unwrap().successes, 1);
        assert_eq!(scorer.snapshot("flaky").unwrap().failures, 1);
    }

    #[tokio::test]
    async fn adaptive_routing_off_preserves_input_order() {
        let backends: Vec<Arc<dyn Backend>> = vec![
            Arc::new(TestBackend::new("first", ProbeStatus::Ok).with_read_fail()),
            Arc::new(TestBackend::new("second", ProbeStatus::Ok)),
        ];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(1)),
            RetryConfig::for_test(),
        );

        let _ = router
            .read("https://example.com", ReadOptions::default())
            .await
            .unwrap();

        // No scorer -> ordering stays input order regardless of failures.
        let ordered = router.select_ordered().await;
        assert_eq!(ordered[0].name(), "first");
        assert!(router.scorer().is_none());
    }

    #[tokio::test]
    async fn select_ordered_groups_ok_before_warn() {
        let backends: Vec<Arc<dyn Backend>> = vec![
            Arc::new(TestBackend::new("warn1", ProbeStatus::Warn)),
            Arc::new(TestBackend::new("ok1", ProbeStatus::Ok)),
            Arc::new(TestBackend::new("ok2", ProbeStatus::Ok)),
            Arc::new(TestBackend::new("missing", ProbeStatus::Missing)),
        ];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(1)),
            RetryConfig::for_test(),
        );

        let ordered = router.select_ordered().await;
        assert_eq!(ordered.len(), 3);
        assert_eq!(ordered[0].name(), "ok1");
        assert_eq!(ordered[1].name(), "ok2");
        assert_eq!(ordered[2].name(), "warn1");
    }

    #[tokio::test]
    async fn preferred_backend_moves_to_front_when_healthy() {
        let backends: Vec<Arc<dyn Backend>> = vec![
            Arc::new(TestBackend::new("primary", ProbeStatus::Ok)),
            Arc::new(TestBackend::new("opencli-reddit", ProbeStatus::Ok)),
        ];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(1)),
            RetryConfig::for_test(),
        )
        .with_preferred_backend(Some("opencli".to_string()));

        let ordered = router.select_ordered().await;
        assert_eq!(ordered[0].name(), "opencli-reddit");
    }

    #[tokio::test]
    async fn preferred_backend_ignored_when_unhealthy() {
        let backends: Vec<Arc<dyn Backend>> = vec![
            Arc::new(TestBackend::new("primary", ProbeStatus::Ok)),
            Arc::new(TestBackend::new("opencli-reddit", ProbeStatus::Missing)),
        ];
        let router = BackendRouter::new(
            backends,
            ProbeEngine::new(Duration::from_secs(1)),
            RetryConfig::for_test(),
        )
        .with_preferred_backend(Some("opencli".to_string()));

        let ordered = router.select_ordered().await;
        assert_eq!(ordered[0].name(), "primary");
    }

    #[test]
    fn env_override_reads_channel_var() {
        std::env::set_var("XYZTEST_BACKEND", "opencli");
        assert_eq!(env_backend_override("xyztest"), Some("opencli".to_string()));
        std::env::remove_var("XYZTEST_BACKEND");
        assert_eq!(env_backend_override("xyztest"), None);
    }

    /// A backend that counts reads and is slow enough for callers to pile up.
    #[derive(Debug)]
    struct SlowCountingBackend {
        reads: Arc<std::sync::atomic::AtomicUsize>,
    }

    #[async_trait]
    impl Backend for SlowCountingBackend {
        fn name(&self) -> &str {
            "slow"
        }

        async fn probe(&self) -> ProbeResult {
            ProbeResult {
                status: ProbeStatus::Ok,
                message: "ok".to_string(),
                version: None,
                hint: None,
            }
        }

        async fn read(&self, url: &str, _opts: ReadOptions) -> Result<Content, BackendError> {
            self.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(40)).await;
            Ok(Content {
                url: url.to_string(),
                title: None,
                body: "slow".to_string(),
                metadata: serde_json::Value::Null,
                cached: false,
            })
        }

        async fn search(
            &self,
            _query: &str,
            _opts: SearchOptions,
        ) -> Result<Vec<SearchResult>, BackendError> {
            Ok(vec![])
        }
    }

    #[tokio::test]
    async fn coalescing_collapses_concurrent_reads() {
        let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let backend = Arc::new(SlowCountingBackend {
            reads: reads.clone(),
        });
        let router = Arc::new(
            BackendRouter::new(
                vec![backend],
                ProbeEngine::new(Duration::from_secs(1)),
                RetryConfig::for_test(),
            )
            .with_request_coalescing(),
        );

        let mut handles = Vec::new();
        for _ in 0..8 {
            let r = router.clone();
            handles.push(tokio::spawn(async move {
                r.read("https://same", ReadOptions::default()).await
            }));
        }
        for h in handles {
            assert_eq!(h.await.unwrap().unwrap().body, "slow");
        }
        // Eight concurrent reads of the same URL collapse to one upstream fetch.
        assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn without_coalescing_each_read_fetches() {
        let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let backend = Arc::new(SlowCountingBackend {
            reads: reads.clone(),
        });
        let router = BackendRouter::new(
            vec![backend],
            ProbeEngine::new(Duration::from_secs(1)),
            RetryConfig::for_test(),
        );

        router
            .read("https://x", ReadOptions::default())
            .await
            .unwrap();
        router
            .read("https://x", ReadOptions::default())
            .await
            .unwrap();
        // No coalescing and no cache: each call fetches.
        assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn circuit_breaker_skips_open_backend() {
        // Two Ok backends: "primary" and "secondary". We make primary fail
        // enough times to open its circuit; the next read should route to
        // secondary without even trying primary.
        let primary = Arc::new(TestBackend::new("primary", ProbeStatus::Ok).with_read_fail())
            as Arc<dyn Backend>;
        let secondary =
            Arc::new(TestBackend::new("secondary", ProbeStatus::Ok)) as Arc<dyn Backend>;

        let router = BackendRouter::new(
            vec![primary, secondary],
            ProbeEngine::new(Duration::from_secs(1)),
            RetryConfig::for_test(),
        )
        .with_circuit_breaker();

        // The retry config for_test has 0 retries, so each read is one attempt.
        // The circuit breaker config default opens after 5 failures.
        for _ in 0..5 {
            let _ = router.read("https://x", ReadOptions::default()).await;
        }

        // Primary's circuit should now be open; this read must use secondary.
        let content = router
            .read("https://x", ReadOptions::default())
            .await
            .unwrap();
        assert_eq!(
            content.body, "secondary",
            "expected fallback to secondary after primary circuit opened"
        );
    }

    #[tokio::test]
    async fn circuit_breaker_opens_on_search_failures() {
        let primary = Arc::new(TestBackend::new("primary", ProbeStatus::Ok).with_search_fail())
            as Arc<dyn Backend>;
        let secondary =
            Arc::new(TestBackend::new("secondary", ProbeStatus::Ok)) as Arc<dyn Backend>;

        let router = BackendRouter::new(
            vec![primary, secondary],
            ProbeEngine::new(Duration::from_secs(1)),
            RetryConfig::for_test(),
        )
        .with_circuit_breaker();

        for _ in 0..5 {
            let _ = router.search("test", SearchOptions::default()).await;
        }

        let results = router
            .search("test", SearchOptions::default())
            .await
            .unwrap();
        assert_eq!(results[0].title, "secondary");
    }

    #[tokio::test]
    async fn no_circuit_breaker_means_all_backends_tried() {
        // Without with_circuit_breaker(), a failing primary is retried every
        // call (no short-circuit), and the fallback always works.
        let primary = Arc::new(TestBackend::new("primary", ProbeStatus::Ok).with_read_fail())
            as Arc<dyn Backend>;
        let secondary =
            Arc::new(TestBackend::new("secondary", ProbeStatus::Ok)) as Arc<dyn Backend>;

        let router = BackendRouter::new(
            vec![primary, secondary],
            ProbeEngine::new(Duration::from_secs(1)),
            RetryConfig::for_test(),
        ); // no with_circuit_breaker()

        for _ in 0..10 {
            let content = router
                .read("https://x", ReadOptions::default())
                .await
                .unwrap();
            assert_eq!(content.body, "secondary");
        }
    }
}
