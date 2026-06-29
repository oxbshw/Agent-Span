//! Axum server entry point.

use agentspan_api::AppState;
use agentspan_core::Config;

#[tokio::main]
async fn main() {
    // Defaults bind to 127.0.0.1. In permissive (require_api_key=false) mode,
    // admin routes are gated off entirely; enable require_api_key for a real
    // multi-user deployment before binding to a public address.
    let config = Config::load().unwrap_or_default();
    let addr = format!("{}:{}", config.server.host, config.server.port);

    let state = AppState::with_config(config);

    // Start the background self-healing monitor: it probes every channel on an
    // interval and feeds the shared healing snapshots.
    let _healer = state.spawn_healer();
    tracing::info!("self-healing monitor started");

    let app = state.clone().router();

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    println!("AgentSpan API listening on http://{addr}");
    println!("  self-healing monitor running (probes every 30s)");
    if !addr.starts_with("127.") && !addr.starts_with("localhost") {
        println!(
            "  warning: bound to a non-local address — set auth.require_api_key=true \
             to protect admin routes"
        );
    }
    tracing::info!(%addr, "AgentSpan API listening");
    axum::serve(listener, app).await.unwrap();
}
