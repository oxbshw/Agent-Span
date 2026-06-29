#!/usr/bin/env python3
"""Create an API key, use it, then revoke it.

Prereq: agentspan serve --port 8080 (with auth.require_api_key=true)
Run:    python examples/python/auth_flow.py
"""

import asyncio
import sys

sys.path.insert(0, "sdk/python")

from agentspan import AgentSpanClient


async def main():
    # Connect as admin (in single-user mode, no key needed for admin routes
    # when auth is disabled; when enabled, use an existing admin key).
    client = AgentSpanClient(base_url="http://localhost:8080")

    # Create a read-only key.
    key = await client.create_key(name="example-reader", scopes=["read"])
    print(f"Created key: {key['id']}")
    print(f"Secret (shown once): {key['secret'][:12]}...")

    # Use it to read a page.
    reader = AgentSpanClient(
        base_url="http://localhost:8080",
        api_key=key["secret"],
    )
    content = await reader.read("https://example.com")
    print(f"Read succeeded: {content.title or '(no title)'}")

    # Revoke the key.
    await client.revoke_key(key["id"])
    print(f"Revoked key: {key['id']}")

    # Verify it no longer works.
    try:
        await reader.read("https://example.com")
        print("ERROR: key still works!")
    except Exception as e:
        print(f"Key correctly rejected: {type(e).__name__}")


if __name__ == "__main__":
    asyncio.run(main())
