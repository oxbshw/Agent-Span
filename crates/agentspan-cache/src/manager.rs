//! Three-tier cache orchestration.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use agentspan_core::cache::Cache;
use agentspan_core::error::Error;
use tokio::task::JoinHandle;
use tracing::{debug, instrument};

use crate::key::CacheKey;

/// Which cache tier satisfied a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheTier {
    /// L1 in-memory hit.
    L1,
    /// L2 disk hit.
    L2,
    /// L3 distributed hit.
    L3,
    /// Not cached; fetched from backend.
    Backend,
}

/// Time-to-live presets for cached content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheTtl {
    /// Frequently accessed, short-lived (60s).
    Hot,
    /// Normal content (1 hour).
    Warm,
    /// Rarely changing content (24 hours).
    Cold,
    /// An explicit duration.
    Custom(Duration),
}

impl CacheTtl {
    /// Resolve this preset to a concrete duration.
    pub fn as_duration(self) -> Duration {
        match self {
            CacheTtl::Hot => Duration::from_secs(60),
            CacheTtl::Warm => Duration::from_secs(3600),
            CacheTtl::Cold => Duration::from_secs(86_400),
            CacheTtl::Custom(d) => d,
        }
    }
}

/// Result of a cached get operation.
#[derive(Debug, Clone)]
pub struct CachedValue {
    pub value: Vec<u8>,
    pub tier: CacheTier,
}

/// Estimated cache footprint across all tiers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CacheSize {
    pub l1_entries: u64,
    pub l1_bytes: u64,
    pub l2_entries: u64,
    pub l2_bytes: u64,
    pub l3_entries: u64,
    pub l3_bytes: u64,
}

impl CacheSize {
    /// Total entries across all tiers.
    pub fn total_entries(&self) -> u64 {
        self.l1_entries + self.l2_entries + self.l3_entries
    }

    /// Total bytes across all tiers.
    pub fn total_bytes(&self) -> u64 {
        self.l1_bytes + self.l2_bytes + self.l3_bytes
    }
}

/// Atomic counters tracking cache behaviour.
#[derive(Debug, Default)]
struct Counters {
    l1_hits: AtomicU64,
    l2_hits: AtomicU64,
    l3_hits: AtomicU64,
    misses: AtomicU64,
    sets: AtomicU64,
    invalidations: AtomicU64,
    evictions: AtomicU64,
}

/// Point-in-time snapshot of cache metrics.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CacheMetrics {
    pub l1_hits: u64,
    pub l2_hits: u64,
    pub l3_hits: u64,
    pub misses: u64,
    pub sets: u64,
    pub invalidations: u64,
    pub evictions: u64,
}

impl CacheMetrics {
    /// Total hits across all tiers.
    pub fn hits(&self) -> u64 {
        self.l1_hits + self.l2_hits + self.l3_hits
    }

    /// Hit rate in `[0.0, 1.0]`. Returns `0.0` when there have been no lookups.
    pub fn hit_rate(&self) -> f64 {
        let lookups = self.hits() + self.misses;
        if lookups == 0 {
            0.0
        } else {
            self.hits() as f64 / lookups as f64
        }
    }
}

/// Orchestrates L1 / L2 / L3 caches.
#[derive(Clone)]
pub struct CacheManager {
    l1: Option<Arc<dyn Cache>>,
    l2: Option<Arc<dyn Cache>>,
    l3: Option<Arc<dyn Cache>>,
    l1_ttl: Duration,
    l2_ttl: Duration,
    l3_ttl: Duration,
    counters: Arc<Counters>,
}

impl std::fmt::Debug for CacheManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CacheManager")
            .field("l1", &self.l1.is_some())
            .field("l2", &self.l2.is_some())
            .field("l3", &self.l3.is_some())
            .field("l1_ttl", &self.l1_ttl)
            .field("l2_ttl", &self.l2_ttl)
            .field("l3_ttl", &self.l3_ttl)
            .finish()
    }
}

impl CacheManager {
    /// Create a cache manager with the given tiers and TTLs.
    pub fn new(
        l1: Option<Arc<dyn Cache>>,
        l2: Option<Arc<dyn Cache>>,
        l3: Option<Arc<dyn Cache>>,
        l1_ttl: Duration,
        l2_ttl: Duration,
        l3_ttl: Duration,
    ) -> Self {
        Self {
            l1,
            l2,
            l3,
            l1_ttl,
            l2_ttl,
            l3_ttl,
            counters: Arc::new(Counters::default()),
        }
    }

    /// Build a cache key for a channel operation.
    pub fn key(channel: &str, op: &str, arg: &str) -> CacheKey {
        CacheKey::new(channel, op, arg)
    }

    /// Read through caches L1 → L2 → L3, backfilling warmer tiers on a hit.
    #[instrument(skip(self), fields(key))]
    pub async fn get(&self, key: &str) -> Result<Option<CachedValue>, Error> {
        if let Some(l1) = &self.l1 {
            if let Some(value) = l1.get(key).await? {
                debug!("L1 cache hit");
                self.counters.l1_hits.fetch_add(1, Ordering::Relaxed);
                return Ok(Some(CachedValue {
                    value,
                    tier: CacheTier::L1,
                }));
            }
        }

        if let Some(l2) = &self.l2 {
            if let Some(value) = l2.get(key).await? {
                debug!("L2 cache hit");
                self.counters.l2_hits.fetch_add(1, Ordering::Relaxed);
                if let Some(l1) = &self.l1 {
                    let _ = l1.set(key, value.clone(), self.l1_ttl.as_secs()).await;
                }
                return Ok(Some(CachedValue {
                    value,
                    tier: CacheTier::L2,
                }));
            }
        }

        if let Some(l3) = &self.l3 {
            if let Some(value) = l3.get(key).await? {
                debug!("L3 cache hit");
                self.counters.l3_hits.fetch_add(1, Ordering::Relaxed);
                if let Some(l1) = &self.l1 {
                    let _ = l1.set(key, value.clone(), self.l1_ttl.as_secs()).await;
                }
                if let Some(l2) = &self.l2 {
                    let _ = l2.set(key, value.clone(), self.l2_ttl.as_secs()).await;
                }
                return Ok(Some(CachedValue {
                    value,
                    tier: CacheTier::L3,
                }));
            }
        }

        self.counters.misses.fetch_add(1, Ordering::Relaxed);
        Ok(None)
    }

    /// Write to all configured tiers using their default TTLs.
    #[instrument(skip(self, value), fields(key))]
    pub async fn set(&self, key: &str, value: Vec<u8>) -> Result<(), Error> {
        if let Some(l1) = &self.l1 {
            l1.set(key, value.clone(), self.l1_ttl.as_secs()).await?;
        }
        if let Some(l2) = &self.l2 {
            l2.set(key, value.clone(), self.l2_ttl.as_secs()).await?;
        }
        if let Some(l3) = &self.l3 {
            l3.set(key, value, self.l3_ttl.as_secs()).await?;
        }
        self.counters.sets.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Write to all tiers using an explicit TTL preset (clamped per-tier so a
    /// warmer tier never outlives a colder one when a short TTL is requested).
    pub async fn set_with_ttl(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: CacheTtl,
    ) -> Result<(), Error> {
        let secs = ttl.as_duration().as_secs().max(1);
        if let Some(l1) = &self.l1 {
            l1.set(key, value.clone(), secs.min(self.l1_ttl.as_secs()))
                .await?;
        }
        if let Some(l2) = &self.l2 {
            l2.set(key, value.clone(), secs).await?;
        }
        if let Some(l3) = &self.l3 {
            l3.set(key, value, secs).await?;
        }
        self.counters.sets.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Invalidate a single key across all tiers.
    pub async fn invalidate(&self, key: &str) -> Result<(), Error> {
        for tier in [&self.l1, &self.l2, &self.l3].into_iter().flatten() {
            tier.remove(key).await?;
        }
        self.counters.invalidations.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Invalidate every entry belonging to a channel (keys prefixed `"{channel}:"`).
    ///
    /// Returns the total number of entries removed across all tiers.
    pub async fn invalidate_channel(&self, channel_name: &str) -> Result<u64, Error> {
        let prefix = format!("{channel_name}:");
        let mut removed = 0;
        for tier in [&self.l1, &self.l2, &self.l3].into_iter().flatten() {
            removed += tier.remove_prefix(&prefix).await?;
        }
        self.counters.invalidations.fetch_add(1, Ordering::Relaxed);
        Ok(removed)
    }

    /// Clear every tier completely.
    pub async fn clear(&self) -> Result<(), Error> {
        for tier in [&self.l1, &self.l2, &self.l3].into_iter().flatten() {
            tier.clear().await?;
        }
        Ok(())
    }

    /// Estimate the cache footprint (entries + bytes) per tier.
    pub async fn estimate_size(&self) -> CacheSize {
        let mut size = CacheSize::default();
        if let Some(l1) = &self.l1 {
            size.l1_entries = l1.entry_count().await;
            size.l1_bytes = l1.size_bytes().await;
        }
        if let Some(l2) = &self.l2 {
            size.l2_entries = l2.entry_count().await;
            size.l2_bytes = l2.size_bytes().await;
        }
        if let Some(l3) = &self.l3 {
            size.l3_entries = l3.entry_count().await;
            size.l3_bytes = l3.size_bytes().await;
        }
        size
    }

    /// Snapshot the current metrics counters.
    pub fn metrics(&self) -> CacheMetrics {
        CacheMetrics {
            l1_hits: self.counters.l1_hits.load(Ordering::Relaxed),
            l2_hits: self.counters.l2_hits.load(Ordering::Relaxed),
            l3_hits: self.counters.l3_hits.load(Ordering::Relaxed),
            misses: self.counters.misses.load(Ordering::Relaxed),
            sets: self.counters.sets.load(Ordering::Relaxed),
            invalidations: self.counters.invalidations.load(Ordering::Relaxed),
            evictions: self.counters.evictions.load(Ordering::Relaxed),
        }
    }

    /// Run a single sweep across all tiers, evicting expired entries.
    ///
    /// Returns the number of entries reclaimed and records them as evictions.
    pub async fn sweep(&self) -> Result<u64, Error> {
        let mut removed = 0;
        for tier in [&self.l1, &self.l2, &self.l3].into_iter().flatten() {
            removed += tier.sweep().await?;
        }
        if removed > 0 {
            self.counters
                .evictions
                .fetch_add(removed, Ordering::Relaxed);
        }
        Ok(removed)
    }

    /// Spawn a background task that sweeps expired entries on a fixed interval.
    ///
    /// The returned [`JoinHandle`] can be aborted to stop the sweeper. The task
    /// holds only `Arc` clones, so it does not keep the manager's owner alive
    /// beyond the shared tier handles.
    pub fn spawn_sweeper(&self, interval: Duration) -> JoinHandle<()> {
        let manager = self.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            // Skip the immediate first tick so the first sweep waits `interval`.
            ticker.tick().await;
            loop {
                ticker.tick().await;
                if let Err(err) = manager.sweep().await {
                    debug!(error = %err, "cache sweep failed");
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disk::DiskCache;
    use crate::memory::MemoryCache;
    use agentspan_core::cache::Cache;

    fn ttls() -> (Duration, Duration, Duration) {
        (
            Duration::from_secs(60),
            Duration::from_secs(3600),
            Duration::from_secs(86_400),
        )
    }

    #[tokio::test]
    async fn manager_read_through_l1() {
        let (t1, t2, t3) = ttls();
        let l1 = Arc::new(MemoryCache::new());
        let manager = CacheManager::new(
            Some(l1.clone()),
            None::<Arc<dyn Cache>>,
            None::<Arc<dyn Cache>>,
            t1,
            t2,
            t3,
        );

        manager.set("k", b"v".to_vec()).await.unwrap();
        let cached = manager.get("k").await.unwrap().unwrap();
        assert_eq!(cached.value, b"v".to_vec());
        assert_eq!(cached.tier, CacheTier::L1);
    }

    #[tokio::test]
    async fn manager_backfills_l1_from_l2() {
        let (t1, t2, t3) = ttls();
        let l1 = Arc::new(MemoryCache::new());
        let l2 = Arc::new(MemoryCache::new());
        let manager = CacheManager::new(
            Some(l1.clone()),
            Some(l2.clone()),
            None::<Arc<dyn Cache>>,
            t1,
            t2,
            t3,
        );

        manager.set("k", b"v".to_vec()).await.unwrap();
        l1.remove("k").await.unwrap();

        let cached = manager.get("k").await.unwrap().unwrap();
        assert_eq!(cached.value, b"v".to_vec());
        assert_eq!(cached.tier, CacheTier::L2);
        assert_eq!(l1.get("k").await.unwrap(), Some(b"v".to_vec()));
    }

    #[tokio::test]
    async fn manager_invalidate_removes_from_all_tiers() {
        let (t1, t2, t3) = ttls();
        let l1 = Arc::new(MemoryCache::new());
        let l2 = Arc::new(MemoryCache::new());
        let manager = CacheManager::new(
            Some(l1.clone()),
            Some(l2.clone()),
            None::<Arc<dyn Cache>>,
            t1,
            t2,
            t3,
        );

        manager.set("k", b"v".to_vec()).await.unwrap();
        manager.invalidate("k").await.unwrap();
        assert!(manager.get("k").await.unwrap().is_none());
        assert_eq!(l1.get("k").await.unwrap(), None);
        assert_eq!(l2.get("k").await.unwrap(), None);
    }

    #[tokio::test]
    async fn manager_invalidate_channel_uses_prefix() {
        let (t1, t2, t3) = ttls();
        let l1 = Arc::new(MemoryCache::new());
        let manager = CacheManager::new(
            Some(l1.clone()),
            None::<Arc<dyn Cache>>,
            None::<Arc<dyn Cache>>,
            t1,
            t2,
            t3,
        );

        let key = CacheManager::key("youtube", "read", "abc");
        manager.set(&key.0, b"v".to_vec()).await.unwrap();
        let other = CacheManager::key("github", "read", "xyz");
        manager.set(&other.0, b"w".to_vec()).await.unwrap();

        let removed = manager.invalidate_channel("youtube").await.unwrap();
        assert_eq!(removed, 1);
        assert!(manager.get(&key.0).await.unwrap().is_none());
        assert!(manager.get(&other.0).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn manager_records_hit_and_miss_metrics() {
        let (t1, t2, t3) = ttls();
        let manager = CacheManager::new(
            Some(Arc::new(MemoryCache::new())),
            None::<Arc<dyn Cache>>,
            None::<Arc<dyn Cache>>,
            t1,
            t2,
            t3,
        );

        manager.set("k", b"v".to_vec()).await.unwrap();
        let _ = manager.get("k").await.unwrap(); // hit
        let _ = manager.get("missing").await.unwrap(); // miss

        let m = manager.metrics();
        assert_eq!(m.l1_hits, 1);
        assert_eq!(m.misses, 1);
        assert_eq!(m.sets, 1);
        assert!((m.hit_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn manager_estimate_size_aggregates_tiers() {
        let (t1, t2, t3) = ttls();
        let dir = tempfile::tempdir().unwrap();
        let l1 = Arc::new(MemoryCache::new());
        let l2 = Arc::new(DiskCache::new(dir.path()).await.unwrap());
        let manager = CacheManager::new(Some(l1), Some(l2), None::<Arc<dyn Cache>>, t1, t2, t3);

        manager.set("k", b"hello".to_vec()).await.unwrap();
        let size = manager.estimate_size().await;
        assert_eq!(size.l1_entries, 1);
        assert_eq!(size.l1_bytes, 5);
        assert_eq!(size.l2_entries, 1);
        assert!(size.total_entries() >= 2);
    }

    #[tokio::test]
    async fn manager_set_with_ttl_clamps_l1() {
        let (t1, t2, t3) = ttls();
        let manager = CacheManager::new(
            Some(Arc::new(MemoryCache::new())),
            None::<Arc<dyn Cache>>,
            None::<Arc<dyn Cache>>,
            t1,
            t2,
            t3,
        );
        manager
            .set_with_ttl("k", b"v".to_vec(), CacheTtl::Cold)
            .await
            .unwrap();
        assert!(manager.get("k").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn manager_sweep_counts_evictions() {
        let (_t1, t2, t3) = ttls();
        let manager = CacheManager::new(
            Some(Arc::new(MemoryCache::new())),
            None::<Arc<dyn Cache>>,
            None::<Arc<dyn Cache>>,
            Duration::from_millis(1),
            t2,
            t3,
        );
        manager.set("k", b"v".to_vec()).await.unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        let removed = manager.sweep().await.unwrap();
        assert_eq!(removed, 1);
        assert_eq!(manager.metrics().evictions, 1);
    }

    #[test]
    fn cache_ttl_durations() {
        assert_eq!(CacheTtl::Hot.as_duration(), Duration::from_secs(60));
        assert_eq!(CacheTtl::Warm.as_duration(), Duration::from_secs(3600));
        assert_eq!(CacheTtl::Cold.as_duration(), Duration::from_secs(86_400));
        assert_eq!(
            CacheTtl::Custom(Duration::from_secs(5)).as_duration(),
            Duration::from_secs(5)
        );
    }
}
