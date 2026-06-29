//! L1 in-memory cache using a concurrent hash map.

use std::sync::Arc;
use std::time::{Duration, Instant};

use agentspan_core::error::Error;
use async_trait::async_trait;
use dashmap::DashMap;

use crate::key::CacheKey;

/// In-memory cache entry with expiration.
#[derive(Debug, Clone)]
struct Entry {
    value: Vec<u8>,
    expires_at: Instant,
}

/// L1 hot cache.
#[derive(Debug, Clone, Default)]
pub struct MemoryCache {
    inner: Arc<DashMap<CacheKey, Entry>>,
}

impl MemoryCache {
    /// Create a new memory cache.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
        }
    }

    /// Remove expired entries (best-effort cleanup).
    pub fn cleanup(&self) {
        let now = Instant::now();
        self.inner.retain(|_, entry| entry.expires_at > now);
    }
}

#[async_trait]
impl agentspan_core::cache::Cache for MemoryCache {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Error> {
        let key = CacheKey::raw(key);
        if let Some(entry) = self.inner.get(&key) {
            if entry.expires_at > Instant::now() {
                return Ok(Some(entry.value.clone()));
            }
        }
        Ok(None)
    }

    async fn set(&self, key: &str, value: Vec<u8>, ttl_seconds: u64) -> Result<(), Error> {
        let ttl = Duration::from_secs(ttl_seconds);
        let entry = Entry {
            value,
            expires_at: Instant::now() + ttl,
        };
        self.inner.insert(CacheKey::raw(key), entry);
        Ok(())
    }

    async fn remove(&self, key: &str) -> Result<(), Error> {
        self.inner.remove(&CacheKey::raw(key));
        Ok(())
    }

    async fn remove_prefix(&self, prefix: &str) -> Result<u64, Error> {
        let doomed: Vec<CacheKey> = self
            .inner
            .iter()
            .filter(|e| e.key().0.starts_with(prefix))
            .map(|e| e.key().clone())
            .collect();
        let removed = doomed.len() as u64;
        for key in doomed {
            self.inner.remove(&key);
        }
        Ok(removed)
    }

    async fn clear(&self) -> Result<(), Error> {
        self.inner.clear();
        Ok(())
    }

    async fn sweep(&self) -> Result<u64, Error> {
        let before = self.inner.len() as u64;
        let now = Instant::now();
        self.inner.retain(|_, entry| entry.expires_at > now);
        Ok(before - self.inner.len() as u64)
    }

    async fn entry_count(&self) -> u64 {
        self.inner.len() as u64
    }

    async fn size_bytes(&self) -> u64 {
        self.inner
            .iter()
            .map(|e| e.value().value.len() as u64)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentspan_core::cache::Cache;

    #[tokio::test]
    async fn set_and_get_value() {
        let cache = MemoryCache::new();
        cache.set("k", b"hello".to_vec(), 60).await.unwrap();
        let value = cache.get("k").await.unwrap();
        assert_eq!(value, Some(b"hello".to_vec()));
    }

    #[tokio::test]
    async fn expired_value_returns_none() {
        let cache = MemoryCache::new();
        cache.set("k", b"hello".to_vec(), 0).await.unwrap();
        // Give the runtime a moment to ensure the expiry is in the past.
        tokio::time::sleep(Duration::from_millis(10)).await;
        let value = cache.get("k").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn remove_deletes_single_key() {
        let cache = MemoryCache::new();
        cache.set("k", b"v".to_vec(), 60).await.unwrap();
        cache.remove("k").await.unwrap();
        assert_eq!(cache.get("k").await.unwrap(), None);
    }

    #[tokio::test]
    async fn remove_prefix_deletes_matching_keys() {
        let cache = MemoryCache::new();
        cache
            .set("youtube:read:1", b"a".to_vec(), 60)
            .await
            .unwrap();
        cache
            .set("youtube:read:2", b"b".to_vec(), 60)
            .await
            .unwrap();
        cache.set("github:read:3", b"c".to_vec(), 60).await.unwrap();

        let removed = cache.remove_prefix("youtube:").await.unwrap();
        assert_eq!(removed, 2);
        assert_eq!(cache.get("youtube:read:1").await.unwrap(), None);
        assert_eq!(
            cache.get("github:read:3").await.unwrap(),
            Some(b"c".to_vec())
        );
    }

    #[tokio::test]
    async fn clear_removes_everything() {
        let cache = MemoryCache::new();
        cache.set("a", b"1".to_vec(), 60).await.unwrap();
        cache.set("b", b"2".to_vec(), 60).await.unwrap();
        cache.clear().await.unwrap();
        assert_eq!(cache.entry_count().await, 0);
    }

    #[tokio::test]
    async fn sweep_evicts_only_expired_entries() {
        let cache = MemoryCache::new();
        cache.set("live", b"1".to_vec(), 60).await.unwrap();
        cache.set("dead", b"2".to_vec(), 0).await.unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;

        let removed = cache.sweep().await.unwrap();
        assert_eq!(removed, 1);
        assert_eq!(cache.get("live").await.unwrap(), Some(b"1".to_vec()));
    }

    #[tokio::test]
    async fn size_bytes_sums_payload_lengths() {
        let cache = MemoryCache::new();
        cache.set("a", b"hello".to_vec(), 60).await.unwrap();
        cache.set("b", b"hi".to_vec(), 60).await.unwrap();
        assert_eq!(cache.size_bytes().await, 7);
        assert_eq!(cache.entry_count().await, 2);
    }
}
