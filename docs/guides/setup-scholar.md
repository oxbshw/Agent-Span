# Setup: Google Scholar

Search academic papers on Google Scholar via SerpAPI. Search-only, Tier 0 with a
graceful no-key fallback (the channel warns until a key is set).

## 1. Get a SerpAPI key
1. https://serpapi.com → sign up (free tier available).
2. Copy your **API key**.

## 2. Configure AgentSpan
```bash
export SERPAPI_KEY="your-serpapi-key"
```

## 3. Verify
```bash
agentspan doctor            # scholar shows [ok] with a key, [warn] without
```

## Usage
- **Search** papers: `GET /api/v1/channels/scholar/search?q=transformer+architecture`

Each result includes the title, link, snippet, and the publication summary (authors/year).
