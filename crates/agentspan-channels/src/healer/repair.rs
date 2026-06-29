//! Auto-repair CLI tools.
//!
//! When a CLI-based backend is `Missing` or `Broken`, [`RepairManager`] tries to
//! reinstall it with the appropriate package manager:
//!
//! - Python tools: `pip install --force-reinstall <tool>`
//! - Node tools:   `npm install -g <tool>`
//! - Rust tools:   `cargo install <tool>`
//!
//! [`RepairManager::verify`] performs the `which <tool>` / PATH check by probing
//! `<tool> --version`. To avoid spinning on a permanently-broken tool, repairs
//! are capped at [`MAX_ATTEMPTS_PER_HOUR`] per tool.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use tokio::process::Command;
use tracing::{info, warn};

use agentspan_core::types::ProbeStatus;

/// Maximum repair attempts per tool within a rolling hour.
pub const MAX_ATTEMPTS_PER_HOUR: usize = 3;

/// The rolling window over which attempts are counted.
const WINDOW: Duration = Duration::from_secs(3600);

/// Which package manager installs/repairs a tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairKind {
    /// `pip install --force-reinstall <tool>`
    Pip,
    /// `npm install -g <tool>`
    Npm,
    /// `cargo install <tool>`
    Cargo,
}

impl RepairKind {
    /// The install/repair command (argv) for `tool`.
    pub fn install_command(&self, tool: &str) -> Vec<String> {
        match self {
            RepairKind::Pip => vec![
                "pip".into(),
                "install".into(),
                "--force-reinstall".into(),
                tool.into(),
            ],
            RepairKind::Npm => vec!["npm".into(), "install".into(), "-g".into(), tool.into()],
            RepairKind::Cargo => vec!["cargo".into(), "install".into(), tool.into()],
        }
    }
}

/// Best-effort guess of which package manager owns a CLI tool, from its name.
///
/// This is a heuristic for the auto-repair path; the manual repair endpoint can
/// always pass an explicit [`RepairKind`].
pub fn infer_kind(tool: &str) -> RepairKind {
    let t = tool.to_lowercase();
    if t.contains("yt-dlp") || t.contains("whisper") || t.contains("python") || t.contains("pip") {
        RepairKind::Pip
    } else if t.ends_with("-cli")
        || t.contains("npm")
        || t.contains("node")
        || t.starts_with("opencli")
    {
        RepairKind::Npm
    } else {
        // Most remaining backends here are Python-based; default to pip.
        RepairKind::Pip
    }
}

/// The outcome of one repair attempt.
#[derive(Debug, Clone)]
pub struct RepairAttempt {
    /// Tool that was repaired.
    pub tool: String,
    /// Package manager used.
    pub kind: RepairKind,
    /// Whether the install command succeeded.
    pub success: bool,
    /// Whether the attempt was skipped because of the hourly cap.
    pub rate_limited: bool,
    /// Human-readable description of what happened.
    pub message: String,
    /// When the attempt was made.
    pub attempted_at: Instant,
}

/// Reinstalls broken/missing CLI tools, rate-limited per tool. Cheap to clone.
#[derive(Debug, Clone, Default)]
pub struct RepairManager {
    /// tool -> timestamps of recent attempts (pruned to the rolling window).
    attempts: Arc<DashMap<String, Vec<Instant>>>,
    /// Total attempts made (for the healing report).
    attempted: Arc<AtomicUsize>,
    /// Total successful repairs (for the healing report).
    succeeded: Arc<AtomicUsize>,
}

impl RepairManager {
    /// Create an empty repair manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Total repair attempts made across all tools.
    pub fn total_attempted(&self) -> usize {
        self.attempted.load(Ordering::Relaxed)
    }

    /// Total successful repairs across all tools.
    pub fn total_succeeded(&self) -> usize {
        self.succeeded.load(Ordering::Relaxed)
    }

    /// Whether another repair of `tool` is allowed under the hourly cap.
    pub fn can_attempt(&self, tool: &str) -> bool {
        self.recent_attempts(tool) < MAX_ATTEMPTS_PER_HOUR
    }

    fn recent_attempts(&self, tool: &str) -> usize {
        match self.attempts.get(tool) {
            Some(times) => times.iter().filter(|t| t.elapsed() < WINDOW).count(),
            None => 0,
        }
    }

    fn record_attempt(&self, tool: &str) {
        let mut entry = self.attempts.entry(tool.to_string()).or_default();
        entry.retain(|t| t.elapsed() < WINDOW);
        entry.push(Instant::now());
    }

    /// Attempt to repair `tool` using `kind`, executing the install command with
    /// `run`. Injecting the runner keeps the rate-limit and bookkeeping logic
    /// testable without invoking a real package manager.
    pub async fn attempt_with<F, Fut>(&self, tool: &str, kind: RepairKind, run: F) -> RepairAttempt
    where
        F: FnOnce(Vec<String>) -> Fut,
        Fut: std::future::Future<Output = bool>,
    {
        if !self.can_attempt(tool) {
            warn!(tool, "repair skipped: hourly attempt cap reached");
            return RepairAttempt {
                tool: tool.to_string(),
                kind,
                success: false,
                rate_limited: true,
                message: format!(
                    "rate-limited: max {MAX_ATTEMPTS_PER_HOUR} repair attempts/hour reached for {tool}"
                ),
                attempted_at: Instant::now(),
            };
        }

        self.record_attempt(tool);
        self.attempted.fetch_add(1, Ordering::Relaxed);

        let cmd = kind.install_command(tool);
        let printable = cmd.join(" ");
        info!(tool, command = %printable, "attempting auto-repair");

        let success = run(cmd).await;
        if success {
            self.succeeded.fetch_add(1, Ordering::Relaxed);
        }

        RepairAttempt {
            tool: tool.to_string(),
            kind,
            success,
            rate_limited: false,
            message: if success {
                format!("repaired via `{printable}`")
            } else {
                format!("repair command failed: `{printable}`")
            },
            attempted_at: Instant::now(),
        }
    }

    /// Attempt to repair `tool`, running the real install command.
    pub async fn repair(&self, tool: &str, kind: RepairKind) -> RepairAttempt {
        self.attempt_with(tool, kind, |argv| async move { run_command(&argv).await })
            .await
    }

    /// Verify `tool` is present and runnable on PATH (the `which <tool>` step):
    /// probes `<tool> --version` and treats `Ok`/`Warn` as present.
    pub async fn verify(&self, tool: &str) -> bool {
        let probe =
            agentspan_probe::probe_command([tool, "--version"], Duration::from_secs(5)).await;
        matches!(probe.status, ProbeStatus::Ok | ProbeStatus::Warn)
    }
}

/// Run a command, returning whether it exited successfully.
async fn run_command(argv: &[String]) -> bool {
    if argv.is_empty() {
        return false;
    }
    match Command::new(&argv[0]).args(&argv[1..]).output().await {
        Ok(output) => output.status.success(),
        Err(e) => {
            warn!(error = %e, command = %argv.join(" "), "repair command failed to spawn");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_command_per_kind() {
        assert_eq!(
            RepairKind::Pip.install_command("yt-dlp"),
            vec!["pip", "install", "--force-reinstall", "yt-dlp"]
        );
        assert_eq!(
            RepairKind::Npm.install_command("opencli"),
            vec!["npm", "install", "-g", "opencli"]
        );
        assert_eq!(
            RepairKind::Cargo.install_command("ripgrep"),
            vec!["cargo", "install", "ripgrep"]
        );
    }

    #[test]
    fn infer_kind_maps_known_tools() {
        assert_eq!(infer_kind("yt-dlp"), RepairKind::Pip);
        assert_eq!(infer_kind("openai-whisper"), RepairKind::Pip);
        assert_eq!(infer_kind("twitter-cli"), RepairKind::Npm);
        assert_eq!(infer_kind("opencli"), RepairKind::Npm);
    }

    #[tokio::test]
    async fn successful_attempt_increments_totals() {
        let mgr = RepairManager::new();
        let attempt = mgr
            .attempt_with("yt-dlp", RepairKind::Pip, |_argv| async { true })
            .await;
        assert!(attempt.success);
        assert!(!attempt.rate_limited);
        assert!(attempt.message.contains("repaired via"));
        assert_eq!(mgr.total_attempted(), 1);
        assert_eq!(mgr.total_succeeded(), 1);
    }

    #[tokio::test]
    async fn failed_attempt_counts_attempt_not_success() {
        let mgr = RepairManager::new();
        let attempt = mgr
            .attempt_with("yt-dlp", RepairKind::Pip, |_argv| async { false })
            .await;
        assert!(!attempt.success);
        assert_eq!(mgr.total_attempted(), 1);
        assert_eq!(mgr.total_succeeded(), 0);
    }

    #[tokio::test]
    async fn rate_limit_blocks_after_three_attempts_per_hour() {
        let mgr = RepairManager::new();
        for _ in 0..MAX_ATTEMPTS_PER_HOUR {
            let a = mgr
                .attempt_with("flaky", RepairKind::Pip, |_argv| async { false })
                .await;
            assert!(!a.rate_limited);
        }
        assert!(!mgr.can_attempt("flaky"));
        let blocked = mgr
            .attempt_with("flaky", RepairKind::Pip, |_argv| async { true })
            .await;
        assert!(blocked.rate_limited);
        assert!(!blocked.success);
        // The blocked attempt must not run the command or bump the counters.
        assert_eq!(mgr.total_attempted(), MAX_ATTEMPTS_PER_HOUR);
        assert_eq!(mgr.total_succeeded(), 0);
    }

    #[tokio::test]
    async fn verify_returns_false_for_missing_tool() {
        let mgr = RepairManager::new();
        assert!(!mgr.verify("definitely_not_a_real_binary_98765").await);
    }

    #[tokio::test]
    async fn rate_limit_is_per_tool() {
        let mgr = RepairManager::new();
        for _ in 0..MAX_ATTEMPTS_PER_HOUR {
            mgr.attempt_with("a", RepairKind::Pip, |_argv| async { false })
                .await;
        }
        assert!(!mgr.can_attempt("a"));
        // A different tool still has its full budget.
        assert!(mgr.can_attempt("b"));
    }
}
