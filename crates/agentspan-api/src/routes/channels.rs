//! Channel routes.

use axum::{extract::State, Json};
use serde_json::{json, Value};

use crate::AppState;

/// List all registered channels.
#[utoipa::path(
    get,
    path = "/api/v1/channels",
    tag = "channels",
    responses(
        (status = 200, description = "List of all registered channels", body = Value,
         example = json!({"channels":[{"name":"web","description":"Read any URL","tier":"Zero"}]}))
    )
)]
pub async fn list_channels(State(state): State<AppState>) -> Json<Value> {
    let channels: Vec<Value> = state
        .registry
        .list()
        .iter()
        .map(|c| {
            json!({
                "name": c.name(),
                "description": c.description(),
                "tier": format!("{:?}", c.tier()),
            })
        })
        .collect();

    Json(json!({ "channels": channels }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn list_channels_returns_default_channels() {
        let state = AppState::default_state();
        let Json(body) = list_channels(State(state)).await;
        let channels = body["channels"].as_array().unwrap();
        assert_eq!(channels.len(), 52);
    }
}
