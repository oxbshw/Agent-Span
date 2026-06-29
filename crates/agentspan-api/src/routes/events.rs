//! `GET /api/v1/events/stream` — live event stream over Server-Sent Events (SSE).
//!
//! Subscribes the client to the [`AppState`] broadcast bus and forwards every
//! published event (request activity, channel-status changes, audit entries) as an
//! SSE `data:` frame. SSE is used instead of WebSocket so the build stays free of
//! the native-TLS/tungstenite toolchain; for a one-way dashboard push it is
//! equivalent and auto-reconnects in the browser via `EventSource`.

use std::convert::Infallible;

use axum::{
    extract::State,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
};
use futures::Stream;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::AppState;

/// Stream live events to the client as SSE.
pub async fn events_sse(State(state): State<AppState>) -> Response {
    let rx = state.events.subscribe();
    let stream = sse_stream(rx, state.registry.list().len());
    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

/// Build the SSE stream: an initial `hello` event, then every broadcast message.
/// Lagged messages (slow client) are skipped rather than closing the stream.
fn sse_stream(
    rx: tokio::sync::broadcast::Receiver<String>,
    channels: usize,
) -> impl Stream<Item = Result<Event, Infallible>> {
    let hello = serde_json::json!({ "type": "hello", "channels": channels }).to_string();
    let head = tokio_stream::once(Ok(Event::default().data(hello)));
    let tail = BroadcastStream::new(rx).filter_map(|msg| match msg {
        Ok(data) => Some(Ok(Event::default().data(data))),
        Err(_) => None, // BroadcastStreamRecvError::Lagged — skip and continue.
    });
    head.chain(tail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppState;

    #[tokio::test]
    async fn publish_event_reaches_subscribers() {
        let state = AppState::default_state();
        let mut rx = state.events.subscribe();
        state.publish_event("hello");
        let got = rx.recv().await.unwrap();
        assert_eq!(got, "hello");
    }

    #[tokio::test]
    async fn publish_without_subscribers_is_silent() {
        let state = AppState::default_state();
        // No subscribers — must not panic or block.
        state.publish_event("dropped");
    }

    #[tokio::test]
    async fn sse_stream_emits_hello_then_events() {
        let state = AppState::default_state();
        let rx = state.events.subscribe();
        let mut stream = Box::pin(sse_stream(rx, 50));

        // First frame is the hello handshake.
        let first = stream.next().await.unwrap().unwrap();
        let rendered = format!("{first:?}");
        assert!(rendered.contains("hello"));

        // A subsequently published event flows through.
        state.publish_event(r#"{"type":"request"}"#);
        let second = stream.next().await.unwrap().unwrap();
        assert!(format!("{second:?}").contains("request"));
    }
}
