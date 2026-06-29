//! Audit logging — a bounded in-memory ring of request records.
//!
//! For local/single-node use this keeps the most recent N entries in memory.
//! A persistent backend (SQLite/PostgreSQL) can implement the same shape later.

use std::collections::VecDeque;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single audited request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: DateTime<Utc>,
    pub tenant_id: String,
    pub api_key_id: Option<String>,
    pub channel: Option<String>,
    pub backend: Option<String>,
    /// The URL read or query searched.
    pub target: String,
    pub status: u16,
    pub latency_ms: u64,
    pub cache_hit: bool,
}

impl AuditEntry {
    /// Start building an entry stamped at the current time.
    pub fn now(tenant_id: impl Into<String>, target: impl Into<String>) -> Self {
        Self {
            timestamp: Utc::now(),
            tenant_id: tenant_id.into(),
            api_key_id: None,
            channel: None,
            backend: None,
            target: target.into(),
            status: 0,
            latency_ms: 0,
            cache_hit: false,
        }
    }
}

/// A bounded, thread-safe audit log.
#[derive(Debug)]
pub struct AuditLog {
    entries: Mutex<VecDeque<AuditEntry>>,
    capacity: usize,
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::new(10_000)
    }
}

impl AuditLog {
    /// Create an audit log holding at most `capacity` recent entries.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Mutex::new(VecDeque::with_capacity(capacity.min(1024))),
            capacity: capacity.max(1),
        }
    }

    /// Record an entry, evicting the oldest if at capacity.
    pub fn record(&self, entry: AuditEntry) {
        let mut guard = self.entries.lock().expect("audit lock poisoned");
        if guard.len() >= self.capacity {
            guard.pop_front();
        }
        guard.push_back(entry);
    }

    /// Return up to `limit` most-recent entries, newest first.
    pub fn recent(&self, limit: usize) -> Vec<AuditEntry> {
        let guard = self.entries.lock().expect("audit lock poisoned");
        guard.iter().rev().take(limit).cloned().collect()
    }

    /// Return up to `limit` most-recent entries for a tenant, newest first.
    pub fn recent_for_tenant(&self, tenant_id: &str, limit: usize) -> Vec<AuditEntry> {
        let guard = self.entries.lock().expect("audit lock poisoned");
        guard
            .iter()
            .rev()
            .filter(|e| e.tenant_id == tenant_id)
            .take(limit)
            .cloned()
            .collect()
    }

    /// Total entries currently retained.
    pub fn len(&self) -> usize {
        self.entries.lock().expect("audit lock poisoned").len()
    }

    /// True when no entries are retained.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(tenant: &str, target: &str, status: u16) -> AuditEntry {
        let mut e = AuditEntry::now(tenant, target);
        e.status = status;
        e
    }

    #[test]
    fn record_and_recent_newest_first() {
        let log = AuditLog::new(10);
        log.record(entry("default", "a", 200));
        log.record(entry("default", "b", 404));
        let recent = log.recent(10);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].target, "b");
        assert_eq!(recent[1].target, "a");
    }

    #[test]
    fn capacity_evicts_oldest() {
        let log = AuditLog::new(2);
        log.record(entry("default", "a", 200));
        log.record(entry("default", "b", 200));
        log.record(entry("default", "c", 200));
        let recent = log.recent(10);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].target, "c");
        assert_eq!(recent[1].target, "b");
    }

    #[test]
    fn recent_for_tenant_filters() {
        let log = AuditLog::new(10);
        log.record(entry("a", "x", 200));
        log.record(entry("b", "y", 200));
        log.record(entry("a", "z", 200));
        let a = log.recent_for_tenant("a", 10);
        assert_eq!(a.len(), 2);
        assert!(a.iter().all(|e| e.tenant_id == "a"));
    }

    #[test]
    fn limit_is_respected() {
        let log = AuditLog::new(100);
        for i in 0..10 {
            log.record(entry("default", &format!("t{i}"), 200));
        }
        assert_eq!(log.recent(3).len(), 3);
        assert_eq!(log.len(), 10);
    }

    #[test]
    fn entry_now_defaults() {
        let e = AuditEntry::now("default", "https://x");
        assert_eq!(e.tenant_id, "default");
        assert_eq!(e.status, 0);
        assert!(!e.cache_hit);
        assert!(e.api_key_id.is_none());
    }
}
