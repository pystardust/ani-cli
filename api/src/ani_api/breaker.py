"""Per-provider circuit breaker, shared through the cache so workers agree."""
from __future__ import annotations

import time

from . import obs
from .cache import cache


class CircuitBreaker:
    """closed -> open after `threshold` consecutive failures -> half_open probe -> closed."""

    def __init__(self, name: str, threshold: int = 4, cooldown: int = 30) -> None:
        self.name, self.threshold, self.cooldown = name, threshold, cooldown

    async def _state(self) -> dict:
        return await cache.get(f"cb:{self.name}") or {"failures": 0, "opened_at": 0}

    async def allow(self) -> bool:
        state = await self._state()
        if not state.get("opened_at"):
            return True
        if time.time() - state["opened_at"] >= self.cooldown:
            obs.counter("breaker_half_open_total", {"provider": self.name})
            return True  # single half-open probe is allowed
        obs.counter("breaker_rejected_total", {"provider": self.name})
        return False

    async def record(self, ok: bool) -> None:
        state = await self._state()
        if ok:
            if state.get("opened_at"):
                obs.counter("breaker_closed_total", {"provider": self.name})
            await cache.set(f"cb:{self.name}", {"failures": 0, "opened_at": 0}, 3600)
            return
        failures = state.get("failures", 0) + 1
        opened = state.get("opened_at", 0)
        if failures >= self.threshold and not opened:
            opened = time.time()
            obs.counter("breaker_opened_total", {"provider": self.name})
        await cache.set(f"cb:{self.name}", {"failures": failures, "opened_at": opened}, 3600)

    async def snapshot(self) -> dict:
        state = await self._state()
        if state.get("opened_at"):
            cooling = time.time() - state["opened_at"] < self.cooldown
            return {"state": "open" if cooling else "half_open", "failures": state["failures"]}
        return {"state": "closed", "failures": state.get("failures", 0)}
