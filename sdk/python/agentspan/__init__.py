"""AgentSpan — async Python client for the AgentSpan API."""

from .client import AgentSpanClient
from .exceptions import (
    AgentSpanError,
    APIError,
    AuthenticationError,
    ChannelError,
    RateLimitError,
)
from .models import ApiKey, ChannelInfo, Content, HealthReport, SearchResult

__version__ = "0.3.0"

__all__ = [
    "AgentSpanClient",
    "AgentSpanError",
    "APIError",
    "AuthenticationError",
    "RateLimitError",
    "ChannelError",
    "Content",
    "SearchResult",
    "ChannelInfo",
    "ApiKey",
    "HealthReport",
    "__version__",
]
