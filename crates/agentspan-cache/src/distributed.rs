//! L3 distributed cache.
//!
//! By default this is a no-op placeholder so the L1/L2 tiers work without any
//! external service. Enable the `redis` feature to back it with a real Redis
//! server (single node or cluster endpoint) for cross-instance caching.

use std::time::Duration;

use agentspan_core::error::Error;
use async_trait::async_trait;

/// L3 distributed cache (no-op stub).
#[cfg(not(feature = "redis"))]
#[derive(Debug, Clone)]
pub struct DistributedCache {
    _url: String,
    _default_ttl: Duration,
}

#[cfg(not(feature = "redis"))]
impl DistributedCache {
    /// Create a new distributed cache connection.
    pub fn new(url: impl Into<String>, default_ttl: Duration) -> Self {
        Self {
            _url: url.into(),
            _default_ttl: default_ttl,
        }
    }
}

#[cfg(not(feature = "redis"))]
#[async_trait]
impl agentspan_core::cache::Cache for DistributedCache {
    async fn get(&self, _key: &str) -> Result<Option<Vec<u8>>, Error> {
        tracing::debug!("L3 distributed cache get (stub)");
        Ok(None)
    }

    async fn set(&self, _key: &str, _value: Vec<u8>, _ttl_seconds: u64) -> Result<(), Error> {
        tracing::debug!("L3 distributed cache set (stub)");
        Ok(())
    }
}

/// L3 distributed cache backed by Redis.
#[cfg(feature = "redis")]
#[derive(Clone)]
pub struct DistributedCache {
    client: redis::Client,
    default_ttl: Duration,
    conn: std::sync::Arc<tokio::sync::Mutex<Option<redis::aio::MultiplexedConnection>>>,
}

#[cfg(feature = "redis")]
impl std::fmt::Debug for DistributedCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DistributedCache")
            .field("default_ttl", &self.default_ttl)
            .finish()
    }
}

#[cfg(feature = "redis")]
impl DistributedCache {
    /// Open a Redis-backed distributed cache. Connections are established lazily.
    pub fn new(url: impl Into<String>, default_ttl: Duration) -> Self {
        let url = url.into();
        let client = redis::Client::open(url).expect("invalid redis url");
        Self {
            client,
            default_ttl,
            conn: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    async fn conn(&self) -> Result<redis::aio::MultiplexedConnection, Error> {
        let mut guard = self.conn.lock().await;
        if let Some(c) = guard.as_ref() {
            return Ok(c.clone());
        }
        let c = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| Error::Backend(format!("redis connect: {e}")))?;
        *guard = Some(c.clone());
        Ok(c)
    }
}

#[cfg(feature = "redis")]
#[async_trait]
impl agentspan_core::cache::Cache for DistributedCache {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Error> {
        use redis::AsyncCommands;
        let mut conn = self.conn().await?;
        conn.get(key)
            .await
            .map_err(|e| Error::Backend(format!("redis get: {e}")))
    }

    async fn set(&self, key: &str, value: Vec<u8>, ttl_seconds: u64) -> Result<(), Error> {
        use redis::AsyncCommands;
        let mut conn = self.conn().await?;
        let ttl = if ttl_seconds == 0 {
            self.default_ttl.as_secs().max(1)
        } else {
            ttl_seconds
        };
        conn.set_ex::<_, _, ()>(key, value, ttl)
            .await
            .map_err(|e| Error::Backend(format!("redis set: {e}")))
    }

    async fn remove(&self, key: &str) -> Result<(), Error> {
        use redis::AsyncCommands;
        let mut conn = self.conn().await?;
        conn.del::<_, ()>(key)
            .await
            .map_err(|e| Error::Backend(format!("redis del: {e}")))
    }

    async fn remove_prefix(&self, prefix: &str) -> Result<u64, Error> {
        use redis::AsyncCommands;
        let mut conn = self.conn().await?;
        let pattern = format!("{prefix}*");
        let keys: Vec<String> = conn
            .keys(&pattern)
            .await
            .map_err(|e| Error::Backend(format!("redis keys: {e}")))?;
        let mut removed = 0u64;
        for key in keys {
            if conn.del::<_, ()>(&key).await.is_ok() {
                removed += 1;
            }
        }
        Ok(removed)
    }
}

#[cfg(all(test, not(feature = "redis")))]
mod tests {
    use super::*;
    use agentspan_core::cache::Cache;

    #[tokio::test]
    async fn distributed_stub_always_misses() {
        let cache = DistributedCache::new("redis://localhost", Duration::from_secs(60));
        cache.set("k", b"v".to_vec(), 60).await.unwrap();
        let value = cache.get("k").await.unwrap();
        assert_eq!(value, None);
    }
}
