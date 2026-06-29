//! Extended v1 REST endpoints: per-channel ops, smart read, stats, config, admin.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension, Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use agentspan_auth::AuthContext;
use agentspan_core::types::{ReadOptions, SearchOptions};
use agentspan_router::health::HealthCheck;

use crate::AppState;

/// Query for per-channel and smart read.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ReadQuery {
    pub url: String,
    #[serde(default)]
    pub force_refresh: bool,
}

/// JSON body for the smart POST /read endpoint.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ReadBody {
    pub url: String,
    #[serde(default)]
    pub force_refresh: bool,
}

/// Query for search.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct SearchQuery {
    pub q: String,
    #[serde(default)]
    pub limit: usize,
}

fn not_found(state: &AppState, name: &str) -> Response {
    let suggestions = state.registry.suggest(name, 3);
    let mut body = json!({ "error": format!("channel not found: {name}") });
    if !suggestions.is_empty() {
        body["did_you_mean"] = json!(suggestions);
    }
    (StatusCode::NOT_FOUND, Json(body)).into_response()
}

/// `GET /api/v1/channels/{name}` — channel metadata plus live health.
#[utoipa::path(
    get,
    path = "/api/v1/channels/{name}",
    tag = "channels",
    params(("name" = String, Path, description = "Channel name, e.g. `github`")),
    responses(
        (status = 200, description = "Channel metadata and per-backend health", body = Value),
        (status = 404, description = "Channel not found", body = Value, example = json!({"error":"channel not found: nope","did_you_mean":["github"]}))
    )
)]
pub async fn channel_info(State(state): State<AppState>, Path(name): Path<String>) -> Response {
    let Some(channel) = state.registry.by_name(&name) else {
        return not_found(&state, &name);
    };
    let healths = channel.check_health().await;
    let backends: Vec<Value> = healths
        .iter()
        .map(|h| {
            json!({
                "backend": h.backend_name,
                "status": format!("{:?}", h.probe.status),
                "message": h.probe.message,
                "latency_ms": h.latency_ms,
            })
        })
        .collect();
    Json(json!({
        "name": channel.name(),
        "description": channel.description(),
        "tier": format!("{:?}", channel.tier()),
        "backends": backends,
    }))
    .into_response()
}

/// `GET /api/v1/channels/{name}/read?url=` — read via a specific channel.
#[utoipa::path(
    get,
    path = "/api/v1/channels/{name}/read",
    tag = "channels",
    params(
        ("name" = String, Path, description = "Channel name"),
        ("url" = String, Query, description = "URL to read"),
        ("force_refresh" = Option<bool>, Query, description = "Skip cache")
    ),
    responses(
        (status = 200, description = "Content read successfully", body = Value),
        (status = 404, description = "Channel not found", body = Value),
        (status = 502, description = "Channel read failed", body = Value)
    )
)]
pub async fn channel_read(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(q): Query<ReadQuery>,
) -> Response {
    let Some(channel) = state.registry.by_name(&name) else {
        return not_found(&state, &name);
    };
    let opts = ReadOptions {
        force_refresh: q.force_refresh,
        ..Default::default()
    };
    match channel.read(&q.url, opts).await {
        Ok(content) => {
            Json(json!({ "channel": channel.name(), "content": content })).into_response()
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": e.to_string(), "channel": channel.name() })),
        )
            .into_response(),
    }
}

/// `GET /api/v1/channels/{name}/search?q=&limit=` — search via a specific channel.
#[utoipa::path(
    get,
    path = "/api/v1/channels/{name}/search",
    tag = "channels",
    params(
        ("name" = String, Path, description = "Channel name"),
        ("q" = String, Query, description = "Search query"),
        ("limit" = Option<usize>, Query, description = "Max results")
    ),
    responses(
        (status = 200, description = "Search results", body = Value),
        (status = 404, description = "Channel not found", body = Value),
        (status = 502, description = "Search failed", body = Value)
    )
)]
pub async fn channel_search(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(q): Query<SearchQuery>,
) -> Response {
    let Some(channel) = state.registry.by_name(&name) else {
        return not_found(&state, &name);
    };
    let opts = SearchOptions {
        limit: q.limit,
        ..Default::default()
    };
    match channel.search(&q.q, opts).await {
        Ok(results) => {
            Json(json!({ "channel": channel.name(), "results": results })).into_response()
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": e.to_string(), "channel": channel.name() })),
        )
            .into_response(),
    }
}

/// `POST /api/v1/read` — smart read; auto-detect the channel from the URL.
#[utoipa::path(
    post,
    path = "/api/v1/read",
    tag = "read",
    request_body = ReadBody,
    responses(
        (status = 200, description = "Content read successfully", body = Value),
        (status = 422, description = "No channel can handle the URL", body = Value),
        (status = 502, description = "Channel read failed", body = Value)
    )
)]
pub async fn smart_read(State(state): State<AppState>, Json(body): Json<ReadBody>) -> Response {
    let Some(channel) = state.registry.by_url(&body.url) else {
        // No channel matched — record demand for the missing platform.
        state.healer.discoverer.record_unsupported(&body.url);
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "error": "no channel can handle this URL", "url": body.url })),
        )
            .into_response();
    };
    let opts = ReadOptions {
        force_refresh: body.force_refresh,
        ..Default::default()
    };
    match channel.read(&body.url, opts).await {
        Ok(content) => {
            Json(json!({ "channel": channel.name(), "content": content })).into_response()
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": e.to_string(), "channel": channel.name() })),
        )
            .into_response(),
    }
}

/// `GET /api/v1/doctor/{channel}` — health for a single channel.
#[utoipa::path(
    get,
    path = "/api/v1/doctor/{channel}",
    tag = "doctor",
    params(("channel" = String, Path, description = "Channel name")),
    responses(
        (status = 200, description = "Channel health report", body = Value),
        (status = 404, description = "Channel not found", body = Value)
    )
)]
pub async fn doctor_channel(State(state): State<AppState>, Path(name): Path<String>) -> Response {
    let Some(channel) = state.registry.by_name(&name) else {
        return not_found(&state, &name);
    };
    let health_check = HealthCheck::new();
    let healths = channel.check_health().await;
    let report = health_check.report(vec![(channel.name().to_string(), healths)]);
    Json(serde_json::to_value(report).unwrap_or(Value::Null)).into_response()
}

/// `GET /api/v1/stats` — channel count and audit summary.
#[utoipa::path(
    get,
    path = "/api/v1/stats",
    tag = "admin",
    responses(
        (status = 200, description = "Gateway stats and recent audit entries", body = Value)
    )
)]
pub async fn stats(State(state): State<AppState>) -> Response {
    let recent = state.auth.audit.recent(10);
    Json(json!({
        "channels": state.registry.list().len(),
        "audit_entries": state.auth.audit.len(),
        "tenants": state.auth.tenants.list().len(),
        "recent": recent,
    }))
    .into_response()
}

/// `GET /api/v1/config` — non-secret configuration view.
#[utoipa::path(
    get,
    path = "/api/v1/config",
    tag = "admin",
    responses(
        (status = 200, description = "Non-secret configuration (secrets masked)", body = Value)
    )
)]
pub async fn config(State(state): State<AppState>) -> Response {
    let c = &state.config;
    Json(json!({
        "server": { "host": c.server.host, "port": c.server.port },
        "cache": {
            "l1_ttl_seconds": c.cache.l1_ttl_seconds,
            "l2_ttl_seconds": c.cache.l2_ttl_seconds,
            "l3_ttl_seconds": c.cache.l3_ttl_seconds,
            "l3_enabled": c.cache.l3_url.is_some(),
        },
        "logging": { "level": c.logging.level, "json": c.logging.json },
        "auth": { "require_api_key": c.auth.require_api_key },
        "channels": state.registry.list().iter().map(|ch| ch.name()).collect::<Vec<_>>(),
    }))
    .into_response()
}

/// `GET /api/v1/admin/audit-log?limit=` — recent audit entries (admin only).
#[derive(Debug, Deserialize)]
pub struct AuditQuery {
    #[serde(default)]
    pub limit: usize,
}

pub async fn audit_log(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Query(q): Query<AuditQuery>,
) -> Response {
    if let Some(resp) = crate::middleware::admin_guard(&state, &ctx) {
        return resp;
    }
    let limit = if q.limit == 0 { 100 } else { q.limit };
    Json(json!({ "entries": state.auth.audit.recent(limit) })).into_response()
}

/// `GET /api/v1/admin/healing-report` — self-healing status.
pub async fn healing_report(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> Response {
    if let Some(resp) = crate::middleware::operational_guard(&state, &ctx) {
        return resp;
    }
    Json(serde_json::to_value(state.healer.report()).unwrap_or(Value::Null)).into_response()
}

/// `GET /api/v1/admin/auto-switches` — backend auto-switch log.
pub async fn auto_switches(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> Response {
    if let Some(resp) = crate::middleware::operational_guard(&state, &ctx) {
        return resp;
    }
    Json(json!({ "auto_switches": state.healer.auto_switches() })).into_response()
}

/// JSON body for the manual repair endpoint.
#[derive(Debug, Deserialize)]
pub struct RepairBody {
    /// CLI tool to repair (e.g. `yt-dlp`).
    pub tool: String,
    /// Package manager: `pip` | `npm` | `cargo`. Inferred when omitted.
    #[serde(default)]
    pub kind: Option<String>,
}

/// `POST /api/v1/admin/repair-channel` — manually reinstall a CLI tool, backing
/// the dashboard's "Repair now" button. Admin only.
pub async fn repair_channel(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Json(body): Json<RepairBody>,
) -> Response {
    use agentspan_channels::healer::{infer_kind, RepairKind};

    if let Some(resp) = crate::middleware::admin_guard(&state, &ctx) {
        return resp;
    }

    let kind = match body.kind.as_deref() {
        Some("pip") => RepairKind::Pip,
        Some("npm") => RepairKind::Npm,
        Some("cargo") => RepairKind::Cargo,
        Some(other) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("unknown repair kind: {other}") })),
            )
                .into_response();
        }
        None => infer_kind(&body.tool),
    };

    let attempt = state.healer.repair.repair(&body.tool, kind).await;
    let status = if attempt.rate_limited {
        StatusCode::TOO_MANY_REQUESTS
    } else if attempt.success {
        StatusCode::OK
    } else {
        StatusCode::BAD_GATEWAY
    };
    (
        status,
        Json(json!({
            "tool": attempt.tool,
            "kind": format!("{:?}", attempt.kind),
            "success": attempt.success,
            "rate_limited": attempt.rate_limited,
            "message": attempt.message,
        })),
    )
        .into_response()
}

/// `GET /api/v1/admin/performance-report` — per channel/backend latency profile.
pub async fn performance_report(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> Response {
    if let Some(resp) = crate::middleware::operational_guard(&state, &ctx) {
        return resp;
    }
    Json(serde_json::to_value(state.profiler.report()).unwrap_or(Value::Null)).into_response()
}

/// `GET /api/v1/admin/analytics` — usage totals, per-channel stats, and the
/// learned per-platform rate-limit profiles.
pub async fn analytics_report(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> Response {
    if let Some(resp) = crate::middleware::operational_guard(&state, &ctx) {
        return resp;
    }
    Json(json!({
        "totals": state.analytics.totals(),
        "channels": state.analytics.all_channel_stats(),
        "rate_profiles": state.adaptive_rate.profiles(),
        "recent": state.analytics.recent(20),
    }))
    .into_response()
}

/// `GET /api/v1/suggestions` — actionable recommendations the system derives from
/// its own usage: TTL tweaks, faster backends, and platforms worth adding.
#[utoipa::path(
    get,
    path = "/api/v1/suggestions",
    tag = "admin",
    responses(
        (status = 200, description = "Actionable suggestions derived from usage", body = Value)
    )
)]
pub async fn suggestions(State(state): State<AppState>) -> Response {
    let mut items: Vec<Value> = Vec::new();

    for adj in state.cache_optimizer.suggestions() {
        items.push(json!({
            "type": "cache_ttl_adjustment",
            "channel": adj.channel,
            "current_ttl": adj.from_secs,
            "suggested_ttl": adj.to_secs,
            "reason": adj.reason,
        }));
    }
    for swap in state.profiler.suggestions() {
        items.push(json!({
            "type": "backend_switch",
            "channel": swap.channel,
            "current_backend": swap.slow_backend,
            "suggested_backend": swap.fast_backend,
            "reason": swap.reason,
        }));
    }
    for p in state.healer.discoverer.weekly_report() {
        items.push(json!({
            "type": "channel_request",
            "platform": p.domain,
            "requests_this_week": p.count,
            "suggested_channel": p.suggested_channel,
            "reason": format!(
                "Agents frequently request {} ({} times this week)",
                p.domain, p.count
            ),
        }));
    }

    Json(json!({ "suggestions": items })).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::middleware::local_admin_context;
    use agentspan_auth::{AuditEntry, Tenant};

    #[tokio::test]
    async fn channel_info_known_channel() {
        let state = AppState::default_state();
        let resp = channel_info(State(state), Path("hackernews".to_string())).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn channel_info_unknown_is_404() {
        let state = AppState::default_state();
        let resp = channel_info(State(state), Path("nope".to_string())).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn channel_info_typo_suggests_did_you_mean() {
        let state = AppState::default_state();
        let resp = channel_info(State(state), Path("githubb".to_string())).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap();
        let suggestions = json["did_you_mean"].as_array().unwrap();
        assert!(suggestions.iter().any(|v| v == "github"));
    }

    #[tokio::test]
    async fn smart_read_unhandled_url_is_422() {
        let state = AppState::default_state();
        let resp = smart_read(
            State(state),
            Json(ReadBody {
                url: "ftp://nope".to_string(),
                force_refresh: false,
            }),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn stats_reports_channel_count() {
        let state = AppState::default_state();
        let resp = stats(State(state)).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn config_does_not_leak_secrets() {
        let mut cfg = agentspan_core::Config::default();
        cfg.api_keys
            .insert("openai".to_string(), "sk-supersecret".to_string());
        let state = AppState::with_config(cfg);
        let resp = config(State(state)).await;
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let text = String::from_utf8_lossy(&body);
        assert!(!text.contains("sk-supersecret"));
        assert!(text.contains("\"server\""));
    }

    #[tokio::test]
    async fn audit_log_allows_admin_when_auth_enabled() {
        let mut cfg = agentspan_core::Config::default();
        cfg.auth.require_api_key = true;
        let state = AppState::with_config(cfg);
        state
            .auth
            .audit
            .record(AuditEntry::now("default", "https://x"));
        let ctx = local_admin_context(Tenant::new("default", "Default"));
        let resp = audit_log(State(state), Extension(ctx), Query(AuditQuery { limit: 0 })).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn audit_log_denied_in_permissive_mode() {
        // M1 regression: permissive mode must not expose the audit log.
        let state = AppState::default_state();
        let ctx = local_admin_context(Tenant::new("default", "Default"));
        let resp = audit_log(State(state), Extension(ctx), Query(AuditQuery { limit: 0 })).await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    fn admin_state() -> (AppState, AuthContext) {
        let mut cfg = agentspan_core::Config::default();
        cfg.auth.require_api_key = true;
        let state = AppState::with_config(cfg);
        let ctx = local_admin_context(Tenant::new("default", "Default"));
        (state, ctx)
    }

    #[tokio::test]
    async fn healing_report_allows_admin_and_has_summary_fields() {
        let (state, ctx) = admin_state();
        let resp = healing_report(State(state), Extension(ctx)).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap();
        assert!(json.get("channels_monitored").is_some());
        assert!(json.get("healthy").is_some());
        assert!(json.get("auto_switches_today").is_some());
    }

    #[tokio::test]
    async fn healing_report_readable_in_permissive_mode() {
        // Read-only observability is allowed on a local single-user gateway, so
        // the dashboard's Healing page works without enabling auth.
        let state = AppState::default_state();
        let ctx = local_admin_context(Tenant::new("default", "Default"));
        let resp = healing_report(State(state), Extension(ctx)).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn auto_switches_allows_admin() {
        let (state, ctx) = admin_state();
        let resp = auto_switches(State(state), Extension(ctx)).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap();
        assert!(json["auto_switches"].is_array());
    }

    #[tokio::test]
    async fn repair_channel_rejects_unknown_kind() {
        let (state, ctx) = admin_state();
        let resp = repair_channel(
            State(state),
            Extension(ctx),
            Json(RepairBody {
                tool: "yt-dlp".to_string(),
                kind: Some("brew".to_string()),
            }),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn repair_channel_denied_in_permissive_mode() {
        let state = AppState::default_state();
        let ctx = local_admin_context(Tenant::new("default", "Default"));
        let resp = repair_channel(
            State(state),
            Extension(ctx),
            Json(RepairBody {
                tool: "yt-dlp".to_string(),
                kind: None,
            }),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn performance_report_allows_admin() {
        let (state, ctx) = admin_state();
        let resp = performance_report(State(state), Extension(ctx)).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn analytics_report_readable_in_permissive_mode() {
        let state = AppState::default_state();
        let ctx = local_admin_context(Tenant::new("default", "Default"));
        let resp = analytics_report(State(state), Extension(ctx)).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn suggestions_is_public_and_lists_demand() {
        // Suggestions are not admin-gated; feed some demand and check it appears.
        let state = AppState::default_state();
        for _ in 0..3 {
            state
                .healer
                .discoverer
                .record_unsupported("https://substack.com/p/x");
        }
        let resp = suggestions(State(state)).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap();
        let items = json["suggestions"].as_array().unwrap();
        let req = items
            .iter()
            .find(|s| s["type"] == "channel_request")
            .expect("expected a channel_request suggestion");
        assert_eq!(req["platform"], "substack.com");
        assert_eq!(req["requests_this_week"], 3);
    }

    #[tokio::test]
    async fn analytics_feeds_from_recorded_requests() {
        let state = AppState::default_state();
        state.analytics.record(
            agentspan_core::RequestRecord::new(Some("github".to_string()))
                .latency(42)
                .status(200),
        );
        assert_eq!(state.analytics.totals().requests, 1);
        let s = state.analytics.channel_stats("github").unwrap();
        assert_eq!(s.avg_latency_ms, 42.0);
    }
}
