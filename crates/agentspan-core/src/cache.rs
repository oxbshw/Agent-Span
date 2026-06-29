//! Caching abstractions.

use async_trait::async_trait;

use crate::error::Error;

/// Multi-tier cache.
///
/// The two required methods (`get`/`set`) form the minimal contract every tier
/// must satisfy. The remaining methods power invalidation, background sweeping,
/// and size estimation; they ship with sensible no-op defaults so that simple or
/// remote backends (e.g. a stubbed L3) can opt out without breaking callers.
#[async_trait]
pub trait Cache: Send + Sync {
    /// Get a cached value by key.
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Error>;

    /// Set a cached value with a TTL in seconds.
    async fn set(&self, key: &str, value: Vec<u8>, ttl_seconds: u64) -> Result<(), Error>;

    /// Remove a single key. Default: no-op.
    async fn remove(&self, _key: &str) -> Result<(), Error> {
        Ok(())
    }

    /// Remove every key whose stored representation starts with `prefix`.
    ///
    /// Returns the number of entries removed. Default: removes nothing.
    async fn remove_prefix(&self, _prefix: &str) -> Result<u64, Error> {
        Ok(0)
    }

    /// Remove all entries. Default: no-op.
    async fn clear(&self) -> Result<(), Error> {
        Ok(())
    }

    /// Evict expired entries, returning the number removed. Default: removes nothing.
    async fn sweep(&self) -> Result<u64, Error> {
        Ok(0)
    }

    /// Approximate number of live entries. Default: `0` (unknown).
    async fn entry_count(&self) -> u64 {
        0
    }

    /// Approximate stored payload size in bytes. Default: `0` (unknown).
    async fn size_bytes(&self) -> u64 {
        0
    }
}
