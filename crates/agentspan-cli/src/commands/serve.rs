//! `agentspan serve` — start the REST API gateway.

use agentspan_api::AppState;
use agentspan_core::Config;
use clap::Args;

use crate::style::{status_info, status_ok, status_warn};

#[derive(Args)]
pub struct ServeArgs {
    /// Host to bind
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Port to bind
    #[arg(long, default_value_t = 8080)]
    pub port: u16,
}

pub async fn run(args: ServeArgs) -> anyhow::Result<()> {
    // Start from the saved config, then apply CLI overrides.
    let mut config = Config::load().unwrap_or_default();
    config.server.host = args.host.clone();
    config.server.port = args.port;

    let addr = format!("{}:{}", args.host, args.port);
    let app = AppState::with_config(config).router();

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    status_info("Starting server...");
    status_ok(&format!("REST API     bound to {addr}"));
    status_ok("SSE endpoint /api/v1/events/stream");
    status_ok("Health monitor spawned");
    if !args.host.starts_with("127.") && args.host != "localhost" {
        status_warn(
            "Bound to a non-local address — set auth.require_api_key=true to protect admin routes.",
        );
    }
    status_info(&format!("Server ready — http://{addr}  (Ctrl+C to stop)"));
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    println!("Shutdown complete.");
    Ok(())
}

/// Resolve when the process receives Ctrl-C or (on Unix) SIGTERM, so in-flight
/// requests can drain before the server stops accepting new connections.
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
    println!("\nSignal received, shutting down gracefully...");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn serve_responds_on_health() {
        // Bind a random port and serve the real router (the same path `run` uses).
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = AppState::default_state().router();
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let body = reqwest::get(format!("http://{addr}/health"))
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(body.contains("ok"), "unexpected /health body: {body}");

        handle.abort();
    }
}
