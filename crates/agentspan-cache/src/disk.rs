//! L2 disk cache using a directory of files keyed by cache key.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use agentspan_core::error::Error;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tracing::{debug, warn};

/// Disk cache entry metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Entry {
    value: Vec<u8>,
    expires_at: u64,
}

/// L2 persistent disk cache.
#[derive(Debug, Clone)]
pub struct DiskCache {
    root: PathBuf,
}

impl DiskCache {
    /// Create or open a disk cache at the given directory.
    pub async fn new<P: AsRef<Path>>(root: P) -> Result<Self, Error> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root)
            .await
            .map_err(|e| Error::Config(format!("failed to create disk cache directory: {}", e)))?;
        Ok(Self { root })
    }

    /// Sanitize a key (or key prefix) for filesystem safety.
    fn sanitize(key: &str) -> String {
        key.chars()
            .map(|c| match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' => c,
                _ => '_',
            })
            .collect()
    }

    fn path(&self, key: &str) -> PathBuf {
        self.root.join(format!("{}.bin", Self::sanitize(key)))
    }

    /// Iterate the file stems (sanitized keys) of all cache entries on disk.
    async fn entry_files(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let mut dir = match fs::read_dir(&self.root).await {
            Ok(d) => d,
            Err(_) => return files,
        };
        while let Ok(Some(entry)) = dir.next_entry().await {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("bin") {
                files.push(path);
            }
        }
        files
    }
}

#[async_trait]
impl agentspan_core::cache::Cache for DiskCache {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Error> {
        let path = self.path(key);
        if !path.exists() {
            return Ok(None);
        }

        let data = match fs::read(&path).await {
            Ok(d) => d,
            Err(e) => {
                warn!(error = %e, path = %path.display(), "failed to read disk cache entry");
                return Ok(None);
            }
        };

        let entry: Entry = match serde_json::from_slice(&data) {
            Ok(e) => e,
            Err(e) => {
                warn!(error = %e, path = %path.display(), "corrupt disk cache entry");
                let _ = fs::remove_file(&path).await;
                return Ok(None);
            }
        };

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if entry.expires_at <= now {
            debug!(path = %path.display(), "disk cache entry expired");
            let _ = fs::remove_file(&path).await;
            return Ok(None);
        }

        Ok(Some(entry.value))
    }

    async fn set(&self, key: &str, value: Vec<u8>, ttl_seconds: u64) -> Result<(), Error> {
        let expires_at = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            + ttl_seconds;
        let entry = Entry { value, expires_at };
        let data = serde_json::to_vec(&entry).map_err(|e| Error::Backend(e.to_string()))?;
        let path = self.path(key);
        fs::write(&path, data)
            .await
            .map_err(|e| Error::Backend(format!("failed to write disk cache: {}", e)))?;
        Ok(())
    }

    async fn remove(&self, key: &str) -> Result<(), Error> {
        let path = self.path(key);
        if path.exists() {
            let _ = fs::remove_file(&path).await;
        }
        Ok(())
    }

    async fn remove_prefix(&self, prefix: &str) -> Result<u64, Error> {
        let sanitized = Self::sanitize(prefix);
        let mut removed = 0u64;
        for path in self.entry_files().await {
            let matches = path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|stem| stem.starts_with(&sanitized))
                .unwrap_or(false);
            if matches && fs::remove_file(&path).await.is_ok() {
                removed += 1;
            }
        }
        Ok(removed)
    }

    async fn clear(&self) -> Result<(), Error> {
        for path in self.entry_files().await {
            let _ = fs::remove_file(&path).await;
        }
        Ok(())
    }

    async fn sweep(&self) -> Result<u64, Error> {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut removed = 0u64;
        for path in self.entry_files().await {
            let Ok(data) = fs::read(&path).await else {
                continue;
            };
            // Reclaim expired entries; a corrupt entry is treated as garbage.
            let should_remove = match serde_json::from_slice::<Entry>(&data) {
                Ok(entry) => entry.expires_at <= now,
                Err(_) => true,
            };
            if should_remove && fs::remove_file(&path).await.is_ok() {
                removed += 1;
            }
        }
        Ok(removed)
    }

    async fn entry_count(&self) -> u64 {
        self.entry_files().await.len() as u64
    }

    async fn size_bytes(&self) -> u64 {
        let mut total = 0u64;
        for path in self.entry_files().await {
            if let Ok(meta) = fs::metadata(&path).await {
                total += meta.len();
            }
        }
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentspan_core::cache::Cache;
    use std::time::Duration;

    #[tokio::test]
    async fn disk_set_and_get() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path()).await.unwrap();
        cache.set("k", b"hello".to_vec(), 3600).await.unwrap();
        let value = cache.get("k").await.unwrap();
        assert_eq!(value, Some(b"hello".to_vec()));
    }

    #[tokio::test]
    async fn disk_missing_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path()).await.unwrap();
        let value = cache.get("missing").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn disk_expired_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path()).await.unwrap();
        cache.set("k", b"hello".to_vec(), 0).await.unwrap();
        tokio::time::sleep(Duration::from_millis(20)).await;
        let value = cache.get("k").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn disk_remove_deletes_entry() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path()).await.unwrap();
        cache.set("k", b"v".to_vec(), 3600).await.unwrap();
        cache.remove("k").await.unwrap();
        assert_eq!(cache.get("k").await.unwrap(), None);
    }

    #[tokio::test]
    async fn disk_remove_prefix_matches_sanitized_channel() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path()).await.unwrap();
        cache
            .set("youtube:read:1", b"a".to_vec(), 3600)
            .await
            .unwrap();
        cache
            .set("youtube:read:2", b"b".to_vec(), 3600)
            .await
            .unwrap();
        cache
            .set("github:read:3", b"c".to_vec(), 3600)
            .await
            .unwrap();

        let removed = cache.remove_prefix("youtube:").await.unwrap();
        assert_eq!(removed, 2);
        assert_eq!(
            cache.get("github:read:3").await.unwrap(),
            Some(b"c".to_vec())
        );
    }

    #[tokio::test]
    async fn disk_clear_and_count() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path()).await.unwrap();
        cache.set("a", b"1".to_vec(), 3600).await.unwrap();
        cache.set("b", b"2".to_vec(), 3600).await.unwrap();
        assert_eq!(cache.entry_count().await, 2);
        cache.clear().await.unwrap();
        assert_eq!(cache.entry_count().await, 0);
    }

    #[tokio::test]
    async fn disk_sweep_evicts_expired() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path()).await.unwrap();
        cache.set("live", b"1".to_vec(), 3600).await.unwrap();
        cache.set("dead", b"2".to_vec(), 0).await.unwrap();
        tokio::time::sleep(Duration::from_millis(20)).await;

        let removed = cache.sweep().await.unwrap();
        assert_eq!(removed, 1);
        assert_eq!(cache.get("live").await.unwrap(), Some(b"1".to_vec()));
    }
}
