"""HTTP surface. Every route is access-controlled; there is no anonymous public mode."""
from __future__ import annotations

import time
import uuid
from urllib.parse import urljoin

import httpx
from pathlib import Path as FsPath

from fastapi import APIRouter, Depends, Path, Query, Request
from fastapi.responses import PlainTextResponse

from ..adapters.hianime import HianimeAdapter
from ..adapters.playlist import parse_master
from ..adapters.zokoanime import ZokoAnimeAdapter
from ..config import settings
from ..errors import AniError
from ..http_client import LoopClient
from ..models import AnimeDetail, AnimeRef, EpisodeRef, ServerRef, StreamResponse
from ..services.security import assert_allowed_host, enforce_rate_limit, require_access, sign_payload, verify_token
from ..services.streams import StreamService
from .. import obs
from ..workers.lifetime_prober import curve, read_rows

router = APIRouter()

# Only hosts our adapters actually produce may be proxied (SSRF prevention).
ALLOWED_HOSTS = {"dramahot.top", "drama1.cfd", "hianime.at"}

_metadata = HianimeAdapter()
_zoko = ZokoAnimeAdapter()
_service = StreamService(_metadata, {"zokoanime": _zoko})
_proxy_http = LoopClient()


def rid(request: Request) -> str:
    return getattr(request.state, "request_id", str(uuid.uuid4()))


@router.get("/health", summary="Liveness; never touches an upstream")
async def health() -> dict:
    return {"status": "ok", "version": "0.1.0"}


@router.get("/health/providers", summary="Per-stage upstream health")
async def health_providers(_: None = Depends(require_access)) -> dict:
    stages: dict[str, dict] = {}

    def timed(name: str, started: float, ok: bool, detail: str = "") -> None:
        stages[name] = {"ok": ok, "ms": int((time.time() - started) * 1000), "detail": detail}

    t = time.time()
    try:
        results = await _metadata.search("one piece")
        timed("metadata_search", t, bool(results), f"{len(results)} results")
        t = time.time()
        episodes = await _metadata.episodes(results[0].id)
        timed("episode_list", t, bool(episodes), f"{len(episodes)} episodes")
        t = time.time()
        servers = await _metadata.servers(episodes[0].id, "sub")
        timed("server_list", t, any(s.supported for s in servers),
              f"{len(servers)} listed, {sum(s.supported for s in servers)} supported")
    except AniError as exc:
        code = exc.detail.get("code") if isinstance(exc.detail, dict) else "ERROR"
        stages.setdefault("metadata", {"ok": False, "detail": code})
    ok = all(s.get("ok") for s in stages.values())
    return {"status": "ok" if ok else "degraded", "stages": stages,
            "note": "segment-level health is reported per stream, not here"}


@router.get("/v1/search", response_model=list[AnimeRef])
async def search(request: Request, q: str = Query(..., min_length=2),
                 limit: int = Query(25, ge=1, le=50), _: None = Depends(require_access)) -> list[AnimeRef]:
    await enforce_rate_limit(request)
    return (await _metadata.search(q))[:limit]


@router.get("/v1/anime/{anime_id}", response_model=AnimeDetail)
async def anime_detail(anime_id: str, _: None = Depends(require_access)) -> AnimeDetail:
    episodes = await _metadata.episodes(anime_id)
    return AnimeDetail(id=anime_id, episode_count=len(episodes), detail_source="episodes_api")


@router.get("/v1/anime/{anime_id}/episodes", response_model=list[EpisodeRef])
async def anime_episodes(anime_id: str, _: None = Depends(require_access)) -> list[EpisodeRef]:
    return await _metadata.episodes(anime_id)


@router.get("/v1/episodes/{episode_id}/servers", response_model=list[ServerRef])
async def episode_servers(episode_id: str = Path(pattern=r"^[0-9]{1,12}$"), audio: str = Query("sub", pattern="^(sub|dub)$"),
                          _: None = Depends(require_access)) -> list[ServerRef]:
    return await _metadata.servers(episode_id, audio)  # type: ignore[arg-type]


@router.get("/v1/streams/{episode_id}", response_model=StreamResponse,
            summary="Resolve an episode and prove the first segment serves")
async def resolve_stream(request: Request, episode_id: str = Path(pattern=r"^[0-9]{1,12}$"),
                         audio: str = Query("sub", pattern="^(sub|dub)$"),
                         quality: str = Query("best"),
                         anime_id: str = Query("", description="optional, echoed back"),
                         refresh: bool = Query(False),
                         _: None = Depends(require_access)) -> StreamResponse:
    await enforce_rate_limit(request)
    return await _service.resolve(episode_id=episode_id, audio=audio, quality=quality,
                                  request_id=rid(request), anime_id=anime_id, refresh=refresh)


@router.get("/metrics", response_class=PlainTextResponse, summary="Prometheus text exposition")
async def metrics(_: None = Depends(require_access)) -> PlainTextResponse:
    return PlainTextResponse(obs.render(), media_type="text/plain; version=0.0.4")


@router.get("/v1/admin/providers", summary="Circuit breaker state per provider")
async def providers(_: None = Depends(require_access)) -> dict:
    return {name: await br.snapshot() for name, br in _service.breakers.items()}


@router.get("/v1/admin/lifetime", summary="Measured URL lifetime curve")
async def lifetime(_: None = Depends(require_access)) -> dict:
    rows = read_rows(FsPath(settings.prober_out))
    return {"observations": curve(rows),
            "note": "checkpoints report 'pending' until a real observation lands in the window"}


@router.get("/v1/stream/{token}/master.m3u8", response_class=PlainTextResponse)
async def proxied_master(token: str) -> PlainTextResponse:
    """Master playlist, Referer applied server-side, variants rewritten to this API."""
    payload = verify_token(token)
    assert_allowed_host(payload["u"], ALLOWED_HOSTS)
    r = await _proxy_http.client.get(payload["u"], headers={"Referer": payload["r"]})
    if r.status_code >= 400:
        raise AniError(502, "PLAYLIST_FETCH_FAILED", "Upstream playlist refused",
                       stage="playlist", detail=f"HTTP {r.status_code}")
    lines, out = r.text.splitlines(), []
    for line in lines:
        if line and not line.startswith("#"):
            absolute = urljoin(payload["u"], line.strip())
            signed = sign_payload({"u": absolute, "r": payload["r"], "k": "media"})
            out.append(f"/v1/stream/{signed}/media.m3u8")
        else:
            out.append(line)
    return PlainTextResponse("\n".join(out), media_type="application/vnd.apple.mpegurl")


@router.get("/v1/stream/{token}/media.m3u8", response_class=PlainTextResponse)
async def proxied_media(token: str) -> PlainTextResponse:
    payload = verify_token(token)
    assert_allowed_host(payload["u"], ALLOWED_HOSTS)
    r = await _proxy_http.client.get(payload["u"], headers={"Referer": payload["r"]})
    if r.status_code >= 400:
        raise AniError(502, "PLAYLIST_FETCH_FAILED", "Upstream playlist refused",
                       stage="playlist", detail=f"HTTP {r.status_code}")
    return PlainTextResponse(r.text, media_type="application/vnd.apple.mpegurl")


@router.get("/v1/subtitles/{token}.vtt", response_class=PlainTextResponse)
async def proxied_subtitles(token: str) -> PlainTextResponse:
    payload = verify_token(token)
    assert_allowed_host(payload["u"], ALLOWED_HOSTS)
    r = await _proxy_http.client.get(payload["u"], headers={"Referer": payload["r"]})
    if r.status_code >= 400:
        raise AniError(502, "SUBTITLE_FETCH_FAILED", "Upstream subtitle refused",
                       stage="subtitles", detail=f"HTTP {r.status_code}")
    return PlainTextResponse(r.text, media_type="text/vtt")
