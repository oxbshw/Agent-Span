//! `GET /metrics` — Prometheus metrics endpoint (public, unauthenticated).

use axum::{extract::State, http::header, response::IntoResponse};

use crate::AppState;

/// Render process metrics in Prometheus text exposition format.
///
/// Deliberately mounted outside the auth layer so monitoring systems can scrape
/// it without an API key, matching the conventional `/metrics` contract.
pub async fn metrics(State(state): State<AppState>) -> impl IntoResponse {
    let body = state.metrics.render(state.registry.list().len());
    ([(header::CONTENT_TYPE, "text/plain; version=0.0.4")], body)
}

#[cfg(test)]
mod tests {
    use crate::AppState;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn metrics_endpoint_is_public_and_prometheus() {
        let app = AppState::default_state().router();
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(text.contains("agentspan_requests_total"));
        assert!(text.contains("agentspan_channels"));
    }
}
