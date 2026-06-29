//! Axum middleware: API-key authentication and rate limiting.

use std::time::Instant;

use axum::{
    extract::{Request, State},
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use serde_json::json;

use agentspan_auth::{ApiKeyInfo, AuditEntry, AuthContext, AuthError, Scope, Tenant};

use crate::AppState;

/// Extract the channel name from an `/api/v1/channels/{name}/...` path.
fn channel_from_path(path: &str) -> Option<String> {
    let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    segs.iter()
        .position(|s| *s == "channels")
        .and_then(|i| segs.get(i + 1))
        .map(|s| s.to_string())
}

/// Synthetic admin context used in single-user (auth-disabled) mode so handlers
/// can uniformly read an [`AuthContext`] and check scopes.
pub fn local_admin_context(tenant: Tenant) -> AuthContext {
    AuthContext {
        key: ApiKeyInfo {
            id: "local".to_string(),
            tenant_id: tenant.id.clone(),
            name: "local".to_string(),
            scopes: vec![Scope::Admin],
            created_at: Utc::now(),
            last_used_at: None,
        },
        tenant,
    }
}

fn error_response(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

/// Guard admin-only routes. In permissive (single-user) mode the synthetic
/// context carries the Admin scope, so admin routes would be wide open; we
/// therefore require `auth.require_api_key=true` AND a real Admin scope.
/// Returns `Some(forbidden)` when access must be denied.
pub fn admin_guard(state: &AppState, ctx: &AuthContext) -> Option<Response> {
    if !state.config.auth.require_api_key {
        return Some(error_response(
            StatusCode::FORBIDDEN,
            "admin routes require auth.require_api_key=true",
        ));
    }
    if !ctx.key.allows(&Scope::Admin) {
        return Some(error_response(
            StatusCode::FORBIDDEN,
            "admin scope required",
        ));
    }
    None
}

/// Guard for read-only observability endpoints (healing report, performance,
/// analytics, auto-switch log).
///
/// Unlike [`admin_guard`], these are allowed in single-user (permissive) mode:
/// it's your own local gateway and the data is non-sensitive operational state,
/// so the dashboard's Healing/Suggestions views work out of the box. When auth is
/// enforced they still require the Admin scope.
pub fn operational_guard(state: &AppState, ctx: &AuthContext) -> Option<Response> {
    if !state.config.auth.require_api_key {
        return None;
    }
    if !ctx.key.allows(&Scope::Admin) {
        return Some(error_response(
            StatusCode::FORBIDDEN,
            "admin scope required",
        ));
    }
    None
}

/// Authenticate the request and attach an [`AuthContext`] to its extensions.
///
/// When `auth.require_api_key` is `false`, requests pass through with a synthetic
/// local-admin context. When `true`, a valid `X-API-Key` header is required and
/// the tenant's rate limit is enforced (HTTP 429 with `Retry-After` on excess).
pub async fn auth_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let start = Instant::now();

    // The outer observe middleware attaches a trace id; surface it in events.
    let trace_id = req
        .extensions()
        .get::<crate::observe::TraceId>()
        .map(|t| t.0.clone());

    // Resolve the auth context (early-return on auth/rate failures).
    let (tenant_id, key_id) = if !state.config.auth.require_api_key {
        let tenant = state
            .auth
            .tenants
            .get("default")
            .unwrap_or_else(|| Tenant::new("default", "Default Tenant"));
        let ctx = local_admin_context(tenant);
        let ids = (ctx.tenant.id.clone(), ctx.key.id.clone());
        req.extensions_mut().insert(ctx);
        ids
    } else {
        let key = req
            .headers()
            .get("x-api-key")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let key = match key {
            Some(k) => k,
            None => return error_response(StatusCode::UNAUTHORIZED, "missing API key"),
        };

        let ctx = match state.auth.authenticate(&key) {
            Ok(ctx) => ctx,
            Err(_) => return error_response(StatusCode::UNAUTHORIZED, "invalid API key"),
        };

        if let Err(AuthError::RateLimited { retry_after_secs }) = state.auth.check_rate(&ctx) {
            let mut resp = error_response(StatusCode::TOO_MANY_REQUESTS, "rate limit exceeded");
            if let Ok(value) = HeaderValue::from_str(&retry_after_secs.to_string()) {
                resp.headers_mut().insert("Retry-After", value);
            }
            return resp;
        }

        let ids = (ctx.tenant.id.clone(), ctx.key.id.clone());
        req.extensions_mut().insert(ctx);
        ids
    };

    let response = next.run(req).await;

    // Record an audit entry (the ring buffer write is fast and non-blocking).
    let mut entry = AuditEntry::now(tenant_id, format!("{method} {path}"));
    entry.api_key_id = Some(key_id);
    entry.channel = channel_from_path(&path);
    entry.status = response.status().as_u16();
    entry.latency_ms = start.elapsed().as_millis() as u64;
    let status = entry.status;
    let latency_ms = entry.latency_ms;
    let channel = entry.channel.clone();
    state.auth.audit.record(entry);

    // Feed usage analytics. We know the channel, latency, and status here; the
    // output-token count is estimated from the response size (~4 bytes/token).
    let tokens_out = response
        .headers()
        .get(axum::http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .map(|bytes| bytes / 4)
        .unwrap_or(0);
    state.analytics.record(
        agentspan_core::RequestRecord::new(channel.clone())
            .latency(latency_ms)
            .status(status)
            .tokens(0, tokens_out),
    );

    // Push a live event to any SSE subscribers (skip the stream endpoint itself).
    if !path.ends_with("/events/stream") {
        state.publish_event(
            json!({
                "type": "request",
                "trace_id": trace_id,
                "method": method.as_str(),
                "path": path,
                "channel": channel,
                "status": status,
                "latency_ms": latency_ms,
            })
            .to_string(),
        );
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    #[test]
    fn channel_from_path_extracts_name() {
        assert_eq!(
            channel_from_path("/api/v1/channels/reddit/search"),
            Some("reddit".to_string())
        );
        assert_eq!(channel_from_path("/api/v1/stats"), None);
    }

    #[tokio::test]
    async fn request_is_audited() {
        // Regression: before the fix, the middleware never recorded audit entries.
        let state = AppState::default_state();
        let auth = state.auth.clone();
        let app = state.router();

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/stats")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        assert!(!auth.audit.is_empty(), "no audit entry was recorded");
        let recent = auth.audit.recent(1);
        assert!(recent[0].target.contains("/api/v1/stats"));
        assert_eq!(recent[0].status, 200);
    }
}
