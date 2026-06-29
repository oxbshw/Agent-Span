//! Circuit breaker for backend protection.
//!
//! Tracks per-backend failure rates and automatically opens when failures exceed
//! a threshold, half-opens after a cooldown, and closes again after enough
//! consecutive successes.

use std::fmt;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

/// State of a circuit breaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation; requests are allowed.
    Closed,
    /// Failure threshold exceeded; requests are blocked.
    Open,
    /// Testing whether the backend has recovered.
    HalfOpen,
}

impl fmt::Display for CircuitState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CircuitState::Closed => write!(f, "closed"),
            CircuitState::Open => write!(f, "open"),
            CircuitState::HalfOpen => write!(f, "half-open"),
        }
    }
}

/// Configuration for a circuit breaker.
#[derive(Debug, Clone, Copy)]
pub struct CircuitBreakerConfig {
    /// Consecutive failures required to open the circuit.
    pub failure_threshold: u32,
    /// Consecutive successes required in half-open to close the circuit.
    pub success_threshold: u32,
    /// Time the circuit stays open before moving to half-open.
    pub open_timeout: Duration,
    /// Maximum number of probe requests allowed in half-open state.
    pub half_open_max_calls: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            open_timeout: Duration::from_secs(30),
            half_open_max_calls: 3,
        }
    }
}

impl CircuitBreakerConfig {
    /// Configuration tuned for fast unit tests.
    pub fn for_test() -> Self {
        Self {
            failure_threshold: 3,
            success_threshold: 2,
            open_timeout: Duration::from_millis(50),
            half_open_max_calls: 2,
        }
    }
}

/// Internal mutable state of the circuit breaker.
#[derive(Debug)]
struct Inner {
    state: CircuitState,
    consecutive_failures: u32,
    consecutive_successes: u32,
    half_open_calls: u32,
    last_failure_time: Option<Instant>,
}

/// A circuit breaker that protects a backend from cascading failures.
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    inner: Mutex<Inner>,
}

impl fmt::Debug for CircuitBreaker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CircuitBreaker")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the given configuration.
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            inner: Mutex::new(Inner {
                state: CircuitState::Closed,
                consecutive_failures: 0,
                consecutive_successes: 0,
                half_open_calls: 0,
                last_failure_time: None,
            }),
        }
    }

    /// Return the current state.
    pub async fn state(&self) -> CircuitState {
        self.inner.lock().await.state
    }

    /// Return true if a request should be allowed through.
    ///
    /// When open, this also checks whether the cooldown has elapsed and
    /// transitions to half-open.
    pub async fn allow_request(&self) -> bool {
        let mut inner = self.inner.lock().await;

        match inner.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                let elapsed = inner
                    .last_failure_time
                    .map(|t| t.elapsed())
                    .unwrap_or(self.config.open_timeout);
                if elapsed >= self.config.open_timeout {
                    inner.state = CircuitState::HalfOpen;
                    inner.consecutive_successes = 0;
                    inner.half_open_calls = 1;
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => {
                if inner.half_open_calls < self.config.half_open_max_calls {
                    inner.half_open_calls += 1;
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Record a successful response.
    pub async fn record_success(&self) {
        let mut inner = self.inner.lock().await;

        match inner.state {
            CircuitState::Closed => {
                inner.consecutive_failures = 0;
            }
            CircuitState::HalfOpen => {
                inner.consecutive_successes += 1;
                if inner.consecutive_successes >= self.config.success_threshold {
                    inner.state = CircuitState::Closed;
                    inner.consecutive_failures = 0;
                    inner.consecutive_successes = 0;
                    inner.half_open_calls = 0;
                }
            }
            CircuitState::Open => {}
        }
    }

    /// Record a failed response.
    pub async fn record_failure(&self) {
        let mut inner = self.inner.lock().await;
        let now = Instant::now();

        match inner.state {
            CircuitState::Closed => {
                inner.consecutive_failures += 1;
                inner.last_failure_time = Some(now);
                if inner.consecutive_failures >= self.config.failure_threshold {
                    inner.state = CircuitState::Open;
                }
            }
            CircuitState::HalfOpen => {
                inner.state = CircuitState::Open;
                inner.last_failure_time = Some(now);
                inner.consecutive_successes = 0;
                inner.half_open_calls = 0;
            }
            CircuitState::Open => {
                inner.last_failure_time = Some(now);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn closed_allows_requests() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::for_test());
        assert_eq!(cb.state().await, CircuitState::Closed);
        assert!(cb.allow_request().await);
    }

    #[tokio::test]
    async fn opens_after_threshold_failures() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::for_test());
        for _ in 0..cb.config.failure_threshold {
            assert!(cb.allow_request().await);
            cb.record_failure().await;
        }

        assert_eq!(cb.state().await, CircuitState::Open);
        assert!(!cb.allow_request().await);
    }

    #[tokio::test]
    async fn half_open_after_timeout_then_closes_on_success() {
        let config = CircuitBreakerConfig::for_test();
        let cb = CircuitBreaker::new(config);

        // Open the circuit.
        for _ in 0..config.failure_threshold {
            cb.record_failure().await;
        }
        assert_eq!(cb.state().await, CircuitState::Open);

        // Wait for the open timeout.
        tokio::time::sleep(config.open_timeout).await;

        // First allowed request transitions to half-open.
        assert!(cb.allow_request().await);
        assert_eq!(cb.state().await, CircuitState::HalfOpen);

        // Record enough successes to close.
        for _ in 0..config.success_threshold {
            cb.record_success().await;
        }
        assert_eq!(cb.state().await, CircuitState::Closed);
        assert!(cb.allow_request().await);
    }

    #[tokio::test]
    async fn half_open_reopens_on_failure() {
        let config = CircuitBreakerConfig::for_test();
        let cb = CircuitBreaker::new(config);

        for _ in 0..config.failure_threshold {
            cb.record_failure().await;
        }

        tokio::time::sleep(config.open_timeout).await;
        assert!(cb.allow_request().await);
        assert_eq!(cb.state().await, CircuitState::HalfOpen);

        cb.record_failure().await;
        assert_eq!(cb.state().await, CircuitState::Open);
        assert!(!cb.allow_request().await);
    }

    #[tokio::test]
    async fn success_resets_failure_count_in_closed_state() {
        let config = CircuitBreakerConfig::for_test();
        let cb = CircuitBreaker::new(config);

        cb.record_failure().await;
        cb.record_failure().await;
        cb.record_success().await;

        // We are still closed because threshold was not reached, and the
        // consecutive failure counter should have been reset.
        assert_eq!(cb.state().await, CircuitState::Closed);

        // Need a full new streak of failures to open.
        for _ in 0..config.failure_threshold {
            cb.record_failure().await;
        }
        assert_eq!(cb.state().await, CircuitState::Open);
    }

    #[tokio::test]
    async fn half_open_limits_concurrent_probe_calls() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            success_threshold: 5,
            open_timeout: Duration::from_millis(1),
            half_open_max_calls: 2,
        };
        let cb = CircuitBreaker::new(config);

        cb.record_failure().await;
        tokio::time::sleep(config.open_timeout).await;

        assert!(cb.allow_request().await);
        assert!(cb.allow_request().await);
        assert!(!cb.allow_request().await);
    }
}
