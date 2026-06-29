//! `agentspan config` — show/get/set settings and import cookies.

use std::path::{Path, PathBuf};

use agentspan_core::Config;
use clap::{Args, Subcommand};

use crate::cookies;
use crate::style;

#[derive(Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub action: Option<ConfigAction>,
    /// Import cookies directly from a browser (chrome/firefox/edge/brave/opera).
    #[arg(long, value_name = "BROWSER")]
    pub from_browser: Option<String>,
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Print the current configuration (secrets masked).
    Show,
    /// Get a dotted config key, e.g. `server.port`.
    Get { key: String },
    /// Set a dotted config key with validation.
    Set { key: String, value: String },
    /// Import cookies from a Cookie-Editor JSON export or a header string.
    Cookies { data: String },
    /// Extract cookies directly from a browser.
    FromBrowser { browser: String },
    /// Back up the current configuration to a file.
    Backup {
        /// Destination path (defaults to a timestamped file in the config dir).
        #[arg(value_name = "PATH")]
        path: Option<PathBuf>,
    },
    /// Restore configuration from a backup file (validated before saving).
    Restore {
        /// Backup file to restore from.
        #[arg(value_name = "PATH")]
        path: PathBuf,
    },
}

/// Whether a config key holds a secret whose value must be masked.
fn is_secret(key: &str) -> bool {
    let k = key.to_lowercase();
    key.starts_with("api_keys.")
        || key.starts_with("cookies.")
        || k.contains("token")
        || k.contains("secret")
        || k.contains("password")
}

fn add_row(rows: &mut Vec<Vec<String>>, key: &str, value: String) {
    let shown = if is_secret(key) {
        style::mask_secret(&value)
    } else {
        value
    };
    rows.push(vec![key.to_string(), shown]);
}

/// Print the configuration as a `Key | Value` table. Secret values (API keys,
/// cookies, tokens) are masked — never shown in full.
fn print_config(config: &Config) {
    let unset = "(unset)".to_string();
    let mut rows: Vec<Vec<String>> = Vec::new();

    add_row(&mut rows, "server.host", config.server.host.clone());
    add_row(&mut rows, "server.port", config.server.port.to_string());
    add_row(
        &mut rows,
        "cache.l1_ttl_seconds",
        config.cache.l1_ttl_seconds.to_string(),
    );
    add_row(
        &mut rows,
        "cache.l2_ttl_seconds",
        config.cache.l2_ttl_seconds.to_string(),
    );
    add_row(
        &mut rows,
        "cache.l3_ttl_seconds",
        config.cache.l3_ttl_seconds.to_string(),
    );
    add_row(
        &mut rows,
        "cache.l3_url",
        config.cache.l3_url.clone().unwrap_or_else(|| unset.clone()),
    );
    add_row(
        &mut rows,
        "probe.timeout_seconds",
        config.probe.timeout_seconds.to_string(),
    );
    add_row(&mut rows, "logging.level", config.logging.level.clone());
    add_row(&mut rows, "logging.json", config.logging.json.to_string());
    add_row(
        &mut rows,
        "auth.require_api_key",
        config.auth.require_api_key.to_string(),
    );
    add_row(
        &mut rows,
        "proxy.url",
        config.proxy.url.clone().unwrap_or_else(|| unset.clone()),
    );

    if config.api_keys.is_empty() {
        add_row(&mut rows, "api_keys", "(none)".to_string());
    } else {
        let mut names: Vec<&String> = config.api_keys.keys().collect();
        names.sort();
        for name in names {
            add_row(
                &mut rows,
                &format!("api_keys.{name}"),
                config.api_keys.get(name).cloned().unwrap_or_default(),
            );
        }
    }
    if config.cookies.is_empty() {
        add_row(&mut rows, "cookies", "(none)".to_string());
    } else {
        let mut names: Vec<&String> = config.cookies.keys().collect();
        names.sort();
        for name in names {
            add_row(
                &mut rows,
                &format!("cookies.{name}"),
                config.cookies.get(name).cloned().unwrap_or_default(),
            );
        }
    }

    style::section("Configuration");
    style::print_table(&["Key", "Value"], &rows);
}

/// Read a dotted key from config. Returns None for unknown keys.
pub fn get_value(config: &Config, key: &str) -> Option<String> {
    match key {
        "server.host" => Some(config.server.host.clone()),
        "server.port" => Some(config.server.port.to_string()),
        "logging.level" => Some(config.logging.level.clone()),
        "logging.json" => Some(config.logging.json.to_string()),
        "cache.l1_ttl_seconds" => Some(config.cache.l1_ttl_seconds.to_string()),
        "cache.l2_ttl_seconds" => Some(config.cache.l2_ttl_seconds.to_string()),
        "cache.l3_ttl_seconds" => Some(config.cache.l3_ttl_seconds.to_string()),
        "cache.l3_url" => config.cache.l3_url.clone(),
        "probe.timeout_seconds" => Some(config.probe.timeout_seconds.to_string()),
        "auth.require_api_key" => Some(config.auth.require_api_key.to_string()),
        "proxy.url" => config.proxy.url.clone(),
        _ => {
            if let Some(name) = key.strip_prefix("api_keys.") {
                config.api_keys.get(name).cloned()
            } else if let Some(name) = key.strip_prefix("cookies.") {
                config.cookies.get(name).cloned()
            } else {
                None
            }
        }
    }
}

/// Set a dotted key with type validation. Does not save.
pub fn set_value(config: &mut Config, key: &str, value: &str) -> Result<(), String> {
    fn parse_u64(v: &str) -> Result<u64, String> {
        let n: u64 = v
            .parse()
            .map_err(|_| format!("expected a number, got '{v}'"))?;
        if n == 0 {
            return Err("value must be greater than zero".to_string());
        }
        Ok(n)
    }
    fn parse_bool(v: &str) -> Result<bool, String> {
        match v.to_lowercase().as_str() {
            "true" | "1" | "yes" => Ok(true),
            "false" | "0" | "no" => Ok(false),
            _ => Err(format!("expected true/false, got '{v}'")),
        }
    }

    match key {
        "server.host" => config.server.host = value.to_string(),
        "server.port" => {
            let p: u16 = value
                .parse()
                .map_err(|_| "port must be 1-65535".to_string())?;
            if p == 0 {
                return Err("port must be non-zero".to_string());
            }
            config.server.port = p;
        }
        "logging.level" => {
            if value.is_empty() {
                return Err("logging.level must not be empty".to_string());
            }
            config.logging.level = value.to_string();
        }
        "logging.json" => config.logging.json = parse_bool(value)?,
        "cache.l1_ttl_seconds" => config.cache.l1_ttl_seconds = parse_u64(value)?,
        "cache.l2_ttl_seconds" => config.cache.l2_ttl_seconds = parse_u64(value)?,
        "cache.l3_ttl_seconds" => config.cache.l3_ttl_seconds = parse_u64(value)?,
        "cache.l3_url" => config.cache.l3_url = Some(value.to_string()),
        "probe.timeout_seconds" => config.probe.timeout_seconds = parse_u64(value)?,
        "auth.require_api_key" => config.auth.require_api_key = parse_bool(value)?,
        "proxy.url" => {
            if value.is_empty() {
                return Err("proxy.url must not be empty".to_string());
            }
            config.proxy.url = Some(value.to_string());
        }
        _ => {
            if let Some(name) = key.strip_prefix("api_keys.") {
                config.api_keys.insert(name.to_string(), value.to_string());
            } else if let Some(name) = key.strip_prefix("cookies.") {
                config.cookies.insert(name.to_string(), value.to_string());
            } else {
                return Err(format!("unknown config key: {key}"));
            }
        }
    }
    Ok(())
}

fn import_cookies(data: &str) -> anyhow::Result<()> {
    let cookies = cookies::parse_cookies(data).map_err(anyhow::Error::msg)?;
    let platforms = cookies::extract_platforms(&cookies);
    if platforms.is_empty() {
        println!("No recognized platform cookies found (Twitter/XiaoHongShu/Bilibili/Xueqiu).");
        return Ok(());
    }
    let mut config = Config::load().unwrap_or_default();
    cookies::apply_to_config(&mut config, &platforms).map_err(anyhow::Error::msg)?;
    for p in &platforms {
        println!("  imported {} ({})", p.platform, p.note);
    }
    println!("Saved to ~/.agentspan/config.yaml");
    Ok(())
}

fn from_browser(browser: &str) -> anyhow::Result<()> {
    println!("Extracting cookies from {browser}...");
    println!();
    println!("Live browser-DB extraction requires the optional `browser-cookies` build.");
    println!("The recommended, always-available flow (also what Agent Reach prefers):");
    println!("  1. Install the Cookie-Editor browser extension");
    println!("  2. Open the logged-in site (x.com, xiaohongshu.com, bilibili.com, xueqiu.com)");
    println!("  3. Cookie-Editor → Export → JSON");
    println!("  4. Run:  agentspan config cookies '<paste the JSON or a name=value; ... string>'");
    Ok(())
}

/// Default backup destination: a timestamped file in the config directory.
fn default_backup_path() -> anyhow::Result<PathBuf> {
    let dir = Config::config_dir()
        .ok_or_else(|| anyhow::anyhow!("unable to determine home directory"))?;
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Ok(dir.join(format!("config.backup.{secs}.yaml")))
}

/// Write the effective configuration to a backup file.
fn backup_config(dest: Option<PathBuf>) -> anyhow::Result<()> {
    let config = Config::load().unwrap_or_default();
    let dest = match dest {
        Some(p) => p,
        None => default_backup_path()?,
    };
    config.save_to(&dest)?;
    println!("Backed up configuration to {}", dest.display());
    Ok(())
}

/// Validate a backup file, then save it as the active user configuration.
fn restore_config(src: &Path) -> anyhow::Result<()> {
    if !src.exists() {
        anyhow::bail!("backup file not found: {}", src.display());
    }
    let config = Config::load_from_file(src)?;
    config.save()?;
    let dest = Config::user_config_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "~/.agentspan/config.yaml".to_string());
    println!("Restored configuration from {} to {dest}", src.display());
    Ok(())
}

pub async fn run(args: ConfigArgs) -> anyhow::Result<()> {
    if let Some(browser) = args.from_browser {
        return from_browser(&browser);
    }

    match args.action {
        None | Some(ConfigAction::Show) => {
            let config = Config::load().unwrap_or_default();
            print_config(&config);
        }
        Some(ConfigAction::Get { key }) => {
            let config = Config::load().unwrap_or_default();
            match get_value(&config, &key) {
                Some(v) => println!("{v}"),
                None => println!("(unset or unknown key: {key})"),
            }
        }
        Some(ConfigAction::Set { key, value }) => {
            let mut config = Config::load().unwrap_or_default();
            set_value(&mut config, &key, &value).map_err(anyhow::Error::msg)?;
            config.save()?;
            println!("set {key} = {value}");
        }
        Some(ConfigAction::Cookies { data }) => import_cookies(&data)?,
        Some(ConfigAction::FromBrowser { browser }) => from_browser(&browser)?,
        Some(ConfigAction::Backup { path }) => backup_config(path)?,
        Some(ConfigAction::Restore { path }) => restore_config(&path)?,
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_known_scalar_keys() {
        let config = Config::default();
        assert_eq!(get_value(&config, "server.port").as_deref(), Some("8080"));
        assert_eq!(get_value(&config, "logging.level").as_deref(), Some("info"));
        assert!(get_value(&config, "nope.nope").is_none());
    }

    #[test]
    fn set_validates_port() {
        let mut config = Config::default();
        assert!(set_value(&mut config, "server.port", "9090").is_ok());
        assert_eq!(config.server.port, 9090);
        assert!(set_value(&mut config, "server.port", "notanumber").is_err());
    }

    #[test]
    fn set_validates_ttl_nonzero() {
        let mut config = Config::default();
        assert!(set_value(&mut config, "cache.l1_ttl_seconds", "0").is_err());
        assert!(set_value(&mut config, "cache.l1_ttl_seconds", "30").is_ok());
        assert_eq!(config.cache.l1_ttl_seconds, 30);
    }

    #[test]
    fn set_generic_maps() {
        let mut config = Config::default();
        set_value(&mut config, "api_keys.exa", "k").unwrap();
        set_value(&mut config, "cookies.twitter", "auth_token=a; ct0=c").unwrap();
        assert_eq!(config.api_keys.get("exa").unwrap(), "k");
        assert_eq!(
            get_value(&config, "cookies.twitter").as_deref(),
            Some("auth_token=a; ct0=c")
        );
    }

    #[test]
    fn set_rejects_unknown_key() {
        let mut config = Config::default();
        assert!(set_value(&mut config, "bogus.key", "x").is_err());
    }

    #[test]
    fn set_validates_bool() {
        let mut config = Config::default();
        assert!(set_value(&mut config, "auth.require_api_key", "true").is_ok());
        assert!(config.auth.require_api_key);
        assert!(set_value(&mut config, "auth.require_api_key", "maybe").is_err());
    }

    #[test]
    fn backup_then_restore_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let backup = dir.path().join("config.backup.yaml");

        let mut config = Config::default();
        config.server.port = 9091;
        config.save_to(&backup).unwrap();

        let restored = Config::load_from_file(&backup).unwrap();
        assert_eq!(restored.server.port, 9091);
    }

    #[test]
    fn restore_missing_file_errors() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope.yaml");
        assert!(restore_config(&missing).is_err());
    }

    #[test]
    fn default_backup_path_is_timestamped_yaml() {
        if let Ok(path) = default_backup_path() {
            let name = path.file_name().unwrap().to_string_lossy();
            assert!(name.starts_with("config.backup."), "got {name}");
            assert!(name.ends_with(".yaml"), "got {name}");
        }
    }
}
