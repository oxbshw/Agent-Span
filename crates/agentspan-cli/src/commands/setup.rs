//! `agentspan setup` — environment summary + guided next steps.
//!
//! Non-interactive by design (safe to run in CI/agents): it detects the
//! environment, reports channel health, and prints the exact commands to unlock
//! more channels.

use agentspan_channels::ChannelRegistry;
use agentspan_core::types::ProbeStatus;
use clap::Args;

use crate::env_detect;

#[derive(Args)]
pub struct SetupArgs;

pub async fn run(_args: SetupArgs) -> anyhow::Result<()> {
    println!("AgentSpan Setup");
    println!("===============");
    let env = env_detect::detect();
    println!("Detected environment: {}", env.label());
    println!();

    let registry = ChannelRegistry::default_channels();
    let mut healthy = Vec::new();
    let mut needs_setup = Vec::new();
    for ch in registry.list() {
        let healths = ch.check_health().await;
        if healths.iter().any(|h| h.probe.status == ProbeStatus::Ok) {
            healthy.push(ch.name().to_string());
        } else {
            needs_setup.push(ch.name().to_string());
        }
    }

    println!("Ready now ({}): {}", healthy.len(), healthy.join(", "));
    if !needs_setup.is_empty() {
        println!();
        println!(
            "Needs setup ({}): {}",
            needs_setup.len(),
            needs_setup.join(", ")
        );
        println!();
        println!("Next steps:");
        println!("  1. Install upstream tools:   agentspan install --channels all");
        println!("  2. Import login cookies:     agentspan config from-browser chrome");
        println!("  3. Add a search/transcribe key:");
        println!("       agentspan config set api_keys.groq gsk_xxx   # free Whisper");
        println!("  4. Register the agent skill: agentspan skill install");
        println!("  5. Re-check:                 agentspan doctor");
    } else {
        println!();
        println!("All channels are ready. Run `agentspan skill install` to register the skill.");
    }
    Ok(())
}
