# Setup: Twitter / X

Twitter search/timeline needs an authenticated session. Two options:

## Option A — OpenCLI (browser session, zero per-site config)
1. `agentspan install --channels opencli` (desktop + Chrome only)
2. Install the OpenCLI Chrome extension and stay logged into x.com.
3. Force it if needed: `TWITTER_BACKEND=opencli agentspan doctor`

## Option B — cookies + twitter-cli
1. `agentspan install --channels twitter`
2. Log into x.com, export with Cookie-Editor, then:
   ```bash
   agentspan config cookies "auth_token=...; ct0=..."
   ```
3. Verify: `agentspan doctor`

> Use a dedicated account — scripted access can get accounts flagged.
