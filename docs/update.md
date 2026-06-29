# Update AgentSpan

Paste this file's URL to your AI agent:

```
Update AgentSpan for me: https://raw.githubusercontent.com/oxbshw/Agent-Span/main/docs/update.md
```

## What the agent will do

1. **Check for a newer version** — `agentspan update` (queries GitHub Releases).
2. **Apply the update** — `agentspan update --apply` (downloads + installs the latest
   binary), or rebuild from source: `git pull && cargo install --path crates/agentspan-cli`.
3. **Refresh upstream tools** — `agentspan install --env auto` re-checks and updates
   the tools channels depend on (backends get re-routed automatically when one fails).
4. **Re-register the skill** — `agentspan skill install`.
5. **Verify** — `agentspan doctor`.

Your config and credentials in `~/.agentspan/config.yaml` are preserved across updates.
