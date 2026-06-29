//! Read content from a URL via the appropriate channel.

use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::AppState;
use agentspan_core::types::ReadOptions;

/// Query parameters for the read endpoint.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ReadQuery {
    pub url: String,
    #[serde(default)]
    pub force_refresh: bool,
}

/// Read a URL and return the content.
#[utoipa::path(
    get,
    path = "/api/v1/read",
    tag = "read",
    params(
        ("url" = String, Query, description = "URL to read"),
        ("force_refresh" = Option<bool>, Query, description = "Skip cache")
    ),
    responses(
        (status = 200, description = "Content read successfully", body = Value),
        (status = 200, description = "No channel matched — error in body", body = Value)
    )
)]
pub async fn read(State(state): State<AppState>, Query(query): Query<ReadQuery>) -> Json<Value> {
    let channel = match state.registry.by_url(&query.url) {
        Some(c) => c,
        None => {
            // No channel matched — note the gap so popular platforms surface.
            state.healer.discoverer.record_unsupported(&query.url);
            return Json(json!({
                "error": "no channel can handle this URL",
                "url": query.url,
            }));
        }
    };

    let opts = ReadOptions {
        force_refresh: query.force_refresh,
        ..Default::default()
    };

    match channel.read(&query.url, opts).await {
        Ok(content) => Json(json!({
            "channel": channel.name(),
            "content": content,
        })),
        Err(e) => Json(json!({
            "error": e.to_string(),
            "channel": channel.name(),
            "url": query.url,
        })),
    }
}
