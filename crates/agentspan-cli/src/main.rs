//! AgentSpan CLI.

mod commands;
mod cookies;
mod env_detect;
mod style;

use clap::{CommandFactory, Parser, Subcommand};
use commands::{
    benchmark, check_update, completions, config, doctor, format, install, loadtest, mcp, plugin,
    serve, setup, skill, transcribe, tunnel, uninstall, version, watch,
};

#[derive(Parser)]
#[command(name = "agentspan")]
#[command(version)]
#[command(about = "The Web Access Gateway for AI Agents")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Install channel dependencies
    Install(install::InstallArgs),
    /// Remove installed skills and config
    Uninstall(uninstall::UninstallArgs),
    /// Environment summary and guided setup
    Setup(setup::SetupArgs),
    /// Run system diagnostics
    Doctor(doctor::DoctorArgs),
    /// Periodic health check (for scheduled tasks)
    Watch(watch::WatchArgs),
    /// Explain how each channel reduces tokens (format_for_llm)
    Format(format::FormatArgs),
    /// Run a synthetic throughput/latency benchmark
    Benchmark(benchmark::BenchmarkArgs),
    /// Load-test an HTTP endpoint (concurrency, throughput, p50/p99/p999)
    LoadTest(loadtest::LoadTestArgs),
    /// Download and transcribe audio/video via Whisper
    Transcribe(transcribe::TranscribeArgs),
    /// Expose the local API on a public URL
    Tunnel(tunnel::TunnelArgs),
    /// Discover and manage community channel plugins
    Plugin(plugin::PluginArgs),
    /// Show or modify configuration; import cookies
    Config(config::ConfigArgs),
    /// Generate and install the agent skill (SKILL.md)
    Skill(skill::SkillArgs),
    /// MCP server config and tool discovery
    Mcp(mcp::McpArgs),
    /// Check crates.io for a newer version
    Update(check_update::CheckUpdateArgs),
    /// Start the API server
    Serve(serve::ServeArgs),
    /// Generate shell completion scripts
    Completions(completions::CompletionsArgs),
    /// Show version information
    Version(version::VersionArgs),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Print the banner for normal commands, but not for version/help output.
    let suppress_banner =
        std::env::args().any(|a| matches!(a.as_str(), "--version" | "-V" | "--help" | "-h"));
    if !suppress_banner {
        style::banner();
    }
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Install(args)) => install::run(args).await,
        Some(Commands::Uninstall(args)) => uninstall::run(args).await,
        Some(Commands::Setup(args)) => setup::run(args).await,
        Some(Commands::Doctor(args)) => doctor::run(args).await,
        Some(Commands::Watch(args)) => watch::run(args).await,
        Some(Commands::Format(args)) => format::run(args).await,
        Some(Commands::Benchmark(args)) => benchmark::run(args).await,
        Some(Commands::LoadTest(args)) => loadtest::run(args).await,
        Some(Commands::Transcribe(args)) => transcribe::run(args).await,
        Some(Commands::Tunnel(args)) => tunnel::run(args).await,
        Some(Commands::Plugin(args)) => plugin::run(args).await,
        Some(Commands::Config(args)) => config::run(args).await,
        Some(Commands::Skill(args)) => skill::run(args).await,
        Some(Commands::Mcp(args)) => mcp::run(args).await,
        Some(Commands::Update(args)) => check_update::run(args).await,
        Some(Commands::Serve(args)) => serve::run(args).await,
        Some(Commands::Completions(args)) => completions::run(args, Cli::command()),
        Some(Commands::Version(args)) => version::run(args).await,
        None => {
            println!("Run 'agentspan --help' for available commands.");
            Ok(())
        }
    }
}
