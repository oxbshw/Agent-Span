//! Multi-tenancy: per-tenant channel access, quotas, and cache policy.

use dashmap::DashMap;
use serde::{Deserialize, Serialize};

/// Per-tenant request quota.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quota {
    /// Maximum requests per rolling minute. `0` means unlimited.
    pub max_requests_per_minute: u32,
    /// Maximum requests per rolling day. `0` means unlimited.
    pub max_requests_per_day: u32,
}

impl Default for Quota {
    fn default() -> Self {
        // Generous single-user defaults.
        Self {
            max_requests_per_minute: 120,
            max_requests_per_day: 50_000,
        }
    }
}

/// Per-tenant configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenantConfig {
    /// Channels this tenant may use. Empty means "all channels".
    #[serde(default)]
    pub enabled_channels: Vec<String>,
    /// Whether responses for this tenant may be cached.
    pub cache_enabled: bool,
}

impl Default for TenantConfig {
    fn default() -> Self {
        Self {
            enabled_channels: Vec::new(),
            cache_enabled: true,
        }
    }
}

/// A tenant: an isolated unit of access, quota, and configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tenant {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub config: TenantConfig,
    #[serde(default)]
    pub quota: Quota,
}

impl Tenant {
    /// Construct a tenant with default config and quota.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            config: TenantConfig::default(),
            quota: Quota::default(),
        }
    }

    /// True when this tenant may use the named channel.
    pub fn allows_channel(&self, channel: &str) -> bool {
        self.config.enabled_channels.is_empty()
            || self.config.enabled_channels.iter().any(|c| c == channel)
    }
}

/// In-memory tenant registry, always seeded with a `"default"` tenant.
#[derive(Debug)]
pub struct TenantManager {
    tenants: DashMap<String, Tenant>,
}

impl Default for TenantManager {
    fn default() -> Self {
        let tenants = DashMap::new();
        tenants.insert(
            "default".to_string(),
            Tenant::new("default", "Default Tenant"),
        );
        Self { tenants }
    }
}

impl TenantManager {
    /// Create a manager seeded with the default tenant.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace a tenant.
    pub fn upsert(&self, tenant: Tenant) {
        self.tenants.insert(tenant.id.clone(), tenant);
    }

    /// Fetch a tenant by id.
    pub fn get(&self, id: &str) -> Option<Tenant> {
        self.tenants.get(id).map(|t| t.clone())
    }

    /// Remove a tenant (the `"default"` tenant cannot be removed).
    pub fn remove(&self, id: &str) -> bool {
        if id == "default" {
            return false;
        }
        self.tenants.remove(id).is_some()
    }

    /// List all tenants.
    pub fn list(&self) -> Vec<Tenant> {
        self.tenants.iter().map(|t| t.clone()).collect()
    }

    /// Convenience: whether a tenant exists and allows a channel.
    pub fn allows_channel(&self, tenant_id: &str, channel: &str) -> bool {
        self.get(tenant_id)
            .map(|t| t.allows_channel(channel))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_manager_has_default_tenant() {
        let mgr = TenantManager::new();
        assert!(mgr.get("default").is_some());
        assert_eq!(mgr.list().len(), 1);
    }

    #[test]
    fn empty_enabled_channels_means_all() {
        let t = Tenant::new("t1", "T1");
        assert!(t.allows_channel("anything"));
    }

    #[test]
    fn explicit_channels_are_enforced() {
        let mut t = Tenant::new("t1", "T1");
        t.config.enabled_channels = vec!["web".into(), "github".into()];
        assert!(t.allows_channel("web"));
        assert!(!t.allows_channel("twitter"));
    }

    #[test]
    fn upsert_and_remove() {
        let mgr = TenantManager::new();
        mgr.upsert(Tenant::new("acme", "Acme"));
        assert!(mgr.get("acme").is_some());
        assert!(mgr.remove("acme"));
        assert!(mgr.get("acme").is_none());
    }

    #[test]
    fn default_tenant_cannot_be_removed() {
        let mgr = TenantManager::new();
        assert!(!mgr.remove("default"));
        assert!(mgr.get("default").is_some());
    }

    #[test]
    fn allows_channel_false_for_unknown_tenant() {
        let mgr = TenantManager::new();
        assert!(!mgr.allows_channel("ghost", "web"));
    }
}
