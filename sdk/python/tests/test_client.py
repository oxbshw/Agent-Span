"""Tests for AgentSpanClient using httpx MockTransport (no network)."""

import httpx
import pytest

from agentspan import AgentSpanClient
from agentspan.exceptions import AuthenticationError, ChannelError, RateLimitError


def make_client(handler) -> AgentSpanClient:
    transport = httpx.MockTransport(handler)
    http = httpx.AsyncClient(
        base_url="http://test", headers={"X-API-Key": "k"}, transport=transport
    )
    return AgentSpanClient(api_key="k", client=http)


async def test_read_returns_content():
    def handler(request: httpx.Request) -> httpx.Response:
        assert request.url.path == "/api/v1/read"
        assert request.url.params["url"] == "https://x"
        return httpx.Response(
            200,
            json={
                "channel": "web",
                "content": {
                    "url": "https://x",
                    "title": "Title",
                    "body": "hello world",
                    "metadata": None,
                    "cached": False,
                },
            },
        )

    client = make_client(handler)
    content = await client.read("https://x")
    assert content.body == "hello world"
    assert content.title == "Title"
    await client.aclose()


async def test_read_channel_error():
    def handler(request: httpx.Request) -> httpx.Response:
        return httpx.Response(200, json={"error": "no channel can handle this URL"})

    client = make_client(handler)
    with pytest.raises(ChannelError):
        await client.read("ftp://x")
    await client.aclose()


async def test_list_channels():
    def handler(request: httpx.Request) -> httpx.Response:
        return httpx.Response(
            200,
            json={"channels": [{"name": "web", "description": "d", "tier": "Zero"}]},
        )

    client = make_client(handler)
    channels = await client.list_channels()
    assert len(channels) == 1
    assert channels[0].name == "web"
    await client.aclose()


async def test_search():
    def handler(request: httpx.Request) -> httpx.Response:
        assert request.url.path == "/api/v1/channels/hackernews/search"
        return httpx.Response(
            200, json={"results": [{"title": "Rust", "url": "https://r", "snippet": "s"}]}
        )

    client = make_client(handler)
    results = await client.search("hackernews", "rust")
    assert results[0].title == "Rust"
    await client.aclose()


async def test_authentication_error():
    def handler(request: httpx.Request) -> httpx.Response:
        return httpx.Response(401, json={"error": "invalid API key"})

    client = make_client(handler)
    with pytest.raises(AuthenticationError):
        await client.list_channels()
    await client.aclose()


async def test_rate_limit_error_carries_retry_after():
    def handler(request: httpx.Request) -> httpx.Response:
        return httpx.Response(
            429, json={"error": "rate limit exceeded"}, headers={"Retry-After": "12"}
        )

    client = make_client(handler)
    with pytest.raises(RateLimitError) as exc:
        await client.read("https://x")
    assert exc.value.retry_after == 12
    await client.aclose()


async def test_create_key():
    def handler(request: httpx.Request) -> httpx.Response:
        assert request.method == "POST"
        return httpx.Response(
            201,
            json={
                "id": "abc",
                "secret": "as_secret",
                "tenant_id": "default",
                "name": "ci",
            },
        )

    client = make_client(handler)
    key = await client.create_key("ci", scopes=["read"])
    assert key.secret == "as_secret"
    assert key.id == "abc"
    await client.aclose()


async def test_health():
    def handler(request: httpx.Request) -> httpx.Response:
        return httpx.Response(200, json={"status": "ok"})

    client = make_client(handler)
    assert await client.health() is True
    await client.aclose()


async def test_health_false_on_error_status():
    def handler(request: httpx.Request) -> httpx.Response:
        return httpx.Response(503, json={"status": "down"})

    client = make_client(handler)
    assert await client.health() is False
    await client.aclose()


async def test_doctor_returns_report():
    def handler(request: httpx.Request) -> httpx.Response:
        assert request.url.path == "/api/v1/doctor"
        return httpx.Response(
            200,
            json={"status": "ok", "channels": [{"name": "web", "status": "Ok"}]},
        )

    client = make_client(handler)
    report = await client.doctor()
    assert report.status == "ok"
    assert report.channels[0]["name"] == "web"
    await client.aclose()


async def test_get_config():
    def handler(request: httpx.Request) -> httpx.Response:
        assert request.url.path == "/api/v1/config"
        return httpx.Response(200, json={"cache": {"enabled": True}, "proxy": None})

    client = make_client(handler)
    cfg = await client.get_config()
    assert cfg["cache"]["enabled"] is True
    await client.aclose()


async def test_batch_read_reads_all_urls():
    def handler(request: httpx.Request) -> httpx.Response:
        url = request.url.params["url"]
        return httpx.Response(
            200,
            json={
                "channel": "web",
                "content": {
                    "url": url,
                    "title": None,
                    "body": f"body for {url}",
                    "metadata": None,
                    "cached": False,
                },
            },
        )

    client = make_client(handler)
    results = await client.batch_read(["https://a", "https://b", "https://c"])
    assert len(results) == 3
    assert results[0].body == "body for https://a"
    assert results[2].url == "https://c"
    await client.aclose()


async def test_search_passes_limit():
    captured = {}

    def handler(request: httpx.Request) -> httpx.Response:
        captured["limit"] = request.url.params["limit"]
        return httpx.Response(200, json={"results": []})

    client = make_client(handler)
    await client.search("hackernews", "rust", limit=7)
    assert captured["limit"] == "7"
    await client.aclose()


async def test_generic_api_error():
    from agentspan.exceptions import APIError

    def handler(request: httpx.Request) -> httpx.Response:
        return httpx.Response(500, json={"error": "boom"})

    client = make_client(handler)
    with pytest.raises(APIError):
        await client.list_channels()
    await client.aclose()


async def test_revoke_key():
    def handler(request: httpx.Request) -> httpx.Response:
        assert request.method == "DELETE"
        assert request.url.path == "/api/v1/auth/keys/abc"
        return httpx.Response(204)

    client = make_client(handler)
    await client.revoke_key("abc")
    await client.aclose()
