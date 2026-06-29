//! `agentspan update` — check crates.io for a newer release (best effort).

use clap::Args;

#[derive(Args)]
pub struct CheckUpdateArgs;

const CURRENT: &str = env!("CARGO_PKG_VERSION");

/// Compare two dotted version strings; true when `latest` is strictly greater.
pub fn is_newer(latest: &str, current: &str) -> bool {
    fn parts(v: &str) -> Vec<u64> {
        v.split(['.', '-', '+'])
            .filter_map(|p| p.parse::<u64>().ok())
            .collect()
    }
    let (l, c) = (parts(latest), parts(current));
    for i in 0..l.len().max(c.len()) {
        let lv = l.get(i).copied().unwrap_or(0);
        let cv = c.get(i).copied().unwrap_or(0);
        if lv != cv {
            return lv > cv;
        }
    }
    false
}

pub async fn run(_args: CheckUpdateArgs) -> anyhow::Result<()> {
    println!("Current version: agentspan {CURRENT}");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .user_agent(format!("agentspan/{CURRENT}"))
        .build()
        .unwrap_or_default();

    match client
        .get("https://crates.io/api/v1/crates/agentspan-cli")
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                if let Some(latest) = json["crate"]["max_version"].as_str() {
                    if is_newer(latest, CURRENT) {
                        println!("Update available: {latest} — run: cargo install agentspan-cli");
                    } else {
                        println!("You're on the latest version.");
                    }
                    return Ok(());
                }
            }
            println!("Could not parse crates.io response; check manually.");
        }
        _ => {
            println!("Could not reach crates.io (offline?). Check manually:");
            println!("  https://crates.io/crates/agentspan-cli");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison() {
        assert!(is_newer("0.4.0", "0.3.0"));
        assert!(is_newer("0.3.1", "0.3.0"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.3.0", "0.3.0"));
        assert!(!is_newer("0.2.0", "0.3.0"));
    }
}
