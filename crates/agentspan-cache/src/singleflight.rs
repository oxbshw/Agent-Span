//! Single-flight request coalescing.
//!
//! When several callers ask for the same thing at once, only the first does the
//! work; the rest wait and share its result. This collapses dogpiles — e.g. five
//! agents requesting the same URL in the same second hit upstream once.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Mutex;

use tokio::sync::broadcast;

/// Coalesces concurrent calls keyed by a string. `V` is `Clone` because the
/// result is fanned out to every waiter.
pub struct SingleFlight<V: Clone> {
    inflight: Mutex<HashMap<String, broadcast::Sender<Option<V>>>>,
}

impl<V: Clone> Default for SingleFlight<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V: Clone> std::fmt::Debug for SingleFlight<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SingleFlight")
            .field("inflight", &self.len())
            .finish()
    }
}

impl<V: Clone> SingleFlight<V> {
    /// Create an empty coalescer.
    pub fn new() -> Self {
        Self {
            inflight: Mutex::new(HashMap::new()),
        }
    }

    /// Number of keys currently in flight.
    pub fn len(&self) -> usize {
        self.inflight.lock().expect("singleflight poisoned").len()
    }

    /// True when nothing is in flight.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Run `f` for `key`, or — if an identical call is already running — wait for
    /// and share its result instead of doing the work again.
    ///
    /// A `None` result is shared like any other (so a coalesced failure fails all
    /// waiters together); but a waiter that finds the leader gone before a value
    /// arrives falls back to running `f` itself, so no caller is ever stranded.
    pub async fn run<F, Fut>(&self, key: &str, f: F) -> Option<V>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Option<V>>,
    {
        // Join an existing flight, or become the leader for this key.
        let mut follower = {
            let mut map = self.inflight.lock().expect("singleflight poisoned");
            match map.get(key) {
                Some(tx) => Some(tx.subscribe()),
                None => {
                    let (tx, _rx) = broadcast::channel(1);
                    map.insert(key.to_string(), tx);
                    None
                }
            }
        };

        match follower.as_mut() {
            Some(rx) => match rx.recv().await {
                Ok(value) => value,
                Err(_) => f().await,
            },
            None => {
                let result = f().await;
                // Clear the slot before publishing so the next call starts fresh.
                let tx = self
                    .inflight
                    .lock()
                    .expect("singleflight poisoned")
                    .remove(key);
                if let Some(tx) = tx {
                    let _ = tx.send(result.clone());
                }
                result
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test]
    async fn coalesces_concurrent_calls_to_one_execution() {
        let sf: Arc<SingleFlight<u64>> = Arc::new(SingleFlight::new());
        let runs = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..12 {
            let sf = sf.clone();
            let runs = runs.clone();
            handles.push(tokio::spawn(async move {
                sf.run("same-key", || async move {
                    runs.fetch_add(1, Ordering::SeqCst);
                    // Hold the flight open long enough for the others to join.
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    Some(42u64)
                })
                .await
            }));
        }

        for h in handles {
            assert_eq!(h.await.unwrap(), Some(42));
        }
        // Twelve concurrent callers, one actual execution.
        assert_eq!(runs.load(Ordering::SeqCst), 1);
        assert!(sf.is_empty(), "slot should be cleared afterwards");
    }

    #[tokio::test]
    async fn distinct_keys_run_independently() {
        let sf: SingleFlight<u64> = SingleFlight::new();
        let a = sf.run("a", || async { Some(1) }).await;
        let b = sf.run("b", || async { Some(2) }).await;
        assert_eq!(a, Some(1));
        assert_eq!(b, Some(2));
    }

    #[tokio::test]
    async fn sequential_calls_each_execute() {
        // Once a flight finishes, the next call for the same key runs again.
        let sf: SingleFlight<u64> = SingleFlight::new();
        let runs = Arc::new(AtomicUsize::new(0));
        for _ in 0..3 {
            let runs = runs.clone();
            sf.run("k", || async move {
                runs.fetch_add(1, Ordering::SeqCst);
                Some(0)
            })
            .await;
        }
        assert_eq!(runs.load(Ordering::SeqCst), 3);
    }
}
