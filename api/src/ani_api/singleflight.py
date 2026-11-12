"""In-process request coalescing.

Five concurrent requests for the same episode must produce one upstream extraction.
Results are shared with every waiter, including exceptions, so a blocked object does
not trigger five identical probes.
"""
from __future__ import annotations

import asyncio

from . import obs


class SingleFlight:
    def __init__(self) -> None:
        self._locks: dict[str, asyncio.Lock] = {}
        self._results: dict[str, object] = {}

    async def do(self, key: str, factory):
        lock = self._locks.setdefault(key, asyncio.Lock())
        if lock.locked():
            async with lock:
                obs.counter("singleflight_joined_total")
                result = self._results.get(key)
                if isinstance(result, Exception):
                    raise result
                return result, True
        async with lock:
            try:
                result = await factory()
            except Exception as exc:  # shared with waiters, re-raised here
                self._results[key] = exc
                self._locks.pop(key, None)
                raise
            self._results[key] = result
            self._locks.pop(key, None)
            obs.counter("singleflight_led_total")
            return result, False
