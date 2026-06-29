//! Verifies the three core discovery endpoints a new client calls first:
//! `GET /api/v1/channels`, `GET /api/v1/doctor`, and `POST /api/v1/read`.
//!
//! These tests run against a fully booted server (no network egress for
//! upstream platforms; `web` channel can hit the real Jina endpoint, so we
//! assert shape rather than content for live reads).

mod common;

use agentspan_api::AppState;
use common::RunningApi;
use serde_json::json;

#[tokio::test]
async fn channels_lists_all_52() {
    let api = RunningApi::start().await;
    let resp = api
        .client()
        .get(format!("{}/api/v1/channels", api.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let channels = body["channels"].as_array().expect("channels is an array");
    assert_eq!(channels.len(), 52, "expected the full channel registry");
    // Every entry has the documented fields.
    for ch in channels {
        assert!(ch["name"].is_string(), "missing name: {ch}");
        assert!(ch["description"].is_string(), "missing description: {ch}");
        assert!(ch["tier"].is_string(), "missing tier: {ch}");
    }
}

#[tokio::test]
async fn channel_info_known_returns_200() {
    let api = RunningApi::start().await;
    let resp = api
        .client()
        .get(format!("{}/api/v1/channels/hackernews", api.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["name"], "hackernews");
    assert!(body["backends"].is_array());
}

#[tokio::test]
async fn channel_info_unknown_returns_404_with_did_you_mean() {
    let api = RunningApi::start().await;
    let resp = api
        .client()
        .get(format!("{}/api/v1/channels/githubb", api.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
    let body: serde_json::Value = resp.json().await.unwrap();
    let suggestions = body["did_you_mean"]
        .as_array()
        .expect("did_you_mean present");
    assert!(suggestions.iter().any(|v| v == "github"));
}

#[tokio::test]
async fn doctor_runs_across_all_channels() {
    let api = RunningApi::start().await;
    let resp = api
        .client()
        .get(format!("{}/api/v1/doctor", api.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["total"], 52);
    let channels = body["channels"].as_array().expect("channels array");
    assert_eq!(channels.len(), 52);
    // Every channel row exposes per-backend health.
    for ch in channels {
        assert!(ch["backends"].is_array());
        assert!(ch["channel"].is_string());
    }
}

#[tokio::test]
async fn smart_read_unhandled_url_records_demand_and_returns_422() {
    let api = RunningApi::start().await;
    let resp = api
        .client()
        .post(format!("{}/api/v1/read", api.base_url))
        .json(&json!({ "url": "ftp://nothing.example.invalid" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 422);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"], "no channel can handle this URL");
}

#[tokio::test]
async fn stats_reports_counts() {
    let api = RunningApi::start().await;
    let resp = api
        .client()
        .get(format!("{}/api/v1/stats", api.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["channels"], 52);
    assert!(body["recent"].is_array());
}

#[tokio::test]
async fn config_does_not_leak_secrets_over_http() {
    let mut cfg = agentspan_core::Config::default();
    cfg.api_keys
        .insert("openai".to_string(), "sk-supersecret-over-http".to_string());
    let state = AppState::with_config(cfg);
    let api = RunningApi::start_with(state).await;

    let resp = api
        .client()
        .get(format!("{}/api/v1/config", api.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let text = resp.text().await.unwrap();
    assert!(
        !text.contains("sk-supersecret-over-http"),
        "secret leaked through /api/v1/config"
    );
    assert!(text.contains("\"server\""));
}
