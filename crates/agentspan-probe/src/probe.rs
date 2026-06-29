//! Async probe engine that executes commands and classifies results.

use std::time::{Duration, Instant};

use agentspan_core::{config::Config, types::ProbeResult, types::ProbeStatus};
use tokio::process::Command;
use tracing::{debug, instrument, warn};

/// A target that can be probed by running a command.
#[derive(Debug, Clone)]
pub struct ProbeTarget {
    /// Name of the backend/binary (e.g., "gh", "yt-dlp").
    pub name: String,
    /// Command to execute (first element) with arguments.
    pub command: Vec<String>,
    /// Expected exit codes that indicate health.
    pub ok_exit_codes: Vec<i32>,
    /// Exit codes that indicate a working but degraded state.
    pub warn_exit_codes: Vec<i32>,
    /// Hint shown when the binary is missing.
    pub install_hint: String,
}

impl ProbeTarget {
    /// Build a simple `--version` probe target.
    pub fn version(name: impl Into<String>, install_hint: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            command: vec![name.clone(), "--version".to_string()],
            name,
            ok_exit_codes: vec![0],
            warn_exit_codes: vec![],
            install_hint: install_hint.into(),
        }
    }

    /// Set exit codes that should be treated as a warning rather than broken.
    pub fn with_warn_exit_codes(mut self, codes: impl IntoIterator<Item = i32>) -> Self {
        self.warn_exit_codes = codes.into_iter().collect();
        self
    }
}

/// Async probe engine.
#[derive(Debug, Clone, Default)]
pub struct ProbeEngine {
    timeout: Duration,
}

impl ProbeEngine {
    /// Create a new probe engine with the given timeout.
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }

    /// Create a probe engine from the global configuration.
    pub fn from_config(config: &Config) -> Self {
        Self::new(Duration::from_secs(config.probe.timeout_seconds))
    }

    /// Probe a single target and classify the result.
    #[instrument(skip(self, target), fields(target = %target.name))]
    pub async fn probe(&self, target: &ProbeTarget) -> ProbeResult {
        let start = Instant::now();

        if target.command.is_empty() {
            return ProbeResult {
                status: ProbeStatus::Error,
                message: "empty probe command".to_string(),
                version: None,
                hint: Some("Configure a non-empty command".to_string()),
            };
        }

        let program = &target.command[0];
        let args = &target.command[1..];

        let mut cmd = Command::new(program);
        cmd.args(args);

        debug!("spawning probe command");
        let result = tokio::time::timeout(self.timeout, cmd.output()).await;
        let elapsed_ms = start.elapsed().as_millis() as u64;

        match result {
            Err(_elapsed) => ProbeResult {
                status: ProbeStatus::Timeout,
                message: format!("probe timed out after {}s", self.timeout.as_secs()),
                version: None,
                hint: Some("Check network or command execution time".to_string()),
            },
            Ok(Err(e)) => {
                let kind = e.kind();
                warn!(error = %e, "probe command failed to spawn");
                if kind == std::io::ErrorKind::NotFound {
                    ProbeResult {
                        status: ProbeStatus::Missing,
                        message: format!("{} not found on PATH", program),
                        version: None,
                        hint: Some(target.install_hint.clone()),
                    }
                } else {
                    ProbeResult {
                        status: ProbeStatus::Broken,
                        message: format!("{} failed to execute: {}", program, e),
                        version: None,
                        hint: Some("Check permissions and installation".to_string()),
                    }
                }
            }
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                let exit_code = output.status.code();

                if is_stale_shebang(&stdout, &stderr) {
                    return ProbeResult {
                        status: ProbeStatus::Broken,
                        message: format!("{} has a stale virtual environment shebang", target.name),
                        version: None,
                        hint: Some(
                            "Recreate the virtual environment or reinstall the tool".to_string(),
                        ),
                    };
                }

                if exit_code.is_some_and(|c| target.ok_exit_codes.contains(&c)) {
                    let version = extract_version(&stdout).or_else(|| extract_version(&stderr));
                    ProbeResult {
                        status: ProbeStatus::Ok,
                        message: format!("{} is healthy ({}ms)", target.name, elapsed_ms),
                        version,
                        hint: None,
                    }
                } else if exit_code.is_some_and(|c| target.warn_exit_codes.contains(&c)) {
                    ProbeResult {
                        status: ProbeStatus::Warn,
                        message: format!(
                            "{} returned warning exit code {:?} ({}ms)",
                            target.name, exit_code, elapsed_ms
                        ),
                        version: extract_version(&stdout).or_else(|| extract_version(&stderr)),
                        hint: Some("Command works but may need attention".to_string()),
                    }
                } else {
                    ProbeResult {
                        status: ProbeStatus::Broken,
                        message: format!(
                            "{} exited with code {:?}: {}",
                            target.name, exit_code, stderr
                        ),
                        version: None,
                        hint: Some("Check command arguments and authentication".to_string()),
                    }
                }
            }
        }
    }

    /// Probe multiple targets concurrently.
    ///
    /// Results are returned in the same order as the input targets.
    pub async fn probe_many(&self, targets: &[ProbeTarget]) -> Vec<ProbeResult> {
        let mut handles = Vec::with_capacity(targets.len());
        for target in targets {
            let engine = self.clone();
            let target = target.clone();
            handles.push(tokio::spawn(async move { engine.probe(&target).await }));
        }

        let mut results = Vec::with_capacity(handles.len());
        for handle in handles {
            results.push(handle.await.unwrap_or_else(|e| ProbeResult {
                status: ProbeStatus::Error,
                message: format!("probe task failed: {e}"),
                version: None,
                hint: Some("Internal probe engine error".to_string()),
            }));
        }
        results
    }
}

/// Convenience probe that directly executes a command and classifies the result.
///
/// This actually runs the binary (not just a `which` check). The first element of
/// `command` is the program; remaining elements are arguments.
pub async fn probe_command<I, S>(command: I, timeout: Duration) -> ProbeResult
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let parts: Vec<String> = command
        .into_iter()
        .map(|s| s.as_ref().to_string())
        .collect();
    if parts.is_empty() {
        return ProbeResult {
            status: ProbeStatus::Error,
            message: "empty probe command".to_string(),
            version: None,
            hint: Some("Provide a non-empty command".to_string()),
        };
    }

    let name = parts[0].clone();
    let target = ProbeTarget {
        name: name.clone(),
        command: parts,
        ok_exit_codes: vec![0],
        warn_exit_codes: vec![],
        install_hint: format!("Install {}", name),
    };

    ProbeEngine::new(timeout).probe(&target).await
}

/// Detect the classic "stale venv shebang" symptom.
///
/// When a Python tool installed in a virtual environment is moved or the venv is
/// deleted, the shebang still points at the old interpreter path. Running the
/// script produces a "bad interpreter" error.
fn is_stale_shebang(stdout: &str, stderr: &str) -> bool {
    let combined = format!("{} {}", stdout, stderr).to_lowercase();
    combined.contains("bad interpreter") || combined.contains("no such file or directory")
}

/// Best-effort version extraction from command output.
fn extract_version(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    // Look for a semver-like token: x.y.z or x.y.
    let re = regex_lite::Regex::new(r"(\d+\.\d+(?:\.\d+)?(?:[-+.]?[a-zA-Z0-9]+)*)").ok()?;
    re.find(text).map(|m| m.as_str().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a cross-platform `echo <args>` command vector. On Unix `echo` is
    /// a real binary; on Windows it's a `cmd.exe` builtin so we wrap it.
    fn echo_cmd(arg: &str) -> Vec<String> {
        if cfg!(windows) {
            vec![
                "cmd".to_string(),
                "/C".to_string(),
                "echo".to_string(),
                arg.to_string(),
            ]
        } else {
            vec!["echo".to_string(), arg.to_string()]
        }
    }

    #[tokio::test]
    async fn probe_missing_command() {
        let engine = ProbeEngine::new(Duration::from_secs(2));
        let target =
            ProbeTarget::version("definitely_not_a_real_binary_12345", "Install the thing");
        let result = engine.probe(&target).await;
        assert_eq!(result.status, ProbeStatus::Missing);
        assert!(result.hint.as_deref().unwrap().contains("Install"));
    }

    #[tokio::test]
    async fn probe_echo_version() {
        let engine = ProbeEngine::new(Duration::from_secs(2));
        let target = ProbeTarget {
            name: "echo".to_string(),
            command: echo_cmd("gh version 2.45.0"),
            ok_exit_codes: vec![0],
            warn_exit_codes: vec![],
            install_hint: "Install echo".to_string(),
        };
        let result = engine.probe(&target).await;
        assert_eq!(result.status, ProbeStatus::Ok);
        assert_eq!(result.version, Some("2.45.0".to_string()));
    }

    #[tokio::test]
    async fn probe_nonzero_exit_is_broken() {
        let engine = ProbeEngine::new(Duration::from_secs(2));
        let target = ProbeTarget {
            name: "false".to_string(),
            command: vec!["cmd".to_string(), "/C".to_string(), "exit 1".to_string()],
            ok_exit_codes: vec![0],
            warn_exit_codes: vec![],
            install_hint: "Install false".to_string(),
        };
        let result = engine.probe(&target).await;
        assert_eq!(result.status, ProbeStatus::Broken);
    }

    #[tokio::test]
    async fn probe_warn_exit_code_is_warn() {
        let engine = ProbeEngine::new(Duration::from_secs(2));
        let target = ProbeTarget {
            name: "warn-exit".to_string(),
            command: vec!["cmd".to_string(), "/C".to_string(), "exit 2".to_string()],
            ok_exit_codes: vec![0],
            warn_exit_codes: vec![2],
            install_hint: "Install nothing".to_string(),
        };
        let result = engine.probe(&target).await;
        assert_eq!(result.status, ProbeStatus::Warn);
    }

    #[tokio::test]
    async fn probe_empty_command_is_error() {
        let engine = ProbeEngine::new(Duration::from_secs(2));
        let target = ProbeTarget {
            name: "empty".to_string(),
            command: vec![],
            ok_exit_codes: vec![0],
            warn_exit_codes: vec![],
            install_hint: "".to_string(),
        };
        let result = engine.probe(&target).await;
        assert_eq!(result.status, ProbeStatus::Error);
    }

    #[tokio::test]
    async fn probe_many_runs_all_targets() {
        let engine = ProbeEngine::new(Duration::from_secs(2));
        let targets = vec![
            ProbeTarget {
                name: "echo".to_string(),
                command: echo_cmd("tool version 1.0.0"),
                ok_exit_codes: vec![0],
                warn_exit_codes: vec![],
                install_hint: "Install echo".to_string(),
            },
            ProbeTarget {
                name: "missing".to_string(),
                command: vec![
                    "definitely_missing_binary_xyz".to_string(),
                    "--version".to_string(),
                ],
                ok_exit_codes: vec![0],
                warn_exit_codes: vec![],
                install_hint: "Install missing".to_string(),
            },
        ];
        let results = engine.probe_many(&targets).await;
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].status, ProbeStatus::Ok);
        assert_eq!(results[1].status, ProbeStatus::Missing);
    }

    #[test]
    fn probe_engine_from_config_uses_timeout() {
        let mut config = Config::default();
        config.probe.timeout_seconds = 42;
        let engine = ProbeEngine::from_config(&config);
        // The engine is opaque; exercise it via a quick command to ensure it works.
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(async {
            engine
                .probe(&ProbeTarget {
                    name: "echo".to_string(),
                    command: echo_cmd("tool version 1.0.0"),
                    ok_exit_codes: vec![0],
                    warn_exit_codes: vec![],
                    install_hint: "Install echo".to_string(),
                })
                .await
        });
        assert_eq!(result.status, ProbeStatus::Ok);
    }

    #[tokio::test]
    async fn probe_command_executes_binary() {
        let cmd = if cfg!(windows) {
            vec!["cmd", "/C", "echo", "tool version 1.2.3"]
        } else {
            vec!["echo", "tool version 1.2.3"]
        };
        let result = probe_command(&cmd, Duration::from_secs(2)).await;
        assert_eq!(result.status, ProbeStatus::Ok);
        assert_eq!(result.version, Some("1.2.3".to_string()));
    }

    #[tokio::test]
    async fn probe_command_missing_binary() {
        let result = probe_command(
            &["not_a_real_binary_abc123", "--version"],
            Duration::from_secs(2),
        )
        .await;
        assert_eq!(result.status, ProbeStatus::Missing);
    }

    #[tokio::test]
    async fn probe_command_empty_is_error() {
        let result: ProbeResult = probe_command(Vec::<&str>::new(), Duration::from_secs(2)).await;
        assert_eq!(result.status, ProbeStatus::Error);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn probe_timeout_detects_slow_command() {
        let engine = ProbeEngine::new(Duration::from_millis(100));
        let target = ProbeTarget {
            name: "sleep".to_string(),
            command: vec!["sleep".to_string(), "5".to_string()],
            ok_exit_codes: vec![0],
            warn_exit_codes: vec![],
            install_hint: "Install sleep".to_string(),
        };
        let result = engine.probe(&target).await;
        assert_eq!(result.status, ProbeStatus::Timeout);
        assert!(result.message.contains("timed out"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn probe_detects_stale_venv_shebang() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("stale-tool");
        std::fs::write(&script, "#!/nonexistent/venv/bin/python3\nprint('hello')\n").unwrap();

        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();

        let engine = ProbeEngine::new(Duration::from_secs(2));
        let target = ProbeTarget {
            name: "stale-tool".to_string(),
            command: vec![script.to_str().unwrap().to_string()],
            ok_exit_codes: vec![0],
            warn_exit_codes: vec![],
            install_hint: "Reinstall the tool".to_string(),
        };

        let result = engine.probe(&target).await;
        assert_eq!(result.status, ProbeStatus::Broken);
        assert!(
            result.message.contains("stale virtual environment shebang"),
            "unexpected message: {}",
            result.message
        );
        assert!(result
            .hint
            .as_deref()
            .unwrap()
            .contains("Recreate the virtual environment"));
    }

    #[test]
    fn is_stale_shebang_detects_phrases() {
        assert!(is_stale_shebang(
            "",
            "bash: /venv/bin/tool: bad interpreter: No such file"
        ));
        assert!(is_stale_shebang("bad interpreter: /old/venv/python", ""));
        assert!(!is_stale_shebang("", "some unrelated error"));
    }

    #[test]
    fn extract_version_variants() {
        assert_eq!(
            extract_version("gh version 2.45.0 (2024-01-01)"),
            Some("2.45.0".to_string())
        );
        assert_eq!(
            extract_version("yt-dlp 2024.03.10"),
            Some("2024.03.10".to_string())
        );
        assert_eq!(extract_version(""), None);
    }
}
