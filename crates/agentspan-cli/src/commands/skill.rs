//! `agentspan skill install|uninstall` — generate and register the agent skill.

use std::path::{Path, PathBuf};

use agentspan_channels::ChannelRegistry;
use clap::{Args, Subcommand};

#[derive(Args)]
pub struct SkillArgs {
    #[command(subcommand)]
    pub action: SkillAction,
}

#[derive(Subcommand)]
pub enum SkillAction {
    /// Generate SKILL.md and install it into agent skill directories.
    Install,
    /// Remove the installed AgentSpan skill from all agent directories.
    Uninstall,
    /// Print the generated SKILL.md to stdout.
    Show,
}

/// Generate the SKILL.md content from the live channel registry.
pub fn generate_skill_md(registry: &ChannelRegistry) -> String {
    let mut s = String::new();
    s.push_str("---\n");
    s.push_str("name: agentspan\n");
    s.push_str("description: Read and search the internet (web, GitHub, YouTube, Reddit, Twitter, Bilibili, Hacker News, V2EX, Exa search, RSS, and more) via the AgentSpan gateway or CLI.\n");
    s.push_str("---\n\n");
    s.push_str("# AgentSpan — Web Access for Agents\n\n");
    s.push_str(
        "AgentSpan gives you eyes on the internet: read any URL as clean text and \
         search across many platforms through one gateway. Use it whenever the user \
         asks you to look something up online, summarize a link, research a topic, \
         read a video/post/repo, or check what people are saying somewhere.\n\n",
    );

    s.push_str("## Trigger keywords\n\n");
    s.push_str(
        "read this link, summarize this page, what does this video say, search \
         Twitter/Reddit/Hacker News, look up on the web, research, find the repo, \
         read this thread, latest discussion, RSS feed, stock quote.\n\n",
    );

    s.push_str("## Channels\n\n");
    s.push_str("| Channel | Tier | What it does |\n");
    s.push_str("|---------|------|--------------|\n");
    for ch in registry.list() {
        let tier = format!("{:?}", ch.tier());
        s.push_str(&format!(
            "| `{}` | {} | {} |\n",
            ch.name(),
            tier,
            ch.description()
        ));
    }
    s.push('\n');

    s.push_str("## How to call it\n\n");
    s.push_str("### Via the running gateway (REST)\n\n");
    s.push_str("```bash\n");
    s.push_str("# Smart read — auto-detect the channel from the URL\n");
    s.push_str("curl -s -X POST localhost:8080/api/v1/read \\\n");
    s.push_str("  -H 'content-type: application/json' \\\n");
    s.push_str("  -d '{\"url\":\"https://news.ycombinator.com/item?id=1\"}'\n\n");
    s.push_str("# Search a specific channel\n");
    s.push_str("curl -s 'localhost:8080/api/v1/channels/exa/search?q=rust+async&limit=5'\n");
    s.push_str("```\n\n");

    s.push_str("### Via the CLI\n\n");
    s.push_str("```bash\n");
    s.push_str("agentspan doctor              # which channels are healthy\n");
    s.push_str("agentspan serve --port 8080  # start the gateway\n");
    s.push_str("```\n\n");

    s.push_str("## Boundaries\n\n");
    s.push_str(
        "- Login-gated platforms (Reddit, XiaoHongShu, Twitter) work best with the \
         OpenCLI browser-session backend or imported cookies (`agentspan config \
         from-browser chrome`).\n",
    );
    s.push_str("- Exa is search-only; the `web` channel is read-only.\n");
    s.push_str("- Run `agentspan doctor` to see the active backend per channel.\n");

    s
}

/// Skill directories to install into: `~/.claude/skills`, `~/.cursor/skills`,
/// and `~/.agents/skills`. Parent directories are created on install if missing.
fn skill_dirs() -> Vec<(PathBuf, &'static str)> {
    let mut dirs = Vec::new();
    if let Some(home) = home_dir() {
        dirs.push((home.join(".claude").join("skills"), "Claude Code"));
        dirs.push((home.join(".cursor").join("skills"), "Cursor"));
        dirs.push((home.join(".agents").join("skills"), "Agents"));
    }
    dirs
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Write the skill markdown to `<skill_root>/agentspan.md`, creating the
/// directory (and any missing parents) first.
pub fn install_skill_to(skill_root: &Path, content: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(skill_root)?;
    let file = skill_root.join("agentspan.md");
    std::fs::write(&file, content)?;
    Ok(file)
}

pub async fn run(args: SkillArgs) -> anyhow::Result<()> {
    let registry = ChannelRegistry::default_channels();
    match args.action {
        SkillAction::Show => {
            println!("{}", generate_skill_md(&registry));
        }
        SkillAction::Install => {
            let content = generate_skill_md(&registry);
            // Install into every agent skill directory, creating parents as needed.
            let mut installed: Vec<PathBuf> = Vec::new();
            for (dir, label) in skill_dirs() {
                match install_skill_to(&dir, &content) {
                    Ok(path) => installed.push(path),
                    Err(e) => eprintln!("  Could not install for {label}: {e}"),
                }
            }
            if installed.is_empty() {
                println!("No home directory detected; could not install the AgentSpan skill.");
            } else {
                let paths = installed
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                println!("Installed AgentSpan skill to: {paths}");
            }
        }
        SkillAction::Uninstall => {
            let mut removed = 0;
            for (dir, label) in skill_dirs() {
                let file = dir.join("agentspan.md");
                if file.is_file() {
                    std::fs::remove_file(&file)?;
                    println!("Removed {label} skill: {}", file.display());
                    removed += 1;
                }
            }
            if removed == 0 {
                println!("No installed AgentSpan skills found.");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_md_lists_channels() {
        let registry = ChannelRegistry::default_channels();
        let md = generate_skill_md(&registry);
        assert!(md.contains("name: agentspan"));
        assert!(md.contains("`exa`"));
        assert!(md.contains("`youtube`"));
        assert!(md.contains("`reddit`"));
        assert!(md.contains("/api/v1/read"));
    }

    #[test]
    fn install_and_remove_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        // `root` does not exist yet — install must create it (and any parents).
        let root = dir.path().join("nested").join("skills");
        let file = install_skill_to(&root, "hello").unwrap();
        assert!(file.exists());
        assert_eq!(file.file_name().unwrap(), "agentspan.md");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "hello");
        std::fs::remove_file(&file).unwrap();
        assert!(!file.exists());
    }
}
