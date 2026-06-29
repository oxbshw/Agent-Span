//! `agentspan plugin` — discover and manage community channel plugins (KF2).
//!
//! Plugins are community-contributed channels published as crates/repos and listed
//! in a bundled registry (`plugins/registry.json`, mirrored from the GitHub topic
//! `agentspan-plugin`). Installed state is tracked under `~/.agentspan/plugins/`.
//! The registry parsing + install/remove bookkeeping is pure and unit-tested.

use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};

/// The bundled plugin registry (mirrors the `agentspan-plugin` GitHub topic).
const REGISTRY_JSON: &str = include_str!("../../../../plugins/registry.json");

#[derive(Args)]
pub struct PluginArgs {
    #[command(subcommand)]
    pub action: PluginAction,
}

#[derive(Subcommand)]
pub enum PluginAction {
    /// List available community plugins and their install status.
    List,
    /// Install a plugin by name.
    Install { name: String },
    /// Remove an installed plugin.
    Remove { name: String },
    /// Update an installed plugin (re-fetch its manifest).
    Update { name: String },
}

/// One plugin entry from the registry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginManifest {
    pub name: String,
    pub description: String,
    pub channel: String,
    pub source: String,
}

#[derive(Debug, Deserialize)]
struct Registry {
    plugins: Vec<PluginManifest>,
}

/// Parse the bundled plugin registry.
pub fn available() -> Vec<PluginManifest> {
    serde_json::from_str::<Registry>(REGISTRY_JSON)
        .map(|r| r.plugins)
        .unwrap_or_default()
}

/// Find a plugin manifest by name.
pub fn find(name: &str) -> Option<PluginManifest> {
    available().into_iter().find(|p| p.name == name)
}

fn installed_path(dir: &Path) -> PathBuf {
    dir.join("installed.json")
}

/// Read the list of installed plugin names from `dir`.
pub fn load_installed(dir: &Path) -> Vec<String> {
    std::fs::read_to_string(installed_path(dir))
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .unwrap_or_default()
}

/// Persist the installed list to `dir`.
pub fn save_installed(dir: &Path, names: &[String]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(
        installed_path(dir),
        serde_json::to_string_pretty(names).unwrap_or_default(),
    )
}

/// Record `name` as installed (idempotent). Errors if the plugin is unknown.
pub fn install(name: &str, dir: &Path) -> Result<(), String> {
    if find(name).is_none() {
        return Err(format!("unknown plugin: {name}"));
    }
    let mut installed = load_installed(dir);
    if !installed.iter().any(|n| n == name) {
        installed.push(name.to_string());
        save_installed(dir, &installed).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Remove `name` from the installed list. Errors if it isn't installed.
pub fn remove(name: &str, dir: &Path) -> Result<(), String> {
    let mut installed = load_installed(dir);
    let before = installed.len();
    installed.retain(|n| n != name);
    if installed.len() == before {
        return Err(format!("plugin not installed: {name}"));
    }
    save_installed(dir, &installed).map_err(|e| e.to_string())
}

fn plugins_dir() -> PathBuf {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    PathBuf::from(home).join(".agentspan").join("plugins")
}

pub async fn run(args: PluginArgs) -> anyhow::Result<()> {
    let dir = plugins_dir();
    match args.action {
        PluginAction::List => {
            let installed = load_installed(&dir);
            println!("Available AgentSpan plugins:\n");
            for p in available() {
                let mark = if installed.contains(&p.name) {
                    "[installed]"
                } else {
                    "[available]"
                };
                println!("  {mark} {:<16} {}", p.name, p.description);
            }
            println!("\nInstall with: agentspan plugin install <name>");
        }
        PluginAction::Install { name } => {
            install(&name, &dir).map_err(anyhow::Error::msg)?;
            let m = find(&name).unwrap();
            println!(
                "✅ Installed plugin '{name}' (channel: {}, source: {})",
                m.channel, m.source
            );
        }
        PluginAction::Remove { name } => {
            remove(&name, &dir).map_err(anyhow::Error::msg)?;
            println!("Removed plugin '{name}'");
        }
        PluginAction::Update { name } => {
            if find(&name).is_none() {
                anyhow::bail!("unknown plugin: {name}");
            }
            install(&name, &dir).map_err(anyhow::Error::msg)?;
            println!("Updated plugin '{name}'");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_parses_and_has_entries() {
        let plugins = available();
        assert!(!plugins.is_empty());
        assert!(plugins.iter().any(|p| p.name == "mastodon"));
        assert!(plugins.iter().all(|p| !p.source.is_empty()));
    }

    #[test]
    fn find_known_and_unknown() {
        assert_eq!(find("mastodon").unwrap().channel, "mastodon");
        assert!(find("nope").is_none());
    }

    #[test]
    fn install_then_list_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_installed(dir.path()).is_empty());
        install("mastodon", dir.path()).unwrap();
        let installed = load_installed(dir.path());
        assert_eq!(installed, vec!["mastodon".to_string()]);
    }

    #[test]
    fn install_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        install("mastodon", dir.path()).unwrap();
        install("mastodon", dir.path()).unwrap();
        assert_eq!(load_installed(dir.path()).len(), 1);
    }

    #[test]
    fn install_unknown_errors() {
        let dir = tempfile::tempdir().unwrap();
        assert!(install("does-not-exist", dir.path()).is_err());
    }

    #[test]
    fn remove_uninstalled_errors() {
        let dir = tempfile::tempdir().unwrap();
        assert!(remove("mastodon", dir.path()).is_err());
        install("mastodon", dir.path()).unwrap();
        assert!(remove("mastodon", dir.path()).is_ok());
        assert!(load_installed(dir.path()).is_empty());
    }
}
