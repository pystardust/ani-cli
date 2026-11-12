"""Phase D unit tests: breaker, coalescing, prober aggregation."""
import asyncio

import pytest

from ani_api import cache as cache_module
from ani_api.breaker import CircuitBreaker
from ani_api.cache import MemoryCache
from ani_api.singleflight import SingleFlight
from ani_api.workers.lifetime_prober import curve


@pytest.fixture(autouse=True)
def fresh_cache(monkeypatch):
    fresh = MemoryCache()
    monkeypatch.setattr(cache_module, "cache", fresh)
    yield fresh


async def test_breaker_opens_and_half_opens():
    br = CircuitBreaker("zoko", threshold=2, cooldown=30)
    assert await br.allow() is True
    await br.record(False)
    await br.record(False)
    assert (await br.snapshot())["state"] == "open"
    assert await br.allow() is False, "must reject while cooling down"
    br.cooldown = 0  # cooldown elapsed -> one half-open probe is allowed
    assert await br.allow() is True
    assert (await br.snapshot())["state"] == "half_open"
    await br.record(True)
    assert (await br.snapshot())["state"] == "closed"


async def test_breaker_recovers_after_success():
    br = CircuitBreaker("zoko", threshold=1, cooldown=0)
    await br.record(False)
    await br.record(True)
    assert (await br.snapshot())["state"] == "closed"


async def test_singleflight_runs_factory_once():
    sf = SingleFlight()
    calls = {"n": 0}

    async def factory():
        calls["n"] += 1
        await asyncio.sleep(0.05)
        return "payload"

    results = await asyncio.gather(*(sf.do("same-key", factory) for _ in range(5)))
    assert calls["n"] == 1, f"expected one upstream call, got {calls['n']}"
    assert [r[0] for r in results] == ["payload"] * 5
    assert sum(1 for r in results if r[1]) >= 4, "waiters should report being shared"


async def test_singleflight_propagates_failure_once():
    sf = SingleFlight()
    calls = {"n": 0}

    async def factory():
        calls["n"] += 1
        await asyncio.sleep(0.02)
        raise RuntimeError("boom")

    outcomes = await asyncio.gather(*(sf.do("k2", factory) for _ in range(3)), return_exceptions=True)
    assert calls["n"] == 1
    assert all(isinstance(o, RuntimeError) for o in outcomes)


def test_lifetime_curve_reports_pending_until_measured():
    rows = [{"age_minutes": 30, "segment_code": 206, "segment_bytes": 1024}]
    summary = curve(rows)
    assert summary["2h"]["status"] == "pending"
    assert summary["2h"]["samples"] == 0
    assert summary["_oldest_age_minutes"] == 30


def test_lifetime_curve_counts_measured_points():
    rows = [{"age_minutes": 125, "segment_code": 206, "segment_bytes": 1024},
            {"age_minutes": 130, "segment_code": 403, "segment_bytes": 548}]
    summary = curve(rows)
    assert summary["2h"]["status"] == "measured"
    assert summary["2h"]["samples"] == 2
    assert summary["2h"]["playable"] == 1
    assert summary["6h"]["status"] == "pending"
