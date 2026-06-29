//! Health check route.

use axum::{http::StatusCode, Json};
use serde_json::{json, Value};

/// Liveness probe — returns `{"status":"ok"}` with no auth.
#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    responses(
        (status = 200, description = "API is healthy", body = Value, example = json!({"status":"ok"}))
    )
)]
pub async fn health() -> (StatusCode, Json<Value>) {
    (StatusCode::OK, Json(json!({ "status": "ok" })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn health_returns_ok() {
        let (status, Json(body)) = health().await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!({ "status": "ok" }));
    }
}
