# Setup: XiaoHongShu (小红书)

XiaoHongShu has no anonymous path — every backend needs a logged-in session.

## Desktop — OpenCLI (recommended)
1. `agentspan install --channels xiaohongshu` (installs OpenCLI; desktop + Chrome only)
2. Install the OpenCLI Chrome extension; log into xiaohongshu.com and keep Chrome open.
3. Verify: `agentspan doctor` (the `xiaohongshu` channel should go healthy).

The extension's service worker sleeps; the first real call wakes it.

## Server — xiaohongshu-mcp
On a headless server, run the `xiaohongshu-mcp` container (self-contained headless
browser, QR login) and point mcporter/OpenCLI at it. See the upstream project for
the binary and QR-login flow, then import cookies if needed:
```bash
agentspan config cookies '<Cookie-Editor JSON for .xiaohongshu.com>'
```

## Usage
Once healthy, read notes and search via the channel:
```bash
curl -s 'localhost:8080/api/v1/channels/xiaohongshu/search?q=keyword'
```
