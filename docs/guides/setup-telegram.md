# Setup: Telegram

Read Telegram channel/chat info and search recent updates via the Bot API.

## 1. Create a bot
1. Message [@BotFather](https://t.me/BotFather) → `/newbot` → follow prompts.
2. Copy the HTTP API token it gives you.

## 2. Configure AgentSpan
```bash
export TELEGRAM_BOT_TOKEN="123456:ABC-DEF..."
```

## 3. Verify
```bash
agentspan doctor            # telegram should show [ok]
```

## Usage
- **Read** a public channel/chat: `GET /api/v1/channels/telegram/read?url=https://t.me/<name>`
- **Search** recent updates the bot can see: `GET /api/v1/channels/telegram/search?q=keyword`

> Bots only see chats they've been added to and messages since they joined; for
> channel posts, add the bot as an admin.
