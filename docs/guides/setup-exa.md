# Setup: Exa (whole-internet search)

Semantic web search across the entire internet. Two backends, tried in order:

1. **mcporter (free, no key)** — the preferred path.
2. **Direct Exa API** — fallback when an `EXA_API_KEY` is set.

## Option A — free via mcporter (recommended)
```bash
npm install -g mcporter
mcporter config add exa https://mcp.exa.ai/mcp
```

## Option B — direct API key
```bash
export EXA_API_KEY="your-exa-key"     # from https://exa.ai
```

## Verify
```bash
agentspan doctor            # exa shows [ok] (mcporter) or [warn] (key needed)
```

## Usage
- **Search**: `GET /api/v1/channels/exa/search?q=best+rust+web+frameworks+2026`
  (Exa is search-only — there is no `read`.)
