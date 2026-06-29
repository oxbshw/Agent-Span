# Setup: Reddit

Reddit has **no zero-config path** — the anonymous JSON API is blocked and the
official API is approval-gated, so every backend needs a logged-in session.

## Desktop (recommended): OpenCLI
Reuses your browser's Reddit login — nothing to paste.
```bash
agentspan install --channels reddit     # installs OpenCLI (desktop only)
```
Then add the OpenCLI Chrome extension (one click) and make sure you're logged into
reddit.com in that browser.

## Server / headless: cookies
1. Log into reddit.com in your browser.
2. Export cookies with the [Cookie-Editor](https://cookie-editor.cgagnier.ca/) extension → JSON.
3. Import:
   ```bash
   agentspan config cookies '<paste the Cookie-Editor JSON>'
   ```

> In mainland-China networks, Reddit also requires a proxy:
> `agentspan install --proxy http://user:pass@host:port`.

## Verify
```bash
agentspan doctor            # reddit shows [ok]/[warn] with its active backend
```

## Usage
- **Read** a post: `GET /api/v1/channels/reddit/read?url=https://www.reddit.com/r/rust/comments/...`
- **Search**: `GET /api/v1/channels/reddit/search?q=async+runtime`
