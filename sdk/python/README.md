# AgentSpan Python SDK

Async Python client for the [AgentSpan](https://github.com/oxbshw/Agent-Span) API.

## Install

```bash
pip install agentspan
# from source:
pip install -e "sdk/python[dev]"
```

## Usage

```python
import asyncio
from agentspan import AgentSpanClient

async def main():
    async with AgentSpanClient(api_key="as_...", base_url="http://localhost:8080") as client:
        # Read any URL via the best matching channel
        content = await client.read("https://news.ycombinator.com/item?id=1")
        print(content.title, content.cached)

        # Search a platform
        results = await client.search("hackernews", "rust", limit=5)
        for r in results:
            print(r.title, r.url)

        # List channels and run diagnostics
        print([c.name for c in await client.list_channels()])
        report = await client.doctor()
        print(report.status)

asyncio.run(main())
```

In single-user mode (`auth.require_api_key = false`) the `api_key` argument is optional.

## Error handling

```python
from agentspan.exceptions import AuthenticationError, RateLimitError, ChannelError

try:
    await client.read(url)
except RateLimitError as e:
    print("retry after", e.retry_after)
except AuthenticationError:
    print("bad key")
except ChannelError as e:
    print("no channel / backend failed:", e)
```

## Develop & test

```bash
cd sdk/python
pip install -e ".[dev]"
pytest          # uses httpx MockTransport — no network required
```
