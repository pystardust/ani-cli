"""HLS playlist parsing. Pure functions, no I/O, so they are cheap to test."""
from __future__ import annotations

import re
from urllib.parse import urljoin

from ..models import Variant

_ATTR = re.compile(r'([A-Z0-9-]+)=("[^"]*"|[^,]*)')


def _attrs(line: str) -> dict[str, str]:
    body = line.split(":", 1)[1] if ":" in line else ""
    out: dict[str, str] = {}
    for key, val in _ATTR.findall(body):
        out[key] = val.strip('"')
    return out


def parse_master(body: str, master_url: str) -> list[Variant]:
    """Parse EXT-X-STREAM-INF variants.

    Labels are normalised to '<height>p' but the raw height is kept, because we
    observed providers emit both bare '1080' and '1080p' depending on whether the
    tag carries a trailing attribute.
    """
    lines = [ln.strip() for ln in body.splitlines()]
    variants: list[Variant] = []
    for i, line in enumerate(lines):
        if not line.startswith("#EXT-X-STREAM-INF"):
            continue
        meta = _attrs(line)
        uri = ""
        for nxt in lines[i + 1:]:
            if nxt and not nxt.startswith("#"):
                uri = nxt
                break
        if not uri:
            continue
        width = height = None
        res = meta.get("RESOLUTION")
        if res and "x" in res:
            w, h = res.lower().split("x", 1)
            width, height = (int(w) if w.isdigit() else None), (int(h) if h.isdigit() else None)
        bw = meta.get("BANDWIDTH")
        variants.append(Variant(
            quality=f"{height}p" if height else "unknown",
            height=height, width=width,
            bandwidth_bps=int(bw) if bw and bw.isdigit() else None,
            playlist_url=urljoin(master_url, uri),
        ))
    # best first; stable for equal heights
    variants.sort(key=lambda v: (v.height or 0, v.bandwidth_bps or 0), reverse=True)
    return variants


def parse_first_segment(body: str, media_url: str) -> str | None:
    """First real segment URI in a media playlist (skips tags, keys, maps)."""
    for raw in body.splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        return urljoin(media_url, line)
    return None


def parse_audio_tracks(body: str, master_url: str) -> list[dict]:
    """EXT-X-MEDIA audio renditions, when a provider offers them."""
    tracks = []
    for line in body.splitlines():
        if not line.startswith("#EXT-X-MEDIA"):
            continue
        meta = _attrs(line)
        if meta.get("TYPE", "").upper() != "AUDIO":
            continue
        tracks.append({
            "uri": urljoin(master_url, meta["URI"]) if meta.get("URI") else None,
            "language": meta.get("LANGUAGE"),
            "name": meta.get("NAME"),
            "default": meta.get("DEFAULT", "").upper() == "YES",
        })
    return tracks
