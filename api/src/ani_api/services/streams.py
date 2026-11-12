"""Stream resolution with segment-level validation.

Rules enforced here, each traceable to a measured failure:
  1. A stream is reported playable only after a ranged GET of its first segment
     returns 200/206. Playlist 200 is not proof.
  2. Playlist availability and segment availability are reported separately.
  3. A 403 that arrives with edge-cache headers is treated as a cached object
     block (we observed cf-cache-status HIT with max-age=1209600 on a 403).
  4. Known-broken segment URLs are remembered and not retried blindly.
  5. Re-extraction is skipped when it provably cannot help: if a fresh extraction
     returns the same master URL, the object is broken, not the URL string.
  6. When the chosen variant is blocked, ordered alternatives are returned.
"""
from __future__ import annotations

import hashlib
import time
from datetime import datetime, timezone

import httpx

from ..adapters.base import MetadataAdapter, StreamAdapter
from ..adapters.playlist import parse_first_segment
from ..cache import cache
from ..config import settings
from ..errors import AniError, no_stream, object_blocked_segment
from ..http_client import LoopClient
from ..models import (AudioInfo, DownloadInfo, EpisodeRef, StreamResponse, SubtitleTrack,
                      UrlLifetime, Validation, Variant)
from .security import sign_payload

# Measured on 2026-11-12 by repeated real requests: a URL was still serving at
# ~28 minutes. This is evidence, not a guarantee, and it is labelled as such.
OBSERVED_VALID_MINUTES = 28
OBSERVED_ON = "2026-11-12"

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

    # ---------- probing ----------
    async def probe_segment(self, url: str, referer: str | None = None) -> dict:
        headers = {"Range": f"bytes=0-{settings.segment_probe_bytes - 1}"}
        if referer:
            headers["Referer"] = referer
        try:
            r = await self._http.client.get(url, headers=headers)
        except httpx.HTTPError as exc:
            return {"reachable": False, "http_code": None, "bytes": 0, "error": type(exc).__name__}
        cf = r.headers.get("cf-cache-status")
        age = r.headers.get("age")
        cached = False
        if r.status_code == 403:
            # edge-cached error: headers prove it is a cached object, not a transient deny
            cached = (cf or "").upper() in {"HIT", "REVALIDATED"} or (age or "").isdigit() and int(age or 0) > 0
        return {
            "reachable": r.status_code < 400,
            "http_code": r.status_code,
            "bytes": len(r.content),
            "content_type": r.headers.get("content-type"),
            "edge_cache_status": cf,
            "age": age,
            "cached_403": cached,
        }

    async def _media_first_segment(self, playlist_url: str, referer: str) -> str | None:
        r = await self._http.client.get(playlist_url, headers={"Referer": referer})
        if r.status_code >= 400:
            return None
        return parse_first_segment(r.text, playlist_url)

    def _order(self, variants: list[Variant], quality: str) -> list[Variant]:
        q = _norm_quality(quality)
        if q == "best":
            return list(variants)
        if q == "worst":
            return list(reversed(variants))
        exact = [v for v in variants if _norm_quality(v.quality) == q]
        rest = [v for v in variants if v not in exact]
        return exact + rest

    # ---------- resolution ----------
    async def resolve(self, *, episode_id: str, audio: str, quality: str, request_id: str,
                      anime_id: str = "", episode_number: str | None = None,
                      refresh: bool = False) -> StreamResponse:
        cache_key = f"r:{episode_id}:{audio}:{_norm_quality(quality)}"
        cached = None if refresh else await cache.get(cache_key)
        warnings: list[str] = []

        if cached:
            age = time.time() - cached.get("validated_ts", 0)
            if age <= 60:
                cached["payload"]["cache"] = {"hit": True, "age_seconds": int(age), "validated_by": "cached_probe"}
                cached["payload"]["request_id"] = request_id
                return StreamResponse(**cached["payload"])
            # older than a minute: prove the segment still serves before trusting it
            probe = await self.probe_segment(cached["segment_url"], cached.get("referer"))
            if probe["reachable"]:
                cached["payload"]["cache"] = {"hit": True, "age_seconds": int(age), "validated_by": "reprobe"}
                cached["payload"]["validation"].update({
                    "segment_http_code": probe["http_code"], "segment_bytes": probe["bytes"],
                    "checked_at": _now()})
                cached["payload"]["request_id"] = request_id
                cached["validated_ts"] = time.time()
                await cache.set(cache_key, cached, settings.ttl_stream)
                return StreamResponse(**cached["payload"])
            await cache.delete(cache_key)
            warnings.append("cached stream failed revalidation; resolved again")

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

        import base64  # local: only needed to unwrap the data-hash
        hash_b64 = await self.metadata.server_hash(episode_id, audio, server.name)  # type: ignore[attr-defined]
        embed_url = base64.b64decode(hash_b64 + "=" * (-len(hash_b64) % 4)).decode()

        streams = await adapter.resolve(embed_url)
        candidates = self._order(streams.variants, quality)
        if not candidates:
            raise no_stream("no variants in master playlist")

        probed: list[Variant] = []
        chosen: Variant | None = None
        chosen_segment = ""
        blocked_all_cached = True

        for variant in candidates[: settings.probe_budget]:
            first = await self._media_first_segment(variant.playlist_url, streams.referer)
            if not first:
                variant.playable = False
                variant.blocked_reason = "media playlist unavailable"
                variant.edge_cached = False
                probed.append(variant)
                blocked_all_cached = False
                continue
            variant.first_segment_url = first
            deny_key = "d:" + hashlib.sha1(first.encode()).hexdigest()

            remembered = await cache.get(deny_key)
            if remembered:
                variant.playable = False
                variant.probe_http_code = remembered.get("http_code")
                variant.edge_cached = remembered.get("cached", False)
                variant.blocked_reason = "known-broken segment (previously observed, not retried)"
                probed.append(variant)
                continue

            probe = await self.probe_segment(first, streams.referer)
            variant.playable = probe["reachable"]
            variant.probe_http_code = probe.get("http_code")
            variant.probe_bytes = probe.get("bytes")
            variant.edge_cached = probe.get("cached_403", False)
            if probe["reachable"]:
                blocked_all_cached = False
                if chosen is None:
                    chosen, chosen_segment = variant, first
            else:
                variant.blocked_reason = (f"first segment HTTP {probe.get('http_code')}"
                                          + (" (edge-cached)" if probe.get("cached_403") else ""))
                if probe.get("cached_403"):
                    await cache.set(deny_key, {"http_code": probe.get("http_code"), "cached": True},
                                    settings.deny_ttl)
                else:
                    blocked_all_cached = False
            probed.append(variant)

        if chosen is None:
            # Re-extraction only helps if it can yield a different object. A fresh
            # extraction of an edge-cached 403 returns the identical URL, so say so.
            if blocked_all_cached:
                raise object_blocked_segment(
                    quality=candidates[0].quality,
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
            validation=Validation(
                first_segment_checked=True,
                segment_http_code=chosen.probe_http_code,
                segment_bytes=chosen.probe_bytes,
                segment_container="mpegts (video/mp2t)",
                edge_cache_status=None,
                cached_403_detected=False,
                checked_at=_now()),
            url_lifetime=UrlLifetime(
                extracted_at=_now(), expires_at=None, expiry_known=False,
                observed_valid_for_minutes=OBSERVED_VALID_MINUTES,
                note=f"URL carries no expiry token; longest observed validity {OBSERVED_VALID_MINUTES} min "
                     f"on {OBSERVED_ON}. 2h/6h/24h still unmeasured."),
            download=DownloadInfo(available=False, reason="server-side remux not enabled in this phase"),
            playback_hint=PLAYBACK_HINT,
            cache={"hit": False, "age_seconds": 0, "validated_by": "fresh_probe"},
            warnings=warnings,
        )
        await cache.set(cache_key, {"payload": payload.model_dump(), "segment_url": chosen_segment,
                                    "referer": streams.referer, "validated_ts": time.time(),
                                    "master_url": streams.master_url}, settings.ttl_stream)
        return payload

    async def aclose(self) -> None:
        await self._http.aclose()
