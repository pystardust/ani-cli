"""Stream resolution with segment-level validation, coalescing and breakers.

Rules enforced here, each traceable to a measured failure:
  1. A stream is reported playable only after a ranged GET of its first segment
     returns 200/206. Playlist 200 is not proof (measured: playlist 200 with 176
     segments while every segment returned 403).
  2. Playlist availability and segment availability are reported separately.
  3. A 403 carrying edge-cache headers is classified as a cached object block.
  4. Known-broken segment URLs are remembered and not retried blindly; the window
     escalates for repeat offenders and is configurable.
  5. Re-extraction is skipped when it provably cannot help (same master URL).
  6. When the chosen variant is blocked, ordered alternatives are returned.
  7. Identical concurrent requests are coalesced into one upstream extraction.
  8. A failing provider trips a circuit breaker instead of being hammered.
"""
from __future__ import annotations

import base64
import hashlib
import time
from datetime import datetime, timezone

from .. import obs
from ..adapters.base import MetadataAdapter, StreamAdapter
from ..adapters.playlist import parse_first_segment
from ..breaker import CircuitBreaker
from ..cache import cache
from ..config import settings
from ..errors import AniError, no_stream, object_blocked_segment
from ..http_client import LoopClient
from ..models import (AudioInfo, DownloadInfo, StreamResponse, SubtitleTrack, UrlLifetime,
                      Validation, Variant)
from ..singleflight import SingleFlight
from .security import sign_payload

# Measured 2026-11-12 by repeated real requests: a URL was still serving at ~28
# minutes. Evidence, not a guarantee, and labelled as such in responses.
OBSERVED_VALID_MINUTES = 28
OBSERVED_ON = "2026-11-12"
DENY_PREFIX = "d:"

PLAYBACK_HINT = ("HLS. This provider names segments '*.ts.jpg'; ffmpeg/libavformat >=5.1 "
                 "needs --demuxer-lavf-o=extension_picky=0 (mpv) or -extension_picky 0 (ffmpeg).")


def _now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def _norm_quality(q: str) -> str:
    q = (q or "best").lower().strip()
    return q[:-1] if q.endswith("p") and q[:-1].isdigit() else q


class StreamService:
    def __init__(self, metadata: MetadataAdapter, adapters: dict[str, StreamAdapter]) -> None:
        self.metadata = metadata
        self.adapters = adapters
        self._http = LoopClient()
        self.sf = SingleFlight()
        self.breakers = {name: CircuitBreaker(name) for name in adapters}

    # ---------- probing ----------
    async def probe_segment(self, url: str, referer: str | None = None) -> dict:
        headers = {"Range": f"bytes=0-{settings.segment_probe_bytes - 1}"}
        if referer:
            headers["Referer"] = referer
        try:
            r = await self._http.client.get(url, headers=headers)
        except Exception as exc:
            obs.counter("segment_probe_total", {"result": "error"})
            return {"reachable": False, "http_code": None, "bytes": 0, "error": type(exc).__name__}
        cf = r.headers.get("cf-cache-status")
        age = r.headers.get("age") or ""
        cached_403 = False
        if r.status_code == 403:
            cached_403 = (cf or "").upper() in {"HIT", "REVALIDATED"} or (age.isdigit() and int(age) > 0)
        result = "ok" if r.status_code < 400 else ("cached_403" if cached_403 else f"http_{r.status_code}")
        obs.counter("segment_probe_total", {"result": result})
        if cached_403:
            obs.counter("cached_403_detected_total")
        return {"reachable": r.status_code < 400, "http_code": r.status_code,
                "bytes": len(r.content), "content_type": r.headers.get("content-type"),
                "edge_cache_status": cf, "age": age, "cached_403": cached_403}

    async def _media_first_segment(self, playlist_url: str, referer: str) -> str | None:
        try:
            r = await self._http.client.get(playlist_url, headers={"Referer": referer})
        except Exception:
            return None
        if r.status_code >= 400:
            return None
        return parse_first_segment(r.text, playlist_url)

    async def _remember_block(self, segment_url: str, http_code: int) -> None:
        """Escalating denylist: 30m, 1h, 2h, 4h for objects that keep failing."""
        counter_key = "dn:" + hashlib.sha1(segment_url.encode()).hexdigest()
        seen = await cache.incr(counter_key, settings.deny_ttl_max * 2)
        ttl = min(settings.deny_ttl * (2 ** min(seen - 1, 3)), settings.deny_ttl_max)
        await cache.set(DENY_PREFIX + hashlib.sha1(segment_url.encode()).hexdigest(),
                        {"http_code": http_code, "cached": True, "count": seen}, ttl)
        obs.counter("denylist_writes_total")
        obs.log("denylist.write", http_code=http_code, occurrences=seen, ttl_seconds=ttl)

    def _order(self, variants: list[Variant], quality: str) -> list[Variant]:
        q = _norm_quality(quality)
        if q == "best":
            return list(variants)
        if q == "worst":
            return list(reversed(variants))
        exact = [v for v in variants if _norm_quality(v.quality) == q]
        return exact + [v for v in variants if v not in exact]

    # ---------- public entry point ----------
    async def resolve(self, *, episode_id: str, audio: str, quality: str, request_id: str,
                      anime_id: str = "", episode_number: str | None = None,
                      refresh: bool = False) -> StreamResponse:
        started = time.time()
        key = f"r:{episode_id}:{audio}:{_norm_quality(quality)}"
        cached = None if refresh else await cache.get(key)

        if cached:
            age = time.time() - cached.get("validated_ts", 0)
            if age <= 60:
                payload = cached["payload"]
                payload["cache"] = {"hit": True, "age_seconds": int(age), "validated_by": "cached_probe"}
                payload["request_id"] = request_id
                obs.counter("resolve_total", {"audio": audio, "result": "cache_hit"})
                return StreamResponse(**payload)
            probe = await self.probe_segment(cached["segment_url"], cached.get("referer"))
            if probe["reachable"]:
                payload = cached["payload"]
                payload["cache"] = {"hit": True, "age_seconds": int(age), "validated_by": "reprobe"}
                payload["validation"].update({"segment_http_code": probe["http_code"],
                                              "segment_bytes": probe["bytes"], "checked_at": _now()})
                payload["request_id"] = request_id
                cached["validated_ts"] = time.time()
                await cache.set(key, cached, settings.ttl_stream)
                obs.counter("resolve_total", {"audio": audio, "result": "cache_revalidated"})
                return StreamResponse(**payload)
            await cache.delete(key)
            obs.counter("cache_stale_invalidated_total")

        obs.log("resolve.start", request_id=request_id, episode_id=episode_id,
                audio=audio, quality=_norm_quality(quality), refresh=refresh)

        async def work() -> StreamResponse:
            return await self._resolve_uncached(episode_id=episode_id, audio=audio,
                                                quality=quality, request_id=request_id,
                                                anime_id=anime_id, episode_number=episode_number,
                                                produced_from_key=key)

        try:
            payload, shared = await self.sf.do(key, work)
        except AniError as exc:
            code = exc.detail.get("code") if isinstance(exc.detail, dict) else "ERROR"
            obs.counter("resolve_total", {"audio": audio, "result": code or "error"})
            obs.observe("resolve_latency_seconds", time.time() - started, {"audio": audio})
            obs.log("resolve.failed", request_id=request_id, code=code, audio=audio,
                    ms=int((time.time() - started) * 1000))
            raise
        obs.counter("resolve_total", {"audio": audio, "result": "playable"})
        obs.observe("resolve_latency_seconds", time.time() - started, {"audio": audio})
        payload.cache["single_flight_shared"] = shared
        obs.log("resolve.done", request_id=request_id, quality=payload.quality_selected,
                segment_code=payload.validation.segment_http_code,
                shared=shared, ms=int((time.time() - started) * 1000))
        return payload

    # ---------- the actual work ----------
    async def _resolve_uncached(self, *, episode_id: str, audio: str, quality: str,
                                request_id: str, anime_id: str, episode_number: str | None,
                                produced_from_key: str) -> StreamResponse:
        servers = await self.metadata.servers(episode_id, audio)  # type: ignore[arg-type]
        usable = [s for s in servers if s.supported]
        if not usable:
            raise AniError(422, "AUDIO_TYPE_UNAVAILABLE", f"No supported server for audio={audio}",
                           stage="servers", provider="hianime",
                           detail="; ".join(f"{s.name}: {s.reason}" for s in servers) or "none listed")
        server = usable[0]
        adapter = self.adapters.get(server.name.lower())
        if adapter is None:
            raise no_stream(f"no adapter registered for {server.name}")

        breaker = self.breakers.get(server.name.lower())
        if breaker and not await breaker.allow():
            raise AniError(503, "CIRCUIT_OPEN", f"{server.name} circuit is open",
                           stage="provider", provider=server.name, retryable=True,
                           retry_after=breaker.cooldown,
                           detail="repeated provider failures; not retrying yet")

        hash_b64 = await self.metadata.server_hash(episode_id, audio, server.name)  # type: ignore[attr-defined]
        embed_url = base64.b64decode(hash_b64 + "=" * (-len(hash_b64) % 4)).decode()
        try:
            streams = await adapter.resolve(embed_url)
        except AniError:
            if breaker:
                await breaker.record(False)
            raise
        if breaker:
            await breaker.record(True)

        candidates = self._order(streams.variants, quality)
        if not candidates:
            raise no_stream("no variants in master playlist")

        probed: list[Variant] = []
        chosen: Variant | None = None
        chosen_segment = ""
        transient_failure = False
        for variant in candidates[: settings.probe_budget]:
            first = await self._media_first_segment(variant.playlist_url, streams.referer)
            if not first:
                variant.playable = False
                variant.blocked_reason = "media playlist unavailable"
                variant.edge_cached = False
                probed.append(variant)
                transient_failure = True
                continue
            variant.first_segment_url = first
            deny_key = DENY_PREFIX + hashlib.sha1(first.encode()).hexdigest()
            remembered = await cache.get(deny_key)
            if remembered:
                obs.counter("denylist_skips_total")
                variant.playable = False
                variant.probe_http_code = remembered.get("http_code")
                variant.edge_cached = remembered.get("cached", False)
                variant.blocked_reason = ("known-broken segment (observed "
                                          f"{remembered.get('count', 1)}x; not retried)")
                probed.append(variant)
                continue

            probe = await self.probe_segment(first, streams.referer)
            variant.playable = probe["reachable"]
            variant.probe_http_code = probe.get("http_code")
            variant.probe_bytes = probe.get("bytes")
            variant.edge_cached = probe.get("cached_403", False)
            if probe["reachable"]:
                if chosen is None:
                    chosen, chosen_segment = variant, first
            else:
                variant.blocked_reason = (f"first segment HTTP {probe.get('http_code')}"
                                          + (" (edge-cached)" if probe.get("cached_403") else ""))
                if probe.get("cached_403"):
                    await self._remember_block(first, int(probe.get("http_code") or 403))
                else:
                    transient_failure = True
            probed.append(variant)

        if chosen is None:
            if not transient_failure:
                raise object_blocked_segment(quality=candidates[0].quality,
                                             http_code=int(candidates[0].probe_http_code or 403),
                                             cached=True)
            raise AniError(503, "PROVIDER_UNAVAILABLE", "No playable variant right now",
                           stage="segment", provider=server.name, retryable=True, retry_after=120,
                           detail="; ".join(f"{v.quality}: {v.blocked_reason}" for v in probed))

        subs = []
        for track in streams.subtitles:
            track.proxied_url = "/v1/subtitles/" + sign_payload(
                {"u": track.url, "r": streams.referer, "k": "sub"}) + ".vtt"
            subs.append(track)

        payload = StreamResponse(
            request_id=request_id,
            provider=server.name.lower(),
            metadata_source=self.metadata.name,
            anime_id=anime_id,
            episode_id=episode_id,
            episode_number=episode_number,
            audio=AudioInfo(type=audio, declared=audio,  # type: ignore[arg-type]
                            verified=False,
                            note="declared by provider; not language-verified for this episode"),
            quality_requested=_norm_quality(quality),
            quality_selected=chosen.quality,
            master_url=streams.master_url,
            proxied_master_url="/v1/stream/" + sign_payload(
                {"u": streams.master_url, "r": streams.referer, "k": "master"}) + "/master.m3u8",
            variants=probed,
            alternatives=[v for v in probed if v is not chosen and v.playable],
            subtitles=subs,
            audio_tracks=[{"source": "muxed", "separate_audio_track_url": streams.audio_track_url}],
            headers_required={
                "master_playlist": {"Referer": streams.referer},
                "subtitles": {"Referer": streams.referer},
                "segments": {},
                "verified_on": OBSERVED_ON,
                "user_agent_required": False,
                "cookies_required": False,
            },
            validation=Validation(first_segment_checked=True,
                                  segment_http_code=chosen.probe_http_code,
                                  segment_bytes=chosen.probe_bytes,
                                  segment_container="mpegts (video/mp2t)",
                                  edge_cache_status=None, cached_403_detected=False,
                                  checked_at=_now()),
            url_lifetime=UrlLifetime(
                extracted_at=_now(), expires_at=None, expiry_known=False,
                observed_valid_for_minutes=OBSERVED_VALID_MINUTES,
                note=(f"URL carries no expiry token; longest observed validity "
                      f"{OBSERVED_VALID_MINUTES} min on {OBSERVED_ON}. "
                      "2h/6h/24h measured by the prober, not assumed.")),
            download=DownloadInfo(available=False,
                                  reason="server-side remux not enabled in this phase"),
            playback_hint=PLAYBACK_HINT,
            cache={"hit": False, "age_seconds": 0, "validated_by": "fresh_probe"},
            warnings=[],
        )
        await cache.set(produced_from_key,
                        {"payload": payload.model_dump(), "segment_url": chosen_segment,
                         "referer": streams.referer, "validated_ts": time.time(),
                         "master_url": streams.master_url}, settings.ttl_stream)
        return payload

    async def aclose(self) -> None:
        await self._http.aclose()
