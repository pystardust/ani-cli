"""Live provider tests for Phase D, plus clearly-labelled failure injection.

LIVE tests (marked network) hit hianime.at and the ZokoAnime CDN.
SIMULATED tests inject a recorded failure shape to exercise a code path; they are
never used as evidence that a live resolution succeeded.
"""
import asyncio
import json
from pathlib import Path

import pytest

from ani_api import obs
from ani_api.adapters.hianime import HianimeAdapter
from ani_api.adapters.zokoanime import ZokoAnimeAdapter
from ani_api.errors import AniError
from ani_api.services.streams import StreamService

EPISODE = "4341"      # Solo Leveling S1E1, discovered via the live episode list
SLUG = "solo-leveling-235"

pytestmark = pytest.mark.network


async def test_live_resolve_reports_alternatives_and_unverified_audio():
    md = HianimeAdapter()
    svc = StreamService(md, {"zokoanime": ZokoAnimeAdapter()})
    try:
        stream = await svc.resolve(episode_id=EPISODE, audio="sub", quality="best",
                                   request_id="live-1", anime_id=SLUG, episode_number="1")
    finally:
        await md.aclose()
        await svc.aclose()
    assert stream.validation.segment_http_code in (200, 206)
    assert stream.validation.segment_bytes and stream.validation.segment_bytes > 0
    assert stream.variants and any(v.playable for v in stream.variants)
    assert all(v.playable is not None for v in stream.variants), "every probed variant states its status"
    # regression: declared dub/sub is never presented as verified audio language
    assert stream.audio.verified is False
    assert "not language-verified" in (stream.audio.note or "")


async def test_live_concurrent_requests_are_coalesced():
    md = HianimeAdapter()
    svc = StreamService(md, {"zokoanime": ZokoAnimeAdapter()})
    before = obs.render().count("singleflight_led_total")
    try:
        results = await asyncio.gather(*(
            svc.resolve(episode_id=EPISODE, audio="sub", quality="best", request_id=f"c{i}",
                        refresh=True)
            for i in range(5)), return_exceptions=True)
    finally:
        await md.aclose()
        await svc.aclose()
    ok = [r for r in results if not isinstance(r, Exception)]
    assert len(ok) >= 4, [repr(r) for r in results if isinstance(r, Exception)]
    masters = {r.master_url for r in ok}
    assert len(masters) == 1, "all callers must receive the same resolved object"
    leaders = obs.render().count("singleflight_led_total") - before
    assert leaders <= 2, f"expected coalescing, got {leaders} upstream leaders"


async def test_live_dub_regression_mislabelled_episode_is_not_claimed():
    """Solo Leveling ep1's 'dub' was measured to be Japanese audio. The API must
    never assert a language it has not verified, whichever audio it returns."""
    md = HianimeAdapter()
    svc = StreamService(md, {"zokoanime": ZokoAnimeAdapter()})
    try:
        try:
            stream = await svc.resolve(episode_id=EPISODE, audio="dub", quality="best",
                                       request_id="dub-1", refresh=True)
        except AniError as exc:
            assert exc.detail["code"] in {"PROVIDER_OBJECT_BLOCKED", "PROVIDER_UNAVAILABLE",
                                          "AUDIO_TYPE_UNAVAILABLE"}
            return
    finally:
        await md.aclose()
        await svc.aclose()
    assert stream.audio.type == "dub" and stream.audio.verified is False


# ---------------- simulated failure injection (NOT live evidence) ----------------

class _Resp:
    def __init__(self, status, headers, content=b"x" * 1024):
        self.status_code = status
        self.headers = headers
        self.content = content
        self.text = ""


class _StubClient:
    def __init__(self, resp):
        self._resp = resp

    async def get(self, *a, **k):
        return self._resp

    async def aclose(self):
        return None


class _StubHttp:
    """Stands in for LoopClient in failure-path tests only."""

    def __init__(self, resp):
        self.client = _StubClient(resp)

    async def aclose(self):
        return None


async def test_simulated_edge_cached_403_is_classified():
    """SIMULATED: recorded header shape from a real 403 (cf-cache-status HIT, age 66)."""
    md = HianimeAdapter()
    svc = StreamService(md, {"zokoanime": ZokoAnimeAdapter()})
    try:
        svc._http = _StubHttp(_Resp(403, {"cf-cache-status": "HIT", "age": "66"}, b"<html>403</html>"))
        probe = await svc.probe_segment("https://hls1.drama1.cfd/s/x/1080/seg_00001.ts.jpg")
    finally:
        await md.aclose()
        await svc.aclose()
    assert probe["reachable"] is False
    assert probe["cached_403"] is True, "edge-cached 403 must be distinguished from a transient deny"
    assert probe["edge_cache_status"] == "HIT"


async def test_simulated_plain_403_is_not_treated_as_cached():
    """SIMULATED: a 403 with no edge-cache headers looks transient, not cached."""
    md = HianimeAdapter()
    svc = StreamService(md, {"zokoanime": ZokoAnimeAdapter()})
    try:
        svc._http = _StubHttp(_Resp(403, {}))
        probe = await svc.probe_segment("https://hls1.drama1.cfd/s/x/1080/seg_00002.ts.jpg")
    finally:
        await md.aclose()
        await svc.aclose()
    assert probe["cached_403"] is False
