"""Async Python client for the AgentSpan API."""

from __future__ import annotations

import asyncio
from types import TracebackType
from typing import Any, Dict, List, Optional, Type

import httpx

from .exceptions import (
    APIError,
    AuthenticationError,
    ChannelError,
    RateLimitError,
)
from .models import ApiKey, ChannelInfo, Content, HealthReport, SearchResult


class AgentSpanClient:
    """Async client for the AgentSpan gateway.

    Example::

        async with AgentSpanClient(api_key="as_...") as client:
            content = await client.read("https://example.com")
            print(content.body)
    """

    def __init__(
        self,
        api_key: Optional[str] = None,
        base_url: str = "http://localhost:8080",
        timeout: float = 30.0,
        client: Optional[httpx.AsyncClient] = None,
    ) -> None:
        self.api_key = api_key
        self.base_url = base_url.rstrip("/")
        headers = {"X-API-Key": api_key} if api_key else {}
        self._client = client or httpx.AsyncClient(
            base_url=self.base_url, headers=headers, timeout=timeout
        )

    async def __aenter__(self) -> "AgentSpanClient":
        return self

    async def __aexit__(
        self,
        exc_type: Optional[Type[BaseException]],
        exc: Optional[BaseException],
        tb: Optional[TracebackType],
    ) -> None:
        await self.aclose()

    async def aclose(self) -> None:
        """Close the underlying HTTP connection pool."""
        await self._client.aclose()

    @staticmethod
    def _check(response: httpx.Response) -> None:
        if response.status_code < 400:
            return
        message = response.text
        try:
            message = response.json().get("error", message)
        except Exception:  # pragma: no cover - non-JSON error body
            pass
        if response.status_code == 401:
            raise AuthenticationError(401, message)
        if response.status_code == 429:
            retry = response.headers.get("Retry-After")
            raise RateLimitError(message, int(retry) if retry else None)
        raise APIError(response.status_code, message)

    async def read(self, url: str, force_refresh: bool = False) -> Content:
        """Read content from a URL via the best matching channel."""
        response = await self._client.get(
            "/api/v1/read",
            params={"url": url, "force_refresh": str(force_refresh).lower()},
        )
        self._check(response)
        data = response.json()
        if "error" in data:
            raise ChannelError(data["error"])
        return Content.model_validate(data["content"])

    async def search(
        self, channel: str, query: str, limit: int = 10
    ) -> List[SearchResult]:
        """Search a platform via a named channel."""
        response = await self._client.get(
            f"/api/v1/channels/{channel}/search",
            params={"q": query, "limit": limit},
        )
        self._check(response)
        data = response.json()
        if "error" in data:
            raise ChannelError(data["error"])
        return [SearchResult.model_validate(r) for r in data.get("results", [])]

    async def batch_read(
        self, urls: List[str], force_refresh: bool = False
    ) -> List[Content]:
        """Read many URLs concurrently (client-side parallelism via asyncio)."""
        return list(
            await asyncio.gather(*(self.read(u, force_refresh) for u in urls))
        )

    async def get_config(self) -> Dict[str, Any]:
        """Fetch the server's non-secret configuration view."""
        response = await self._client.get("/api/v1/config")
        self._check(response)
        return response.json()

    async def list_channels(self) -> List[ChannelInfo]:
        """List the available channels."""
        response = await self._client.get("/api/v1/channels")
        self._check(response)
        return [
            ChannelInfo.model_validate(c) for c in response.json().get("channels", [])
        ]

    async def doctor(self) -> HealthReport:
        """Run health diagnostics across all channels."""
        response = await self._client.get("/api/v1/doctor")
        self._check(response)
        payload = response.json()
        return HealthReport(
            status=payload.get("status"),
            channels=payload.get("channels", []),
            raw=payload,
        )

    async def health(self) -> bool:
        """Return True when the server's /health endpoint is OK."""
        response = await self._client.get("/health")
        return response.status_code == 200

    async def create_key(
        self, name: str, scopes: Optional[List[str]] = None, tenant_id: str = "default"
    ) -> ApiKey:
        """Create a new API key (requires admin scope)."""
        response = await self._client.post(
            "/api/v1/auth/keys",
            json={"name": name, "scopes": scopes or ["read"], "tenant_id": tenant_id},
        )
        self._check(response)
        return ApiKey.model_validate(response.json())

    async def revoke_key(self, key_id: str) -> None:
        """Revoke an API key by id (requires admin scope)."""
        response = await self._client.delete(f"/api/v1/auth/keys/{key_id}")
        self._check(response)
