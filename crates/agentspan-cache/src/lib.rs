//! Multi-tier caching for AgentSpan.

pub mod disk;
pub mod distributed;
pub mod key;
pub mod manager;
pub mod memory;
pub mod optimizer;
pub mod singleflight;

pub use disk::DiskCache;
pub use distributed::DistributedCache;
pub use key::CacheKey;
pub use manager::{CacheManager, CacheMetrics, CacheSize, CacheTier, CacheTtl, CachedValue};
pub use memory::MemoryCache;
pub use optimizer::{CacheOptimizer, TtlAdjustment};
pub use singleflight::SingleFlight;
