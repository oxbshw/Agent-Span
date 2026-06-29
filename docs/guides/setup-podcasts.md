# Setup: Podcasts (Podcast Index)

Search podcasts and read episode lists via the free Podcast Index API.

## 1. Get free credentials
1. https://api.podcastindex.org/signup → sign up (free).
2. Copy your **API Key** and **API Secret**.

## 2. Configure AgentSpan
```bash
export PODCASTINDEX_KEY="your-key"
export PODCASTINDEX_SECRET="your-secret"
```

(AgentSpan signs each request with the required SHA-1 `Authorization` header automatically.)

## 3. Verify
```bash
agentspan doctor            # podcasts should show [ok]
```

## Usage
- **Search** podcasts: `GET /api/v1/channels/podcasts/search?q=lex+fridman`
- **Read** a feed's episodes: `GET /api/v1/channels/podcasts/read?url=<feed-url>`
