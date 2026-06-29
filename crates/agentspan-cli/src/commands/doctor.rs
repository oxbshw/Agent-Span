//! `agentspan doctor` — live channel health with the active backend marked.

use agentspan_channels::ChannelRegistry;
use agentspan_core::types::{BackendHealth, ProbeStatus};
use clap::Args;
use serde_json::json;

use crate::style;

#[derive(Args)]
pub struct DoctorArgs {
    /// Output machine-readable JSON instead of the text report.
    #[arg(long)]
    pub json: bool,
}

/// Index of the backend currently serving a channel: first `Ok`, else first `Warn`.
fn active_index(healths: &[BackendHealth]) -> Option<usize> {
    healths
        .iter()
        .position(|h| h.probe.status == ProbeStatus::Ok)
        .or_else(|| {
            healths
                .iter()
                .position(|h| h.probe.status == ProbeStatus::Warn)
        })
}

pub async fn run(args: DoctorArgs) -> anyhow::Result<()> {
    let registry = ChannelRegistry::default_channels();

    let mut channels_json = Vec::new();
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut ok_count = 0usize;
    let mut degraded = 0usize;
    let mut offline = 0usize;
    let total = registry.list().len();

    for ch in registry.list() {
        let healths = ch.check_health().await;
        let active = active_index(&healths);

        let status_str = match active {
            Some(i) if healths[i].probe.status == ProbeStatus::Ok => {
                ok_count += 1;
                "ok"
            }
            Some(_) => {
                degraded += 1;
                "degraded"
            }
            None => {
                offline += 1;
                "offline"
            }
        };

        let primary = active
            .map(|i| healths[i].backend_name.clone())
            .or_else(|| healths.first().map(|h| h.backend_name.clone()))
            .unwrap_or_else(|| "—".to_string());
        let fallback = healths
            .iter()
            .map(|h| h.backend_name.clone())
            .find(|n| *n != primary)
            .unwrap_or_else(|| "—".to_string());
        rows.push(vec![
            ch.name().to_string(),
            primary,
            status_str.to_string(),
            fallback,
        ]);

        let active_name = active.map(|i| healths[i].backend_name.clone());
        channels_json.push(json!({
            "channel": ch.name(),
            "description": ch.description(),
            "tier": format!("{:?}", ch.tier()),
            "active_backend": active_name,
            "backends": healths.iter().map(|h| json!({
                "backend": h.backend_name,
                "status": format!("{:?}", h.probe.status).to_lowercase(),
                "latency_ms": h.latency_ms,
                "hint": h.probe.hint,
            })).collect::<Vec<_>>(),
        }));
    }

    if args.json {
        let out = json!({
            "ok": ok_count,
            "total": total,
            "channels": channels_json,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        style::section("Channel Diagnostics");
        style::print_table(&["Channel", "Primary", "Status", "Fallback"], &rows);
        println!();
        println!(
            "{}{ok_count} OK{}  {}{degraded} degraded{}  {}{offline} offline{}",
            style::green(),
            style::reset(),
            style::yellow(),
            style::reset(),
            style::red(),
            style::reset(),
        );
    }
    Ok(())
}
