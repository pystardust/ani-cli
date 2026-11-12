"""hianime.at metadata adapter.

Ported from the proven shell implementation in ani-cli (hianime_search,
hianime_episodes, hianime_m3u8). The shell script remains the executable spec;
tests/test_parity.py asserts both produce identical output.
"""
from __future__ import annotations

import base64
import json
import re
from urllib.parse import quote

import httpx

from ..config import settings
from ..errors import embed_decode_failed, no_results, unsupported_server, upstream_blocked
from ..http_client import LoopClient
from ..models import AnimeRef, AudioType, EpisodeRef, ServerRef

_FILM = re.compile(
    r'<h3 class="film-name">\s*<a href="[^"]*/([^"/]*)"\s*title="([^"]*)"', re.S)
_EP = re.compile(r'data-number="([^"]*)".*?data-id="([0-9]+)".*?/watch/[^?]*\?ep=', re.S)
_SERVER = re.compile(
    r'data-type="(?P<type>sub|dub)".*?data-server-name="(?P<name>[^"]+)".*?data-hash="(?P<hash>[^"]*)"', re.S)
_CHALLENGE = ("Just a moment", "cf-challenge", "Attention Required")

# Servers listed upstream but proven non-extractable. hd1 serves an AES-encrypted
# 'enc' field decrypted by its own JS; we do not fake support for it.
_UNSUPPORTED = {
    "hd-1": "sources are AES-encrypted in the embed payload; no plaintext stream URL",
}


def _plain_text(html: str) -> str:
    return (html.replace("&#039;", "'").replace("&quot;", '"').replace("&amp;", "&"))


class HianimeAdapter:
    name = "hianime"

    def __init__(self) -> None:
        self._http = LoopClient(base_url=settings.metadata_base)

    async def _get(self, path: str, stage: str) -> str:
        r = await self._http.client.get(path)
        body = r.text
        if any(marker.lower() in body[:8000].lower() for marker in _CHALLENGE):
            raise upstream_blocked(stage, f"HTTP {r.status_code}")
        if r.status_code >= 400:
            # 5xx / 403 without a challenge is still an upstream failure, never "no results"
            raise upstream_blocked(stage, f"HTTP {r.status_code}")
        return body

    async def search(self, query: str) -> list[AnimeRef]:
        body = await self._get(f"/search?keyword={quote(query)}", "search")
        # The sidebar repeats result markup. Truncating at it is fragile (its position
        # varies), so parse the whole page and dedupe by slug instead.
        body = body.replace("\n", " ")
        seen, out = set(), []
        for slug, title in _FILM.findall(body):
            if slug in seen:
                continue
            seen.add(slug)
            out.append(AnimeRef(id=slug, title=_plain_text(title).strip()))
        if not out:
            raise no_results(query)
        return out

    async def episodes(self, anime_id: str) -> list[EpisodeRef]:
        numeric = anime_id.rsplit("-", 1)[-1]
        body = await self._get(f"/api/theme/episode/list/{numeric}", "episodes")
        flat = body.replace("\\", "").replace("ep-item", "\nep-item")
        out, seen = [], set()
        for number, ep_id in _EP.findall(flat):
            if ep_id in seen:
                continue
            seen.add(ep_id)
            out.append(EpisodeRef(id=ep_id, number=number))
        return out

    async def servers(self, episode_id: str, audio: AudioType) -> list[ServerRef]:
        body = await self._get(f"/api/theme/episode/servers?episodeId={episode_id}", "servers")
        flat = body.replace('\\"', '"').replace("server-item", "\nserver-item")
        out, seen = [], set()
        for m in _SERVER.finditer(flat):
            if m.group("type") != audio:
                continue
            name = m.group("name")
            key = name.lower()
            if key in seen:
                continue
            seen.add(key)
            reason = _UNSUPPORTED.get(key)
            out.append(ServerRef(name=name, audio=audio, supported=reason is None, reason=reason))
        return out

    async def server_hash(self, episode_id: str, audio: AudioType, server: str) -> str:
        """Base64 data-hash for a specific server, which decodes to the embed URL."""
        body = await self._get(f"/api/theme/episode/servers?episodeId={episode_id}", "servers")
        flat = body.replace('\\"', '"').replace("server-item", "\nserver-item")
        for m in _SERVER.finditer(flat):
            if m.group("type") == audio and m.group("name").lower() == server.lower():
                return m.group("hash")
        raise unsupported_server(server, "not listed for this episode")

    async def aclose(self) -> None:
        await self._http.aclose()
