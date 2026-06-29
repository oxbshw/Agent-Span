//! API route handlers.

use axum::{
    extract::DefaultBodyLimit,
    routing::{delete, get, post},
    Router,
};

use crate::AppState;

pub mod auth;
pub mod batch;
pub mod channels;
pub mod doctor;
pub mod events;
pub mod health;
pub mod memory;
pub mod metrics;
pub mod openapi;
pub mod read;
pub mod search;
pub mod v1;

#[cfg(feature = "websocket")]
pub mod websocket;

/// Maximum accepted request body size (2 MiB) — guards against oversized uploads.
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

/// Build the full application router.
///
/// `/health`, `/metrics`, `/docs`, and `/openapi.json` are public; everything
/// under `/api/v1` passes through the auth middleware (permissive in
/// single-user mode, enforcing when `auth.require_api_key` is set). The whole
/// router is wrapped by the observe middleware (trace IDs, metrics,
/// concurrency limiting) and a body-size limit.
pub fn build_router(state: AppState) -> Router {
    let protected = Router::new()
        .route("/api/v1/channels", get(channels::list_channels))
        .route("/api/v1/channels/{name}", get(v1::channel_info))
        .route("/api/v1/channels/{name}/read", get(v1::channel_read))
        .route("/api/v1/channels/{name}/search", get(v1::channel_search))
        .route("/api/v1/read", get(read::read).post(v1::smart_read))
        .route("/api/v1/batch/read", post(batch::batch_read))
        .route("/api/v1/batch/search", post(batch::batch_search))
        .route("/api/v1/search/federated", post(search::federated_search))
        .route("/api/v1/memory/{namespace}", get(memory::list_memory))
        .route(
            "/api/v1/memory/{namespace}/{key}",
            axum::routing::put(memory::set_memory)
                .get(memory::get_memory)
                .delete(memory::delete_memory),
        )
        .route("/api/v1/events/stream", get(events::events_sse))
        .route("/api/v1/doctor", get(doctor::doctor))
        .route("/api/v1/doctor/{channel}", get(v1::doctor_channel))
        .route("/api/v1/stats", get(v1::stats))
        .route("/api/v1/config", get(v1::config))
        .route("/api/v1/admin/audit-log", get(v1::audit_log))
        .route("/api/v1/admin/healing-report", get(v1::healing_report))
        .route("/api/v1/admin/auto-switches", get(v1::auto_switches))
        .route("/api/v1/admin/repair-channel", post(v1::repair_channel))
        .route(
            "/api/v1/admin/performance-report",
            get(v1::performance_report),
        )
        .route("/api/v1/admin/analytics", get(v1::analytics_report))
        .route("/api/v1/suggestions", get(v1::suggestions))
        .route(
            "/api/v1/auth/keys",
            post(auth::create_key).get(auth::list_keys),
        )
        .route("/api/v1/auth/keys/{id}", delete(auth::revoke_key));

    // WebSocket endpoint — only compiled when the `websocket` feature is on.
    #[cfg(feature = "websocket")]
    let protected = protected.route("/ws/v1/stream", get(websocket::ws_events));

    let protected = protected.route_layer(axum::middleware::from_fn_with_state(
        state.clone(),
        crate::middleware::auth_middleware,
    ));

    Router::new()
        .route("/health", get(health::health))
        .route("/metrics", get(metrics::metrics))
        .route("/openapi.json", get(openapi::openapi_json))
        .route("/docs", get(openapi::swagger_ui_html))
        .merge(protected)
        // Inner: cap request body size. Outer (added last): observe every
        // request for trace IDs, metrics, and concurrency limiting.
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::observe::observe_middleware,
        ))
        .with_state(state)
}
