# Why AgentSpan?

The questions we get most, answered concretely. The one-line pitch: **AgentSpan
is a self-hosted gateway that does the reading for your agents** — 52 platforms
behind one REST API, one MCP server, and 9 SDKs, with caching, routing,
self-healing, and auth handled once, in one place.

## "Why not just use MCP directly?"

MCP is a *protocol*, not an implementation — you still need servers behind it.
The typical alternative to AgentSpan is a pile of single-purpose MCP servers
(one for Reddit, one for YouTube, one for search…), each with its own process,
its own auth story, its own failure modes, and no shared cache.

AgentSpan **is** an MCP server (91 tools over stdio or HTTP), so nothing about
choosing it moves you off MCP. What you gain over per-platform servers:

- **One process, one config** instead of N servers × M agents.
- **A shared 3-tier cache** — five agents reading the same URL in the same
  minute hit the upstream once, not five times.
- **Backend routing + self-healing** — when a platform's primary backend breaks
  (a CLI tool, an API), the gateway auto-switches to a fallback and alerts you;
  a lone MCP server just starts failing.
- **REST + SDKs too** — anything that can't speak MCP (cron jobs, CI, plain
  scripts) uses the same gateway over HTTP.

## "Why not a wrapper library / framework (LangChain, CrewAI)?"

Different layer. Frameworks orchestrate *reasoning* — chains, agents, memory,
prompts — inside one application's process. AgentSpan is *infrastructure*: a
long-running service that any number of applications (in any language) share.

Use both: your LangChain/CrewAI app calls AgentSpan for web access and gets
caching, rate-limit handling, and failover it would otherwise reimplement per
project. Nothing in AgentSpan assumes an LLM framework — it's plain HTTP/MCP.

## "Why not OpenRouter?"

OpenRouter routes **model inference** (one API for many LLMs). AgentSpan routes
**content access** (one API for many platforms). They're complementary: agents
often use OpenRouter to think and AgentSpan to read. If you want one gateway
for LLM calls, OpenRouter is the right tool; AgentSpan deliberately does not
proxy model inference.

## "Why not Composio / Zapier / n8n?"

Those are *action* platforms — hosted hubs for triggering workflows and writing
to SaaS apps, usually with per-seat or per-task pricing and your credentials in
their cloud. AgentSpan is a self-hosted **read/search layer**: MIT-licensed,
your infra, your keys, no per-call billing. It intentionally does not run
workflows or take actions on your behalf; when you need "read Reddit, search
docs, fetch a page, transcribe a talk" for an agent, a gateway you own is the
simpler, cheaper primitive.

## "Why not SearXNG?"

SearXNG federates **search engines** and returns links. AgentSpan federates
**platforms** and returns *content* — a Reddit thread's comments, a YouTube
video's transcript, an arXiv abstract, formatted for LLM consumption
(`format_for_llm` trims tokens per channel). AgentSpan's federated search also
de-duplicates and re-ranks across channels. If all you need is meta-search,
SearXNG is great; AgentSpan starts where the link ends.

## "Why not w3m / curl / 'just fetch the page'?"

For a single static page, do that! AgentSpan earns its keep when:

- the content is behind an API or app shell (Reddit, Twitter, Bilibili,
  Discord…) where raw HTML fetching gets you loading spinners or login walls;
- you read at volume and want caching, conditional revalidation (ETag → 304),
  and request coalescing instead of hammering upstreams;
- you want one auth/cookie store instead of embedding credentials in every
  script;
- you need output an LLM can afford — token-reduced formatting, extractive
  summaries, structured extraction — not 500 KB of markup.

## "Why not Browser-Use / Playwright?"

Browser automation *acts* (click, log in, fill forms) and pays for it in
latency, flakiness, and compute. AgentSpan *reads* through the cheapest viable
path — direct APIs, CLI tools, or a reader service — in milliseconds, with no
browser at all. They compose: use AgentSpan for the 95% of agent web access
that is reading, and a browser agent for the 5% that needs hands.

## What AgentSpan is *not*

- Not a browser-automation agent — it can't click or log in interactively.
- Not an LLM inference router — see OpenRouter.
- Not a workflow/action platform — it reads and searches; it doesn't act.
- Not a hosted SaaS — it's a binary you run. That's a feature.

## Architecture decisions worth knowing

- **Rust, async (Tokio)** — one small static binary, p99 under a millisecond
  on cache-adjacent paths at 1000 RPS (see [BENCHMARKS.md](../BENCHMARKS.md)).
- **Channels are compiled in, backends are swappable at runtime** — the
  router health-checks each channel's backends and fails over automatically;
  `<CHANNEL>_BACKEND` env vars override selection.
- **Local-first security** — binds `127.0.0.1`, admin routes locked until you
  opt into API keys; keys stored as SHA-256 hashes.
- **SSE over WebSocket** for live events — one fewer protocol to operate, and
  every HTTP client already speaks it.
