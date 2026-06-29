//! API key generation, hashing, and validation.
//!
//! Keys are random 32-byte secrets, returned to the caller exactly once at
//! creation time. Only the SHA-256 hash of a key is ever stored, so a leak of
//! the backing store does not expose usable credentials.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Permission scope attached to an API key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Scope {
    /// May call read endpoints.
    Read,
    /// May call search endpoints.
    Search,
    /// May manage keys, tenants, and configuration.
    Admin,
    /// Access restricted to a single named channel.
    Channel(String),
}

impl Scope {
    /// True when a key holding `self` is permitted to perform `required`.
    ///
    /// `Admin` implies every other scope. A bare `Channel(x)` only satisfies the
    /// same `Channel(x)`.
    pub fn allows(&self, required: &Scope) -> bool {
        matches!(self, Scope::Admin) || self == required
    }
}

/// Public metadata about a stored API key (never contains the secret).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiKeyInfo {
    pub id: String,
    pub tenant_id: String,
    pub name: String,
    pub scopes: Vec<Scope>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

impl ApiKeyInfo {
    /// True when any held scope satisfies `required`.
    pub fn allows(&self, required: &Scope) -> bool {
        self.scopes.iter().any(|s| s.allows(required))
    }

    /// True when this key may use the named channel.
    pub fn allows_channel(&self, channel: &str) -> bool {
        self.scopes.iter().any(|s| match s {
            Scope::Admin | Scope::Read | Scope::Search => true,
            Scope::Channel(name) => name == channel,
        })
    }
}

/// A freshly created API key. The `secret` is shown only once.
#[derive(Debug, Clone)]
pub struct ApiKey {
    pub secret: String,
    pub info: ApiKeyInfo,
}

/// Hash a key secret with SHA-256 and return a lowercase hex digest.
pub fn hash_key(secret: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn random_bytes<const N: usize>() -> [u8; N] {
    let mut buf = [0u8; N];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    buf
}

/// In-memory API key store. Maps SHA-256(secret) → metadata.
#[derive(Debug, Default)]
pub struct ApiKeyManager {
    by_hash: DashMap<String, ApiKeyInfo>,
}

impl ApiKeyManager {
    /// Create an empty manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Mint a new API key for a tenant. The returned `ApiKey.secret` is the only
    /// time the plaintext is available.
    pub fn create_key(&self, tenant_id: &str, name: &str, scopes: Vec<Scope>) -> ApiKey {
        let secret = format!("as_{}", URL_SAFE_NO_PAD.encode(random_bytes::<32>()));
        let id = URL_SAFE_NO_PAD.encode(random_bytes::<12>());
        let info = ApiKeyInfo {
            id,
            tenant_id: tenant_id.to_string(),
            name: name.to_string(),
            scopes,
            created_at: Utc::now(),
            last_used_at: None,
        };
        self.by_hash.insert(hash_key(&secret), info.clone());
        ApiKey { secret, info }
    }

    /// Validate a presented key secret, updating `last_used_at` on success.
    pub fn validate_key(&self, secret: &str) -> Option<ApiKeyInfo> {
        let hash = hash_key(secret);
        let mut entry = self.by_hash.get_mut(&hash)?;
        entry.last_used_at = Some(Utc::now());
        Some(entry.clone())
    }

    /// Revoke a key by its public id. Returns true if a key was removed.
    pub fn revoke_key(&self, key_id: &str) -> bool {
        let hash = self
            .by_hash
            .iter()
            .find(|e| e.value().id == key_id)
            .map(|e| e.key().clone());
        match hash {
            Some(h) => self.by_hash.remove(&h).is_some(),
            None => false,
        }
    }

    /// List metadata for all keys belonging to a tenant.
    pub fn list_keys(&self, tenant_id: &str) -> Vec<ApiKeyInfo> {
        self.by_hash
            .iter()
            .filter(|e| e.value().tenant_id == tenant_id)
            .map(|e| e.value().clone())
            .collect()
    }

    /// Total number of stored keys.
    pub fn len(&self) -> usize {
        self.by_hash.len()
    }

    /// True when no keys are stored.
    pub fn is_empty(&self) -> bool {
        self.by_hash.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_stable_and_not_plaintext() {
        let h1 = hash_key("secret");
        let h2 = hash_key("secret");
        assert_eq!(h1, h2);
        assert_ne!(h1, "secret");
        assert_eq!(h1.len(), 64); // 32 bytes hex
    }

    #[test]
    fn create_and_validate_roundtrip() {
        let mgr = ApiKeyManager::new();
        let key = mgr.create_key("default", "ci", vec![Scope::Read]);
        assert!(key.secret.starts_with("as_"));

        let info = mgr.validate_key(&key.secret).expect("valid");
        assert_eq!(info.tenant_id, "default");
        assert_eq!(info.name, "ci");
        assert!(info.last_used_at.is_some());
    }

    #[test]
    fn unknown_key_is_rejected() {
        let mgr = ApiKeyManager::new();
        assert!(mgr.validate_key("as_nope").is_none());
    }

    #[test]
    fn revoked_key_no_longer_validates() {
        let mgr = ApiKeyManager::new();
        let key = mgr.create_key("default", "tmp", vec![Scope::Read]);
        assert!(mgr.revoke_key(&key.info.id));
        assert!(mgr.validate_key(&key.secret).is_none());
        assert!(!mgr.revoke_key(&key.info.id)); // second revoke is a no-op
    }

    #[test]
    fn list_keys_filters_by_tenant() {
        let mgr = ApiKeyManager::new();
        mgr.create_key("a", "k1", vec![Scope::Read]);
        mgr.create_key("a", "k2", vec![Scope::Read]);
        mgr.create_key("b", "k3", vec![Scope::Read]);
        assert_eq!(mgr.list_keys("a").len(), 2);
        assert_eq!(mgr.list_keys("b").len(), 1);
    }

    #[test]
    fn keys_are_unique() {
        let mgr = ApiKeyManager::new();
        let k1 = mgr.create_key("default", "a", vec![Scope::Read]);
        let k2 = mgr.create_key("default", "b", vec![Scope::Read]);
        assert_ne!(k1.secret, k2.secret);
        assert_ne!(k1.info.id, k2.info.id);
    }

    #[test]
    fn admin_scope_implies_all() {
        assert!(Scope::Admin.allows(&Scope::Read));
        assert!(Scope::Admin.allows(&Scope::Channel("youtube".into())));
        assert!(!Scope::Read.allows(&Scope::Admin));
    }

    #[test]
    fn channel_scope_is_specific() {
        let info = ApiKeyInfo {
            id: "x".into(),
            tenant_id: "default".into(),
            name: "scoped".into(),
            scopes: vec![Scope::Channel("youtube".into())],
            created_at: Utc::now(),
            last_used_at: None,
        };
        assert!(info.allows_channel("youtube"));
        assert!(!info.allows_channel("twitter"));
        assert!(!info.allows(&Scope::Read));
    }
}
