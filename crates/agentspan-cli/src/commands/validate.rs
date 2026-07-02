//! `agentspan validate` — check configuration before deploying it.
//!
//! Loads either the discovered configuration (the same chain `serve` uses) or
//! an explicit `--file`, runs the semantic checks in [`Config::validate`], and
//! exits non-zero on any problem so scripts and CI can gate on it.

use std::path::{Path, PathBuf};

use agentspan_core::Config;
use clap::Args;

use crate::style;

#[derive(Args)]
pub struct ValidateArgs {
    /// Validate a specific YAML file instead of the discovered configuration.
    #[arg(long, value_name = "PATH")]
    pub file: Option<PathBuf>,
}

/// Key effective values worth echoing back after a successful validation.
#[derive(Debug, PartialEq)]
struct Summary {
    server: String,
    require_api_key: bool,
    cache_ttls: (u64, u64, u64),
    proxy: bool,
}

/// Load + validate, returning either a summary or a one-line problem report.
/// Kept free of printing so tests can assert on it directly.
fn validate(file: Option<&Path>) -> Result<Summary, String> {
    let config = match file {
        Some(path) => Config::load_from_file(path).map_err(|e| e.to_string())?,
        None => Config::load().map_err(|e| e.to_string())?,
    };
    // load_from_file / load already run Config::validate, but be explicit so
    // this command stays correct if loading ever stops validating.
    config.validate().map_err(|e| e.to_string())?;

    Ok(Summary {
        server: format!("{}:{}", config.server.host, config.server.port),
        require_api_key: config.auth.require_api_key,
        cache_ttls: (
            config.cache.l1_ttl_seconds,
            config.cache.l2_ttl_seconds,
            config.cache.l3_ttl_seconds,
        ),
        proxy: config.proxy.url.is_some(),
    })
}

pub async fn run(args: ValidateArgs) -> anyhow::Result<()> {
    style::section("Validate configuration");

    match &args.file {
        Some(path) => style::status_info(&format!("source: {}", path.display())),
        None => {
            style::status_info("source: discovered configuration");
            if let Some(user) = Config::user_config_path() {
                let state = if user.exists() {
                    "found"
                } else {
                    "not present"
                };
                style::status_info(&format!("  user config: {} ({state})", user.display()));
            }
        }
    }

    match validate(args.file.as_deref()) {
        Ok(summary) => {
            style::status_ok("configuration is valid");
            let rows = vec![
                vec!["server".to_string(), summary.server],
                vec![
                    "auth.require_api_key".to_string(),
                    summary.require_api_key.to_string(),
                ],
                vec![
                    "cache TTLs (l1/l2/l3)".to_string(),
                    format!(
                        "{}s / {}s / {}s",
                        summary.cache_ttls.0, summary.cache_ttls.1, summary.cache_ttls.2
                    ),
                ],
                vec![
                    "proxy".to_string(),
                    (if summary.proxy { "set" } else { "none" }).to_string(),
                ],
            ];
            style::print_table(&["setting", "value"], &rows);
            Ok(())
        }
        Err(problem) => {
            style::status_err(&problem);
            anyhow::bail!("configuration is invalid: {problem}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_config(dir: &tempfile::TempDir, name: &str, body: &str) -> PathBuf {
        let path = dir.path().join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(body.as_bytes()).unwrap();
        path
    }

    #[test]
    fn valid_file_passes_with_effective_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_config(
            &dir,
            "agentspan.yaml",
            "server:\n  host: 127.0.0.1\n  port: 9999\n",
        );
        let summary = validate(Some(&path)).unwrap();
        assert_eq!(summary.server, "127.0.0.1:9999");
        assert!(!summary.proxy);
    }

    #[test]
    fn semantic_error_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_config(&dir, "bad-port.yaml", "server:\n  port: 0\n");
        let err = validate(Some(&path)).unwrap_err();
        assert!(err.contains("server.port"), "unexpected error: {err}");
    }

    #[test]
    fn malformed_yaml_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_config(&dir, "broken.yaml", "server: [not: a mapping\n");
        assert!(validate(Some(&path)).is_err());
    }

    #[test]
    fn missing_file_is_reported() {
        let err = validate(Some(Path::new("/definitely/not/here.yaml"))).unwrap_err();
        assert!(err.contains("not found"), "unexpected error: {err}");
    }
}
