//! Doctor route.

use axum::{extract::State, Json};
use serde_json::{json, Value};

use agentspan_core::types::{BackendHealth, ProbeStatus};

use crate::AppState;

/// Name + status of the backend currently serving a channel: first `Ok`, else
/// first `Warn`.
fn active_backend(healths: &[BackendHealth]) -> Option<(String, ProbeStatus)> {
    healths
        .iter()
        .find(|h| h.probe.status == ProbeStatus::Ok)
        .or_else(|| healths.iter().find(|h| h.probe.status == ProbeStatus::Warn))
        .map(|h| (h.backend_name.clone(), h.probe.status))
}

/// Run health checks across all registered channels and return an aggregated report.
#[utoipa::path(
    get,
    path = "/api/v1/doctor",
    tag = "doctor",
    responses(
        (status = 200, description = "Aggregated health report across all channels", body = Value,
         example = json!({"ok":38,"total":50,"channels":[{"channel":"web","tier":"Zero","backends":[{"backend":"jina","status":"ok","active":true}]}]}))
    )
)]
pub async fn doctor(State(state): State<AppState>) -> Json<Value> {
    let mut channels = Vec::new();
    let mut ok = 0usize;
    let total = state.registry.list().len();

    for channel in state.registry.list() {
        let channel = channel.clone();
        let healths = channel.check_health().await;
        let active = active_backend(&healths);
        if active
            .as_ref()
            .map(|(_, s)| *s == ProbeStatus::Ok)
            .unwrap_or(false)
        {
            ok += 1;
        }

        channels.push(json!({
            "channel": channel.name(),
            "description": channel.description(),
            "tier": format!("{:?}", channel.tier()),
            "active_backend": active.as_ref().map(|(n, _)| n.clone()),
            "backends": healths.iter().map(|h| json!({
                "backend": h.backend_name,
                "status": format!("{:?}", h.probe.status).to_lowercase(),
                "active": active.as_ref().map(|(n, _)| n == &h.backend_name).unwrap_or(false),
                "latency_ms": h.latency_ms,
                "hint": h.probe.hint,
            })).collect::<Vec<_>>(),
        }));
    }

    Json(json!({
        "ok": ok,
        "total": total,
        "channels": channels,
    }))
}
