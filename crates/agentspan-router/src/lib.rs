//! Multi-backend routing engine.

pub mod adaptive;
pub mod circuit_breaker;
pub mod health;
pub mod monitor;
pub mod retry;
pub mod router;

pub use adaptive::{BackendScorer, BackendStat};
pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitState};
pub use monitor::{HealthAlert, HealthMonitor};
pub use router::{env_backend_override, select_backend, BackendRouter, WarmReport};
