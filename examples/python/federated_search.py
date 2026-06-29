#!/usr/bin/env python3
"""Federated search across Hacker News, Reddit, and Lobsters.

Prereq: agentspan serve --port 8080
Run:    python examples/python/federated_search.py
"""

import asyncio
import sys

sys.path.insert(0, "sdk/python")

from agentspan import AgentSpanClient


async def main():
    client = AgentSpanClient(base_url="http://localhost:8080")

    # Search across three link-aggregator channels at once.
    response = await client.session.post(
        f"{client.base_url}/api/v1/search/federated",
        json={
            "query": "rust async runtime",
            "channels": ["hackernews", "reddit", "lobsters"],
            "limit": 10,
            "rerank": True,
            "collapse": True,
        },
    )
    data = response.json()

    print(f"Query: {data['query']}")
    print(f"Searched: {', '.join(data.get('searched', []))}")
    print(f"Results: {len(data.get('results', []))}\n")

    for i, result in enumerate(data.get("results", []), 1):
        sources = ", ".join(result.get("channels", []))
        print(f"  {i}. [{sources}] {result['title']}")
        print(f"     {result['url']}")
        print(f"     {result['snippet'][:120]}...")
        print()


if __name__ == "__main__":
    asyncio.run(main())
