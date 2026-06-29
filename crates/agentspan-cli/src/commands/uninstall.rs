//! `agentspan uninstall` — remove installed skills and (optionally) config.

use std::path::{Path, PathBuf};

use clap::Args;

#[derive(Args)]
pub struct UninstallArgs {
    /// Show what would be removed without making changes.
    #[arg(long)]
    pub dry_run: bool,
    /// Keep `~/.agentspan/` config and tokens; remove only skills.
    #[arg(long)]
    pub keep_config: bool,
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Compute the directories that would be removed.
pub fn removal_targets(home: &Path, keep_config: bool) -> Vec<PathBuf> {
    let mut targets = vec![
        home.join(".claude").join("skills").join("agentspan"),
        home.join(".cursor").join("skills").join("agentspan"),
        home.join(".agents").join("skills").join("agentspan"),
    ];
    if !keep_config {
        targets.push(home.join(".agentspan"));
    }
    targets
}

pub async fn run(args: UninstallArgs) -> anyhow::Result<()> {
    let Some(home) = home_dir() else {
        println!("No home directory detected; nothing to remove.");
        return Ok(());
    };

    println!("AgentSpan Uninstaller");
    println!("=====================");
    if args.dry_run {
        println!("DRY RUN — no changes will be made");
    }
    if args.keep_config {
        println!("Keeping ~/.agentspan/ config (--keep-config)");
    }
    println!();

    let mut removed = 0;
    for target in removal_targets(&home, args.keep_config) {
        if !target.exists() {
            continue;
        }
        if args.dry_run {
            println!("  [dry-run] would remove {}", target.display());
            removed += 1;
        } else {
            match std::fs::remove_dir_all(&target) {
                Ok(()) => {
                    println!("  removed {}", target.display());
                    removed += 1;
                }
                Err(e) => eprintln!("  could not remove {}: {e}", target.display()),
            }
        }
    }

    println!();
    if removed == 0 {
        println!("Nothing to remove — already clean.");
    } else if args.dry_run {
        println!("Dry run complete. Re-run without --dry-run to apply.");
    } else {
        println!("Removed {removed} item(s).");
    }
    println!("To remove the binary itself: cargo uninstall agentspan-cli");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_include_skills_and_config_by_default() {
        let home = Path::new("/home/u");
        let targets = removal_targets(home, false);
        assert!(targets.iter().any(|t| t.ends_with("agentspan")));
        assert!(targets.iter().any(|t| t.ends_with(".agentspan")));
    }

    #[test]
    fn keep_config_excludes_config_dir() {
        let home = Path::new("/home/u");
        let targets = removal_targets(home, true);
        assert!(!targets.iter().any(|t| t.ends_with(".agentspan")));
        // Skills are still removed.
        assert_eq!(targets.len(), 3);
    }
}
