//! `agentspan watch` — periodic health check for scheduled monitoring.

use std::time::Duration;

use agentspan_channels::ChannelRegistry;
use agentspan_core::types::ProbeStatus;
use clap::Args;

#[derive(Args)]
pub struct WatchArgs {
    /// Seconds between checks.
    #[arg(long, default_value_t = 60)]
    pub interval: u64,
    /// Run a single check and exit.
    #[arg(long)]
    pub once: bool,
}

/// Run one health pass and return `(ok, total)`.
async fn tick() -> (usize, usize) {
    let registry = ChannelRegistry::default_channels();
    let total = registry.list().len();
    let mut ok = 0;
    for ch in registry.list() {
        let healths = ch.check_health().await;
        if healths.iter().any(|h| h.probe.status == ProbeStatus::Ok) {
            ok += 1;
        }
    }
    (ok, total)
}

pub async fn run(args: WatchArgs) -> anyhow::Result<()> {
    loop {
        let (ok, total) = tick().await;
        println!("healthy: {ok}/{total} channels");
        if args.once {
            break;
        }
        tokio::time::sleep(Duration::from_secs(args.interval.max(1))).await;
    }
    Ok(())
}
