# Configuration

AgentSpan is configured via a layered system: code > profile YAML > user YAML
> environment variables > defaults.

## Quick reference

### Server

| Key | Env var | Default | Description |
|-----|---------|---------|-------------|
| `server.host` | `AGENTSPAN_SERVER__HOST` | `127.0.0.1` | Bind address |
| `server.port` | `AGENTSPAN_SERVER__PORT` | `8080` | Listen port |

### Cache

| Key | Env var | Default | Description |
|-----|---------|---------|-------------|
| `cache.l1_ttl_seconds` | `AGENTSPAN_CACHE__L1_TTL_SECONDS` | `300` | L1 (memory) TTL |
| `cache.l2_ttl_seconds` | `AGENTSPAN_CACHE__L2_TTL_SECONDS` | `3600` | L2 (disk) TTL |
| `cache.l3_ttl_seconds` | `AGENTSPAN_CACHE__L3_TTL_SECONDS` | `86400` | L3 (Redis) TTL |
| `cache.l2_path` | `AGENTSPAN_CACHE__L2_PATH` | `~/.agentspan/cache` | Disk cache directory |
| `cache.l3_url` | `AGENTSPAN_CACHE__L3_URL` | (none) | Redis URL, e.g. `redis://localhost:6379` |

### Probe

| Key | Env var | Default | Description |
|-----|---------|---------|-------------|
| `probe.timeout_seconds` | `AGENTSPAN_PROBE__TIMEOUT_SECONDS` | `5` | Per-backend probe timeout |

### Auth

| Key | Env var | Default | Description |
|-----|---------|---------|-------------|
| `auth.require_api_key` | `AGENTSPAN_AUTH__REQUIRE_API_KEY` | `false` | Require API key on all protected routes |

### Logging

| Key | Env var | Default | Description |
|-----|---------|---------|-------------|
| `logging.level` | `RUST_LOG` | `info` | Log level (trace/debug/info/warn/error) |
| `logging.json` | `AGENTSPAN_LOGGING__JSON` | `false` | Structured JSON logs |

### API keys (per channel)

These are stored in the `api_keys` map in `config.yaml` or set as env vars:

| Channel | Env var |
|---------|---------|
| OpenAI | `OPENAI_API_KEY` |
| Anthropic | `ANTHROPIC_API_KEY` |
| Brave Search | `BRAVE_API_KEY` |
| Bing Search | `BING_API_KEY` |
| Google Search | `GOOGLE_API_KEY` + `GOOGLE_CSE_ID` |
| Notion | `NOTION_API_KEY` |
| Slack | `SLACK_API_KEY` |
| Exa | `EXA_API_KEY` |
| Jina | `JINA_API_KEY` |
| Spotify | `SPOTIFY_CLIENT_ID` + `SPOTIFY_CLIENT_SECRET` |
| Twitch | `TWITCH_CLIENT_ID` + `TWITCH_CLIENT_SECRET` |
| Discord | `DISCORD_API_KEY` |
| Telegram | `TELEGRAM_API_KEY` |
| Podcast Index | `PODCASTINDEX_KEY` + `PODCASTINDEX_SECRET` |
| Scholar | `SCHOLAR_API_KEY` |
| Aviationstack | `AVIATIONSTACK_API_KEY` |

### Backend overrides

Force a specific backend for a channel:

| Channel | Env var | Example |
|---------|---------|---------|
| Reddit | `REDDIT_BACKEND` | `opencli` |
| Twitter | `TWITTER_BACKEND` | `twitter-cli` |
| Bilibili | `BILIBILI_BACKEND` | `bili-cli` |

### Proxy

| Key | Env var | Description |
|-----|---------|-------------|
| `proxy.url` | `AGENTSPAN_PROXY__URL` | HTTP proxy URL |

## YAML example

```yaml
# ~/.agentspan/config.yaml
server:
  host: 0.0.0.0
  port: 8080

cache:
  l1_ttl_seconds: 300
  l2_ttl_seconds: 3600
  l3_ttl_seconds: 86400
  l3_url: redis://redis:6379

auth:
  require_api_key: true

api_keys:
  openai: sk-...
  anthropic: sk-ant-...
  brave: BSA...
  exa: exa-...

cookies:
  twitter: "auth_token=...; ct0=..."
  bilibili: "SESSDATA=...; bili_jct=..."

proxy:
  url: http://user:pass@proxy.example.com:8080
```

## CLI

```bash
agentspan config set openai_api_key sk-...
agentspan config set auth.require_api_key true
agentspan config get
agentspan config backup
agentspan config restore backup.yaml
```

## Security

- The config file is written with `0600` permissions.
- API keys are stored hashed (SHA-256) in memory; the raw key is shown once
  on creation.
- `GET /api/v1/config` returns a non-secret view only (values masked).
- Never commit `config.yaml` or paste secrets into issues.
