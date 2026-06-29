//! `GET /ws/v1/stream` — live event stream over WebSocket.
//!
//! This is the spec-mandated bidirectional transport, gated behind the
//! `websocket` cargo feature. When the feature is off (the default, so the
//! build stays clean on windows-gnu where axum's `ws` feature doesn't
//! compile), clients should use the SSE endpoint at `/api/v1/events/stream`
//! instead — it carries the same events one-way.
//!
//! Auth: browsers cannot set custom headers on a WebSocket upgrade, so the
//! API key is read from the `token` query parameter when `auth.require_api_key`
//! is set. The key is validated exactly as the `X-API-Key` header would be.

#![cfg(feature = "websocket")]

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio_stream::wrappers::BroadcastStream;

use crate::AppState;

/// Query parameters accepted by the WebSocket endpoint.
#[derive(Debug, Deserialize)]
pub struct WsAuth {
    /// API key — required when `auth.require_api_key` is true. Browsers can't
    /// set headers on WS upgrades, so we accept it as a query param.
    pub token: Option<String>,
}

/// WebSocket upgrade handler. Validates auth, then streams broadcast events.
pub async fn ws_events(
    State(state): State<AppState>,
    Query(q): Query<WsAuth>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    // Validate the API key if auth is enforced.
    if state.config.auth.require_api_key {
        let token = q.token.as_deref().unwrap_or("");
        if state.auth.authenticate(token).is_err() {
            return axum::http::StatusCode::UNAUTHORIZED.into_response();
        }
    }
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Drive a connected WebSocket: forward broadcast events as text messages.
async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // Send a hello handshake so the client knows it connected.
    let hello =
        serde_json::json!({ "type": "hello", "channels": state.registry.list().len() }).to_string();
    let _ = sender.send(Message::Text(hello.into())).await;

    // Subscribe to the broadcast bus and forward events.
    let rx = state.events.subscribe();
    let mut stream = BroadcastStream::new(rx);

    // Pump events to the client; also drain incoming messages (ping/pong/close)
    // so the server doesn't stall on a quiet client.
    loop {
        tokio::select! {
            // Forward a broadcast event to the client.
            Some(Ok(msg)) = stream.next() => {
                if sender.send(Message::Text(msg.into())).await.is_err() {
                    break;
                }
            }
            // Handle a client → server message (close, ping, etc).
            maybe_msg = receiver.next() => {
                match maybe_msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn ws_auth_query_parses_token() {
        let q = WsAuth {
            token: Some("ask_test".to_string()),
        };
        assert_eq!(q.token.as_deref(), Some("ask_test"));
    }
}
