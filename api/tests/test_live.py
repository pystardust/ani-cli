"""Live provider tests. Run with: pytest -m network

Each assertion is the contract we promised: a stream counts as playable only
when its first media segment actually serves bytes.
"""
import pytest

from ani_api.adapters.hianime import HianimeAdapter
from ani_api.adapters.zokoanime import ZokoAnimeAdapter
from ani_api.errors import AniError
from ani_api.services.streams import StreamService

pytestmark = pytest.mark.network

EPISODE_SOLO_S1E1 = "4341"
SLUG_SOLO = "solo-leveling-235"


async def test_search_and_episodes_live():
    md = HianimeAdapter()
    try:
        results = await md.search("solo leveling")
        assert results and results[0].id == SLUG_SOLO
        eps = await md.episodes(SLUG_SOLO)
        assert len(eps) > 0 and eps[0].id == EPISODE_SOLO_S1E1
    finally:
        await md.aclose()


@pytest.mark.parametrize("audio", ["sub", "dub"])
async def test_resolve_proves_segment_delivery(audio):
    md = HianimeAdapter()
    svc = StreamService(md, {"zokoanime": ZokoAnimeAdapter()})
    try:
        stream = await svc.resolve(episode_id=EPISODE_SOLO_S1E1, audio=audio,
                                   quality="best", request_id="test")
    except AniError as exc:
        # Object-level blocking is a legitimate real outcome; anything else is a bug.
        assert exc.detail["code"] == "PROVIDER_OBJECT_BLOCKED", exc.detail
        return
    finally:
        await md.aclose()
        await svc.aclose()
    assert stream.validation.first_segment_checked is True
    assert stream.validation.segment_http_code in (200, 206)
    assert (stream.validation.segment_bytes or 0) > 0
    assert stream.variants, "variants must be reported"
    assert stream.master_url.endswith(".m3u8")
    assert stream.url_lifetime.expiry_known is False, "we do not invent expiries"


async def test_unsupported_server_is_reported_not_hidden():
    md = HianimeAdapter()
    try:
        servers = await md.servers(EPISODE_SOLO_S1E1, "sub")
        names = {s.name: s for s in servers}
        assert "ZokoAnime" in names and names["ZokoAnime"].supported
        if "HD-1" in names:
            assert names["HD-1"].supported is False and names["HD-1"].reason
    finally:
        await md.aclose()
