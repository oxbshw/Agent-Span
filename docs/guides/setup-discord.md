# Setup: Discord

Read messages from a Discord channel and search a guild via the official Bot API.

## 1. Create a bot & get a token
1. https://discord.com/developers/applications → **New Application**.
2. **Bot** tab → **Reset Token** → copy the token.
3. Enable **Message Content Intent** (Bot → Privileged Gateway Intents).
4. Invite the bot to your server (OAuth2 → URL Generator → scope `bot`, permission
   *Read Message History*).

## 2. Configure AgentSpan
Discord credentials are read from environment variables:

```bash
export DISCORD_BOT_TOKEN="your-bot-token"
export DISCORD_GUILD_ID="123456789012345678"   # required for search
```

(On Windows PowerShell: `$env:DISCORD_BOT_TOKEN = "..."`.)

## 3. Verify
```bash
agentspan doctor            # discord should show [ok]
```

## Usage
- **Read** a channel: `GET /api/v1/channels/discord/read?url=https://discord.com/channels/<guild>/<channel>`
- **Search** the guild: `GET /api/v1/channels/discord/search?q=keyword`
