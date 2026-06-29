//! REST API and WebSocket server for AgentSpan.

use std::sync::Arc;

use agentspan_auth::{AdaptiveRateLimiter, AuthManager};
use agentspan_cache::CacheOptimizer;
use agentspan_channels::healer::Healer;
use agentspan_channels::registry::ChannelRegistry;
use agentspan_core::{Analytics, Config, Profiler};
use tokio::sync::{broadcast, Semaphore};

pub mod memory;
pub mod metrics;
pub mod middleware;
pub mod observe;
pub mod routes;

use memory::MemoryStore;
use metrics::Metrics;

/// Capacity of the live-events broadcast channel (per the `/ws/v1/events` stream).
const EVENT_BUFFER: usize = 256;

/// Maximum number of in-flight requests before the gateway sheds load (503).
pub const MAX_CONCURRENT_REQUESTS: usize = 1024;

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    /// Registry of available channels.
    pub registry: ChannelRegistry,
    /// Authentication, tenants, rate limiting, and audit.
    pub auth: Arc<AuthManager>,
    /// Loaded server configuration.
    pub config: Arc<Config>,
    /// Broadcaster for real-time events pushed to WebSocket clients.
    pub events: broadcast::Sender<String>,
    /// Process metrics exported at `GET /metrics`.
    pub metrics: Arc<Metrics>,
    /// Permits bounding the number of concurrently-served requests.
    pub inflight: Arc<Semaphore>,
    /// Namespaced key/value scratchpad for agents (`/api/v1/memory`).
    pub memory: MemoryStore,
    /// Self-healing subsystem: monitor, switcher, repair, alerts.
    pub healer: Arc<Healer>,
    /// Per-request usage analytics feeding the self-improving suggestions.
    pub analytics: Arc<Analytics>,
    /// Latency profiling per channel/backend.
    pub profiler: Arc<Profiler>,
    /// Learns a good cache TTL per channel from hit rates.
    pub cache_optimizer: Arc<CacheOptimizer>,
    /// Learns per-platform rate limits from observed 429s.
    pub adaptive_rate: Arc<AdaptiveRateLimiter>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("channels", &self.registry.list().len())
            .field("require_api_key", &self.config.auth.require_api_key)
            .finish()
    }
}

impl AppState {
    /// Create default application state (default config, no required auth).
    pub fn default_state() -> Self {
        Self {
            registry: ChannelRegistry::default_channels(),
            auth: Arc::new(AuthManager::new()),
            config: Arc::new(Config::default()),
            events: broadcast::channel(EVENT_BUFFER).0,
            metrics: Arc::new(Metrics::default()),
            inflight: Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS)),
            memory: MemoryStore::new(),
            healer: Arc::new(Healer::new()),
            analytics: Arc::new(Analytics::new()),
            profiler: Arc::new(Profiler::new()),
            cache_optimizer: Arc::new(CacheOptimizer::default()),
            adaptive_rate: Arc::new(AdaptiveRateLimiter::default()),
        }
    }

    /// Create application state from an explicit configuration.
    pub fn with_config(config: Config) -> Self {
        Self {
            registry: ChannelRegistry::default_channels(),
            auth: Arc::new(AuthManager::new()),
            config: Arc::new(config),
            events: broadcast::channel(EVENT_BUFFER).0,
            metrics: Arc::new(Metrics::default()),
            inflight: Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS)),
            memory: MemoryStore::new(),
            healer: Arc::new(Healer::new()),
            analytics: Arc::new(Analytics::new()),
            profiler: Arc::new(Profiler::new()),
            cache_optimizer: Arc::new(CacheOptimizer::default()),
            adaptive_rate: Arc::new(AdaptiveRateLimiter::default()),
        }
    }

    /// Spawn the background self-healing monitor.
    ///
    /// Probes every channel on the healer's interval, updating the shared
    /// snapshots that the `/admin/healing-report` endpoint reads. Returns the
    /// task handle; the server keeps it alive for its lifetime.
    pub fn spawn_healer(&self) -> tokio::task::JoinHandle<()> {
        self.healer.spawn(self.registry.clone())
    }

    /// Publish a live event to any connected WebSocket clients.
    ///
    /// Best-effort: returns silently when there are no subscribers.
    pub fn publish_event(&self, message: impl Into<String>) {
        let _ = self.events.send(message.into());
    }

    /// Build the full Axum router for this state.
    pub fn router(self) -> axum::Router {
        routes::build_router(self)
    }
}
