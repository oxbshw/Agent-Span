"""Exception hierarchy for the AgentSpan client."""

from __future__ import annotations

from typing import Optional


class AgentSpanError(Exception):
    """Base class for all AgentSpan client errors."""


class APIError(AgentSpanError):
    """The API returned a non-success HTTP status."""

    def __init__(self, status_code: int, message: str) -> None:
        self.status_code = status_code
        self.message = message
        super().__init__(f"HTTP {status_code}: {message}")


class AuthenticationError(APIError):
    """The API key was missing or invalid (HTTP 401)."""


class RateLimitError(APIError):
    """The tenant quota was exceeded (HTTP 429)."""

    def __init__(self, message: str, retry_after: Optional[int] = None) -> None:
        self.retry_after = retry_after
        super().__init__(429, message)


class ChannelError(AgentSpanError):
    """A channel could not handle the request or the backend failed."""
