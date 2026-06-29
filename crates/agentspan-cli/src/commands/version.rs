//! Version subcommand.

use clap::Args;

#[derive(Args)]
pub struct VersionArgs;

pub async fn run(_args: VersionArgs) -> anyhow::Result<()> {
    println!("AgentSpan {}", env!("CARGO_PKG_VERSION"));
    Ok(())
}
