# Setup: Twitch

Search Twitch channels and read live stream info via the official Helix API.

## 1. Register an app
1. https://dev.twitch.tv/console/apps → **Register Your Application**.
2. OAuth Redirect URL: `http://localhost` (unused for app tokens).
3. Copy the **Client ID** and generate a **Client Secret**.

## 2. Configure AgentSpan
AgentSpan uses the app client-credentials flow:

```bash
export TWITCH_CLIENT_ID="your-client-id"
export TWITCH_CLIENT_SECRET="your-client-secret"
```

## 3. Verify
```bash
agentspan doctor            # twitch should show [ok]
```

## Usage
- **Search** channels: `GET /api/v1/channels/twitch/search?q=speedrun`
- **Read** a channel's live stream: `GET /api/v1/channels/twitch/read?url=https://twitch.tv/<login>`
