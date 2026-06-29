//! Authentication and authorization abstractions.

use async_trait::async_trait;

use crate::error::Error;

/// Authentication credential.
#[derive(Debug, Clone)]
pub enum Credential {
    ApiKey(String),
    OAuth { token: String },
    None,
}

/// Authenticated principal.
#[derive(Debug, Clone)]
pub struct Principal {
    pub tenant_id: String,
    pub user_id: String,
    pub roles: Vec<String>,
}

/// Role-based access control check.
#[async_trait]
pub trait Auth: Send + Sync {
    /// Authenticate a credential and return a principal.
    async fn authenticate(&self, credential: Credential) -> Result<Principal, Error>;

    /// Authorize a principal to perform an action on a resource.
    async fn authorize(
        &self,
        principal: &Principal,
        action: &str,
        resource: &str,
    ) -> Result<bool, Error>;
}
