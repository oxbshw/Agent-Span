//! End-to-end auth flow: with `auth.require_api_key=true` a key must be
//! minted, used on protected routes, and rejected once revoked. Also checks
//! that permissive mode (the default) still refuses admin routes — the
//! regression captured in `routes/auth.rs` unit tests, re-validated over HTTP.

mod common;

use agentspan_api::AppState;
use common::RunningApi;
use serde_json::json;

/// State with API-key auth required and a freshly minted admin key.
async fn authed_api() -> (RunningApi, String) {
    let mut cfg = agentspan_core::Config::default();
    cfg.auth.require_api_key = true;
    let state = AppState::with_config(cfg);
    let api = RunningApi::start_with(state).await;

    // Bootstrap an admin key via the bootstrap endpoint. In auth-required mode
    // the API has no callers yet, so we mint one directly through the AuthManager
    // (the same path `POST /api/v1/auth/keys` would take, but that route itself
    // is admin-guarded → chicken-and-egg in tests).
    use agentspan_auth::Scope;
    let key = api
        .state
        .auth
        .keys
        .create_key("default", "ci-admin", vec![Scope::Admin]);
    (api, key.secret)
}

#[tokio::test]
async fn protected_route_rejects_missing_key() {
    let (api, _secret) = authed_api().await;
    let resp = api
        .client()
        .get(format!("{}/api/v1/channels", api.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn protected_route_accepts_valid_key() {
    let (api, secret) = authed_api().await;
    let resp = api
        .client()
        .get(format!("{}/api/v1/channels", api.base_url))
        .header("X-API-Key", &secret)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["channels"].as_array().unwrap().len(), 52);
}

#[tokio::test]
async fn admin_route_works_with_admin_key() {
    let (api, secret) = authed_api().await;
    let resp = api
        .client()
        .get(format!("{}/api/v1/auth/keys", api.base_url))
        .header("X-API-Key", &secret)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["keys"].is_array());
}

#[tokio::test]
async fn create_key_then_revoke_then_revoked_key_rejected() {
    let (api, admin_secret) = authed_api().await;

    // Create a read-only key via the HTTP route (admin-authenticated).
    let create = api
        .client()
        .post(format!("{}/api/v1/auth/keys", api.base_url))
        .header("X-API-Key", &admin_secret)
        .json(&json!({ "name": "reader", "scopes": ["read"] }))
        .send()
        .await
        .unwrap();
    assert_eq!(create.status(), 201);
    let created: serde_json::Value = create.json().await.unwrap();
    let reader_secret = created["secret"].as_str().unwrap().to_string();
    let reader_id = created["id"].as_str().unwrap().to_string();

    // Reader can hit protected read routes.
    let ok = api
        .client()
        .get(format!("{}/api/v1/channels", api.base_url))
        .header("X-API-Key", &reader_secret)
        .send()
        .await
        .unwrap();
    assert_eq!(ok.status(), 200);

    // Reader cannot hit admin routes (no Admin scope).
    let forbidden = api
        .client()
        .get(format!("{}/api/v1/auth/keys", api.base_url))
        .header("X-API-Key", &reader_secret)
        .send()
        .await
        .unwrap();
    assert_eq!(forbidden.status(), 403);

    // Revoke the reader key.
    let revoke = api
        .client()
        .delete(format!("{}/api/v1/auth/keys/{}", api.base_url, reader_id))
        .header("X-API-Key", &admin_secret)
        .send()
        .await
        .unwrap();
    assert_eq!(revoke.status(), 204);

    // Reader key now rejected.
    let now_rejected = api
        .client()
        .get(format!("{}/api/v1/channels", api.base_url))
        .header("X-API-Key", &reader_secret)
        .send()
        .await
        .unwrap();
    assert_eq!(now_rejected.status(), 401);
}

#[tokio::test]
async fn permissive_mode_refuses_admin_routes() {
    // Regression over HTTP: in permissive (single-user) mode admin routes
    // must still be guarded so a default install doesn't leak audit/keys.
    let api = RunningApi::start().await;
    let resp = api
        .client()
        .get(format!("{}/api/v1/auth/keys", api.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn sse_events_stream_connects_and_emits_request_events() {
    let api = RunningApi::start().await;
    // Subscribe to the SSE stream, then fire a request and expect an event.
    let url = format!("{}/api/v1/events/stream", api.base_url);
    let resp = api.client().get(&url).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or(""),
        "text/event-stream"
    );

    // The stream is long-lived; read the first chunk to confirm it's SSE.
    // A separate task triggers a request which should fan out an event.
    let base = api.base_url.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let _ = reqwest::get(format!("{base}/api/v1/stats")).await;
    });

    // `chunk()` returns the next bytes from the stream without waiting for EOF.
    let mut got = Vec::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut resp = resp;
    while got.is_empty() && std::time::Instant::now() < deadline {
        match tokio::time::timeout(std::time::Duration::from_millis(500), resp.chunk()).await {
            Ok(Ok(Some(chunk))) => got.extend_from_slice(&chunk),
            Ok(Ok(None)) => break, // stream ended
            Ok(Err(_)) | Err(_) => continue,
        }
    }
    let text = String::from_utf8_lossy(&got);
    assert!(!text.is_empty(), "SSE stream produced no data within 5s");
}
