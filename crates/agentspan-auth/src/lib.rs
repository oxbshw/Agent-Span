//! Authentication, multi-tenancy, rate limiting, and audit logging.
//!
//! Everything here is in-memory and dependency-free so single-node deployments
//! work with zero setup. The types are shaped so a persistent store (SQLite /
//! PostgreSQL / Redis) can be slotted in behind the same façade later.

pub mod adaptive_rate;
pub mod audit;
pub mod key;
pub mod rate_limit;
pub mod tenant;

pub use adaptive_rate::{AdaptiveRateLimiter, RateProfile};
pub use audit::{AuditEntry, AuditLog};
pub use key::{hash_key, ApiKey, ApiKeyInfo, ApiKeyManager, Scope};
pub use rate_limit::{RateLimitDecision, RateLimiter};
pub use tenant::{Quota, Tenant, TenantConfig, TenantManager};

/// The result of authenticating and rate-limiting a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthContext {
    pub key: ApiKeyInfo,
    pub tenant: Tenant,
}

/// Why a request was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    /// No key supplied where one was required.
    MissingKey,
    /// Key did not validate.
    InvalidKey,
    /// Key valid but its tenant no longer exists.
    UnknownTenant,
    /// Key lacks the scope required for the operation.
    Forbidden,
    /// Tenant quota exceeded; carries the suggested retry delay in seconds.
    RateLimited { retry_after_secs: u64 },
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::MissingKey => write!(f, "missing API key"),
            AuthError::InvalidKey => write!(f, "invalid API key"),
            AuthError::UnknownTenant => write!(f, "unknown tenant"),
            AuthError::Forbidden => write!(f, "insufficient scope"),
            AuthError::RateLimited { retry_after_secs } => {
                write!(f, "rate limited; retry after {retry_after_secs}s")
            }
        }
    }
}

impl std::error::Error for AuthError {}

/// Facade combining key validation, tenant lookup, rate limiting, and audit.
#[derive(Debug, Default)]
pub struct AuthManager {
    pub keys: ApiKeyManager,
    pub tenants: TenantManager,
    pub limiter: RateLimiter,
    pub audit: AuditLog,
}

impl AuthManager {
    /// Create a manager seeded with the default tenant.
    pub fn new() -> Self {
        Self::default()
    }

    /// Validate a key and resolve its tenant. Does not apply rate limiting.
    pub fn authenticate(&self, secret: &str) -> Result<AuthContext, AuthError> {
        let key = self
            .keys
            .validate_key(secret)
            .ok_or(AuthError::InvalidKey)?;
        let tenant = self
            .tenants
            .get(&key.tenant_id)
            .ok_or(AuthError::UnknownTenant)?;
        Ok(AuthContext { key, tenant })
    }

    /// Apply the tenant's quota to a request from the given key id.
    pub fn check_rate(&self, ctx: &AuthContext) -> Result<(), AuthError> {
        let decision = self.limiter.check(
            &ctx.key.id,
            ctx.tenant.quota.max_requests_per_minute,
            ctx.tenant.quota.max_requests_per_day,
        );
        if decision.allowed {
            Ok(())
        } else {
            Err(AuthError::RateLimited {
                retry_after_secs: decision
                    .retry_after
                    .map(|d| d.as_secs().max(1))
                    .unwrap_or(1),
            })
        }
    }

    /// Authenticate, enforce a required scope, and apply rate limiting in one call.
    pub fn authorize(&self, secret: &str, required: &Scope) -> Result<AuthContext, AuthError> {
        let ctx = self.authenticate(secret)?;
        if !ctx.key.allows(required) {
            return Err(AuthError::Forbidden);
        }
        self.check_rate(&ctx)?;
        Ok(ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorize_happy_path() {
        let auth = AuthManager::new();
        let key = auth.keys.create_key("default", "ci", vec![Scope::Read]);
        let ctx = auth.authorize(&key.secret, &Scope::Read).unwrap();
        assert_eq!(ctx.tenant.id, "default");
        assert_eq!(ctx.key.name, "ci");
    }

    #[test]
    fn authorize_rejects_invalid_key() {
        let auth = AuthManager::new();
        assert_eq!(
            auth.authorize("as_bogus", &Scope::Read),
            Err(AuthError::InvalidKey)
        );
    }

    #[test]
    fn authorize_enforces_scope() {
        let auth = AuthManager::new();
        let key = auth.keys.create_key("default", "ro", vec![Scope::Read]);
        assert_eq!(
            auth.authorize(&key.secret, &Scope::Admin),
            Err(AuthError::Forbidden)
        );
    }

    #[test]
    fn authorize_enforces_rate_limit() {
        let auth = AuthManager::new();
        // Tight quota for the test.
        let mut tenant = Tenant::new("tight", "Tight");
        tenant.quota.max_requests_per_minute = 1;
        auth.tenants.upsert(tenant);
        let key = auth.keys.create_key("tight", "k", vec![Scope::Read]);

        assert!(auth.authorize(&key.secret, &Scope::Read).is_ok());
        match auth.authorize(&key.secret, &Scope::Read) {
            Err(AuthError::RateLimited { retry_after_secs }) => assert!(retry_after_secs >= 1),
            other => panic!("expected rate limit, got {other:?}"),
        }
    }

    #[test]
    fn unknown_tenant_is_rejected() {
        let auth = AuthManager::new();
        let key = auth.keys.create_key("ghost", "k", vec![Scope::Read]);
        // Tenant "ghost" was never created.
        assert_eq!(
            auth.authenticate(&key.secret),
            Err(AuthError::UnknownTenant)
        );
    }

    #[test]
    fn audit_records_through_manager() {
        let auth = AuthManager::new();
        auth.audit.record(AuditEntry::now("default", "https://x"));
        assert_eq!(auth.audit.len(), 1);
    }
}
