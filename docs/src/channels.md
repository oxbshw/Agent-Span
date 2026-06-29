# Channels

52 platforms, each behind a health-checked backend router. Tier 0 works with no
config; Tier 1 needs an API key or browser cookies (see the
[setup guides](https://github.com/oxbshw/Agent-Span/tree/main/docs/guides)).

## Tier 0 — zero-config

| Channel | Read | Search | Backend |
|---|:---:|:---:|---|
| web | ✅ | — | Jina Reader → direct HTTP |
| github | ✅ | ✅ | gh CLI / REST API |
| youtube | ✅ | ✅ | yt-dlp |
| tiktok | ✅ | — | yt-dlp |
| rss | ✅ | — | feed parser |
| hackernews | ✅ | ✅ | Algolia HN API |
| v2ex | ✅ | ✅ | V2EX API |
| exa | — | ✅ | mcporter / Exa API |
| wikipedia | ✅ | ✅ | MediaWiki API |
| arxiv | ✅ | ✅ | arXiv Atom API |
| quora | ✅ | ✅ | Jina Reader |
| pinterest | ✅ | ✅ | Jina Reader |
| npm | ✅ | ✅ | npm registry API |
| crates | ✅ | ✅ | crates.io API |
| pypi | ✅ | ✅ | PyPI JSON API |
| gitlab | ✅ | ✅ | GitLab API |
| dockerhub | ✅ | ✅ | Docker Hub API |
| wayback | ✅ | — | Internet Archive API |
| maps | ✅ | ✅ | OpenStreetMap Nominatim |
| weather | ✅ | ✅ | Open-Meteo API |
| coinbase | ✅ | ✅ | Coinbase API |
| duckduckgo | ✅ | ✅ | DuckDuckGo Instant Answer |
| gnews | ✅ | ✅ | Google News RSS |
| statuspage | ✅ | ✅ | Atlassian Statuspage API |
| huggingface | ✅ | ✅ | HF Hub API |
| devto | ✅ | ✅ | DEV Community API |
| openlibrary | ✅ | ✅ | Open Library API |
| gutenberg | ✅ | ✅ | Gutendex API |
| lobsters | ✅ | ✅ | lobste.rs API |
| wikidata | ✅ | ✅ | Wikidata API |

## Tier 1 — needs API key or login

| Channel | Read | Search | Backend |
|---|:---:|:---:|---|
| twitter | ✅ | ✅ | twitter-cli / OpenCLI |
| reddit | ✅ | ✅ | OpenCLI / rdt-cli |
| bilibili | ✅ | ✅ | bili-cli / web API |
| xiaohongshu | ✅ | ✅ | OpenCLI |
| instagram | ✅ | ✅ | OpenCLI / instaloader |
| linkedin | ✅ | ✅ | OpenCLI / Jina Reader |
| xueqiu | ✅ | ✅ | Xueqiu API (cookie) |
| xiaoyuzhou | ✅ | — | Whisper transcription |
| discord | ✅ | ✅ | Discord Bot API |
| telegram | ✅ | ✅ | Telegram Bot API |
| spotify | ✅ | ✅ | Spotify Web API (OAuth) |
| twitch | ✅ | ✅ | Twitch Helix API |
| scholar | — | ✅ | SerpAPI |
| podcasts | ✅ | ✅ | Podcast Index API |
| openai | ✅ | ✅ | OpenAI API |
| anthropic | ✅ | ✅ | Anthropic API |
| brave | ✅ | ✅ | Brave Search API |
| bing | ✅ | ✅ | Bing Search API |
| google | ✅ | ✅ | Google Custom Search |
| notion | ✅ | ✅ | Notion API |
| slack | ✅ | ✅ | Slack Web API |
| flight | ✅ | ✅ | Aviationstack API |

Run `agentspan doctor` to see which backend is currently serving each channel, and
`agentspan format <channel>` to see how it reduces tokens before handing text to an LLM.
