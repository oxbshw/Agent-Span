"""Pydantic models mirroring the AgentSpan API response shapes."""

from __future__ import annotations

from typing import Any, Dict, List, Optional

from pydantic import BaseModel, Field


class Content(BaseModel):
    """Content returned from a read operation."""

    url: str
    title: Optional[str] = None
    body: str = ""
    metadata: Any = None
    cached: bool = False


class SearchResult(BaseModel):
    """A single search result."""

    title: str = ""
    url: str = ""
    snippet: str = ""
    author: Optional[str] = None
    timestamp: Optional[str] = None
    metadata: Any = None


class ChannelInfo(BaseModel):
    """Metadata about a registered channel."""

    name: str
    description: str = ""
    tier: str = ""


class ApiKey(BaseModel):
    """A newly minted API key (secret shown once)."""

    id: str
    secret: str
    tenant_id: str
    name: str


class HealthReport(BaseModel):
    """Aggregated doctor output. The full structure is backend-defined, so the
    raw payload is preserved under ``raw`` alongside any recognised fields."""

    status: Optional[str] = None
    channels: List[Dict[str, Any]] = Field(default_factory=list)
    raw: Dict[str, Any] = Field(default_factory=dict)
