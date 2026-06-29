//! Batch endpoints — parallel multi-URL reads and multi-query searches.
//!
//! Both run their items concurrently via [`futures::future::join_all`] and return
//! a per-item `{ok|error}` array so one failure never sinks the whole batch.

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use agentspan_core::types::{ReadOptions, SearchOptions};

use crate::AppState;

/// Maximum items accepted in a single batch request.
const MAX_BATCH: usize = 50;

/// Body for `POST /api/v1/batch/read`.
#[derive(Debug, Deserialize)]
pub struct BatchReadBody {
    pub urls: Vec<String>,
    #[serde(default)]
    pub force_refresh: bool,
}

/// Body for `POST /api/v1/batch/search`.
#[derive(Debug, Deserialize)]
pub struct BatchSearchBody {
    pub channel: String,
    pub queries: Vec<String>,
    #[serde(default)]
    pub limit: usize,
}

fn too_many() -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({ "error": format!("batch exceeds the limit of {MAX_BATCH} items") })),
    )
        .into_response()
}

/// `POST /api/v1/batch/read` — read many URLs in parallel (smart channel detection).
pub async fn batch_read(
    State(state): State<AppState>,
    Json(body): Json<BatchReadBody>,
) -> Response {
    if body.urls.len() > MAX_BATCH {
        return too_many();
    }
    let force = body.force_refresh;
    let jobs = body.urls.into_iter().map(|url| {
        let channel = state.registry.by_url(&url);
        async move {
            match channel {
                Some(ch) => {
                    let opts = ReadOptions {
                        force_refresh: force,
                        ..Default::default()
                    };
                    match ch.read(&url, opts).await {
                        Ok(content) => json!({
                            "url": url, "ok": true, "channel": ch.name(), "content": content
                        }),
                        Err(e) => json!({
                            "url": url, "ok": false, "channel": ch.name(), "error": e.to_string()
                        }),
                    }
                }
                None => {
                    json!({ "url": url, "ok": false, "error": "no channel can handle this URL" })
                }
            }
        }
    });
    let results: Vec<Value> = futures::future::join_all(jobs).await;
    Json(json!({ "count": results.len(), "results": results })).into_response()
}

/// `POST /api/v1/batch/search` — run many queries against one channel in parallel.
pub async fn batch_search(
    State(state): State<AppState>,
    Json(body): Json<BatchSearchBody>,
) -> Response {
    if body.queries.len() > MAX_BATCH {
        return too_many();
    }
    let Some(channel) = state.registry.by_name(&body.channel) else {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "error": format!("channel not found: {}", body.channel) })),
        )
            .into_response();
    };
    let limit = body.limit;
    let jobs = body.queries.into_iter().map(|query| {
        let channel = channel.clone();
        async move {
            let opts = SearchOptions {
                limit,
                ..Default::default()
            };
            match channel.search(&query, opts).await {
                Ok(results) => json!({ "query": query, "ok": true, "results": results }),
                Err(e) => json!({ "query": query, "ok": false, "error": e.to_string() }),
            }
        }
    });
    let results: Vec<Value> = futures::future::join_all(jobs).await;
    Json(json!({ "channel": channel.name(), "count": results.len(), "results": results }))
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    async fn body_json(resp: Response) -> (StatusCode, Value) {
        let status = resp.status();
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    #[tokio::test]
    async fn batch_read_reports_unhandled_urls_without_network() {
        // Non-http(s) URLs match no channel, so this exercises aggregation only.
        let state = AppState::default_state();
        let body = BatchReadBody {
            urls: vec!["ftp://x".to_string(), "mailto:y".to_string()],
            force_refresh: false,
        };
        let resp = batch_read(State(state), Json(body)).await;
        let (status, json) = body_json(resp).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["count"], 2);
        assert_eq!(json["results"][0]["ok"], false);
        assert!(json["results"][1]["error"]
            .as_str()
            .unwrap()
            .contains("no channel"));
    }

    #[tokio::test]
    async fn batch_read_rejects_oversized_batch() {
        let state = AppState::default_state();
        let body = BatchReadBody {
            urls: vec!["ftp://x".to_string(); MAX_BATCH + 1],
            force_refresh: false,
        };
        let resp = batch_read(State(state), Json(body)).await;
        let (status, _) = body_json(resp).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn batch_search_unknown_channel_is_422() {
        let state = AppState::default_state();
        let body = BatchSearchBody {
            channel: "does-not-exist".to_string(),
            queries: vec!["rust".to_string()],
            limit: 5,
        };
        let resp = batch_search(State(state), Json(body)).await;
        let (status, json) = body_json(resp).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert!(json["error"]
            .as_str()
            .unwrap()
            .contains("channel not found"));
    }

    #[tokio::test]
    async fn batch_search_rejects_oversized_batch() {
        let state = AppState::default_state();
        let body = BatchSearchBody {
            channel: "hackernews".to_string(),
            queries: vec!["q".to_string(); MAX_BATCH + 1],
            limit: 5,
        };
        let resp = batch_search(State(state), Json(body)).await;
        let (status, _) = body_json(resp).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }
}
