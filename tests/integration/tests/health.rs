//! `/health` is the liveness probe used by Docker, k8s, and load balancers.
//! It must return 200 `{"status":"ok"}` with no auth and minimal latency.

mod common;

use common::RunningApi;
use serde_json::json;

#[tokio::test]
async fn health_returns_200_ok_without_auth() {
    let api = RunningApi::start().await;
    let resp = api
        .client()
        .get(format!("{}/health", api.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body, json!({ "status": "ok" }));
}

#[tokio::test]
async fn metrics_returns_200_text_without_auth() {
    let api = RunningApi::start().await;
    let resp = api
        .client()
        .get(format!("{}/metrics", api.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let text = resp.text().await.unwrap();
    // Prometheus exposition format starts with # HELP / # TYPE or a metric name.
    assert!(
        text.contains("agentspan") || text.contains("#"),
        "metrics body did not look like Prometheus exposition: {text}"
    );
}

#[tokio::test]
async fn openapi_json_returns_valid_spec() {
    let api = RunningApi::start().await;
    let resp = api
        .client()
        .get(format!("{}/openapi.json", api.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["openapi"], "3.0.3");
    assert!(body["paths"].is_object(), "paths missing from spec");
    assert!(
        body["paths"]["/api/v1/channels"].is_object(),
        "channels path missing from spec"
    );
    assert!(
        body["paths"]["/api/v1/doctor"].is_object(),
        "doctor path missing from spec"
    );
    assert!(
        body["components"]["schemas"]["Content"].is_object(),
        "Content schema missing"
    );
}

#[tokio::test]
async fn docs_returns_html_swagger_ui() {
    let api = RunningApi::start().await;
    let resp = api
        .client()
        .get(format!("{}/docs", api.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let text = resp.text().await.unwrap();
    assert!(text.contains("swagger-ui"), "Swagger UI not in /docs");
    assert!(text.contains("/openapi.json"), "spec URL not wired");
}
