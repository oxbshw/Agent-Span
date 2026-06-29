//! `agentspan install` — install the upstream tools channels shell out to.
//!
//! Mirrors Agent Reach's installer: environment-aware (`--env auto`), with
//! `--safe` (inspect only) and `--dry-run` (preview) modes, plus optional
//! per-channel tool installs (`--channels twitter,reddit,...`).

use clap::Args;

use crate::env_detect::{self, Environment};

#[derive(Args)]
pub struct InstallArgs {
    /// Target environment: local, server, or auto-detect.
    #[arg(long, default_value = "auto")]
    pub env: String,
    /// Inspect only — report what's missing without changing the system.
    #[arg(long)]
    pub safe: bool,
    /// Preview the plan without making changes.
    #[arg(long)]
    pub dry_run: bool,
    /// Comma-separated optional channels to install tools for (or "all").
    #[arg(long, default_value = "")]
    pub channels: String,
    /// Save an HTTP(S) proxy to config for agents/backends in restricted networks.
    #[arg(long, default_value = "")]
    pub proxy: String,
}

/// A single upstream tool to install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSpec {
    pub id: &'static str,
    /// Binary used to detect presence.
    pub bin: &'static str,
    /// Candidate install commands, tried in order.
    pub install: &'static [&'static [&'static str]],
    pub hint: &'static str,
    /// Desktop-only tools are skipped on servers.
    pub desktop_only: bool,
}

const CORE: &[ToolSpec] = &[
    ToolSpec {
        id: "gh",
        bin: "gh",
        install: &[&["gh", "--version"]],
        hint: "GitHub CLI — https://cli.github.com (apt install gh / brew install gh)",
        desktop_only: false,
    },
    ToolSpec {
        id: "node",
        bin: "node",
        install: &[],
        hint: "Node.js (needed for mcporter/opencli) — https://nodejs.org",
        desktop_only: false,
    },
    ToolSpec {
        id: "yt-dlp",
        bin: "yt-dlp",
        install: &[
            &["pipx", "install", "yt-dlp"],
            &["pip", "install", "yt-dlp"],
        ],
        hint: "yt-dlp (YouTube) — pipx install yt-dlp",
        desktop_only: false,
    },
    ToolSpec {
        id: "mcporter",
        bin: "mcporter",
        install: &[&["npm", "install", "-g", "mcporter"]],
        hint: "mcporter (Exa search) — npm install -g mcporter",
        desktop_only: false,
    },
];

fn channel_tool(channel: &str) -> Option<ToolSpec> {
    match channel {
        "twitter" => Some(ToolSpec {
            id: "twitter-cli",
            bin: "twitter",
            install: &[
                &["pipx", "install", "twitter-cli"],
                &["uv", "tool", "install", "twitter-cli"],
            ],
            hint: "twitter-cli — pipx install twitter-cli",
            desktop_only: false,
        }),
        "bilibili" => Some(ToolSpec {
            id: "bili-cli",
            bin: "bili",
            install: &[&["pipx", "install", "bilibili-cli"]],
            hint: "bili-cli — pipx install bilibili-cli",
            desktop_only: false,
        }),
        "reddit" => Some(ToolSpec {
            id: "rdt-cli",
            bin: "rdt",
            install: &[&["pipx", "install", "rdt-cli"]],
            hint: "rdt-cli — pipx install rdt-cli (or use OpenCLI)",
            desktop_only: false,
        }),
        "opencli" | "xiaohongshu" | "linkedin" => Some(ToolSpec {
            id: "opencli",
            bin: "opencli",
            install: &[&["npm", "install", "-g", "@jackwener/opencli"]],
            hint: "OpenCLI (browser session, desktop only) — npm install -g @jackwener/opencli",
            desktop_only: true,
        }),
        _ => None,
    }
}

/// Build the ordered list of tools to install for an environment + channel set.
///
/// Pure and deterministic so it can be unit-tested without touching the system.
pub fn build_plan(env: Environment, channels: &[String]) -> Vec<ToolSpec> {
    let mut plan: Vec<ToolSpec> = CORE.to_vec();

    let requested: Vec<String> = if channels.iter().any(|c| c == "all") {
        vec![
            "twitter".into(),
            "bilibili".into(),
            "reddit".into(),
            "opencli".into(),
        ]
    } else {
        channels.to_vec()
    };

    for ch in &requested {
        if let Some(tool) = channel_tool(ch) {
            if tool.desktop_only && env == Environment::Server {
                continue;
            }
            if !plan.iter().any(|t| t.id == tool.id) {
                plan.push(tool);
            }
        }
    }
    plan
}

/// True if `bin` is found on PATH.
fn on_path(bin: &str) -> bool {
    let path = match std::env::var_os("PATH") {
        Some(p) => p,
        None => return false,
    };
    let exts: &[&str] = if cfg!(windows) {
        &["", ".exe", ".cmd", ".bat"]
    } else {
        &[""]
    };
    std::env::split_paths(&path).any(|dir| {
        exts.iter()
            .any(|ext| dir.join(format!("{bin}{ext}")).is_file())
    })
}

fn parse_channels(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

pub async fn run(args: InstallArgs) -> anyhow::Result<()> {
    let env = match args.env.as_str() {
        "local" => Environment::Local,
        "server" => Environment::Server,
        _ => env_detect::detect(),
    };

    println!("AgentSpan Installer");
    println!("====================");
    println!("Environment: {}", env.label());
    if args.dry_run {
        println!("DRY RUN — no changes will be made");
    }
    if args.safe {
        println!("SAFE MODE — inspect only, no automatic installs");
    }
    println!();

    if !args.proxy.is_empty() {
        if args.dry_run {
            println!("  [plan] would save proxy.url = {}", args.proxy);
        } else if args.safe {
            println!("  [safe] proxy not saved in safe mode: {}", args.proxy);
        } else {
            let mut config = agentspan_core::Config::load().unwrap_or_default();
            crate::commands::config::set_value(&mut config, "proxy.url", &args.proxy)
                .map_err(anyhow::Error::msg)?;
            config.save()?;
            println!("  [ok] saved proxy.url = {}", args.proxy);
        }
        println!();
    }

    let channels = parse_channels(&args.channels);
    let plan = build_plan(env, &channels);

    for tool in &plan {
        let present = on_path(tool.bin);
        if present {
            println!("  [ok] {} already installed", tool.id);
            continue;
        }
        if args.dry_run {
            let how = tool
                .install
                .first()
                .map(|c| c.join(" "))
                .unwrap_or_else(|| tool.hint.to_string());
            println!("  [plan] would install {} via: {}", tool.id, how);
        } else if args.safe {
            println!("  [missing] {} — {}", tool.id, tool.hint);
        } else {
            install_tool(tool).await;
        }
    }

    println!();
    if args.dry_run {
        println!("Dry run complete. Re-run without --dry-run to apply.");
    } else if args.safe {
        println!("Safe-mode check complete. Install the listed tools manually.");
    } else {
        println!("Install complete. Run `agentspan doctor` to verify channels.");
        println!("Tip: import login cookies with `agentspan config from-browser chrome`.");
    }
    Ok(())
}

async fn install_tool(tool: &ToolSpec) {
    for cmd in tool.install {
        if cmd.is_empty() {
            continue;
        }
        let bin: &str = cmd[0];
        let rest: &[&str] = &cmd[1..];
        // Skip a candidate whose package manager isn't available.
        if !on_path(bin) {
            continue;
        }
        println!("  Installing {} via `{}`...", tool.id, cmd.join(" "));
        let status = tokio::process::Command::new(bin).args(rest).status().await;
        if matches!(status, Ok(s) if s.success()) && on_path(tool.bin) {
            println!("  [ok] {} installed", tool.id);
            return;
        }
    }
    println!("  [!] could not auto-install {} — {}", tool.id, tool.hint);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(plan: &[ToolSpec]) -> Vec<&'static str> {
        plan.iter().map(|t| t.id).collect()
    }

    #[test]
    fn core_tools_always_present() {
        let plan = build_plan(Environment::Local, &[]);
        let ids = ids(&plan);
        assert!(ids.contains(&"gh"));
        assert!(ids.contains(&"mcporter"));
        assert!(ids.contains(&"yt-dlp"));
    }

    #[test]
    fn channel_tools_are_added() {
        let plan = build_plan(Environment::Local, &["twitter".into(), "bilibili".into()]);
        let ids = ids(&plan);
        assert!(ids.contains(&"twitter-cli"));
        assert!(ids.contains(&"bili-cli"));
    }

    #[test]
    fn opencli_skipped_on_server() {
        let local = build_plan(Environment::Local, &["xiaohongshu".into()]);
        assert!(ids(&local).contains(&"opencli"));
        let server = build_plan(Environment::Server, &["xiaohongshu".into()]);
        assert!(!ids(&server).contains(&"opencli"));
    }

    #[test]
    fn all_expands_to_every_channel() {
        let plan = build_plan(Environment::Local, &["all".into()]);
        let ids = ids(&plan);
        assert!(ids.contains(&"twitter-cli"));
        assert!(ids.contains(&"bili-cli"));
        assert!(ids.contains(&"rdt-cli"));
        assert!(ids.contains(&"opencli"));
    }

    #[test]
    fn no_duplicate_tools() {
        // reddit + opencli both could pull opencli-like tools; ensure dedupe by id.
        let plan = build_plan(
            Environment::Local,
            &["xiaohongshu".into(), "linkedin".into()],
        );
        let count = plan.iter().filter(|t| t.id == "opencli").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn parse_channels_handles_spaces_and_case() {
        assert_eq!(
            parse_channels(" Twitter, reddit ,, "),
            vec!["twitter".to_string(), "reddit".to_string()]
        );
    }
}
