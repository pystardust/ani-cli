"""Cache with a Redis backend when available and an in-process fallback.

The fallback keeps the service runnable in environments without Redis; it is
process-local, so multi-worker deployments must configure Redis.
"""
from __future__ import annotations

import json
import time
from typing import Any, Optional

from .config import settings


class MemoryCache:
    backend = "memory"

    def __init__(self) -> None:
        self._data: dict[str, tuple[float, str]] = {}

    async def get(self, key: str) -> Optional[Any]:
        hit = self._data.get(key)
        if not hit:
            return None
        expires, raw = hit
        if expires < time.time():
            self._data.pop(key, None)
            return None
        return json.loads(raw)

    async def set(self, key: str, value: Any, ttl: int) -> None:
        self._data[key] = (time.time() + ttl, json.dumps(value))

    async def delete(self, key: str) -> None:
        self._data.pop(key, None)

    async def incr(self, key: str, ttl: int) -> int:
        cur = await self.get(key) or 0
        await self.set(key, cur + 1, ttl)
        return cur + 1


class RedisCache:
    backend = "redis"

    def __init__(self, url: str) -> None:
        import redis.asyncio as aioredis  # imported lazily: optional dependency

        self._r = aioredis.from_url(url, decode_responses=True)

    async def get(self, key: str) -> Optional[Any]:
        raw = await self._r.get(key)
        return json.loads(raw) if raw else None

    async def set(self, key: str, value: Any, ttl: int) -> None:
        await self._r.set(key, json.dumps(value), ex=ttl)

    async def delete(self, key: str) -> None:
        await self._r.delete(key)

    async def incr(self, key: str, ttl: int) -> int:
        val = await self._r.incr(key)
        if val == 1:
            await self._r.expire(key, ttl)
        return val


def get_cache():
    if settings.redis_url:
        try:
            return RedisCache(settings.redis_url)
        except Exception:
            pass
    return MemoryCache()


cache = get_cache()
