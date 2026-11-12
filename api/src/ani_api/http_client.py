"""Loop-aware async HTTP client.

A cached httpx.AsyncClient is bound to the loop that created it. Reusing one across
loops (test clients, reloads, or a forked worker) raises "Event loop is closed", so
the client is recreated when the running loop changes.
"""
from __future__ import annotations

import asyncio

import httpx

from .config import settings


class LoopClient:
    def __init__(self, **kwargs) -> None:
        self._kwargs = {"headers": {"User-Agent": settings.user_agent},
                        "timeout": settings.upstream_timeout,
                        "follow_redirects": True}
        self._kwargs.update(kwargs)
        self._cli: httpx.AsyncClient | None = None
        self._loop = None

    @property
    def client(self) -> httpx.AsyncClient:
        loop = asyncio.get_running_loop()
        if self._cli is None or self._loop is not loop:
            self._cli = httpx.AsyncClient(**self._kwargs)
            self._loop = loop
        return self._cli

    async def aclose(self) -> None:
        if self._cli is not None:
            await self._cli.aclose()
            self._cli = None
