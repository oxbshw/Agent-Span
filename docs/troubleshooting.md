# Troubleshooting

Run `agentspan doctor` first — it shows every channel, its active backend, and a
fix hint for anything unhealthy.

## A channel shows `[x]` (missing)
The upstream tool isn't installed. Install it:
```bash
agentspan install --channels all      # everything
agentspan install --channels twitter  # just one
```
Common tools: `gh` (GitHub), `yt-dlp` (YouTube), `mcporter` (Exa search),
`opencli` (Reddit/XHS/Twitter/Bilibili/LinkedIn), `twitter`/`bili`/`rdt` CLIs.

## A channel shows `[!]` (warn — needs login/config)
- Reddit / XiaoHongShu / Twitter: import cookies (see `cookie-export.md`) or
  install OpenCLI and stay logged into Chrome.
- Xueqiu: `agentspan config from-browser chrome` after logging into xueqiu.com.
- Xiaoyuzhou (podcasts): set a Whisper key — `agentspan config set api_keys.groq gsk_xxx` (free).

## "command exists but cannot execute" (broken)
Usually a stale virtualenv after a Python upgrade. Reinstall the tool:
```bash
pipx reinstall <tool>     # or: uv tool install --force <tool>
```

## Reddit returns 403 / Bilibili returns 412
The anonymous paths are blocked upstream. AgentSpan prefers the OpenCLI
browser-session backend for these; install OpenCLI and stay logged in, or force it:
```bash
REDDIT_BACKEND=opencli agentspan doctor
```

## Behind a proxy / restricted network
```bash
agentspan config set proxy.url http://user:pass@host:port
```
All HTTP backends pick this up automatically.

## Forcing a specific backend
Set `<CHANNEL>_BACKEND` (env) or `<channel>_backend` (config) to move a backend
to the front when it's healthy, e.g. `BILIBILI_BACKEND=bili-cli`.
