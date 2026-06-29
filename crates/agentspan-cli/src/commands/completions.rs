//! `agentspan completions <shell>` — generate shell completion scripts.

use clap::{Args, Command};
use clap_complete::{generate, Shell};

#[derive(Args)]
pub struct CompletionsArgs {
    /// Shell to generate completions for (bash, zsh, fish, powershell, elvish).
    #[arg(value_enum)]
    pub shell: Shell,
}

/// Write a completion script for the requested shell to stdout.
///
/// `cmd` is the fully-built CLI command (passed from `main` so completions stay
/// in sync with the real argument parser). Example:
/// `agentspan completions zsh > _agentspan`.
pub fn run(args: CompletionsArgs, mut cmd: Command) -> anyhow::Result<()> {
    let name = cmd.get_name().to_string();
    generate(args.shell, &mut cmd, name, &mut std::io::stdout());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a small stand-in command and confirm a script is produced.
    fn sample_command() -> Command {
        Command::new("agentspan")
            .subcommand(Command::new("serve"))
            .subcommand(Command::new("doctor"))
    }

    #[test]
    fn generates_bash_completion() {
        let mut buf = Vec::new();
        let mut cmd = sample_command();
        let name = cmd.get_name().to_string();
        generate(Shell::Bash, &mut cmd, name, &mut buf);
        let script = String::from_utf8(buf).unwrap();
        assert!(script.contains("agentspan"));
        assert!(script.contains("serve"));
    }

    #[test]
    fn generates_for_all_supported_shells() {
        for shell in [
            Shell::Bash,
            Shell::Zsh,
            Shell::Fish,
            Shell::PowerShell,
            Shell::Elvish,
        ] {
            let mut buf = Vec::new();
            let mut cmd = sample_command();
            let name = cmd.get_name().to_string();
            generate(shell, &mut cmd, name, &mut buf);
            assert!(!buf.is_empty(), "no output for {shell:?}");
        }
    }
}
