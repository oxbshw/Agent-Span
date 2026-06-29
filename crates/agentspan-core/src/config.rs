//! Layered configuration:
//! code overrides > profile YAML > base YAML > user config (`~/.agentspan/config.yaml`) > env > defaults.

use std::collections::HashMap;
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};

use figment::{
    providers::{Env, Format, Serialized, Yaml},
    Figment,
};
use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::types::Tier;

/// Application configuration.
#[derive(Clone, Deserialize, Serialize, PartialEq)]
pub struct Config {
    pub server: ServerConfig,
    pub cache: CacheConfig,
    pub probe: ProbeConfig,
    pub logging: LoggingConfig,
    pub auth: AuthConfig,
    /// Per-provider API keys (e.g., `openai`, `exa`, `jina`).
    #[serde(default)]
    pub api_keys: HashMap<String, String>,
    /// Per-channel cookie strings for authenticated platforms.
    #[serde(default)]
    pub cookies: HashMap<String, String>,
    /// HTTP proxy configuration.
    #[serde(default)]
    pub proxy: ProxyConfig,
    /// Default tier and per-channel overrides.
    #[serde(default)]
    pub tiers: TierSettings,
}

/// Server binding configuration.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

/// Cache tier TTLs and settings.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct CacheConfig {
    pub l1_ttl_seconds: u64,
    pub l2_ttl_seconds: u64,
    pub l3_ttl_seconds: u64,
    pub l2_path: String,
    pub l3_url: Option<String>,
}

/// Probe engine settings.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ProbeConfig {
    pub timeout_seconds: u64,
}

/// Logging configuration.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct LoggingConfig {
    /// Log level filter (e.g., "info", "debug", "warn").
    pub level: String,
    /// If true, emit structured JSON logs suitable for production aggregation.
    pub json: bool,
}

/// Authentication and authorization configuration.
#[derive(Clone, Deserialize, Serialize, PartialEq)]
pub struct AuthConfig {
    /// If true, require a valid API key on protected endpoints.
    pub require_api_key: bool,
    /// Optional comma-separated list of API key hashes (SHA-256) accepted by the server.
    pub api_key_hashes: Option<Vec<String>>,
    /// JWT secret used for token signing when OAuth/token auth is enabled.
    pub jwt_secret: Option<String>,
}

/// HTTP proxy settings.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
pub struct ProxyConfig {
    /// HTTP/HTTPS proxy URL (e.g., `http://127.0.0.1:8080`).
    pub url: Option<String>,
    /// Hosts/domains that bypass the proxy.
    #[serde(default)]
    pub no_proxy: Vec<String>,
}

/// Tier settings with a global default and per-channel overrides.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct TierSettings {
    /// Default tier for channels without an explicit override.
    #[serde(default)]
    pub default: Tier,
    /// Per-channel tier overrides (channel name -> tier).
    #[serde(default)]
    pub overrides: HashMap<String, Tier>,
}

impl Default for TierSettings {
    fn default() -> Self {
        Self {
            default: Tier::Zero,
            overrides: HashMap::new(),
        }
    }
}

impl fmt::Debug for AuthConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthConfig")
            .field("require_api_key", &self.require_api_key)
            .field(
                "api_key_hashes",
                &self
                    .api_key_hashes
                    .as_ref()
                    .map(|hashes| hashes.iter().map(mask_secret).collect::<Vec<_>>()),
            )
            .field("jwt_secret", &self.jwt_secret.as_deref().map(mask_secret))
            .finish()
    }
}

impl fmt::Display for AuthConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl Config {
    /// Return a copy of `api_keys` with all values masked.
    fn masked_api_keys(&self) -> HashMap<&String, String> {
        self.api_keys
            .iter()
            .map(|(k, v)| (k, mask_secret(v)))
            .collect()
    }

    /// Return a copy of `cookies` with all values masked.
    fn masked_cookies(&self) -> HashMap<&String, String> {
        self.cookies
            .iter()
            .map(|(k, v)| (k, mask_secret(v)))
            .collect()
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("server", &self.server)
            .field("cache", &self.cache)
            .field("probe", &self.probe)
            .field("logging", &self.logging)
            .field("auth", &self.auth)
            .field("api_keys", &self.masked_api_keys())
            .field("cookies", &self.masked_cookies())
            .field("proxy", &self.proxy)
            .field("tiers", &self.tiers)
            .finish()
    }
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// Mask a secret value, keeping at most the last four characters.
fn mask_secret(value: impl AsRef<str>) -> String {
    let value = value.as_ref();
    if value.len() <= 4 {
        "***".to_string()
    } else {
        format!("***{}", &value[value.len() - 4..])
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
            },
            cache: CacheConfig {
                l1_ttl_seconds: 60,
                l2_ttl_seconds: 3600,
                l3_ttl_seconds: 86400,
                l2_path: "data/cache.l2".to_string(),
                l3_url: None,
            },
            probe: ProbeConfig {
                timeout_seconds: 10,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                json: false,
            },
            auth: AuthConfig {
                require_api_key: false,
                api_key_hashes: None,
                jwt_secret: None,
            },
            api_keys: HashMap::new(),
            cookies: HashMap::new(),
            proxy: ProxyConfig::default(),
            tiers: TierSettings::default(),
        }
    }
}

impl Config {
    /// Default user configuration directory (`~/.agentspan`).
    pub fn config_dir() -> Option<PathBuf> {
        home_dir().map(|home| home.join(".agentspan"))
    }

    /// Default user configuration file path (`~/.agentspan/config.yaml`).
    pub fn user_config_path() -> Option<PathBuf> {
        Self::config_dir().map(|dir| dir.join("config.yaml"))
    }

    /// Load configuration with the following precedence (highest to lowest):
    /// 1. Code overrides passed to `load_with_overrides` or set after loading.
    /// 2. `agentspan.{profile}.yaml` profile-specific file.
    /// 3. `agentspan.yaml` base file.
    /// 4. `~/.agentspan/config.yaml` user config file.
    /// 5. `AGENTSPAN_` environment variables.
    /// 6. Built-in defaults.
    ///
    /// The configuration also supports per-provider API keys, per-channel
    /// cookies, HTTP proxy settings, and per-channel tier overrides.
    ///
    /// The profile is read from `AGENTSPAN_PROFILE` env var and defaults to "default".
    pub fn load() -> Result<Self, Error> {
        let profile = std::env::var("AGENTSPAN_PROFILE").unwrap_or_else(|_| "default".to_string());
        Self::load_with_profile(&profile)
    }

    /// Load configuration using an explicit profile name.
    pub fn load_with_profile(profile: &str) -> Result<Self, Error> {
        let base = Self::discover_config_file("agentspan.yaml");
        let profile_path = Self::discover_config_file(&format!("agentspan.{}.yaml", profile));
        let user = Self::user_config_path().filter(|p| p.exists());
        Self::load_from_files(base.as_deref(), profile_path.as_deref(), user.as_deref())
    }

    /// Load configuration from an explicit YAML path.
    ///
    /// Returns an error if the file does not exist.
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(Error::Config(format!(
                "config file not found: {}",
                path.display()
            )));
        }
        Self::load_from_files(Some(path), None::<&Path>, None::<&Path>)
    }

    /// Load configuration from optional base, profile, and user YAML paths.
    pub fn load_from_files<Pb: AsRef<Path>, Pp: AsRef<Path>, Pu: AsRef<Path>>(
        base: Option<Pb>,
        profile: Option<Pp>,
        user: Option<Pu>,
    ) -> Result<Self, Error> {
        Self::build_figment(base, profile, user)
            .extract()
            .map_err(|e| Error::Config(e.to_string()))
            .and_then(|config: Config| {
                config.validate()?;
                Ok(config)
            })
    }

    /// Load configuration and apply programmatic overrides on top.
    ///
    /// This is the highest precedence layer: code > YAML > env > defaults.
    pub fn load_with_overrides(overrides: Config) -> Result<Self, Error> {
        let profile = std::env::var("AGENTSPAN_PROFILE").unwrap_or_else(|_| "default".to_string());
        let base = Self::discover_config_file("agentspan.yaml");
        let profile_path = Self::discover_config_file(&format!("agentspan.{}.yaml", profile));
        let user = Self::user_config_path().filter(|p| p.exists());

        let figment =
            Self::build_figment(base.as_deref(), profile_path.as_deref(), user.as_deref())
                .merge(Serialized::defaults(overrides));

        let config: Config = figment
            .extract()
            .map_err(|e| Error::Config(e.to_string()))?;
        config.validate()?;
        Ok(config)
    }

    /// Save this configuration to `~/.agentspan/config.yaml` atomically.
    ///
    /// The parent directory is created if it does not exist. The file is written
    /// to a temporary sibling and renamed into place. On Unix the file is created
    /// with mode `0o600` (owner read/write only).
    pub fn save(&self) -> Result<(), Error> {
        let path = Self::user_config_path()
            .ok_or_else(|| Error::Config("unable to determine home directory".to_string()))?;
        self.save_to(path)
    }

    /// Save this configuration to a specific path atomically.
    pub fn save_to<P: AsRef<Path>>(&self, path: P) -> Result<(), Error> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::Config(format!("failed to create config directory: {e}")))?;
        }

        let yaml = serde_yaml::to_string(self)
            .map_err(|e| Error::Config(format!("failed to serialize config: {e}")))?;

        let tmp = path.with_extension("yaml.tmp");
        {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&tmp)
                .map_err(|e| Error::Config(format!("failed to open temporary config file: {e}")))?;

            file.write_all(yaml.as_bytes())
                .map_err(|e| Error::Config(format!("failed to write config: {e}")))?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = file
                    .metadata()
                    .map_err(|e| Error::Config(format!("failed to read file metadata: {e}")))?
                    .permissions();
                perms.set_mode(0o600);
                std::fs::set_permissions(&tmp, perms)
                    .map_err(|e| Error::Config(format!("failed to set config permissions: {e}")))?;
            }
        }

        std::fs::rename(&tmp, path)
            .map_err(|e| Error::Config(format!("failed to finalize config file: {e}")))?;

        Ok(())
    }

    /// Build the Figment provider chain.
    fn build_figment<Pb: AsRef<Path>, Pp: AsRef<Path>, Pu: AsRef<Path>>(
        base: Option<Pb>,
        profile: Option<Pp>,
        user: Option<Pu>,
    ) -> Figment {
        let mut figment = Figment::new().merge(Serialized::defaults(Self::default()));

        figment = figment.merge(Env::prefixed("AGENTSPAN_").split("__"));

        if let Some(path) = user {
            if path.as_ref().exists() {
                figment = figment.merge(Yaml::file(path.as_ref()));
            }
        }

        if let Some(path) = base {
            if path.as_ref().exists() {
                figment = figment.merge(Yaml::file(path.as_ref()));
            }
        }

        if let Some(path) = profile {
            if path.as_ref().exists() {
                figment = figment.merge(Yaml::file(path.as_ref()));
            }
        }

        figment
    }

    /// Discover a config file in the current directory or a `config/` subdirectory.
    fn discover_config_file(name: &str) -> Option<PathBuf> {
        let cwd = PathBuf::from(name);
        if cwd.exists() {
            return Some(cwd);
        }
        let in_config = PathBuf::from("config").join(name);
        if in_config.exists() {
            return Some(in_config);
        }
        None
    }

    /// Validate the loaded configuration.
    pub fn validate(&self) -> Result<(), Error> {
        if let Some(url) = &self.proxy.url {
            if url.is_empty() {
                return Err(Error::Config("proxy.url must not be empty".to_string()));
            }
        }

        if self.server.port == 0 {
            return Err(Error::Config("server.port must be non-zero".to_string()));
        }
        if self.cache.l1_ttl_seconds == 0 {
            return Err(Error::Config(
                "cache.l1_ttl_seconds must be non-zero".to_string(),
            ));
        }
        if self.cache.l2_ttl_seconds == 0 {
            return Err(Error::Config(
                "cache.l2_ttl_seconds must be non-zero".to_string(),
            ));
        }
        if self.cache.l3_ttl_seconds == 0 {
            return Err(Error::Config(
                "cache.l3_ttl_seconds must be non-zero".to_string(),
            ));
        }
        if self.probe.timeout_seconds == 0 {
            return Err(Error::Config(
                "probe.timeout_seconds must be non-zero".to_string(),
            ));
        }
        if self.logging.level.is_empty() {
            return Err(Error::Config("logging.level must not be empty".to_string()));
        }
        Ok(())
    }
}

/// Resolve the user's home directory from `HOME` (Unix) or `USERPROFILE` (Windows).
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::sync::Mutex;

    use super::*;

    // Serialise tests that mutate process environment variables.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn default_config_is_valid() {
        let config = Config::default();
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.cache.l1_ttl_seconds, 60);
        assert_eq!(config.probe.timeout_seconds, 10);
        assert_eq!(config.logging.level, "info");
        assert!(!config.logging.json);
        assert!(!config.auth.require_api_key);
    }

    #[test]
    fn env_overrides_defaults() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("AGENTSPAN_SERVER__PORT", "7070");
        let config = Config::load_from_files(None::<&Path>, None::<&Path>, None::<&Path>).unwrap();
        std::env::remove_var("AGENTSPAN_SERVER__PORT");

        assert_eq!(config.server.port, 7070);
    }

    #[test]
    fn yaml_overrides_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agentspan.yaml");
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(b"server:\n  host: 0.0.0.0\n  port: 9090\ncache:\n  l1_ttl_seconds: 120\n")
            .unwrap();

        std::env::set_var("AGENTSPAN_SERVER__PORT", "7070");
        let config = Config::load_from_file(&path).unwrap();
        std::env::remove_var("AGENTSPAN_SERVER__PORT");

        // YAML takes precedence over env.
        assert_eq!(config.server.port, 9090);
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.cache.l1_ttl_seconds, 120);
        // Unspecified fields keep defaults (or env values if set).
        assert_eq!(config.cache.l2_ttl_seconds, 3600);
    }

    #[test]
    fn profile_overrides_base() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();

        let base = dir.path().join("agentspan.yaml");
        std::fs::write(&base, b"server:\n  port: 8080\nlogging:\n  level: info\n").unwrap();

        let profile = dir.path().join("agentspan.prod.yaml");
        std::fs::write(&profile, b"server:\n  port: 443\nlogging:\n  level: warn\n").unwrap();

        let config = Config::load_from_files(Some(&base), Some(&profile), None::<&Path>).unwrap();
        assert_eq!(config.server.port, 443);
        assert_eq!(config.logging.level, "warn");
    }

    #[test]
    fn code_overrides_yaml() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agentspan.yaml");
        std::fs::write(&path, b"server:\n  port: 9090\n").unwrap();

        std::env::set_var("AGENTSPAN_PROFILE", "default");
        let overrides = Config {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 7777,
            },
            ..Config::default()
        };
        let config = Config::load_with_overrides(overrides).unwrap();
        std::env::remove_var("AGENTSPAN_PROFILE");

        // Code overrides take precedence over YAML and env.
        assert_eq!(config.server.port, 7777);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");

        let original = Config {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 9090,
            },
            cache: CacheConfig {
                l1_ttl_seconds: 120,
                ..Config::default().cache
            },
            ..Config::default()
        };

        original.save_to(&path).unwrap();
        assert!(path.exists());

        let loaded = Config::load_from_file(&path).unwrap();
        assert_eq!(loaded, original);
    }

    #[test]
    fn save_creates_parent_directory() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("a/b/c/config.yaml");

        Config::default().save_to(&nested).unwrap();
        assert!(nested.exists());
    }

    #[test]
    fn user_config_overrides_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let user = dir.path().join("config.yaml");
        std::fs::write(&user, b"server:\n  port: 6060\n").unwrap();

        std::env::set_var("AGENTSPAN_SERVER__PORT", "7070");
        let config = Config::load_from_files(None::<&Path>, None::<&Path>, Some(&user)).unwrap();
        std::env::remove_var("AGENTSPAN_SERVER__PORT");

        assert_eq!(config.server.port, 6060);
    }

    #[test]
    fn validation_rejects_zero_port() {
        let config = Config {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 0,
            },
            ..Config::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn validation_rejects_zero_ttl() {
        let mut config = Config::default();
        config.cache.l1_ttl_seconds = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn validation_accepts_defaults() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn config_roundtrips_api_keys_and_proxy() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agentspan.yaml");

        let mut config = Config::default();
        config
            .api_keys
            .insert("openai".to_string(), "sk-xxx".to_string());
        config
            .cookies
            .insert("twitter".to_string(), "session=abc".to_string());
        config.proxy.url = Some("http://127.0.0.1:8080".to_string());
        config.proxy.no_proxy = vec!["localhost".to_string()];
        config
            .tiers
            .overrides
            .insert("github".to_string(), Tier::One);

        config.save_to(&path).unwrap();
        let loaded = Config::load_from_file(&path).unwrap();

        assert_eq!(loaded.api_keys.get("openai"), Some(&"sk-xxx".to_string()));
        assert_eq!(
            loaded.cookies.get("twitter"),
            Some(&"session=abc".to_string())
        );
        assert_eq!(loaded.proxy.url, Some("http://127.0.0.1:8080".to_string()));
        assert_eq!(loaded.proxy.no_proxy, vec!["localhost".to_string()]);
        assert_eq!(loaded.tiers.overrides.get("github"), Some(&Tier::One));
    }

    #[test]
    fn config_to_string_masks_secrets() {
        let mut config = Config::default();
        config
            .api_keys
            .insert("openai".to_string(), "sk-supersecretkey".to_string());
        config
            .cookies
            .insert("twitter".to_string(), "session=abc123".to_string());
        config.auth.jwt_secret = Some("jwt-super-secret".to_string());
        config.auth.api_key_hashes = Some(vec!["hash1".to_string(), "hash2".to_string()]);

        let output = config.to_string();

        assert!(!output.contains("sk-supersecretkey"));
        assert!(!output.contains("session=abc123"));
        assert!(!output.contains("jwt-super-secret"));
        assert!(!output.contains("hash1"));
        assert!(!output.contains("hash2"));

        assert!(output.contains("openai"));
        assert!(output.contains("twitter"));
    }

    #[test]
    fn config_debug_masks_secrets() {
        let mut config = Config::default();
        config
            .api_keys
            .insert("exa".to_string(), "exa-12345".to_string());

        let output = format!("{config:?}");
        assert!(!output.contains("exa-12345"));
        assert!(output.contains("exa"));
    }

    #[test]
    fn all_layers_precedence() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();

        let user = dir.path().join("user.yaml");
        std::fs::write(&user, b"logging:\n  level: warn\n").unwrap();

        let base = dir.path().join("agentspan.yaml");
        std::fs::write(&base, b"logging:\n  level: info\n").unwrap();

        let profile = dir.path().join("agentspan.prod.yaml");
        std::fs::write(&profile, b"logging:\n  level: debug\n").unwrap();

        std::env::set_var("AGENTSPAN_LOGGING__LEVEL", "error");

        let overrides = Config {
            logging: LoggingConfig {
                level: "trace".to_string(),
                ..Config::default().logging
            },
            ..Config::default()
        };

        let config = Config::load_from_files(Some(&base), Some(&profile), Some(&user)).unwrap();
        // Without code overrides: profile > base > user > env > defaults.
        assert_eq!(config.logging.level, "debug");

        let config = Config::load_with_overrides(overrides).unwrap();
        // Code overrides sit above everything.
        assert_eq!(config.logging.level, "trace");

        std::env::remove_var("AGENTSPAN_LOGGING__LEVEL");
    }

    #[test]
    fn load_missing_file_fails() {
        let _guard = ENV_LOCK.lock().unwrap();
        let path = std::path::PathBuf::from("/nonexistent/agentspan.yaml");
        assert!(Config::load_from_file(&path).is_err());
    }

    #[test]
    fn load_invalid_yaml_fails() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agentspan.yaml");
        std::fs::write(&path, b"server: port: not: valid").unwrap();
        assert!(Config::load_from_file(&path).is_err());
    }

    #[test]
    fn load_empty_yaml_uses_defaults() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agentspan.yaml");
        std::fs::write(&path, b"").unwrap();
        let config = Config::load_from_file(&path).unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn validation_rejects_empty_proxy_url() {
        let config = Config {
            proxy: ProxyConfig {
                url: Some("".to_string()),
                ..Config::default().proxy
            },
            ..Config::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn validation_rejects_empty_logging_level() {
        let config = Config {
            logging: LoggingConfig {
                level: "".to_string(),
                ..Config::default().logging
            },
            ..Config::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn mask_secret_edge_cases() {
        assert_eq!(mask_secret(""), "***");
        assert_eq!(mask_secret("ab"), "***");
        assert_eq!(mask_secret("abcd"), "***");
        assert_eq!(mask_secret("abcdef"), "***cdef");
        assert_eq!(mask_secret("secret"), "***cret");
    }

    #[test]
    fn save_is_atomic_no_leftover_temp() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");

        Config::default().save_to(&path).unwrap();

        assert!(path.exists());
        assert!(!path.with_extension("yaml.tmp").exists());
    }

    #[cfg(unix)]
    #[test]
    fn save_sets_0o600_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");

        Config::default().save_to(&path).unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
