# Setup: Spotify

Search and read Spotify tracks/albums/artists via the official Web API.

## 1. Create an app
1. https://developer.spotify.com/dashboard → **Create app**.
2. Copy the **Client ID** and **Client Secret** (Settings).

## 2. Configure AgentSpan
AgentSpan uses the client-credentials flow (no user login needed):

```bash
export SPOTIFY_CLIENT_ID="your-client-id"
export SPOTIFY_CLIENT_SECRET="your-client-secret"
```

## 3. Verify
```bash
agentspan doctor            # spotify should show [ok]
```

## Usage
- **Search** tracks: `GET /api/v1/channels/spotify/search?q=daft+punk`
- **Read** a track: `GET /api/v1/channels/spotify/read?url=https://open.spotify.com/track/<id>`
