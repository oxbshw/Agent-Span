//! `agentspan tunnel` — expose the local API on a public URL (KF1).
//!
//! Prefers `cloudflared` (most reliable, free), falling back to `localtunnel`
//! (`lt`) or `npx localtunnel`. Useful for testing webhooks, sharing with a team,
//! or mobile development against a local `agentspan serve`.

use clap::Args;

#[derive(Args)]
pub struct TunnelArgs {
    /// Local port the AgentSpan API is listening on.
    #[arg(long, default_value_t = 8080)]
    pub port: u16,
}

/// A tunnel provider and how to invoke it for a given port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelBackend {
    pub name: &'static str,
    /// Binary used to detect availability and to run.
    pub bin: &'static str,
    pub hint: &'static str,
}

/// Candidate providers, in preference order.
const BACKENDS: &[TunnelBackend] = &[
    TunnelBackend {
        name: "cloudflared",
        bin: "cloudflared",
        hint: "Install cloudflared: https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/",
    },
    TunnelBackend {
        name: "localtunnel",
        bin: "lt",
        hint: "Install localtunnel: npm install -g localtunnel",
    },
];

/// Build the argv for a backend exposing `port`.
pub fn command_for(backend: &TunnelBackend, port: u16) -> Vec<String> {
    let url = format!("http://localhost:{port}");
    match backend.name {
        "cloudflared" => vec!["tunnel".into(), "--url".into(), url],
        "localtunnel" => vec!["--port".into(), port.to_string()],
        _ => vec![],
    }
}

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

/// First installed backend, if any.
pub fn detect_backend() -> Option<&'static TunnelBackend> {
    BACKENDS.iter().find(|b| on_path(b.bin))
}

pub async fn run(args: TunnelArgs) -> anyhow::Result<()> {
    println!("AgentSpan Tunnel");
    println!("================");
    let Some(backend) = detect_backend() else {
        println!("No tunnel provider found on PATH. Install one of:");
        for b in BACKENDS {
            println!("  • {} — {}", b.name, b.hint);
        }
        return Ok(());
    };

    let cmd = command_for(backend, args.port);
    println!(
        "Exposing http://localhost:{} via {} (Ctrl+C to stop)...\n",
        args.port, backend.name
    );

    // Stream the provider's output so the user sees the public URL it prints.
    let status = tokio::process::Command::new(backend.bin)
        .args(&cmd)
        .status()
        .await?;
    if !status.success() {
        anyhow::bail!("{} exited with status {status}", backend.name);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloudflared_command_uses_url_flag() {
        let backend = &BACKENDS[0];
        assert_eq!(backend.name, "cloudflared");
        let cmd = command_for(backend, 9000);
        assert_eq!(cmd, vec!["tunnel", "--url", "http://localhost:9000"]);
    }

    #[test]
    fn localtunnel_command_uses_port_flag() {
        let backend = &BACKENDS[1];
        let cmd = command_for(backend, 8080);
        assert_eq!(cmd, vec!["--port", "8080"]);
    }

    #[test]
    fn backends_are_ordered_cloudflared_first() {
        let names: Vec<_> = BACKENDS.iter().map(|b| b.name).collect();
        assert_eq!(names, vec!["cloudflared", "localtunnel"]);
    }
}
