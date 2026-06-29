//! Shared harness for integration tests: boots the AgentSpan API on an
//! ephemeral port and provides a configured [`reqwest::Client`].
//!
//! Tests exercise the full HTTP stack — router, middleware, auth, handlers —
//! exactly as a real client would. Network-touching channels are mocked or
//! shape-checked rather than asserted for content.

use agentspan_api::AppState;
use axum::serve;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

/// A running API instance reachable over real HTTP on `base_url`.
#[allow(dead_code)] // `state` is read by some test binaries but not all.
pub struct RunningApi {
    pub base_url: String,
    pub state: AppState,
    _server: JoinHandle<std::io::Result<()>>,
}

impl RunningApi {
    /// Boot the API with `AppState::default_state()` (permissive auth) on an
    /// OS-assigned port. Returns once `/health` responds.
    pub async fn start() -> Self {
        Self::start_with(AppState::default_state()).await
    }

    /// Boot with caller-supplied state (e.g. auth required).
    pub async fn start_with(state: AppState) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = state.clone().router();

        let server = tokio::spawn(async move { serve(listener, app.into_make_service()).await });

        let base_url = format!("http://{addr}");
        let api = Self {
            base_url,
            state,
            _server: server,
        };
        api.wait_for_health().await;
        api
    }

    async fn wait_for_health(&self) {
        let client = reqwest::Client::new();
        for _ in 0..50 {
            if let Ok(r) = client.get(format!("{}/health", self.base_url)).send().await {
                if r.status().is_success() {
                    return;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        panic!("API never became healthy at {}", self.base_url);
    }

    /// A fresh reqwest client with a 10s timeout.
    pub fn client(&self) -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap()
    }
}
