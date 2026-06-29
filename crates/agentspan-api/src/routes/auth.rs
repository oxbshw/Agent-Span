//! API key management routes (admin-scoped).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension, Json,
};
use serde::Deserialize;
use serde_json::json;

use agentspan_auth::{AuthContext, Scope};

use crate::AppState;

/// Parse a textual scope token into a [`Scope`].
///
/// Accepts `read`, `search`, `admin`, or `channel:<name>`; anything else is
/// treated as a channel name.
fn parse_scope(raw: &str) -> Scope {
    match raw {
        "read" => Scope::Read,
        "search" => Scope::Search,
        "admin" => Scope::Admin,
        other => match other.strip_prefix("channel:") {
            Some(name) => Scope::Channel(name.to_string()),
            None => Scope::Channel(other.to_string()),
        },
    }
}

fn default_tenant() -> String {
    "default".to_string()
}

/// Request body for creating an API key.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CreateKeyRequest {
    #[serde(default = "default_tenant")]
    pub tenant_id: String,
    pub name: String,
    #[serde(default)]
    pub scopes: Vec<String>,
}

/// `POST /api/v1/auth/keys` — mint a new API key (secret shown once).
#[utoipa::path(
    post,
    path = "/api/v1/auth/keys",
    tag = "auth",
    request_body = CreateKeyRequest,
    responses(
        (status = 201, description = "API key created (secret shown once)", body = Value,
         example = json!({"id":"key_abc","secret":"ask_...","tenant_id":"default","name":"ci","warning":"store this secret now; it cannot be retrieved again"})),
        (status = 403, description = "Admin scope required", body = Value)
    )
)]
pub async fn create_key(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Json(req): Json<CreateKeyRequest>,
) -> Response {
    if let Some(resp) = crate::middleware::admin_guard(&state, &ctx) {
        return resp;
    }

    let scopes = if req.scopes.is_empty() {
        vec![Scope::Read]
    } else {
        req.scopes.iter().map(|s| parse_scope(s)).collect()
    };

    let key = state
        .auth
        .keys
        .create_key(&req.tenant_id, &req.name, scopes);
    (
        StatusCode::CREATED,
        Json(json!({
            "id": key.info.id,
            "secret": key.secret,
            "tenant_id": key.info.tenant_id,
            "name": key.info.name,
            "warning": "store this secret now; it cannot be retrieved again",
        })),
    )
        .into_response()
}

/// `GET /api/v1/auth/keys` — list key metadata for the caller's tenant.
#[utoipa::path(
    get,
    path = "/api/v1/auth/keys",
    tag = "auth",
    responses(
        (status = 200, description = "Key metadata for the caller's tenant", body = Value),
        (status = 403, description = "Admin scope required", body = Value)
    )
)]
pub async fn list_keys(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> Response {
    if let Some(resp) = crate::middleware::admin_guard(&state, &ctx) {
        return resp;
    }
    let keys = state.auth.keys.list_keys(&ctx.tenant.id);
    Json(json!({ "keys": keys })).into_response()
}

/// `DELETE /api/v1/auth/keys/{id}` — revoke an API key.
#[utoipa::path(
    delete,
    path = "/api/v1/auth/keys/{id}",
    tag = "auth",
    params(("id" = String, Path, description = "Key ID to revoke")),
    responses(
        (status = 204, description = "Key revoked"),
        (status = 403, description = "Admin scope required", body = Value),
        (status = 404, description = "Key not found", body = Value)
    )
)]
pub async fn revoke_key(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path(id): Path<String>,
) -> Response {
    if let Some(resp) = crate::middleware::admin_guard(&state, &ctx) {
        return resp;
    }
    if state.auth.keys.revoke_key(&id) {
        state.auth.limiter.reset(&id);
        StatusCode::NO_CONTENT.into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "key not found" })),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::middleware::local_admin_context;
    use agentspan_auth::Tenant;

    fn admin_ctx() -> AuthContext {
        local_admin_context(Tenant::new("default", "Default"))
    }

    /// State with API-key auth required (admin routes are only usable then).
    fn auth_state() -> AppState {
        let mut cfg = agentspan_core::Config::default();
        cfg.auth.require_api_key = true;
        AppState::with_config(cfg)
    }

    #[tokio::test]
    async fn create_key_adds_to_store() {
        let state = auth_state();
        let req = CreateKeyRequest {
            tenant_id: "default".into(),
            name: "ci".into(),
            scopes: vec!["read".into()],
        };
        let _ = create_key(State(state.clone()), Extension(admin_ctx()), Json(req)).await;
        assert_eq!(state.auth.keys.len(), 1);
    }

    #[tokio::test]
    async fn list_keys_returns_tenant_keys() {
        let state = auth_state();
        state
            .auth
            .keys
            .create_key("default", "a", vec![Scope::Read]);
        let _ = list_keys(State(state.clone()), Extension(admin_ctx())).await;
        assert_eq!(state.auth.keys.list_keys("default").len(), 1);
    }

    #[tokio::test]
    async fn revoke_key_removes_from_store() {
        let state = auth_state();
        let key = state
            .auth
            .keys
            .create_key("default", "tmp", vec![Scope::Read]);
        let _ = revoke_key(
            State(state.clone()),
            Extension(admin_ctx()),
            Path(key.info.id.clone()),
        )
        .await;
        assert_eq!(state.auth.keys.len(), 0);
    }

    #[tokio::test]
    async fn admin_route_denied_in_permissive_mode() {
        // Regression for M1: with require_api_key=false, admin routes must be
        // refused even though the synthetic context carries the Admin scope.
        let state = AppState::default_state();
        let resp = list_keys(State(state.clone()), Extension(admin_ctx())).await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn parse_scope_variants() {
        assert_eq!(parse_scope("read"), Scope::Read);
        assert_eq!(parse_scope("admin"), Scope::Admin);
        assert_eq!(
            parse_scope("channel:youtube"),
            Scope::Channel("youtube".into())
        );
        assert_eq!(parse_scope("youtube"), Scope::Channel("youtube".into()));
    }
}
