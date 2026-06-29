//! Retry with exponential backoff, per-attempt timeouts, and selective error handling.

use std::time::Duration;

use agentspan_core::error::BackendError;

/// Retry configuration.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of attempts before giving up.
    pub max_attempts: u32,
    /// Initial delay between retries.
    pub base_delay: Duration,
    /// Maximum delay between retries (caps exponential growth).
    pub max_delay: Duration,
    /// Maximum time allowed for each individual attempt.
    pub timeout_per_attempt: Duration,
    /// Apply full jitter to each backoff delay to avoid thundering-herd retries.
    ///
    /// When `true`, the actual sleep between attempts is sampled uniformly from
    /// `[0, delay]` rather than using the exact exponential `delay`. The
    /// exponential schedule still governs the upper bound, so retries stay
    /// bounded by `max_delay` while being de-correlated across callers.
    pub jitter: bool,
    /// Predicate that decides whether an error is worth retrying.
    pub retryable: fn(&BackendError) -> bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            timeout_per_attempt: Duration::from_secs(30),
            jitter: true,
            retryable: default_retryable,
        }
    }
}

impl RetryConfig {
    /// Create a retry config intended for quick unit tests.
    pub fn for_test() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
            timeout_per_attempt: Duration::from_millis(50),
            // Deterministic delays keep timing-sensitive unit tests stable.
            jitter: false,
            retryable: default_retryable,
        }
    }
}

/// Apply full jitter to a backoff delay.
///
/// Returns the delay unchanged when `jitter` is disabled or the delay is zero,
/// otherwise a value sampled uniformly from `[0, delay]`. Sampling happens
/// synchronously (no RNG is held across an `.await`).
fn jittered_delay(delay: Duration, jitter: bool) -> Duration {
    if !jitter || delay.is_zero() {
        return delay;
    }
    delay.mul_f64(rand::random::<f64>())
}

/// Default retry predicate: retries transient/network-style errors only.
///
/// Non-retryable errors (auth, not found, parse, missing command) fail fast.
pub fn default_retryable(error: &BackendError) -> bool {
    matches!(
        error,
        BackendError::RequestFailed(_, _)
            | BackendError::Timeout(_)
            | BackendError::Other(_, _)
            | BackendError::CommandFailed(_, _)
    )
}

/// Retry an async operation with exponential backoff and per-attempt timeouts.
///
/// Only errors matching `config.retryable` are retried; permanent errors fail
/// immediately. Each attempt is bounded by `config.timeout_per_attempt`.
pub async fn retry<F, Fut, T>(config: &RetryConfig, mut f: F) -> Result<T, BackendError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, BackendError>>,
{
    let mut delay = config.base_delay;

    for attempt in 0..config.max_attempts {
        let result = tokio::time::timeout(config.timeout_per_attempt, f()).await;

        match result {
            Ok(Ok(value)) => return Ok(value),
            Ok(Err(error)) => {
                if attempt + 1 == config.max_attempts || !(config.retryable)(&error) {
                    return Err(error);
                }
                tokio::time::sleep(jittered_delay(delay, config.jitter)).await;
                delay = (delay * 2).min(config.max_delay);
            }
            Err(_elapsed) => {
                let error = BackendError::Timeout("retry attempt exceeded timeout".to_string());
                if attempt + 1 == config.max_attempts {
                    return Err(error);
                }
                tokio::time::sleep(jittered_delay(delay, config.jitter)).await;
                delay = (delay * 2).min(config.max_delay);
            }
        }
    }

    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn retry_succeeds_eventually() {
        let mut attempts = 0;
        let config = RetryConfig::for_test();

        let result = retry(&config, || {
            attempts += 1;
            async move {
                if attempts < 2 {
                    Err(BackendError::RequestFailed(
                        "test".to_string(),
                        "transient".to_string(),
                    ))
                } else {
                    Ok(42)
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempts, 2);
    }

    #[tokio::test]
    async fn retry_fails_after_max_attempts() {
        let config = RetryConfig {
            max_attempts: 2,
            ..RetryConfig::for_test()
        };

        let result = retry(&config, || async {
            Err::<i32, _>(BackendError::RequestFailed(
                "test".to_string(),
                "always".to_string(),
            ))
        })
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn non_retryable_error_fails_fast() {
        let mut attempts = 0;
        let config = RetryConfig::for_test();

        let result = retry(&config, || {
            attempts += 1;
            async move { Err::<i32, _>(BackendError::AuthRequired("test".to_string())) }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(attempts, 1);
    }

    #[tokio::test]
    async fn retry_times_out_slow_attempt() {
        let config = RetryConfig {
            max_attempts: 2,
            timeout_per_attempt: Duration::from_millis(10),
            base_delay: Duration::from_millis(1),
            ..Default::default()
        };

        let result = retry(&config, || async {
            tokio::time::sleep(Duration::from_secs(60)).await;
            Ok::<i32, BackendError>(42)
        })
        .await;

        assert!(matches!(result, Err(BackendError::Timeout(_))));
    }

    #[tokio::test]
    async fn retryable_timeout_is_retried() {
        let mut attempts = 0;
        let config = RetryConfig {
            max_attempts: 3,
            timeout_per_attempt: Duration::from_millis(10),
            base_delay: Duration::from_millis(1),
            ..Default::default()
        };

        let result = retry(&config, || {
            attempts += 1;
            async move {
                if attempts < 2 {
                    tokio::time::sleep(Duration::from_secs(60)).await;
                    Ok::<i32, BackendError>(42)
                } else {
                    Ok(42)
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempts, 2);
    }

    #[tokio::test]
    async fn retry_makes_exactly_max_attempts() {
        let mut attempts = 0;
        let config = RetryConfig {
            max_attempts: 4,
            base_delay: Duration::from_millis(1),
            ..RetryConfig::for_test()
        };

        let result = retry(&config, || {
            attempts += 1;
            async move {
                Err::<i32, _>(BackendError::RequestFailed(
                    "test".to_string(),
                    "always".to_string(),
                ))
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(attempts, config.max_attempts);
    }

    #[tokio::test(start_paused = true)]
    async fn retry_uses_exponential_backoff() {
        let mut attempts = 0;
        let config = RetryConfig {
            max_attempts: 4,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
            timeout_per_attempt: Duration::from_millis(100),
            jitter: false,
            retryable: default_retryable,
        };

        let start = tokio::time::Instant::now();
        let result = retry(&config, || {
            attempts += 1;
            async move {
                if attempts < 4 {
                    Err(BackendError::RequestFailed(
                        "test".to_string(),
                        "transient".to_string(),
                    ))
                } else {
                    Ok(42)
                }
            }
        })
        .await;

        let elapsed = start.elapsed();
        assert_eq!(result.unwrap(), 42);
        // Delays: 1ms, 2ms, 4ms = 7ms of backoff before the 4th attempt succeeds.
        assert!(elapsed >= Duration::from_millis(7), "elapsed: {elapsed:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn retry_backoff_respects_max_delay() {
        let mut attempts = 0;
        let config = RetryConfig {
            max_attempts: 4,
            base_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(15),
            timeout_per_attempt: Duration::from_millis(100),
            jitter: false,
            retryable: default_retryable,
        };

        let start = tokio::time::Instant::now();
        let result = retry(&config, || {
            attempts += 1;
            async move {
                if attempts < 4 {
                    Err(BackendError::RequestFailed(
                        "test".to_string(),
                        "transient".to_string(),
                    ))
                } else {
                    Ok(42)
                }
            }
        })
        .await;

        let elapsed = start.elapsed();
        assert_eq!(result.unwrap(), 42);
        // Delays: 10ms, 15ms (capped), 15ms (capped) = 40ms.
        assert!(elapsed >= Duration::from_millis(40), "elapsed: {elapsed:?}");
    }

    #[test]
    fn default_retryable_predicate() {
        assert!(default_retryable(&BackendError::RequestFailed(
            "b".to_string(),
            "e".to_string()
        )));
        assert!(default_retryable(&BackendError::Timeout("b".to_string())));
        assert!(default_retryable(&BackendError::CommandFailed(
            "b".to_string(),
            "e".to_string()
        )));

        assert!(!default_retryable(&BackendError::AuthRequired(
            "b".to_string()
        )));
        assert!(!default_retryable(&BackendError::NotFound("b".to_string())));
        assert!(!default_retryable(&BackendError::Parse(
            "b".to_string(),
            "e".to_string()
        )));
        assert!(!default_retryable(&BackendError::CommandNotFound(
            "b".to_string()
        )));
    }

    #[test]
    fn jitter_disabled_returns_exact_delay() {
        let delay = Duration::from_millis(100);
        assert_eq!(jittered_delay(delay, false), delay);
    }

    #[test]
    fn jitter_zero_delay_is_zero() {
        assert!(jittered_delay(Duration::ZERO, true).is_zero());
    }

    #[test]
    fn jitter_stays_within_bounds() {
        // Full jitter must always land in [0, delay], sampled many times.
        let delay = Duration::from_millis(100);
        for _ in 0..1_000 {
            let d = jittered_delay(delay, true);
            assert!(d <= delay, "jittered delay {d:?} exceeded {delay:?}");
        }
    }
}
