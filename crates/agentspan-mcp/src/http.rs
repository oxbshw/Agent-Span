//! MCP HTTP transport — serves JSON-RPC 2.0 over HTTP POST and SSE.
//!
//! Exposed when the `http` cargo feature is enabled. Endpoints:
//!
//! - `POST /mcp` — single JSON-RPC request → single JSON-RPC response.
//! - `GET /mcp/sse` — SSE stream of server-to-client events (initialize
//!   handshake, tool results for long-running calls).
//!
//! This lets non-stdio MCP clients (remote agents, web-based MCP consumers,
//! multi-tenant gateways) talk to AgentSpan without spawning a subprocess.

#![cfg(feature = "http")]

use axum::{
    extract::State,
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use tokio_stream::Stream;

use crate::McpServer;

/// Shared state for the HTTP transport.
#[derive(Clone)]
struct HttpState {
    server: McpServer,
}

/// Run the MCP HTTP transport on the given address.
pub async fn run_http(server: McpServer, addr: &str) -> anyhow::Result<()> {
    let state = HttpState { server };
    let app = Router::new()
        .route("/mcp", post(handle_jsonrpc))
        .route("/mcp/sse", get(handle_sse))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("MCP HTTP transport listening on {addr}");
    axum::serve(listener, app.into_make_service()).await?;
    Ok(())
}

/// `POST /mcp` — handle a single JSON-RPC request.
async fn handle_jsonrpc(State(state): State<HttpState>, Json(req): Json<Value>) -> Response {
    match state.server.handle_request(&req).await {
        Some(resp) => Json(resp).into_response(),
        None => StatusCode::NO_CONTENT.into_response(), // notification
    }
}

/// `GET /mcp/sse` — SSE stream. Sends an initial `endpoint` event pointing
/// the client to `POST /mcp`, then keeps the connection alive.
async fn handle_sse(State(_state): State<HttpState>) -> Response {
    let stream = sse_stream();
    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

fn sse_stream() -> impl Stream<Item = Result<Event, std::convert::Infallible>> {
    let endpoint = json!({ "endpoint": "/mcp" }).to_string();
    let head = tokio_stream::once(Ok(Event::default().event("endpoint").data(endpoint)));
    head
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::McpServer;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn app() -> Router {
        let state = HttpState {
            server: McpServer::new(),
        };
        Router::new()
            .route("/mcp", post(handle_jsonrpc))
            .route("/mcp/sse", get(handle_sse))
            .with_state(state)
    }

    #[tokio::test]
    async fn http_initialize_returns_server_info() {
        let app = app();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mcp")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["result"]["serverInfo"]["name"], "agentspan");
    }

    #[tokio::test]
    async fn http_tools_list_returns_tools() {
        let app = app();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mcp")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap();
        assert!(json["result"]["tools"].is_array());
        assert!(json["result"]["tools"].as_array().unwrap().len() >= 15);
    }

    #[tokio::test]
    async fn http_notification_returns_204() {
        let app = app();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mcp")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn http_unknown_method_returns_error() {
        let app = app();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mcp")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"jsonrpc":"2.0","id":3,"method":"no/such/method"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["error"]["code"], -32601);
    }

    #[tokio::test]
    async fn http_sse_returns_endpoint_event() {
        let app = app();
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/mcp/sse")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers()
                .get("content-type")
                .unwrap()
                .to_str()
                .unwrap(),
            "text/event-stream"
        );
    }
}
