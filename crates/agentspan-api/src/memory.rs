//! Agent memory: a small namespaced key/value scratchpad for agents to persist
//! state across requests (notes, cursors, dedup sets, ...).
//!
//! In-memory and process-local — values survive across requests but not a
//! restart. Each entry may carry a TTL; expired entries are dropped lazily on
//! access. This is deliberately simple: a durable backend (the L2/L3 cache) can
//! be layered later without changing the API.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;

/// One stored value plus its optional expiry.
#[derive(Clone, Debug)]
struct Entry {
    value: Value,
    expires_at: Option<Instant>,
}

impl Entry {
    fn is_expired(&self, now: Instant) -> bool {
        self.expires_at.is_some_and(|e| e <= now)
    }
}

/// A namespaced key/value store shared across the API via `AppState`.
#[derive(Clone, Default)]
pub struct MemoryStore {
    inner: Arc<Mutex<HashMap<String, HashMap<String, Entry>>>>,
}

impl MemoryStore {
    /// Create an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set `key` in `namespace` to `value`, optionally expiring after `ttl`.
    pub fn set(&self, namespace: &str, key: &str, value: Value, ttl: Option<Duration>) {
        let expires_at = ttl.map(|d| Instant::now() + d);
        let mut guard = self.inner.lock().unwrap();
        guard
            .entry(namespace.to_string())
            .or_default()
            .insert(key.to_string(), Entry { value, expires_at });
    }

    /// Get the value for `key` in `namespace`, if present and not expired.
    pub fn get(&self, namespace: &str, key: &str) -> Option<Value> {
        let now = Instant::now();
        let mut guard = self.inner.lock().unwrap();
        let ns = guard.get_mut(namespace)?;
        match ns.get(key) {
            Some(entry) if entry.is_expired(now) => {
                ns.remove(key);
                None
            }
            Some(entry) => Some(entry.value.clone()),
            None => None,
        }
    }

    /// List the live (non-expired) keys in `namespace`, sorted.
    pub fn list(&self, namespace: &str) -> Vec<String> {
        let now = Instant::now();
        let mut guard = self.inner.lock().unwrap();
        let Some(ns) = guard.get_mut(namespace) else {
            return Vec::new();
        };
        ns.retain(|_, e| !e.is_expired(now));
        let mut keys: Vec<String> = ns.keys().cloned().collect();
        keys.sort();
        keys
    }

    /// Delete `key` from `namespace`; returns whether it existed (and was live).
    pub fn delete(&self, namespace: &str, key: &str) -> bool {
        let now = Instant::now();
        let mut guard = self.inner.lock().unwrap();
        let Some(ns) = guard.get_mut(namespace) else {
            return false;
        };
        match ns.remove(key) {
            Some(entry) => !entry.is_expired(now),
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn set_get_roundtrip() {
        let store = MemoryStore::new();
        store.set("agent1", "cursor", json!({"page": 3}), None);
        assert_eq!(store.get("agent1", "cursor"), Some(json!({"page": 3})));
        assert_eq!(store.get("agent1", "missing"), None);
        assert_eq!(store.get("other", "cursor"), None);
    }

    #[test]
    fn overwrite_replaces_value() {
        let store = MemoryStore::new();
        store.set("ns", "k", json!(1), None);
        store.set("ns", "k", json!(2), None);
        assert_eq!(store.get("ns", "k"), Some(json!(2)));
    }

    #[test]
    fn list_returns_sorted_live_keys() {
        let store = MemoryStore::new();
        store.set("ns", "b", json!(1), None);
        store.set("ns", "a", json!(1), None);
        assert_eq!(store.list("ns"), vec!["a".to_string(), "b".to_string()]);
        assert!(store.list("empty").is_empty());
    }

    #[test]
    fn delete_reports_existence() {
        let store = MemoryStore::new();
        store.set("ns", "k", json!(1), None);
        assert!(store.delete("ns", "k"));
        assert!(!store.delete("ns", "k"));
        assert_eq!(store.get("ns", "k"), None);
    }

    #[test]
    fn expired_entries_are_dropped() {
        let store = MemoryStore::new();
        store.set("ns", "k", json!("v"), Some(Duration::from_millis(0)));
        // A zero TTL expires immediately (expires_at <= now).
        assert_eq!(store.get("ns", "k"), None);
        assert!(store.list("ns").is_empty());
    }
}
