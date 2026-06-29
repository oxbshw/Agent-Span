//! Outermost observability middleware: trace-ID propagation, request metrics,
//! and a global concurrency limit (load shedding).

use axum::{
    extract::{Request, State},
    http::{HeaderName, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::time::Instant;
use tracing::Instrument;

use crate::AppState;

/// Header used to read and echo a request's trace ID.
pub const TRACE_HEADER: &str = "x-trace-id";

/// A request-scoped trace identifier, stored in request extensions so inner
/// handlers and middleware (e.g. audit, event publishing) can correlate logs.
#[derive(Debug, Clone)]
pub struct TraceId(pub String);

/// Generate a fresh 128-bit trace ID as lowercase hex.
fn generate_trace_id() -> String {
    format!("{:032x}", rand::random::<u128>())
}

fn set_trace_header(resp: &mut Response, trace_id: &str) {
    if let Ok(value) = HeaderValue::from_str(trace_id) {
        resp.headers_mut()
            .insert(HeaderName::from_static(TRACE_HEADER), value);
    }
}

/// The outermost middleware. It:
/// 1. resolves an incoming `x-trace-id`/`x-request-id` or generates one, and
///    attaches it to the request extensions and the response headers;
/// 2. enforces a global in-flight concurrency limit, shedding excess load with
///    `503 Service Unavailable` rather than exhausting memory; and
/// 3. records per-request metrics (count, errors, latency).
pub async fn observe_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    let trace_id = req
        .headers()
        .get(TRACE_HEADER)
        .or_else(|| req.headers().get("x-request-id"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(generate_trace_id);
    req.extensions_mut().insert(TraceId(trace_id.clone()));

    // Shed load when saturated instead of queueing unboundedly.
    let _permit = match state.inflight.clone().try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => {
            state.metrics.record_rejected();
            let mut resp = (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "server at capacity", "trace_id": trace_id })),
            )
                .into_response();
            set_trace_header(&mut resp, &trace_id);
            return resp;
        }
    };

    let span = tracing::info_span!("request", trace_id = %trace_id);
    let start = Instant::now();
    let mut resp = next.run(req).instrument(span).await;
    let latency_ms = start.elapsed().as_millis() as u64;

    state.metrics.record(resp.status().as_u16(), latency_ms);
    set_trace_header(&mut resp, &trace_id);
    resp
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppState;
    use axum::body::Body;
    use axum::http::Request as HttpRequest;
    use tower::ServiceExt;

    #[tokio::test]
    async fn generates_and_echoes_trace_id() {
        let app = AppState::default_state().router();
        let resp = app
            .oneshot(
                HttpRequest::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            resp.headers().get(TRACE_HEADER).is_some(),
            "response should carry a generated trace id"
        );
    }

    #[tokio::test]
    async fn preserves_incoming_trace_id() {
        let app = AppState::default_state().router();
        let resp = app
            .oneshot(
                HttpRequest::builder()
                    .uri("/health")
                    .header(TRACE_HEADER, "abc123")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            resp.headers().get(TRACE_HEADER).unwrap().to_str().unwrap(),
            "abc123"
        );
    }
}
