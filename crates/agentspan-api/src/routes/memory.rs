//! Agent memory routes — a namespaced key/value scratchpad.
//!
//! - `PUT    /api/v1/memory/{namespace}/{key}`  set a value (optional `ttl_secs`)
//! - `GET    /api/v1/memory/{namespace}/{key}`  read a value (404 if absent)
//! - `GET    /api/v1/memory/{namespace}`        list live keys
//! - `DELETE /api/v1/memory/{namespace}/{key}`  delete a value

use std::time::Duration;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::AppState;

/// Body for setting a memory entry.
#[derive(Debug, Deserialize)]
pub struct SetMemoryRequest {
    /// The value to store (any JSON).
    pub value: Value,
    /// Optional time-to-live in seconds; omit to keep until overwritten.
    #[serde(default)]
    pub ttl_secs: Option<u64>,
}

/// `PUT /api/v1/memory/{namespace}/{key}`
pub async fn set_memory(
    State(state): State<AppState>,
    Path((namespace, key)): Path<(String, String)>,
    Json(req): Json<SetMemoryRequest>,
) -> Json<Value> {
    let ttl = req.ttl_secs.map(Duration::from_secs);
    state.memory.set(&namespace, &key, req.value, ttl);
    Json(json!({ "ok": true, "namespace": namespace, "key": key }))
}

/// `GET /api/v1/memory/{namespace}/{key}`
pub async fn get_memory(
    State(state): State<AppState>,
    Path((namespace, key)): Path<(String, String)>,
) -> Response {
    match state.memory.get(&namespace, &key) {
        Some(value) => {
            Json(json!({ "namespace": namespace, "key": key, "value": value })).into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "not found", "namespace": namespace, "key": key })),
        )
            .into_response(),
    }
}

/// `GET /api/v1/memory/{namespace}`
pub async fn list_memory(
    State(state): State<AppState>,
    Path(namespace): Path<String>,
) -> Json<Value> {
    let keys = state.memory.list(&namespace);
    Json(json!({ "namespace": namespace, "keys": keys }))
}

/// `DELETE /api/v1/memory/{namespace}/{key}`
pub async fn delete_memory(
    State(state): State<AppState>,
    Path((namespace, key)): Path<(String, String)>,
) -> Json<Value> {
    let deleted = state.memory.delete(&namespace, &key);
    Json(json!({ "deleted": deleted }))
}

#[cfg(test)]
mod tests {
    use crate::AppState;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn body_json(resp: axum::response::Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn set_get_list_delete_lifecycle() {
        let app = AppState::default_state().router();

        // PUT a value.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/v1/memory/agent1/cursor")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"value":{"page":3}}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // GET it back.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/memory/agent1/cursor")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(body_json(resp).await["value"]["page"], 3);

        // LIST the namespace.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/memory/agent1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(body_json(resp).await["keys"][0], "cursor");

        // DELETE it.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/v1/memory/agent1/cursor")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(body_json(resp).await["deleted"], true);
    }

    #[tokio::test]
    async fn get_missing_returns_404() {
        let app = AppState::default_state().router();
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/memory/nobody/nothing")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
