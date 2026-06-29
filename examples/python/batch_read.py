#!/usr/bin/env python3
"""Read multiple URLs in parallel and print token counts.

Prereq: agentspan serve --port 8080
Run:    python examples/python/batch_read.py
"""

import asyncio
import sys

sys.path.insert(0, "sdk/python")

from agentspan import AgentSpanClient


async def main():
    client = AgentSpanClient(base_url="http://localhost:8080")

    urls = [
        "https://news.ycombinator.com",
        "https://blog.rust-lang.org",
        "https://github.com/tokio-rs/tokio",
    ]

    results = await client.batch_read(urls)

    for r in results:
        if r.get("ok"):
            body = r["content"]["body"]
            tokens = len(body) // 4  # rough estimate
            print(f"  {r['url']}: {tokens} tokens, {len(body)} chars")
        else:
            print(f"  {r['url']}: FAILED — {r.get('error', 'unknown')}")

    print(f"\n{len(urls)} URLs processed in parallel.")


if __name__ == "__main__":
    asyncio.run(main())
