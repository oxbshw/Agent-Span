//! Federated search endpoint — one query, many channels.

use axum::{extract::State, Json};
use serde::Deserialize;

use crate::AppState;

/// Body for `POST /api/v1/search/federated`.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct FederatedRequest {
    pub query: String,
    /// Channels to search by name; omit to search them all.
    #[serde(default)]
    pub channels: Option<Vec<String>>,
    /// Max merged results (0 -> default of 10).
    #[serde(default)]
    pub limit: usize,
    /// Re-rank merged results by lexical relevance to the query instead of the
    /// default source-count ordering.
    #[serde(default)]
    pub rerank: bool,
    /// Collapse near-duplicate results (same story under different URLs) by
    /// title similarity, merging their source channels.
    #[serde(default)]
    pub collapse: bool,
}

/// Search multiple channels concurrently and return merged, de-duplicated
/// results with per-source attribution.
#[utoipa::path(
    post,
    path = "/api/v1/search/federated",
    tag = "search",
    request_body = FederatedRequest,
    responses(
        (status = 200, description = "Merged, de-duplicated search results with per-source attribution", body = Value)
    )
)]
pub async fn federated_search(
    State(state): State<AppState>,
    Json(req): Json<FederatedRequest>,
) -> Json<serde_json::Value> {
    let limit = if req.limit == 0 {
        10
    } else {
        req.limit.min(50)
    };
    let mut results = state
        .registry
        .federated_search(&req.query, req.channels.as_deref(), limit)
        .await;
    if req.rerank {
        results.rerank(&req.query);
    }
    if req.collapse {
        results.collapse_near_duplicates(0.85);
    }
    Json(serde_json::to_value(results).unwrap_or(serde_json::Value::Null))
}

#[cfg(test)]
mod tests {
    use crate::AppState;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn federated_route_returns_merged_shape() {
        let app = AppState::default_state().router();
        // Empty channel list -> deterministic, no upstream calls.
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/search/federated")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"query":"rust","channels":[]}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["query"], "rust");
        assert!(json["results"].is_array());
        assert!(json["searched"].as_array().unwrap().is_empty());
    }
}
